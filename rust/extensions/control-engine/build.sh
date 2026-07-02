#!/usr/bin/env bash
# Build the `control-engine` extension — the native (Tier-2) CE bridge extension
# (control-engine scope). A host-platform binary AND a workspace member, so it builds
# for the host target via the shared workspace target/ dir. Produces:
#   rust/target/release/control-engine  (the binary the host supervisor spawns over stdio).
set -euo pipefail
cd "$(dirname "$0")"
# -p against the workspace so it shares the workspace lockfile and target dir.
cargo build --release -p control-engine
echo "built: control-engine (workspace target/release/control-engine)"
