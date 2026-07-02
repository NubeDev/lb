#!/usr/bin/env bash
# Build the `control-engine` extension — the native (Tier-2) CE bridge extension
# (control-engine scope). A host-platform binary AND a workspace member, so it builds
# for the host target via the shared workspace target/ dir. Produces:
#   rust/target/release/control-engine   (the binary the host supervisor spawns over stdio)
#   ui/dist/remoteEntry.js               (the S7 federated wiresheet page the shell dynamic-imports)
set -euo pipefail
cd "$(dirname "$0")"
# -p against the workspace so it shares the workspace lockfile and target dir.
cargo build -p control-engine
cargo build --release -p control-engine
echo "built: control-engine (workspace target/{debug,release}/control-engine)"

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
