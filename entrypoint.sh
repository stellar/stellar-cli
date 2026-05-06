#!/bin/bash
set -e

if ! rustup target add wasm32v1-none; then
  echo "warning: failed to install rust target wasm32v1-none; continuing so non-build commands can still run" >&2
fi
exec "$@"
