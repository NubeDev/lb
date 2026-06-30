#!/usr/bin/env bash
# docker/build/build.sh — the in-container build driver (entrypoint of the build image).
#
# Builds the `node` binary (default) or any -p crate for one TARGET, using the real
# GCC cross-toolchain baked into the image. No zig, no host C-toolchain surprises.
#
# Usage (normally invoked via the Makefile `docker-*` targets, not by hand):
#   lb-build <target> [extra cargo args...]
#
# Recognised <target> values (add more by extending the case below + the Dockerfile):
#   linux-x86_64   x86_64-unknown-linux-gnu        (the default; plain `node` binary)
#   linux-arm64    aarch64-unknown-linux-gnu        (64-bit Pi / generic arm64)
#   linux-armv7    armv7-unknown-linux-gnueabifh    (32-bit Pi / armv7 edge)
#   windows-x86_64 x86_64-pc-windows-gnu            (node.exe)
#   deb            a .deb of the host x86_64 build (via cargo-deb)
#
# Env knobs:
#   PKG       cargo package to build (default: node)
#   PROFILE   release|debug (default: release)
#   FEATURES  cargo --features list (default: federation built with postgres below)
set -euo pipefail

TARGET_ALIAS="${1:-linux-x86_64}"; shift || true
PKG="${PKG:-node}"
PROFILE="${PROFILE:-release}"

# The repo's rust/.cargo/config.toml is HOST-specific: that box has no system C
# compiler, so it pins the linker/CC/AR/RANLIB at a personal zig toolchain
# (~/.local/bin/zig*). Inside this image those paths don't exist (→ `zigcc not
# found`), and we have a real GCC toolchain anyway. cargo's config `[env]` does not
# override a process env var unless it sets `force = true` (this one doesn't), so we
# win simply by exporting the genuine tools here. We also override the host
# `[target.*].linker`, which config CAN'T be beaten by env alone, via --config flags
# passed to every cargo call below.
host_override=(
  --config 'target.x86_64-unknown-linux-gnu.linker="x86_64-linux-gnu-gcc"'
)
export CC=x86_64-linux-gnu-gcc AR=x86_64-linux-gnu-ar RANLIB=x86_64-linux-gnu-ranlib
export CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc
export AR_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ar
export RANLIB_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ranlib

profile_flag="--release"; [ "$PROFILE" = "debug" ] && profile_flag=""

# The federation sidecar is a separate crate that needs `--features postgres` (the
# vendored-OpenSSL path this whole image exists to make work). The `node` build does
# NOT need that feature, so we only thread FEATURES through when the caller sets it.
feat_flag=""; [ -n "${FEATURES:-}" ] && feat_flag="--features ${FEATURES}"

run() { echo "+ $*" >&2; "$@"; }

case "$TARGET_ALIAS" in
  linux-x86_64)   RUST_TARGET=x86_64-unknown-linux-gnu ;;
  linux-arm64)    RUST_TARGET=aarch64-unknown-linux-gnu ;;
  linux-armv7)    RUST_TARGET=armv7-unknown-linux-gnueabihf ;;
  windows-x86_64) RUST_TARGET=x86_64-pc-windows-gnu ;;
  deb)
    # Build the host binary, then package it. cargo-deb reads [package.metadata.deb].
    run cargo build "${host_override[@]}" -p "$PKG" $profile_flag $feat_flag "$@"
    run cargo deb -p "$PKG" --no-build
    echo "→ .deb under target/debian/" >&2
    exit 0
    ;;
  *)
    echo "unknown target alias: $TARGET_ALIAS" >&2
    echo "known: linux-x86_64 linux-arm64 linux-armv7 windows-x86_64 deb" >&2
    exit 2
    ;;
esac

run cargo build "${host_override[@]}" -p "$PKG" --target "$RUST_TARGET" $profile_flag $feat_flag "$@"
echo "→ binary under target/${RUST_TARGET}/${PROFILE}/" >&2
