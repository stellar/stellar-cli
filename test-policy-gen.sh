#!/bin/bash
set -e

# Create test directories
mkdir -p test-policy/time-based
mkdir -p test-policy/amount-based
mkdir -p test-policy/multi-sig

# Build the test contract first
cargo build --target wasm32-unknown-unknown --release

# Test time-based policy
echo "Generating time-based policy..."
cargo run --bin soroban -- contract policy \
    --wasm target/wasm32-unknown-unknown/release/test_contract.wasm \
    --policy-type time-based \
    --out-dir test-policy/time-based \
    --params '{"duration": 3600}'

# Test amount-based policy
echo "Generating amount-based policy..."
cargo run --bin soroban -- contract policy \
    --wasm target/wasm32-unknown-unknown/release/test_contract.wasm \
    --policy-type amount-based \
    --out-dir test-policy/amount-based \
    --params '{"limit": 5000}'

# Test multi-sig policy
echo "Generating multi-sig policy..."
cargo run --bin soroban -- contract policy \
    --wasm target/wasm32-unknown-unknown/release/test_contract.wasm \
    --policy-type multi-sig \
    --out-dir test-policy/multi-sig \
    --params '{"required_signatures": 3}'

# List generated files
echo "Generated policy contracts:"
ls -R test-policy/ 