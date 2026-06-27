#!/usr/bin/env bash
# Build every Lazybones extension. The single entry point — CI and humans run this.
#
# Each extension owns a `build.sh` next to its Cargo.toml (it knows whether it is a
# wasm32-wasip2 component built in its own target/, or a native workspace binary). This
# orchestrator just discovers and runs them, so a new extension is picked up the moment it
# adds a build.sh — no edit here required.
#
# Usage:
#   scripts/build-extensions.sh             # build all extensions
#   scripts/build-extensions.sh hello       # build only the named extension(s)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EXT_DIR="$ROOT/rust/extensions"

# Resolve the build list: explicit args, else every extension with a build.sh.
if [ "$#" -gt 0 ]; then
  targets=("$@")
else
  targets=()
  for s in "$EXT_DIR"/*/build.sh; do
    [ -f "$s" ] || continue
    targets+=("$(basename "$(dirname "$s")")")
  done
fi

if [ "${#targets[@]}" -eq 0 ]; then
  echo "no extensions to build under $EXT_DIR" >&2
  exit 1
fi

fail=0
for name in "${targets[@]}"; do
  script="$EXT_DIR/$name/build.sh"
  if [ ! -f "$script" ]; then
    echo "::error::no build.sh for extension '$name' ($script)" >&2
    fail=1
    continue
  fi
  echo "==> building $name"
  if ! bash "$script"; then
    echo "::error::build failed: $name" >&2
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  echo "one or more extensions failed to build" >&2
  exit 1
fi
echo "all extensions built (${#targets[@]}): ${targets[*]}"
