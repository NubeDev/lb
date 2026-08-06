#!/usr/bin/env bash
# scripts/build-extension-artifact.sh — build + sign ANY lb extension (wasm or native tier) into
# the artifact lb-pack/the gateway's `POST /extensions` accept, in one call.
#
# Generalizes the manual dance used to ship `federation` as a signed zip artifact: resolve the
# extension, build it for the right target, then `lb-pack` it. Wasm and native tiers build
# completely differently (see below) — this script reads `[runtime].tier` from the extension's
# own `extension.toml` and picks the right path, so the caller never has to know which kind of
# extension they're holding.
#
# Usage:
#   scripts/build-extension-artifact.sh <name> [--target linux-x86_64|linux-arm64|linux-armv7]
#       [--key-file PATH] [--key-id ID] [--out DIR] [--format json|zip]
#
# <name> resolves against rust/extensions/<name>/ first, then rust/crates/<name>/ (federation's
# home — it predates the extensions/ convention and never moved). Examples:
#   scripts/build-extension-artifact.sh hello                        # wasm, host-portable
#   scripts/build-extension-artifact.sh echo-sidecar --target linux-arm64
#   scripts/build-extension-artifact.sh federation --target linux-x86_64
#
# WASM tier: built via the extension's own build.sh (already exists for every wasm extension —
# see `scripts/build-extensions.sh`), which produces a `wasm32-wasip2` component in the
# extension's own isolated target/ dir. Wasm is portable across host archs, so there is no
# --target/cross-build step for this tier — `--target` is ignored with a note if passed.
# Packaged `--format json` by default (small, the JSON int-array encoding is fine at this size).
#
# NATIVE tier: cross-built via the docker/build/ toolchain image (build once:
#   docker build -t lazybones-build docker/build
# ), the same image `make docker-build`/`lb-build` use. Requires a live Docker daemon. Packaged
# `--format zip` by default — the whole reason that format exists is a real native binary
# blowing past what JSON's ~4-8x inflation can survive on the wire (extension-artifact-upload-
# size fix). `mqtt` (descriptor only, no implementation) and any extension.toml with no matching
# Cargo.toml/build.sh fail here with a clear message, not a cryptic build error.
#
# Signing: a fresh PER-DEVELOPER dev key by default (never a release key) — same posture as
# rubix-ai's scripts/build-rubixd-package.sh. First run against a new --key-file generates it and
# prints the trusted-pubkey line for whoever holds gateway config access.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_DIR="$ROOT/rust"
OUT_DIR="${OUT_DIR:-$ROOT/dist/extensions}"
# A DEDICATED extension-signing key, deliberately separate from rubixd's `operator.key` (which
# signs the rubix-ai/rubixd PACKAGE and is trusted via rubixd's own `[[trusted_pubkey]]` list —
# a different trust domain entirely). Extension artifacts are trusted by the GATEWAY's
# `LB_TRUSTED_PUBKEYS` env var, not rubixd, so they get their own identity — reused across every
# extension this script builds (one fixed path = one key = one line to add to
# LB_TRUSTED_PUBKEYS, covering every extension a developer publishes), auto-generated on first
# use. Never point this at rubixd's operator.key — same key across both domains defeats the
# point of having two separate trust lists in the first place.
KEY_FILE="${KEY_FILE:-$OUT_DIR/dev-publisher.key}"
KEY_ID="${KEY_ID:-dev-$(whoami)-$(hostname -s 2>/dev/null || hostname)-ext}"
KEY_ID="$(printf '%s' "$KEY_ID" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9._-' '-')"
TARGET_ALIAS="linux-x86_64"
FORMAT=""
DOCKER_IMAGE="lazybones-build"

say() { printf '\n\033[1;36m▸ %s\033[0m\n' "$*"; }
ok()  { printf '  \033[1;32m✓\033[0m %s\n' "$*"; }
die() { printf '\n\033[1;31m✗ %s\033[0m\n' "$*" >&2; exit 1; }

[[ $# -ge 1 ]] || die "usage: $0 <extension-name> [--target linux-x86_64|linux-arm64|linux-armv7] [--key-file PATH] [--key-id ID] [--out DIR] [--format json|zip]"
NAME="$1"; shift
while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)   TARGET_ALIAS="$2"; shift 2 ;;
    --key-file) KEY_FILE="$2"; shift 2 ;;
    --key-id)   KEY_ID="$2"; shift 2 ;;
    --out)      OUT_DIR="$2"; shift 2 ;;
    --format)   FORMAT="$2"; shift 2 ;;
    *) die "unknown flag $1" ;;
  esac
done
mkdir -p "$OUT_DIR"

# ── resolve the extension dir + manifest ──────────────────────────────────────────────────────
if [[ -d "$RUST_DIR/extensions/$NAME" ]]; then
  EXT_DIR="$RUST_DIR/extensions/$NAME"
elif [[ -d "$RUST_DIR/crates/$NAME" ]]; then
  EXT_DIR="$RUST_DIR/crates/$NAME"
else
  die "no extension or crate named '$NAME' under rust/extensions/ or rust/crates/"
fi
MANIFEST="$EXT_DIR/extension.toml"
[[ -f "$MANIFEST" ]] || die "no extension.toml at $MANIFEST"

TIER="$(grep -m1 '^tier' "$MANIFEST" | sed -E 's/^tier[[:space:]]*=[[:space:]]*"([a-z]+)".*/\1/')"
[[ -n "$TIER" ]] || die "could not read [runtime].tier from $MANIFEST"
ok "resolved $NAME: $EXT_DIR (tier: $TIER)"

# ── build ──────────────────────────────────────────────────────────────────────────────────────
case "$TIER" in
  wasm)
    [[ -f "$EXT_DIR/build.sh" ]] || die "$NAME is wasm-tier but has no build.sh at $EXT_DIR/build.sh"
    say "build $NAME (wasm32-wasip2, host-portable — no cross-build)"
    bash "$EXT_DIR/build.sh"
    WASM_OUT="$(find "$EXT_DIR/target/wasm32-wasip2/release" -maxdepth 1 -name '*.wasm' 2>/dev/null | head -1)"
    [[ -n "$WASM_OUT" ]] || die "build.sh ran but no .wasm found under $EXT_DIR/target/wasm32-wasip2/release/"
    ok "built $WASM_OUT"
    BINARY="$WASM_OUT"
    FORMAT="${FORMAT:-json}"
    ;;
  native)
    EXEC="$(grep -m1 '^exec' "$MANIFEST" | sed -E 's/^exec[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/')"
    [[ -n "$EXEC" ]] || die "$NAME is native-tier but extension.toml has no [native].exec (e.g. mqtt is a descriptor with no implementation — nothing to build)"
    (cd "$RUST_DIR" && cargo pkgid -p "$EXEC" >/dev/null 2>&1) || die "extension.toml declares [native].exec = \"$EXEC\" but no such package exists in the workspace — it's a descriptor with no implementation (nothing to build)"
    case "$TARGET_ALIAS" in
      linux-x86_64)  RUST_TARGET=x86_64-unknown-linux-gnu ;;
      linux-arm64)   RUST_TARGET=aarch64-unknown-linux-gnu ;;
      linux-armv7)   RUST_TARGET=armv7-unknown-linux-gnueabihf ;;
      *) die "unsupported --target '$TARGET_ALIAS' (want linux-x86_64, linux-arm64, or linux-armv7)" ;;
    esac
    docker image inspect "$DOCKER_IMAGE" >/dev/null 2>&1 || die "docker image '$DOCKER_IMAGE' not found — run: docker build -t $DOCKER_IMAGE docker/build"
    FEATURES=""
    [[ "$EXEC" == "federation" ]] && FEATURES="postgres" # the only extension needing this today
    say "cross-build $NAME ($EXEC) for $TARGET_ALIAS via $DOCKER_IMAGE"
    docker run --rm \
      -v "$RUST_DIR:/work" \
      -v lazybones-cargo-cache:/usr/local/cargo/registry \
      -e PKG="$EXEC" -e PROFILE=release -e FEATURES="$FEATURES" -e CARGO_BUILD_JOBS=2 \
      --name "${NAME}-build" \
      "$DOCKER_IMAGE" bash -c "cd /work && rustup target add $RUST_TARGET && lb-build $TARGET_ALIAS"
    BINARY="$RUST_DIR/target/$RUST_TARGET/release/$EXEC"
    [[ -x "$BINARY" ]] || die "expected binary at $BINARY but it's not there — cross-build likely failed above"
    ok "built $BINARY"
    FORMAT="${FORMAT:-zip}"
    ;;
  *)
    die "unknown tier '$TIER' in $MANIFEST (want wasm or native)"
    ;;
esac

# ── sign + package ───────────────────────────────────────────────────────────────────────────
LB_PACK="$RUST_DIR/target/release/lb-pack"
if [[ ! -x "$LB_PACK" ]]; then
  say "build lb-pack (native, one-time)"
  (cd "$RUST_DIR" && cargo build -p lb-pack --release)
fi

EXT_OUT="$OUT_DIR/$NAME.$([[ "$FORMAT" == zip ]] && echo zip || echo json)"
say "sign + package ($FORMAT) → $EXT_OUT"
"$LB_PACK" "$MANIFEST" "$BINARY" "$KEY_FILE" --key-id "$KEY_ID" --format "$FORMAT" --out "$EXT_OUT"

# One trusted-pubkey.txt per key file, kept in sync — a quick `cat` instead of re-deriving via
# `lb-pack pubkey` every time (same convenience rubix-ai's build-rubixd-package.sh gives).
PUBKEY_FILE="$OUT_DIR/trusted-pubkey.txt"
"$LB_PACK" pubkey "$KEY_FILE" --key-id "$KEY_ID" > "$PUBKEY_FILE"
ok "trusted-pubkey saved to $PUBKEY_FILE"

CONTENT_TYPE="application/json"
[[ "$FORMAT" == zip ]] && CONTENT_TYPE="application/zip"
say "DONE"
ok "artifact: $EXT_OUT"
echo "  Upload via Studio's \"Upload signed artifact\", or:"
echo "  curl -X POST <gateway>/extensions -H 'Content-Type: $CONTENT_TYPE' \\"
echo "    -H \"Authorization: Bearer \$TOKEN\" --data-binary @$EXT_OUT"
echo "  Trust the printed key ($PUBKEY_FILE) on the target node's LB_TRUSTED_PUBKEYS env if not already trusted."
