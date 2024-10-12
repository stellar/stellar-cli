use assert_cmd::Command;

use soroban_cli::xdr::{
    self, CreateContractArgsV2, HostFunction, InvokeHostFunctionOp, Limits, OperationBody, ReadXdr,
    Transaction, TransactionV1Envelope,
};
use soroban_test::{AssertExt, TestEnv};

use super::util::CONSTRUCTOR;

fn constructor_cmd(sandbox: &TestEnv, value: u32, arg: &str) -> Command {
    let mut cmd = sandbox.new_assert_cmd("contract");
    cmd.arg("deploy")
        .arg("--wasm")
        .arg(CONSTRUCTOR.path())
        .arg("--alias=init");
    if !arg.is_empty() {
        cmd.arg(arg);
    }
    cmd.arg("--").arg("--counter").arg(value.to_string());
    cmd
}

#[tokio::test]
async fn deploy_constructor_contract() {
    let sandbox = TestEnv::new();
    let value = 100;
    let build = constructor_cmd(&sandbox, value, "--build-only")
        .assert()
        .stdout_as_str();
    let tx = xdr::TransactionEnvelope::from_xdr_base64(&build, Limits::none()).unwrap();
    let ops = if let xdr::TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: Transaction { operations, .. },
        ..
    }) = tx
    {
        operations
    } else {
        panic!()
    }
    .to_vec();
    let first = ops.first().unwrap();
    let args = match first {
        xdr::Operation {
            body:
                OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function:
                        HostFunction::CreateContractV2(CreateContractArgsV2 {
                            constructor_args, ..
                        }),
                    ..
                }),
            ..
        } => constructor_args,
        _ => panic!("Expected ManageData operation"),
    }
    .to_vec();

    match args.first().unwrap() {
        xdr::ScVal::U32(u32) => assert_eq!(*u32, value),
        _ => panic!("Expected U32"),
    }

    constructor_cmd(&sandbox, value, "").assert().success();

    let res = sandbox
        .new_assert_cmd("contract")
        .args(["invoke", "--id=init", "--", "counter"])
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(res.trim(), value.to_string());
}
