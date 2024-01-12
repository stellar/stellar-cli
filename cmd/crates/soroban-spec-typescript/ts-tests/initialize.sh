#!/bin/bash

# read .env file, but prefer explicitly set environment variables
IFS=$'\n'
for l in $(cat .env); do
    IFS='=' read -ra VARVAL <<< "$l"
    # If variable with such name already exists, preserves its value
    eval "export ${VARVAL[0]}=\${${VARVAL[0]}:-${VARVAL[1]}}"
done
unset IFS

echo Network
echo "  RPC:        $SOROBAN_RPC_URL"
echo "  Passphrase: \"$SOROBAN_NETWORK_PASSPHRASE\""

NETWORK_STATUS=$(curl -s -X POST "http://localhost:8000/soroban/rpc" -H "Content-Type: application/json" -d '{ "jsonrpc": "2.0", "id": 8675309, "method": "getHealth" }' | sed 's/.*"status":"\(.*\)".*/\1/') || { echo "Make sure you're running local RPC network on localhost:8000" && exit 1; }
echo "  Status:     $NETWORK_STATUS"

if [[ "$NETWORK_STATUS" != "healthy" ]]; then
  echo "Network is not healthy (not running?), exiting"
  exit 1
fi

# Print command before executing, from https://stackoverflow.com/a/23342259/249801
# Discussion: https://github.com/stellar/soroban-tools/pull/1034#pullrequestreview-1690667116
exe() { echo"${@/eval/}" ; "$@" ; }

function fund_all() {
  exe eval "./soroban keys generate root"
  exe eval "./soroban keys fund root"
  exe eval "./soroban keys generate alice"
  exe eval "./soroban keys fund alice"
  exe eval "./soroban keys generate bob"
  exe eval "./soroban keys fund bob"
}
function upload() {
  exe eval "(./soroban contract $1 --source root --wasm $2 --ignore-checks) > $3"
}
function deploy() {
  exe eval "(./soroban contract deploy --source root --wasm-hash $(cat $1) --ignore-checks) > $2"
}
function deploy_all() {
  upload deploy ../../../../target/wasm32-unknown-unknown/test-wasms/test_custom_types.wasm contract-id-custom-types.txt
  upload deploy ../../../../target/wasm32-unknown-unknown/test-wasms/test_hello_world.wasm contract-id-hello-world.txt
  upload deploy ../../../../target/wasm32-unknown-unknown/test-wasms/test_swap.wasm contract-id-swap.txt
  upload install ../../../../target/wasm32-unknown-unknown/test-wasms/test_token.wasm contract-token-hash.txt
  deploy contract-token-hash.txt contract-id-token-a.txt
  deploy contract-token-hash.txt contract-id-token-b.txt
}
function initialize() {
   exe eval "./soroban contract invoke --source root --id $(cat $1) -- initialize --admin $(./soroban keys address root) --decimal 0 --name 'Token $2' --symbol '$2'"
}
function initialize_all() {
  initialize contract-id-token-a.txt A
  initialize contract-id-token-b.txt B
}
function bind() {
  exe eval "./soroban contract bindings typescript --contract-id $(cat $1) --output-dir ./node_modules/$2 --overwrite"
}
function bind_all() {
  bind contract-id-custom-types.txt test-custom-types
  bind contract-id-hello-world.txt test-hello-world
  bind contract-id-swap.txt test-swap
  bind contract-id-token-a.txt token
}

function mint() {
  exe eval "./soroban contract invoke --source root --id $(cat $1) -- mint --amount 2000000 --to $(./soroban keys address $2)"
}
function mint_all() {
  mint contract-id-token-a.txt alice
  mint contract-id-token-b.txt bob
}

fund_all
deploy_all
initialize_all
mint_all
bind_all
