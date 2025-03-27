# Function-Based Policy Generation Bug

## Issue Description

There is a bug in the function-based policy generation that causes the final policy not to properly include the selected functions. When generating a function-based policy using the interactive mode, the selected functions are correctly collected, but they don't appear in the final policy implementation.

## Root Cause

The issue is in how the policy template is being rendered in `cmd/crates/soroban-policy-generator/src/policy/function_based.rs`. 

Currently, the code is:

```rust
// First generate the policy implementation
let policy_impl = handlebars.render(
    "function_based_policy",
    &json!({
        "allowed_methods": method_configs,
    }),
).map_err(|e| Error::Render(e))?;

// Then generate the full contract
handlebars.render(
    "lib_rs",
    &json!({
        "policy_impl": policy_impl,
    }),
).map_err(|e| Error::Render(e))
```

But the `lib_rs` template doesn't actually use the `policy_impl` variable. Instead, it directly uses the `allowed_methods` context:

```
impl PolicyInterface for Contract {
    fn policy__(env: Env, _source: Address, _signer: SignerKey, contexts: Vec<Context>) {
        for context in contexts.iter() {
            match context {
                Context::Contract(ContractContext { fn_name, args, .. }) => {
{{#each allowed_methods}}                    if fn_name == symbol_short!("{{truncate this 9}}") { return; }
{{/each}}                }
                _ => panic_with_error!(&env, Error::NotAllowed),
            }
        }
        panic_with_error!(&env, Error::NotAllowed)
    }
}
```

## Fix

The fix is to pass the `allowed_methods` array directly to the `lib_rs` template instead of passing a pre-rendered `policy_impl`:

```rust
// Generate the full contract directly with the allowed methods
handlebars.render(
    "lib_rs",
    &json!({
        "allowed_methods": method_configs,
    }),
).map_err(|e| Error::Render(e))
```

Alternatively, the `lib_rs` template could be modified to use the `policy_impl` variable instead of directly using the `allowed_methods` context.

## Implementation Plan

1. Modify the `function_based.rs` file to correctly pass the `allowed_methods` directly to the `lib_rs` template.
2. Add a test case for function-based policy generation to ensure the bug doesn't reappear.
3. Consider adding validation to ensure the generated policy contains the expected functions. 