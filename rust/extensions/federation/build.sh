#!/usr/bin/env bash
# Build the `federation` extension — the native (Tier-2) datasources extension (datasources scope).
# A host-platform binary AND a workspace member, so it builds for the host target via the shared
# workspace target/ dir. Produces:
#   rust/target/release/federation   (the binary the host supervisor spawns over stdio).
set -euo pipefail
cd "$(dirname "$0")"
# -p against the workspace so it shares the workspace lockfile and target dir.
cargo build --release -p federation
echo "built: federation (workspace target/release/federation)"
