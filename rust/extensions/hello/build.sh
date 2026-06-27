#!/usr/bin/env bash
# Build the `hello` wasm extension — a wasm32-wasip2 component (excluded from the workspace,
# so it builds in its own target/ dir). Produces:
#   target/wasm32-wasip2/release/hello_ext.wasm   (the path crates/host tests load).
set -euo pipefail
cd "$(dirname "$0")"
cargo build --target wasm32-wasip2 --release
echo "built: $(pwd)/target/wasm32-wasip2/release/hello_ext.wasm"
