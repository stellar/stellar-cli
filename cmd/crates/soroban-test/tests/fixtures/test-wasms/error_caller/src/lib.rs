#![no_std]
use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, panic_with_error, Address, Env, IntoVal,
    InvokeError, Symbol,
};

/// Mirror of the inner contract's error enum.
/// Must match the error codes defined in custom_types contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InnerError {
    /// Please provide an odd number
    NumberMustBeOdd = 1,
}

/// Minimal client interface for the custom_types contract.
#[contractclient(name = "CustomTypesClient")]
pub trait CustomTypesInterface {
    fn u32_fail_on_even(env: Env, u32_: u32) -> Result<u32, InnerError>;
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OuterError {
    /// Caught inner error and remapped
    RemappedInner = 10,
    /// Uses the same error code as the inner contract
    SameCodeAsInner = 1,
}

#[contract]
pub struct ErrorCallerContract;

#[contractimpl]
impl ErrorCallerContract {
    /// Try-calls inner's u32_fail_on_even. Catches error, returns OuterError::RemappedInner.
    pub fn catch_call(env: Env, inner: Address, u32_: u32) -> Result<u32, OuterError> {
        match env.try_invoke_contract::<u32, InvokeError>(
            &inner,
            &Symbol::new(&env, "u32_fail_on_even"),
            (u32_,).into_val(&env),
        ) {
            Ok(Ok(val)) => Ok(val),
            _ => Err(OuterError::RemappedInner),
        }
    }

    /// Try-calls inner's u32_fail_on_even. Catches error and returns SameCodeAsInner.
    pub fn catch_call_same_code(env: Env, inner: Address, u32_: u32) -> Result<u32, OuterError> {
        match env.try_invoke_contract::<u32, InvokeError>(
            &inner,
            &Symbol::new(&env, "u32_fail_on_even"),
            (u32_,).into_val(&env),
        ) {
            Ok(Ok(val)) => Ok(val),
            _ => Err(OuterError::SameCodeAsInner),
        }
    }

    /// Try-calls inner via contractclient. Catches error, returns OuterError::RemappedInner.
    pub fn catch_call_import(env: Env, inner: Address, u32_: u32) -> Result<u32, OuterError> {
        let client = CustomTypesClient::new(&env, &inner);
        match client.try_u32_fail_on_even(&u32_) {
            Ok(Ok(val)) => Ok(val),
            _ => Err(OuterError::RemappedInner),
        }
    }

    /// Non-try call to inner's u32_fail_on_even. If inner fails, propagates as VM trap.
    pub fn call(env: Env, inner: Address, u32_: u32) -> Result<u32, OuterError> {
        Ok(env.invoke_contract(
            &inner,
            &Symbol::new(&env, "u32_fail_on_even"),
            (u32_,).into_val(&env),
        ))
    }

    /// Non-try call to inner via contractclient. If inner fails, propagates as VM trap.
    pub fn call_import(env: Env, inner: Address, u32_: u32) -> Result<u32, OuterError> {
        let client = CustomTypesClient::new(&env, &inner);
        Ok(client.u32_fail_on_even(&u32_))
    }

    /// Try-calls inner but returns non-Result type. Panics with error if inner fails.
    /// Since this function doesn't return Result, the error shouldn't be resolved.
    pub fn catch_panic_no_result(env: Env, inner: Address, u32_: u32) -> u32 {
        let client = CustomTypesClient::new(&env, &inner);
        match client.try_u32_fail_on_even(&u32_) {
            Ok(Ok(val)) => val,
            _ => panic_with_error!(&env, OuterError::RemappedInner),
        }
    }
}
