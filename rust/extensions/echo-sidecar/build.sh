#!/usr/bin/env bash
# Build the `echo-sidecar` extension — the reference NATIVE Tier-2 extension. Unlike the wasm
# extensions, this is a host-platform binary AND a workspace member, so it builds for the host
# target via the shared workspace target/ dir (not wasm, not an isolated target/). Produces:
#   rust/target/release/echo-sidecar   (the binary the host supervisor spawns over stdio).
set -euo pipefail
cd "$(dirname "$0")"
# -p against the workspace so it shares the workspace lockfile and target dir.
cargo build --release -p echo-sidecar
echo "built: echo-sidecar (workspace target/release/echo-sidecar)"
