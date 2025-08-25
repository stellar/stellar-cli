use crate::integration::util::{deploy_hello, HELLO_WORLD};
use markdown::mdast::{Code, Node};
use markdown::ParseOptions;
use predicates::prelude::predicate;
use soroban_cli::config::network::passphrase::LOCAL;
use soroban_test::{AssertExt, TestEnv};
use std::collections::BTreeMap;
use std::fs;
use std::ops::Deref;
use std::path::PathBuf;

fn parse_command(command: &str) -> Vec<String> {
    let normalized = command.replace("\\\n", " ");
    shell_words::split(&normalized).unwrap_or_else(|_| {
        // Fallback to simple whitespace splitting if shell_words fails
        normalized.split_whitespace().map(String::from).collect()
    })
}

struct CookbookCommand {
    pub command: String,
    pub meta: BTreeMap<String, Option<String>>,
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn run_command(
    sandbox: &TestEnv,
    cookbook_command: &CookbookCommand,
    wasm_path: &str,
    wasm_hash: &str,
    source: &str,
    contract_id: &str,
    native_id: &str,
    key_xdr: &str,
) -> Result<(), String> {
    let CookbookCommand { command, meta } = cookbook_command;
    if meta.contains_key("cookbooktest.ignore") {
        return Ok(());
    }
    let args = parse_command(command);
    if args.is_empty() {
        return Err("Empty command".to_string());
    }
    let cmd = args[1].clone();
    let mut modified_args: Vec<String> = Vec::new();
    let mut skip_next = false;

    for (index, arg) in args[2..].iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }

        match arg.as_str() {
            "--wasm" => {
                modified_args.push(arg.to_string());
                modified_args.push(wasm_path.to_string());
                skip_next = true;
            }
            "--wasm-hash" => {
                modified_args.push(arg.to_string());
                modified_args.push(wasm_hash.to_string());
                skip_next = true;
            }
            "--network-passphrase" => {
                modified_args.push(arg.to_string());
                modified_args.push(LOCAL.to_string());
                skip_next = true;
            }
            "--network" => {
                modified_args.push(arg.to_string());
                modified_args.push("local".to_string());
                skip_next = true;
            }
            "--alias" => {
                modified_args.push(arg.to_string());
                modified_args.push("mycontract".to_string());
                skip_next = true;
            }
            "--key-xdr" => {
                modified_args.push(arg.to_string());
                modified_args.push(key_xdr.to_string());
                skip_next = true;
            }
            "<DURABILITY>" => {
                modified_args.push("persistent".to_string());
                skip_next = false;
            }
            "<KEY>" => {
                modified_args.push("COUNTER".to_string());
                skip_next = false;
            }
            "<ASSET_CONTRACT_ID>" => {
                modified_args.push(native_id.to_string());
                skip_next = false;
            }
            "<FUNCTION>" => {
                modified_args.push("inc".to_string());
                skip_next = false;
            }
            "C..." => {
                modified_args.push(contract_id.to_string());
                skip_next = false;
            }
            "S..." => {
                modified_args.push(source.to_string());
                skip_next = false;
            }
            _ => modified_args.push(arg.to_string()),
        }

        // If this is the last argument, don't skip the next one
        if index == args[2..].len() - 1 {
            skip_next = false;
        }
    }

    println!("Executing command: {} {}", cmd, modified_args.join(" "));
    let mut assert = sandbox
        .new_assert_cmd(&cmd)
        // Cookbook tests are responsible for setting a source account.
        .env_remove("SOROBAN_ACCOUNT")
        .args(&modified_args)
        .assert();
    eprintln!("{}", assert.stderr_as_str());
    println!("{}", assert.stdout_as_str());
    if let Some(Some(expect)) = meta.get("cookbooktest.stderr") {
        assert = assert.stderr(predicate::str::contains(expect));
    }
    if let Some(Some(expect)) = meta.get("cookbooktest.stdout") {
        assert = assert.stdout(predicate::str::contains(expect));
    }
    if meta.contains_key("cookbooktest.fail") {
        assert.failure();
    } else {
        assert.success();
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn test_mdx_file_with_sandbox_and_setup(
    sandbox: &TestEnv,
    file_path: &str,
    wasm_path: &str,
    wasm_hash: &str,
    source: &str,
    contract_id: &str,
    native_id: &str,
    key_xdr: &str,
) -> Result<(), String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file {file_path}: {e}"))?;
    let md = markdown::to_mdast(&content, &ParseOptions::mdx())
        .map_err(|e| format!("Failed to parse markdown/mdx file: {e}"))?;
    let code_blocks = code_blocks(&md);

    // Find bash code blocks and store the contents and the meta for the test.
    let commands = code_blocks
        .iter()
        .map(Deref::deref)
        .filter(|c| c.lang.as_deref() == Some("bash"))
        .map(code_block_to_cookbook_command);

    println!("Testing commands from file: {file_path}");

    for (i, command) in commands.enumerate() {
        println!(
            "Running command {}: {} ({:?})",
            i + 1,
            command.command,
            command.meta
        );
        run_command(
            sandbox,
            &command,
            wasm_path,
            wasm_hash,
            source,
            contract_id,
            native_id,
            key_xdr,
        )?;
    }

    Ok(())
}

async fn test_mdx_file(file_path: &str) -> Result<(), String> {
    let sandbox = TestEnv::new();
    let wasm = HELLO_WORLD;
    let wasm_path = wasm.path();
    let wasm_hash = wasm.hash().expect("should exist").to_string();
    let source = "test";

    // TODO: Instead of building in default setup that runs for every cookbook like deploying a
    // contract, calling its functions, and preparing variables like the "KEY". Add "invisible"
    // code blocks to cookbooks that the developer docs don't display, but that the test runs as
    // test setup. Why: It will make each cookbook fully isolated and standalone, and give us more
    // flexibility without the disjoint but coupled test setup here with the cookbooks which is not
    // obvious and difficult to maintain.
    sandbox
        .new_assert_cmd("keys")
        .arg("fund")
        .arg(source)
        .assert()
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .assert()
        .success();
    sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg("--asset")
        .arg("native")
        .arg("--source-account")
        .arg(source)
        .output()
        .expect("Failed to execute command");
    let native_id = sandbox
        .new_assert_cmd("contract")
        .arg("id")
        .arg("asset")
        .arg("--asset")
        .arg("native")
        .assert()
        .stdout_as_str();
    let contract_id = deploy_hello(&sandbox).await;
    sandbox
        .invoke_with(&["--id", &contract_id, "--", "inc"], source)
        .await
        .unwrap();
    let read_xdr = sandbox
        .new_assert_cmd("contract")
        .arg("read")
        .arg("--id")
        .arg(contract_id.clone())
        .arg("--output")
        .arg("xdr")
        .arg("--key")
        .arg("COUNTER")
        .assert()
        .stdout_as_str();
    let key_xdr = read_xdr.split(',').next().unwrap_or("").trim();
    test_mdx_file_with_sandbox_and_setup(
        &sandbox,
        file_path,
        wasm_path.to_str().unwrap(),
        &wasm_hash,
        source,
        &contract_id,
        &native_id,
        key_xdr,
    )
}

fn get_repo_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let mut path = PathBuf::from(manifest_dir);
    for _ in 0..3 {
        path.pop();
    }
    path
}

fn code_blocks<'a>(root: &'a Node) -> Vec<&'a Code> {
    let mut blocks = Vec::new();
    collect(root, &mut blocks);
    fn collect<'a>(n: &'a Node, blocks: &mut Vec<&'a Code>) {
        for n in n.children().map(|v| &v[..]).unwrap_or(&[]) {
            if let Node::Code(code) = n {
                blocks.push(code);
            }
            collect(n, blocks);
        }
    }
    blocks
}

fn code_block_to_cookbook_command(block: &Code) -> CookbookCommand {
    CookbookCommand {
        command: block.value.clone(),
        meta: {
            let meta = block.meta.as_deref().unwrap_or_default();
            let metas = shell_words::split(meta).unwrap();
            let mut map = BTreeMap::new();
            for meta in metas {
                let mut parts = meta.splitn(2, "=");
                let key = parts.next().unwrap();
                let val = parts.next().map(|v| v.trim_matches('"'));
                map.insert(key.to_owned(), val.map(ToOwned::to_owned));
            }
            map
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_all_mdx_files() {
        let repo_root = get_repo_root();
        let docs_dir = repo_root.join("cookbook");
        assert!(docs_dir.is_dir(), "docs directory not found");

        for entry in fs::read_dir(docs_dir).expect("Failed to read docs directory") {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("mdx") {
                let file_path = path.to_str().unwrap();
                match test_mdx_file(file_path).await {
                    Ok(()) => println!("Successfully tested all commands in {file_path}"),
                    Err(e) => panic!("Error testing {file_path}: {e}"),
                }
            }
        }
    }
}
