#!/bin/bash
set -e

rustup target add wasm32v1-none
exec "$@"
