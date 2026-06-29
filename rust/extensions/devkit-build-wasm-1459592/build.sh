#!/usr/bin/env sh
# Build both halves of a generated extension. The host devkit uses its Toolchain trait directly, but
# this script keeps the folder compatible with the existing reference-extension dev flow.
set -e
cd "$(dirname "$0")"

if grep -q 'tier = "wasm"' extension.toml; then
  cargo build --target wasm32-wasip2 --release
else
  cargo build --release
fi

if [ -d ui ]; then
  cd ui
  pnpm install
  pnpm exec vite build
fi
