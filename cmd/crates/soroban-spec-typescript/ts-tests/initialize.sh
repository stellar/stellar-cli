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

NETWORK_STATUS=$(curl -s -X POST "http://localhost:8000/soroban/rpc" -H "Content-Type: application/json" -d '{ "jsonrpc": "2.0", "id": 8675309, "method": "getHealth" }' | sed 's/.*"status":"\([^"]*\)".*/\1/') || { echo "Make sure you're running local RPC network on localhost:8000" && exit 1; }
echo "  Status:     $NETWORK_STATUS"

if [[ "$NETWORK_STATUS" != "healthy" ]]; then
  echo "Network is not healthy (not running?), exiting"
  exit 1
fi

# Print command before executing, from https://stackoverflow.com/a/23342259/249801
# Discussion: https://github.com/stellar/soroban-tools/pull/1034#pullrequestreview-1690667116
exe() { echo"${@/eval/}" ; "$@" ; }

function fund_all() {
  exe eval "./stellar keys generate root"
  exe eval "./stellar keys fund root"
}
function deploy() {
  exe eval "(./stellar contract deploy --quiet --source root --wasm $1 --ignore-checks) > $2"
}
function deploy_all() {
  deploy ../../../../target/wasm32-unknown-unknown/test-wasms/test_custom_types.wasm contract-id-custom-types.txt
}
function bind() {
  exe eval "./stellar contract bindings typescript --contract-id $(cat $1) --output-dir ./node_modules/$2 --overwrite"
}
function bind_all() {
  bind contract-id-custom-types.txt test-custom-types
}

fund_all
deploy_all
bind_all
