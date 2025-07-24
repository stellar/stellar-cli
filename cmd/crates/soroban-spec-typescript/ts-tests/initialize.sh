#!/bin/bash

set -e

# export .env file as env vars
set -a
source .env
set +a

echo Network
echo "  RPC:        $STELLAR_RPC_URL"
echo "  Passphrase: \"$STELLAR_NETWORK_PASSPHRASE\""

NETWORK_STATUS=$(curl -s -X POST "http://localhost:8000/rpc" -H "Content-Type: application/json" -d '{ "jsonrpc": "2.0", "id": 8675309, "method": "getHealth" }' | sed 's/.*"status":"\([^"]*\)".*/\1/') || { echo "Make sure you're running local RPC network on localhost:8000" && exit 1; }
echo "  Status:     $NETWORK_STATUS"

if [[ "$NETWORK_STATUS" != "healthy" ]]; then
  echo "Network is not healthy (not running?), exiting"
  exit 1
fi

function fund_all() {
  local output
  local exit_code

  set +e
  output=$(./stellar keys generate root 2>&1)
  set -e
  exit_code=$?

  if [[ "$output" == *"already exists"* ]]; then
    echo "Reusing existing root account"
  elif [ $exit_code -ne 0 ]; then
    echo "Failed to generate root account:"
    echo "$output"
    exit 1
  fi

  ./stellar keys fund root
}

function upload() {
  ./stellar contract $1 --source root --wasm $2 > $3
}

function deploy_all() {
  upload deploy ../../../../target/wasm32v1-none/test-wasms/test_custom_types.wasm contract-id-custom-types.txt
  upload upload ../../../../target/wasm32v1-none/test-wasms/test_constructor.wasm contract-wasm-hash-constructor.txt

  set +e
  output=$(./stellar contract asset deploy --asset native --source root 2>&1)
  exit_code=$?
  set -e

  if [[ "$output" == *"contract already exists"* ]]; then
    echo "Native contract already deployed"
  elif [ $exit_code -ne 0 ]; then
    echo "Native contract deployment failed with error:"
    echo "$output"
    exit 1
  fi
}

function bind() {
  ./stellar contract bindings typescript $1 $2 --output-dir ./node_modules/$3 --overwrite
  sh -c "cd ./node_modules/$3 && npm install && npm run build"
}

function bind_all() {
  bind --contract-id $(cat contract-id-custom-types.txt) test-custom-types
  bind --wasm-hash $(cat contract-wasm-hash-constructor.txt) test-constructor
  bind --contract-id $(./stellar contract id asset --asset native) xlm
}

set -x
fund_all
deploy_all
bind_all
