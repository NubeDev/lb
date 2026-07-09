#!/usr/bin/env bash
# The Windows desktop shell cross-build entrypoint. Runs from WORKDIR=/build — the repo
# root, bind-mounted by `make windows-executable` / `make windows-full`. Produces the bare
# .exe at ui/src-tauri/target/x86_64-pc-windows-msvc/release/lazybones-shell.exe.
#
# One verb per file (docs/FILE-LAYOUT.md §9): this script cross-builds and nothing else.
# Same shape as build.sh (the Linux lane); the only deltas are the target triple and the
# cargo-xwin runner, which fetches the Windows SDK/CRT into $XWIN_CACHE_DIR and drives
# clang-cl + lld-link. `--no-bundle` = plain executable — no MSI/NSIS installer (the
# packaging scope's "bare binary first" ask, mirrored from the Linux slice).
#
# TWO build modes (see build.sh for the full contract): LB_SHELL_FEATURES=desktop (thin
# shell, default) vs LB_SHELL_FEATURES=desktop,full (standalone full stack). `full` bakes
# VITE_GATEWAY_URL into the UI so the webview talks to the in-process loopback gateway.
set -euo pipefail

FEATURES="${LB_SHELL_FEATURES:-desktop}"

case ",$FEATURES," in
  *,full,*)
    ADDR="${LB_DESKTOP_GATEWAY_ADDR:-127.0.0.1:8800}"
    export VITE_GATEWAY_URL="http://${ADDR}"
    echo "-> full-stack mode: VITE_GATEWAY_URL=$VITE_GATEWAY_URL (LB_DESKTOP_GATEWAY_ADDR=$ADDR)"
    # Cross-build the federation datasources sidecar for the full desktop bundle (desktop-federation-
    # bundle scope). SQLITE ONLY (no `--features postgres`): the sqlite path is default-on and links
    # `rusqlite` bundled — it compiles its own sqlite3 under cargo-xwin, no system C dep and no
    # TLS/OpenSSL. Same cargo-xwin runner + target as the shell, so one toolchain. The Makefile copies
    # `federation.exe` beside the shell.
    echo "-> cross-building federation.exe (sqlite-only) for the full desktop bundle"
    # --manifest-path from the repo root, NOT `cd rust`: entering rust/ makes rustup try to sync the
    # `rust-toolchain.toml` wasm target/components into the root-owned rustup home (Permission denied
    # for the non-root container user). The repo-root cwd has no toolchain file (like the tauri build),
    # so the pre-installed stable toolchain + the windows target added in the image are used as-is.
    RUSTUP_TOOLCHAIN=stable cargo xwin build --manifest-path rust/Cargo.toml -p federation --release --target x86_64-pc-windows-msvc
    ;;
  *)
    echo "-> thin-shell mode: LB_SHELL_FEATURES=$FEATURES (UI uses Tauri IPC)"
    ;;
esac

pnpm install --frozen-lockfile

cd ui
exec pnpm tauri build --no-bundle \
  --runner cargo-xwin \
  --target x86_64-pc-windows-msvc \
  -- --features "$FEATURES"
