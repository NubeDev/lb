#!/usr/bin/env bash
# Build the `fleet-monitor` extension — BOTH halves of the one self-contained extension:
#   1. the BACKEND native Tier-2 binary (host-target, a workspace member, shared target/ dir):
#        rust/target/release/fleet-monitor   (the binary the host supervisor spawns over stdio)
#   2. the FRONTEND module-federation remote (the shadcn page + two widgets), emitted to
#        ui/dist/ — copied under the gateway's LB_EXT_UI_DIR/fleet-monitor/ to be served.
set -euo pipefail
cd "$(dirname "$0")"

echo "==> backend: cargo build --release -p fleet-monitor"
cargo build --release -p fleet-monitor

echo "==> frontend: pnpm install && pnpm build (module-federation remote)"
cd ui
pnpm install --frozen-lockfile 2>/dev/null || pnpm install
pnpm build
cd ..

# Stage the built remote where the gateway serves extension UIs: it reads `{LB_EXT_UI_DIR}/{ext}/{path}`
# (default `extensions-ui/` beside the node's cwd), so the served container ends up at
# `{LB_EXT_UI_DIR}/fleet-monitor/assets/remoteEntry.js` — exactly the manifest's `[ui] entry`.
EXT_UI_DIR="${LB_EXT_UI_DIR:-../../extensions-ui}"
DEST="$EXT_UI_DIR/fleet-monitor"
mkdir -p "$DEST"
rm -rf "$DEST"/*
cp -r ui/dist/* "$DEST"/

echo "built: fleet-monitor backend (target/release/fleet-monitor) + frontend remote staged at $DEST"
