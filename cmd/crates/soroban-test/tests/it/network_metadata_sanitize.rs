use httpmock::{prelude::*, Mock};
use serde_json::json;
use soroban_rpc::{GetNetworkResponse, GetVersionInfoResponse};
use soroban_test::{AssertExt, TestEnv};

// A hostile RPC / history-archive peer must not be able to smuggle terminal
// escape sequences through metadata the CLI renders. `network info`, `doctor`,
// and `snapshot create` display strings taken straight from a peer response, so
// those strings are run through `soroban_spec_tools::sanitize` before printing.

const ANSI: &str = "\u{1b}[2J\u{1b}[1;1H***PWNED***";

// The injected clear-screen escape must never reach the terminal raw, while the
// harmless `***PWNED***` text survives as proof the peer field was rendered at
// all (so the assertion can't pass vacuously). `\u{1b}[2J` is distinct from the
// CLI's own cursor-control escapes, so matching it won't flag benign output.
fn assert_sanitized(context: &str, stderr: &str) {
    assert!(
        stderr.contains("***PWNED***"),
        "{context} did not render the peer-controlled field:\n{stderr:?}"
    );
    assert!(
        !stderr.contains("\u{1b}[2J"),
        "{context} leaked a raw terminal-escape sequence:\n{stderr:?}"
    );
}

fn mock_get_network(server: &MockServer) -> Mock<'_> {
    server.mock(|when, then| {
        when.method(POST).path("/").body_contains("getNetwork");
        then.status(200).json_body(json!({
            "jsonrpc": "2.0",
            "id": 0,
            "result": GetNetworkResponse {
                friendbot_url: None,
                passphrase: format!("Test SDF Network{ANSI}"),
                protocol_version: 22,
            }
        }));
    })
}

fn mock_get_version_info(server: &MockServer, id: u32) -> Mock<'_> {
    server.mock(|when, then| {
        when.method(POST).path("/").body_contains("getVersionInfo");
        then.status(200).json_body(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": GetVersionInfoResponse {
                version: format!("24.1.0{ANSI}"),
                commmit_hash: format!("deadbeef{ANSI}"),
                build_timestamp: "2026-10-19T00:00:00Z".to_string(),
                captive_core_version: format!("stellar-core 23.0{ANSI}"),
                protocol_version: 23,
            }
        }));
    })
}

#[test]
fn network_info_sanitizes_rpc_metadata() {
    let server = MockServer::start();
    let get_network = mock_get_network(&server);
    // `network info` reuses one client: getNetwork is request id 0, getVersionInfo id 1.
    let get_version_info = mock_get_version_info(&server, 1);
    let sandbox = TestEnv::with_rpc_provider(&server.url(""), vec![]);

    let stderr = sandbox
        .new_assert_cmd("network")
        .arg("info")
        .assert()
        .success()
        .stderr_as_str();

    get_network.assert();
    get_version_info.assert();

    assert_sanitized("network info", &stderr);
}

#[test]
fn doctor_sanitizes_rpc_version() {
    let server = MockServer::start();
    // `doctor` uses a fresh client per network, so getVersionInfo is request id 0.
    let get_version_info = mock_get_version_info(&server, 0);
    let sandbox = TestEnv::default();

    sandbox
        .new_assert_cmd("network")
        .args([
            "add",
            "evilnet",
            "--rpc-url",
            &server.url(""),
            "--network-passphrase",
            "Test SDF Network ; September 2015",
        ])
        .assert()
        .success();

    let stderr = sandbox.new_assert_cmd("doctor").assert().stderr_as_str();

    get_version_info.assert();

    assert_sanitized("doctor", &stderr);
}

#[test]
fn snapshot_create_sanitizes_archive_passphrase() {
    let server = MockServer::start();
    let history = server.mock(|when, then| {
        when.method(GET).path("/.well-known/stellar-history.json");
        then.status(200).json_body(json!({
            "currentLedger": 100,
            "currentBuckets": [],
            "networkPassphrase": format!("Test SDF Network{ANSI}"),
        }));
    });
    let sandbox = TestEnv::default();

    let stderr = sandbox
        .new_assert_cmd("snapshot")
        .args(["create", "--archive-url", &server.url("")])
        .assert()
        .stderr_as_str();

    history.assert();

    assert_sanitized("snapshot create", &stderr);
}
