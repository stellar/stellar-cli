# Contract Invocation Best Practices

This document provides best practices for assembling argument objects when invoking Stellar contracts via the CLI, especially when using bash scripts or dynamic JSON generation.

## Common Issues and Solutions

### 1. JSON Syntax Errors

**Problem**: Malformed JSON in dynamically generated arguments
```bash
# ❌ WRONG - Missing quotes around string values
stellar contract invoke --id $CONTRACT_ID -- transfer --from alice --to bob --amount {value: 100}

# ❌ WRONG - Trailing comma
stellar contract invoke --id $CONTRACT_ID -- transfer --from alice --to bob --amount {"value": 100,}
```

**Solution**: Always validate JSON syntax
```bash
# ✅ CORRECT - Proper JSON formatting
stellar contract invoke --id $CONTRACT_ID -- transfer --from alice --to bob --amount '{"value": 100}'

# ✅ CORRECT - Use jq to validate JSON before passing
AMOUNT_JSON='{"value": 100}'
echo "$AMOUNT_JSON" | jq . > /dev/null && echo "Valid JSON" || echo "Invalid JSON"
stellar contract invoke --id $CONTRACT_ID -- transfer --from alice --to bob --amount "$AMOUNT_JSON"
```

### 2. Type Mismatches

**Problem**: Passing wrong data types
```bash
# ❌ WRONG - Passing string when number expected
stellar contract invoke --id $CONTRACT_ID -- set_balance --account alice --balance "100"

# ❌ WRONG - Passing number when string expected  
stellar contract invoke --id $CONTRACT_ID -- set_name --account alice --name 123
```

**Solution**: Match the expected types from contract specification
```bash
# ✅ CORRECT - Number without quotes
stellar contract invoke --id $CONTRACT_ID -- set_balance --account alice --balance 100

# ✅ CORRECT - String with quotes
stellar contract invoke --id $CONTRACT_ID -- set_name --account alice --name "Alice Smith"
```

### 3. Address Format Issues

**Problem**: Incorrect address formats
```bash
# ❌ WRONG - Invalid address format
stellar contract invoke --id $CONTRACT_ID -- transfer --from invalid_address --to bob --amount 100
```

**Solution**: Use proper address formats
```bash
# ✅ CORRECT - Account address (starts with G)
stellar contract invoke --id $CONTRACT_ID -- transfer --from GDAT5HWTGIU4TSSZ4752OUC4SABDLTLZFRPZUJ3D6LKBNEPA7V2CIG54 --to bob --amount 100

# ✅ CORRECT - Contract address (starts with C)
stellar contract invoke --id $CONTRACT_ID -- call_contract --target CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM

# ✅ CORRECT - Identity name
stellar contract invoke --id $CONTRACT_ID -- transfer --from alice --to bob --amount 100
```

### 4. Complex Object Assembly

**Problem**: Errors in complex nested objects
```bash
# ❌ WRONG - Malformed nested JSON
COMPLEX_ARG='{
  "user": {
    "name": "Alice"
    "age": 30
  }
}'
```

**Solution**: Use proper JSON construction techniques
```bash
# ✅ CORRECT - Use jq for complex JSON construction
COMPLEX_ARG=$(jq -n \
  --arg name "Alice" \
  --argjson age 30 \
  '{
    user: {
      name: $name,
      age: $age
    }
  }')

stellar contract invoke --id $CONTRACT_ID -- update_user --data "$COMPLEX_ARG"

# ✅ CORRECT - Use file-based arguments for very complex data
echo '{
  "user": {
    "name": "Alice",
    "age": 30,
    "preferences": {
      "theme": "dark",
      "notifications": true
    }
  }
}' > user_data.json

stellar contract invoke --id $CONTRACT_ID -- update_user --data-file-path user_data.json
```

## Bash Script Best Practices

### 1. Variable Escaping
```bash
# ✅ CORRECT - Proper variable escaping
USER_NAME="Alice Smith"
AMOUNT=100

# Use double quotes to allow variable expansion, single quotes for JSON
stellar contract invoke --id "$CONTRACT_ID" -- transfer \
  --from alice \
  --to bob \
  --amount "$AMOUNT" \
  --memo '{"note": "Payment for services"}'
```

### 2. Error Handling
```bash
#!/bin/bash

# ✅ CORRECT - Add error handling
set -e  # Exit on any error

CONTRACT_ID="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM"
AMOUNT=100

# Validate inputs
if [[ -z "$CONTRACT_ID" ]]; then
  echo "Error: CONTRACT_ID is required"
  exit 1
fi

if [[ ! "$AMOUNT" =~ ^[0-9]+$ ]]; then
  echo "Error: AMOUNT must be a number"
  exit 1
fi

# Execute with error handling
if ! stellar contract invoke --id "$CONTRACT_ID" -- transfer --from alice --to bob --amount "$AMOUNT"; then
  echo "Error: Contract invocation failed"
  exit 1
fi

echo "Contract invocation successful"
```

### 3. JSON Validation Function
```bash
#!/bin/bash

# Helper function to validate JSON
validate_json() {
  local json="$1"
  if echo "$json" | jq . >/dev/null 2>&1; then
    return 0
  else
    echo "Error: Invalid JSON: $json" >&2
    return 1
  fi
}

# Usage
USER_DATA='{"name": "Alice", "age": 30}'
if validate_json "$USER_DATA"; then
  stellar contract invoke --id "$CONTRACT_ID" -- update_user --data "$USER_DATA"
else
  echo "Skipping contract invocation due to invalid JSON"
  exit 1
fi
```

## File-Based Arguments

For complex arguments, use file-based input:

```bash
# Create argument file
cat > transfer_args.json << EOF
{
  "from": "alice",
  "to": "bob", 
  "amount": 100,
  "memo": {
    "type": "payment",
    "description": "Monthly subscription",
    "metadata": {
      "invoice_id": "INV-2024-001",
      "due_date": "2024-01-31"
    }
  }
}
EOF

# Validate the JSON file
jq . transfer_args.json > /dev/null

# Use file-based arguments
stellar contract invoke --id "$CONTRACT_ID" -- complex_transfer \
  --transfer-data-file-path transfer_args.json
```

## Debugging Tips

### 1. Use --help to see expected argument types
```bash
stellar contract invoke --id "$CONTRACT_ID" -- --help
stellar contract invoke --id "$CONTRACT_ID" -- function_name --help
```

### 2. Test JSON separately
```bash
# Test your JSON with jq before using it
echo '{"key": "value"}' | jq .
```

### 3. Use dry-run mode (if available)
```bash
# Use simulation mode to test without executing
stellar contract invoke --id "$CONTRACT_ID" --send=no -- function_name --arg value
```

## Error Message Interpretation

The enhanced CLI now provides detailed error messages:

- **JSON Validation Errors**: Check for missing quotes, trailing commas, or malformed syntax
- **Type Mismatch Errors**: Verify the argument type matches the contract specification  
- **Missing Argument Errors**: Ensure all required arguments are provided
- **Address Format Errors**: Use proper Stellar address formats (G..., C..., M..., or identity names)

For more help, use `stellar contract invoke --help` or check the contract specification with `stellar contract inspect --wasm contract.wasm`.
