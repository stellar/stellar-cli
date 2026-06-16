use soroban_cli::commands;
use soroban_test::TestEnv;

use crate::integration::util::{
    deploy_contract, deploy_custom, deploy_error_caller, extend_contract, DeployOptions,
    CUSTOM_TYPES,
};

#[tokio::test]
async fn direct_result_error_resolves_name() {
    let sandbox = &TestEnv::new();
    let id = &deploy_custom(sandbox).await;
    extend_contract(sandbox, id).await;

    let err = sandbox
        .invoke_with_test(&["--id", id, "--", "u32_fail_on_even", "--u32_=2"])
        .await
        .unwrap_err();

    match &err {
        commands::contract::invoke::Error::ContractInvoke {
            detail, message, ..
        } => {
            assert!(
                detail.starts_with("NumberMustBeOdd"),
                "expected detail to start with 'NumberMustBeOdd', got: {detail}"
            );
            assert!(
                message.contains("NumberMustBeOdd"),
                "expected message to include 'NumberMustBeOdd', got: {message}"
            );
        }
        other => panic!("expected ContractInvoke error, got: {other:#?}"),
    }
}

#[tokio::test]
async fn panic_with_error_resolves_name() {
    let sandbox = &TestEnv::new();
    let id = &deploy_custom(sandbox).await;
    extend_contract(sandbox, id).await;

    let err = sandbox
        .invoke_with_test(&["--id", id, "--", "panic_on_even", "--u32_=2"])
        .await
        .unwrap_err();

    match &err {
        commands::contract::invoke::Error::ContractInvoke {
            detail, message, ..
        } => {
            assert!(
                detail.starts_with("NumberMustBeOdd"),
                "expected detail to start with 'NumberMustBeOdd', got: {detail}"
            );
            assert!(
                message.contains("NumberMustBeOdd"),
                "expected message to include 'NumberMustBeOdd', got: {message}"
            );
        }
        other => panic!("expected ContractInvoke error, got: {other:#?}"),
    }
}

#[tokio::test]
async fn cross_contract_catch_call_resolves_outer_error() {
    let sandbox = &TestEnv::new();

    let inner_id = &deploy_contract(sandbox, CUSTOM_TYPES, DeployOptions::default()).await;
    extend_contract(sandbox, inner_id).await;

    let outer_id = &deploy_error_caller(sandbox).await;
    extend_contract(sandbox, outer_id).await;

    let err = sandbox
        .invoke_with_test(&[
            "--id",
            outer_id,
            "--",
            "catch_call",
            "--inner",
            inner_id,
            "--u32_=2",
        ])
        .await
        .unwrap_err();

    match &err {
        commands::contract::invoke::Error::ContractInvoke {
            detail, message, ..
        } => {
            assert!(
                detail.starts_with("RemappedInner"),
                "expected detail to start with 'RemappedInner', got: {detail}"
            );
            assert!(
                message.contains("RemappedInner"),
                "expected message to include 'RemappedInner', got: {message}"
            );
        }
        other => panic!("expected ContractInvoke error, got: {other:#?}"),
    }
}

#[tokio::test]
async fn cross_contract_same_code_prefers_outer_error_name() {
    let sandbox = &TestEnv::new();

    let inner_id = &deploy_contract(sandbox, CUSTOM_TYPES, DeployOptions::default()).await;
    extend_contract(sandbox, inner_id).await;

    let outer_id = &deploy_error_caller(sandbox).await;
    extend_contract(sandbox, outer_id).await;

    let err = sandbox
        .invoke_with_test(&[
            "--id",
            outer_id,
            "--",
            "catch_call_same_code",
            "--inner",
            inner_id,
            "--u32_=2",
        ])
        .await
        .unwrap_err();

    match &err {
        commands::contract::invoke::Error::ContractInvoke {
            detail, message, ..
        } => {
            assert!(
                detail.starts_with("SameCodeAsInner"),
                "expected detail to start with 'SameCodeAsInner', got: {detail}"
            );
            assert!(
                message.contains("SameCodeAsInner"),
                "expected message to include 'SameCodeAsInner', got: {message}"
            );
        }
        other => panic!("expected ContractInvoke error, got: {other:#?}"),
    }
}

#[tokio::test]
async fn cross_contract_import_try_resolves_outer_error() {
    let sandbox = &TestEnv::new();

    let inner_id = &deploy_contract(sandbox, CUSTOM_TYPES, DeployOptions::default()).await;
    extend_contract(sandbox, inner_id).await;

    let outer_id = &deploy_error_caller(sandbox).await;
    extend_contract(sandbox, outer_id).await;

    let err = sandbox
        .invoke_with_test(&[
            "--id",
            outer_id,
            "--",
            "catch_call_import",
            "--inner",
            inner_id,
            "--u32_=2",
        ])
        .await
        .unwrap_err();

    match &err {
        commands::contract::invoke::Error::ContractInvoke {
            detail, message, ..
        } => {
            assert!(
                detail.starts_with("RemappedInner"),
                "expected detail to start with 'RemappedInner', got: {detail}"
            );
            assert!(
                message.contains("RemappedInner"),
                "expected message to include 'RemappedInner', got: {message}"
            );
        }
        other => panic!("expected ContractInvoke error, got: {other:#?}"),
    }
}

#[tokio::test]
async fn cross_contract_import_non_try_does_not_resolve() {
    let sandbox = &TestEnv::new();

    let inner_id = &deploy_contract(sandbox, CUSTOM_TYPES, DeployOptions::default()).await;
    extend_contract(sandbox, inner_id).await;

    let outer_id = &deploy_error_caller(sandbox).await;
    extend_contract(sandbox, outer_id).await;

    let err = sandbox
        .invoke_with_test(&[
            "--id",
            outer_id,
            "--",
            "call_import",
            "--inner",
            inner_id,
            "--u32_=2",
        ])
        .await
        .unwrap_err();

    assert!(
        !matches!(
            &err,
            commands::contract::invoke::Error::ContractInvoke { .. }
        ),
        "expected non-ContractInvoke error for trapped cross-contract call, got: {err:#?}"
    );
}

#[tokio::test]
async fn cross_contract_non_try_does_not_resolve() {
    let sandbox = &TestEnv::new();

    let inner_id = &deploy_contract(sandbox, CUSTOM_TYPES, DeployOptions::default()).await;
    extend_contract(sandbox, inner_id).await;

    let outer_id = &deploy_error_caller(sandbox).await;
    extend_contract(sandbox, outer_id).await;

    let err = sandbox
        .invoke_with_test(&[
            "--id", outer_id, "--", "call", "--inner", inner_id, "--u32_=2",
        ])
        .await
        .unwrap_err();

    assert!(
        !matches!(
            &err,
            commands::contract::invoke::Error::ContractInvoke { .. }
        ),
        "expected non-ContractInvoke error for trapped cross-contract call, got: {err:#?}"
    );
}

#[tokio::test]
async fn panic_with_error_no_result_type_does_not_resolve() {
    let sandbox = &TestEnv::new();

    let inner_id = &deploy_contract(sandbox, CUSTOM_TYPES, DeployOptions::default()).await;
    extend_contract(sandbox, inner_id).await;

    let outer_id = &deploy_error_caller(sandbox).await;
    extend_contract(sandbox, outer_id).await;

    // catch_panic_no_result uses try_* but returns u32, not Result.
    // When inner fails, it panics with OuterError::RemappedInner.
    // Since the function doesn't return Result, the error shouldn't be resolved.
    let err = sandbox
        .invoke_with_test(&[
            "--id",
            outer_id,
            "--",
            "catch_panic_no_result",
            "--inner",
            inner_id,
            "--u32_=2",
        ])
        .await
        .unwrap_err();

    assert!(
        !matches!(
            &err,
            commands::contract::invoke::Error::ContractInvoke { .. }
        ),
        "expected non-ContractInvoke error when function doesn't return Result, got: {err:#?}"
    );
}
