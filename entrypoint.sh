#!/bin/bash
set -e

cd /source
rustup target add wasm32v1-none
exec "$@"
