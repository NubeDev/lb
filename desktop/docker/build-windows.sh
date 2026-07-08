#!/usr/bin/env bash
# The Windows desktop shell cross-build entrypoint. Runs from WORKDIR=/build — the repo
# root, bind-mounted by `make windows-executable`. Produces the bare .exe at
# ui/src-tauri/target/x86_64-pc-windows-msvc/release/lazybones-shell.exe.
#
# One verb per file (docs/FILE-LAYOUT.md §9): this script cross-builds and nothing else.
# Same shape as build.sh (the Linux lane); the only deltas are the target triple and the
# cargo-xwin runner, which fetches the Windows SDK/CRT into $XWIN_CACHE_DIR and drives
# clang-cl + lld-link. `--no-bundle` = plain executable — no MSI/NSIS installer (the
# packaging scope's "bare binary first" ask, mirrored from the Linux slice).
set -euo pipefail

pnpm install --frozen-lockfile

cd ui
exec pnpm tauri build --no-bundle \
  --runner cargo-xwin \
  --target x86_64-pc-windows-msvc \
  -- --features desktop
