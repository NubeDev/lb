#!/usr/bin/env bash
# Build the `control-engine` extension — the native (Tier-2) CE bridge extension
# (control-engine scope). EXCLUDED from the host workspace (it pins `rubix-ce`, a private
# git dep), so this builds it STANDALONE from its own dir. Produces:
#   rust/target/{debug,release}/control-engine   (the binary the host supervisor spawns over stdio)
#   ui/dist/remoteEntry.js                        (the S7 federated wiresheet page the shell dynamic-imports)
set -euo pipefail
cd "$(dirname "$0")"
# Share the workspace target dir so the supervisor + host integration test find the binary at
# rust/target/{debug,release}/control-engine (unchanged from when this was a workspace member).
# CWD is rust/extensions/control-engine, so ../.. = rust/.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(cd ../.. && pwd)/target}"
cargo build
cargo build --release
echo "built: control-engine (${CARGO_TARGET_DIR}/{debug,release}/control-engine)"

# S7 federated UI bundle. The ext UI is a STANDALONE package (its own lockfile, --ignore-workspace) that
# resolves the vendored `@nube/ce-wiresheet` by ALIAS to its BUILT dist (vite.config.ts) — so build the
# vendored lib FIRST, then the CE remote. Mirrors proof-panel/build.sh: invoke the local vite binary
# directly (pnpm's pre-run deps-status check hard-fails under a restrictive build-scripts policy).
if [ -d ui ]; then
  REPO_ROOT="$(cd ../../.. && pwd)"
  echo "==> building vendored @nube/ce-wiresheet dist (the ext UI aliases it)"
  ( cd "$REPO_ROOT/packages/ce-wiresheet" && pnpm install --frozen-lockfile || pnpm install || true; \
    pnpm run build:lib )
  echo "built: $REPO_ROOT/packages/ce-wiresheet/dist/ce-wiresheet.js"

  echo "==> building federated UI bundle"
  cd ui
  pnpm install --ignore-workspace --frozen-lockfile || pnpm install --ignore-workspace || true
  ./node_modules/.bin/vite build
  cd ..
  echo "built: $(pwd)/ui/dist/remoteEntry.js"
fi
