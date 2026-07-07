#!/usr/bin/env bash
# The Linux desktop shell build entrypoint. Runs from WORKDIR=/build — the repo root,
# bind-mounted in dev mode, COPYied in CI mode. Produces the bare ELF at
# ui/src-tauri/target/release/lazybones-shell.
#
# One verb per file (docs/FILE-LAYOUT.md §9): this script builds and nothing else.
# The in-container smoke (xvfb-run) is a separate `docker run` invocation documented
# in desktop/docker/README.md — do not bolt it on here.
set -euo pipefail

# Install the full pnpm workspace (ui/ + packages/* + path-dep rust/crates/* pulled by
# ui/src-tauri). --frozen-lockfile = CI-strict; fails if the lockfile is out of date
# rather than silently resolving.
pnpm install --frozen-lockfile

# `tauri build --no-bundle` = the bare binary only, no AppImage/deb/rpm (the packaging
# scope's "plain executable" ask). `-- --features desktop` forwards to cargo, turning
# ON the optional `dep:tauri` seam (ui/src-tauri/Cargo.toml) that the headless
# command-layer build/test deliberately skips — this is what brings the window in.
cd ui
exec pnpm tauri build --no-bundle -- --features desktop
