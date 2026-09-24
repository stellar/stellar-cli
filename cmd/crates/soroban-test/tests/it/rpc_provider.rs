use httpmock::{prelude::*, Mock};
use serde_json::json;
use soroban_rpc::{GetEventsResponse, GetNetworkResponse};
use soroban_test::{AssertExt, TestEnv, LOCAL_NETWORK_PASSPHRASE};

// When the RPC responds with a JSON-RPC error body, `--output json` must surface
// the structured `{ code, message }` (not just the `{ message }` fallback). This
// exercises the `ErrorObjectOwned` downcast end-to-end, guarding the
// `jsonrpsee-types` version pin: if jsonrpsee drifts to two versions in the tree
// the downcast would silently degrade to the fallback and `error.code` would be
// absent, failing this test.
#[test]
fn json_error_envelope_includes_structured_rpc_code() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/");
        then.status(200).json_body(json!({
            "jsonrpc": "2.0",
            "id": 0,
            "error": { "code": -32603, "message": "DB is empty" }
        }));
    });

    let sandbox = TestEnv::with_rpc_provider(&server.url(""), vec![]);
    let stdout = sandbox
        .new_assert_cmd("network")
        .arg("health")
        .arg("--output=json")
        .assert()
        .failure()
        .stdout_as_str();

    mock.assert();

    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["error"]["code"], json!(-32603));
    assert_eq!(value["error"]["message"], json!("DB is empty"));
}

#[tokio::test]
async fn test_use_rpc_provider_with_auth_header() {
    // mock out http request to rpc provider
    let server = MockServer::start();
    let get_network_mock = mock_get_network(&server);
    let get_events_mock = mock_get_events(&server);

    // create a new test environment with the mock server
    let rpc_url = server.url("");
    let rpc_headers = vec![("Authorization".to_string(), "Bearer test-token".to_string())];
    let sandbox = &TestEnv::with_rpc_provider(&rpc_url, rpc_headers);

    sandbox
        .new_assert_cmd("events")
        .arg("--start-ledger")
        .arg("1000")
        .assert()
        .success();

    // get_network and get_events are being called in the `stellar events` command
    get_network_mock.assert();
    get_events_mock.assert();
}

fn mock_get_network(server: &MockServer) -> Mock<'_> {
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Bearer test-token")
            .json_body(json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "getNetwork"
            }));

        then.status(200).json_body(json!({
            "jsonrpc": "2.0",
            "id": 0,
            "result": GetNetworkResponse {
                friendbot_url: None,
                passphrase: LOCAL_NETWORK_PASSPHRASE.to_string(),
                protocol_version: 22}
        }));
    })
}

fn mock_get_events(server: &MockServer) -> Mock<'_> {
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Bearer test-token")
            .json_body(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getEvents",
                "params": {
                    "startLedger": 1000,
                    "filters": [
                        {
                            "contractIds": [],
                            "topics": []
                        }
                    ],
                    "pagination": {
                        "limit": 10
                    }
                }
            }));

        then.status(200).json_body(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": GetEventsResponse {
                events: vec![],
                latest_ledger: 1000,
                cursor: "1234-5".to_string(),
                latest_ledger_close_time: "2023-10-01T00:00:00Z".to_string(),
                oldest_ledger: 1,
                oldest_ledger_close_time: "2023-01-01T00:00:00Z".to_string(),
            }
        }));
    })
}
