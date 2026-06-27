#!/usr/bin/env bash
# Build the `hello-v2` wasm extension — the hot-reload swap target (same `hello` id, bumped
# version). A wasm32-wasip2 component, excluded from the workspace, built in its own target/.
# Produces:
#   target/wasm32-wasip2/release/hello_v2_ext.wasm   (the path crates/host hot_reload_test loads).
set -euo pipefail
cd "$(dirname "$0")"
cargo build --target wasm32-wasip2 --release
echo "built: $(pwd)/target/wasm32-wasip2/release/hello_v2_ext.wasm"
