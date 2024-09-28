use predicates::prelude::*;
use soroban_cli::config::network::passphrase::LOCAL;
use soroban_test::{AssertExt, TestEnv};
use std::fs;
use std::path::PathBuf;

fn parse_command(command: &str) -> Vec<String> {
    command
        .replace("\\\n", " ")
        .split_whitespace()
        .map(String::from)
        .collect()
}

async fn run_command(
    sandbox: &TestEnv,
    command: &str,
    wasm_path: &str,
    wasm_hash: &str,
    source: &str,
    contract_id: &str,
    bob_id: &str,
    native_id: &str,
    key_xdr: &str,
) -> Result<(), String> {
    if command.contains("export") {
        return Ok(());
    }
    let args = parse_command(command);
    if args.is_empty() {
        return Err("Empty command".to_string());
    }
    if command.contains("contract asset deploy") {
        return Ok(());
    }
    /*if command.contains("keys generate"){
        return Ok(());
    }*/
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
            "--source" | "--source-account" => {
                modified_args.push(arg.to_string());
                modified_args.push(source.to_string());
                skip_next = true;
            }
            "--contract-id" | "--id" => {
                modified_args.push(arg.to_string());
                modified_args.push(contract_id.to_string());
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
            "<Bob_ID>" => {
                modified_args.push(bob_id.to_string());
                skip_next = false;
            }
            "<asset_contract_ID>" => {
                modified_args.push(native_id.to_string());
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
    let result = sandbox.new_assert_cmd(&cmd).args(&modified_args).assert();

    if command.contains("keys generate") {
        result
            .code(predicates::ord::eq(0).or(predicates::ord::eq(1)))
            .stderr(
                predicate::str::is_empty().or(predicates::str::contains("Generated new key for")
                    .or(predicates::str::contains("The identity")
                        .and(predicates::str::contains("already exists")))),
            );
    } else if command.contains("contract invoke") {
        result
            .failure()
            .stderr(predicates::str::contains("error: unrecognized subcommand"));
    } else if command.contains("contract restore") {
        result
            .failure()
            .stderr(predicates::str::contains("TxSorobanInvalid"));
    } else {
        result.success();
    }

    Ok(())
}

async fn test_mdx_file(
    sandbox: &TestEnv,
    file_path: &str,
    wasm_path: &str,
    wasm_hash: &str,
    source: &str,
    contract_id: &str,
    bob_id: &str,
    native_id: &str,
    key_xdr: &str,
) -> Result<(), String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;

    let commands: Vec<&str> = content
        .split("```bash")
        .skip(1)
        .filter_map(|block| block.split("```").next())
        .collect();

    println!("Testing commands from file: {}", file_path);

    for (i, command) in commands.iter().enumerate() {
        println!("Running command {}: {}", i + 1, command);
        run_command(
            sandbox,
            command,
            wasm_path,
            wasm_hash,
            source,
            contract_id,
            bob_id,
            native_id,
            key_xdr,
        )
        .await?;
    }

    Ok(())
}

fn get_repo_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let mut path = PathBuf::from(manifest_dir);
    for _ in 0..3 {
        path.pop();
    }
    path
}

#[cfg(test)]
mod tests {
    use crate::integration::util::{deploy_hello, HELLO_WORLD};

    use super::*;

    #[tokio::test]
    async fn test_all_mdx_files() {
        let sandbox = TestEnv::default();
        let wasm = HELLO_WORLD;
        let wasm_path = wasm.path();
        let wasm_hash = wasm.hash().expect("should exist").to_string();
        let source = "test";

        sandbox
            .new_assert_cmd("keys")
            .arg("fund")
            .arg(source)
            .assert()
            .success();

        sandbox
            .new_assert_cmd("keys")
            .arg("generate")
            .arg("bob")
            .assert()
            .success();
        let bob_id = sandbox
            .new_assert_cmd("keys")
            .arg("address")
            .arg("bob")
            .assert()
            .success()
            .stdout_as_str();
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
            .arg("--source-account")
            .arg(source)
            .assert()
            .stdout_as_str();
        let contract_id = deploy_hello(&sandbox).await;
        sandbox
            .invoke_with_test(&["--id", &contract_id, "--", "inc"])
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
            .arg("--source-account")
            .arg(source)
            .assert()
            .stdout_as_str();
        let key_xdr = read_xdr.split(',').next().unwrap_or("").trim();
        let repo_root = get_repo_root();
        let docs_dir = repo_root.join("cookbook");
        if !docs_dir.is_dir() {
            panic!("docs directory not found");
        }

        for entry in fs::read_dir(docs_dir).expect("Failed to read docs directory") {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("mdx") {
                let file_path = path.to_str().unwrap();
                match test_mdx_file(
                    &sandbox,
                    file_path,
                    &wasm_path.to_str().unwrap(),
                    &wasm_hash,
                    source,
                    &contract_id,
                    &bob_id,
                    &native_id,
                    &key_xdr,
                )
                .await
                {
                    Ok(_) => println!("Successfully tested all commands in {}", file_path),
                    Err(e) => panic!("Error testing {}: {}", file_path, e),
                }
            }
        }
    }
}
