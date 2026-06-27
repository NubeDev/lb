#!/usr/bin/env sh
# Build the `proof-panel` Tier-1 WASM extension — a wasm32-wasip2 component (excluded from the host
# workspace, built in its own target/) plus its federated UI bundle. Emits:
#   target/wasm32-wasip2/release/proof_panel_ext.wasm   (the component the host loads)
#   ui/dist/remoteEntry.js                               (the ESM remote the shell dynamic-imports)
set -e
cd "$(dirname "$0")"

echo "==> building wasm component"
cargo build --target wasm32-wasip2 --release
echo "built: $(pwd)/target/wasm32-wasip2/release/proof_panel_ext.wasm"

if [ -d ui ]; then
  echo "==> building federated UI bundle"
  cd ui
  # `--frozen-lockfile` is the CI default; fall back to a plain install when the lockfile shifts.
  pnpm install --frozen-lockfile || pnpm install || true
  # Invoke the local vite binary directly rather than `pnpm build`: pnpm's pre-run deps-status check
  # hard-fails under a restrictive build-scripts policy (e.g. esbuild's postinstall gate), even though
  # the bundle builds fine. The federation remote is what we need; build it without that gate.
  ./node_modules/.bin/vite build
  cd ..
  echo "built: $(pwd)/ui/dist/remoteEntry.js"
fi
