#!/usr/bin/env sh
# Build the `proof-panel` Tier-1 WASM extension — a wasm32-wasip2 component (excluded from the host
# workspace, built in its own target/) plus its federated UI bundle. Emits:
#   target/wasm32-wasip2/release/proof_panel_ext.wasm   (the component the host loads)
#   ui/dist/assets/remoteEntry.js                        (the federation remote the shell mounts)
set -e
cd "$(dirname "$0")"

echo "==> building wasm component"
cargo build --target wasm32-wasip2 --release
echo "built: $(pwd)/target/wasm32-wasip2/release/proof_panel_ext.wasm"

if [ -d ui ]; then
  echo "==> building federated UI bundle"
  (cd ui && pnpm install --frozen-lockfile && pnpm build)
  echo "built: $(pwd)/ui/dist/assets/remoteEntry.js"
fi
