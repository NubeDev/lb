#!/usr/bin/env sh
# Build the `thecrew` Tier-1 WASM extension — a wasm32-wasip2 component (excluded from the host
# workspace, built in its own target/) plus its federated UI bundle. The proof-panel pattern.
# Emits:
#   target/wasm32-wasip2/release/thecrew_ext.wasm   (the zero-tool component the host loads)
#   ui/dist/remoteEntry.js                           (the ESM remote, built by vite)
#   {LB_EXT_UI_DIR}/thecrew/remoteEntry.js           (STAGED where the gateway actually serves it)
set -e
cd "$(dirname "$0")"

echo "==> building wasm component"
cargo build --target wasm32-wasip2 --release
echo "built: $(pwd)/target/wasm32-wasip2/release/thecrew_ext.wasm"

if [ -d ui ]; then
  echo "==> building federated UI bundle"
  cd ui
  # `--ignore-workspace`: this extension UI is a STANDALONE package (its own lockfile), NOT a member
  # of the repo-root pnpm workspace (ui/ + packages/*). Without the flag, pnpm walks up to the root
  # workspace and never installs THIS package's deps. `--frozen-lockfile` is the CI default; fall
  # back to a plain (ignore-workspace) install when the lockfile shifts.
  pnpm install --ignore-workspace --frozen-lockfile ||
    pnpm install --ignore-workspace || true
  # Invoke the local vite binary directly rather than `pnpm build`: pnpm's pre-run deps-status
  # check hard-fails under a restrictive build-scripts policy, even though the bundle builds fine.
  ./node_modules/.bin/vite build
  cd ..
  echo "built: $(pwd)/ui/dist/remoteEntry.js"

  # Stage the built remote where the gateway SERVES extension UIs — it reads
  # `{LB_EXT_UI_DIR}/{ext}/{path}` (ext_ui.rs), default `extensions-ui/` beside the repo's rust/.
  # Without this the shell keeps loading a stale copy no matter how often you rebuild (the
  # fleet-monitor pattern; a `vite build` into ui/dist/ alone never reaches the served dir).
  EXT_UI_DIR="${LB_EXT_UI_DIR:-../../extensions-ui}"
  DEST="$EXT_UI_DIR/thecrew"
  mkdir -p "$DEST"
  cp -r ui/dist/* "$DEST"/
  echo "staged: $DEST/remoteEntry.js"
fi
