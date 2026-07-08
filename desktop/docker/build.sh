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
