#!/usr/bin/env bash
# The Linux desktop shell build entrypoint. Runs from WORKDIR=/build — the repo root,
# bind-mounted in dev mode, COPYied in CI mode. Produces the bare ELF at
# ui/src-tauri/target/release/lazybones-shell.
#
# One verb per file (docs/FILE-LAYOUT.md §9): this script builds and nothing else.
# The in-container smoke (xvfb-run) is a separate `docker run` invocation documented
# in desktop/docker/README.md — do not bolt it on here.
#
# TWO build modes (docs/scope/desktop/desktop-standalone-backend-scope.md), selected by
# `LB_SHELL_FEATURES` (a comma-separated cargo feature list):
#   default (LB_SHELL_FEATURES=desktop)               → the THIN shell: Tauri window + the 5 IPC commands.
#   LB_SHELL_FEATURES=desktop,full                    → the FULL standalone backend: mounts the SSE/HTTP
#                                                       gateway in-process on loopback + runs the boot
#                                                       seeders, so login/MCP/SSE/agents all work standalone.
# In `full` mode the UI is built talking to that loopback gateway (VITE_GATEWAY_URL baked in), so the
# webview reuses the entire HTTP surface the browser is built against — no per-verb IPC mirroring.
# The loopback origin defaults to 127.0.0.1:8800 (override BOTH the build + the runtime via
# LB_DESKTOP_GATEWAY_ADDR — they must match).
set -euo pipefail

FEATURES="${LB_SHELL_FEATURES:-desktop}"

# When `full` is on, bake VITE_GATEWAY_URL into the UI so the webview talks to the in-process
# loopback gateway (not the dev node). Exported → inherited by tauri's `beforeBuildCommand`
# (`pnpm build`) → Vite exposes it on `import.meta.env`. The origin is read from the SAME env
# knob the binary binds at runtime, so a rebuild with a custom port stays consistent.
case ",$FEATURES," in
  *,full,*)
    ADDR="${LB_DESKTOP_GATEWAY_ADDR:-127.0.0.1:8800}"
    export VITE_GATEWAY_URL="http://${ADDR}"
    echo "-> full-stack mode: VITE_GATEWAY_URL=$VITE_GATEWAY_URL (LB_DESKTOP_GATEWAY_ADDR=$ADDR)"
    # The FULL desktop bundles the federation datasources sidecar so a standalone binary can
    # register AND query the shipped sqlite demo (desktop-federation-bundle scope). SQLITE ONLY:
    # NO `--features postgres` — sqlite is the default feature set, `rusqlite` is bundled (compiles
    # its own sqlite3, no system C dep, no TLS/OpenSSL). The Makefile copies the built binary beside
    # the shell. Built here (not a separate `docker run`) so it links against the SAME toolchain.
    echo "-> building federation sidecar (sqlite-only) for the full desktop bundle"
    # Build via --manifest-path from the repo root, NOT `cd rust`: the rust/ workspace pins a
    # `rust-toolchain.toml` (channel=stable + a wasm32-wasip2 target + clippy/rustfmt). Entering that
    # dir makes rustup try to SYNC that target/components into the root-owned /usr/local/rustup — which
    # the non-root container user cannot write (Permission denied). Staying at the repo root (whose cwd
    # has no toolchain file, exactly like the `ui/` tauri build) uses the pre-installed stable toolchain
    # as-is. The sidecar is a native binary and needs none of the wasm target anyway. RUSTUP_TOOLCHAIN
    # pins it belt-and-braces so no stray override re-triggers a sync.
    RUSTUP_TOOLCHAIN=stable cargo build --manifest-path rust/Cargo.toml -p federation --release
    ;;
  *)
    echo "-> thin-shell mode: LB_SHELL_FEATURES=$FEATURES (UI uses Tauri IPC)"
    ;;
esac

# Install the full pnpm workspace (ui/ + packages/* + path-dep rust/crates/* pulled by
# ui/src-tauri). --frozen-lockfile = CI-strict; fails if the lockfile is out of date
# rather than silently resolving.
pnpm install --frozen-lockfile

# `tauri build --no-bundle` = the bare binary only, no AppImage/deb/rpm (the packaging
# scope's "plain executable" ask). `-- --features $FEATURES` forwards to cargo: `desktop`
# turns ON the optional `dep:tauri` seam (the window); `full` additionally turns on the
# in-process gateway + the identity-seed deps (see ui/src-tauri/Cargo.toml).
cd ui
exec pnpm tauri build --no-bundle -- --features "$FEATURES"
