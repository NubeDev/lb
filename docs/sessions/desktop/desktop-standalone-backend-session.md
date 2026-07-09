# Desktop standalone full-stack build mode — session

**Scope:** [`scope/desktop/desktop-standalone-backend-scope.md`](../../scope/desktop/desktop-standalone-backend-scope.md)
**Date:** 2026-07-08
**Status:** shipped (code + tests + docs); container smoke wired, local windowed smoke blocked by a host-only snap quirk (debugged, not a code bug).

## The ask

The desktop `lazybones-shell` binary shipped in a **thin IPC mode**: a Tauri window + 5
`#[tauri::command]` verbs (`channel_*` + `agent_invoke`) over a hardcoded `user:me` demo
principal. The React UI bundled into the webview, though, is built to talk to the **HTTP/SSE
gateway** over `VITE_GATEWAY_URL` — so login and every gateway verb had nothing to answer
them in the packaged binary. The ask: an option to build the shell EITHER as-is (thin) OR as
a **100% standalone full stack** ("the user can do everything as normal", no external node).

## What shipped

A second cargo feature on `lazybones-shell`: **`full`** (implies `desktop`). Built with
`--features desktop,full`, the binary boots the node, mounts the SSE/HTTP gateway in-process
on `127.0.0.1:8800`, runs the boot seeders (identity + skill/agent/persona catalogs), spawns
the four background reactors, and opens the window. The webview talks to that loopback gateway
over HTTP — exactly as the browser does against `make dev`. The thin shell stays the default
(`--features desktop`).

### The moving parts

1. **`ui/src-tauri/Cargo.toml`** — `full = ["desktop", "dep:lb-role-gateway", "dep:lb-authz"]`.
   The optional-dep seam the packaging scope established extends cleanly: the headless build
   + the thin shell never pull in the gateway crate.
2. **`ui/src-tauri/src/full.rs`** (NEW, one responsibility: "boot the standalone backend
   onto a node") — `seed_dev_identity` (mirrored from `rust/node/src/main.rs:22`, idempotent),
   the catalog seeders, the four reactors, and `Gateway::new_live` + `serve_listener` on the
   loopback port. Returns the bound address so a caller can pass `127.0.0.1:0` (the test) or
   a fixed port (the desktop).
3. **`ui/src-tauri/src/desktop.rs::run()`** — boots the `NodeHandle` (shared with the thin
   mode), then `#[cfg(feature = "full")]` mounts the gateway off the SAME node before opening
   the window. One branch, one place; the window wiring stays one path.
4. **`ui/src/lib/ipc/invoke.ts`** — when `VITE_GATEWAY_URL` is explicitly defined (the full
   build), the HTTP transport wins even inside the Tauri webview; else Tauri IPC (the thin
   build). One-line priority flip; the browser path is unchanged.
5. **`rust/role/gateway/src/server.rs`** — added `serve_listener(gw, listener)` (the
   construction-is-not-serving split extended to the socket) so a caller can bind
   `127.0.0.1:0` and learn the chosen port. One sibling function to `serve`.
6. **`desktop/docker/build.sh` + `build-windows.sh`** — accept `LB_SHELL_FEATURES` (default
   `desktop`; set `desktop,full` for full) and, when `full` is on, bake
   `VITE_GATEWAY_URL=http://${LB_DESKTOP_GATEWAY_ADDR}` into the UI before `tauri build`
   (Vite exposes it on `import.meta.env`). The same env knob the binary binds at runtime.
7. **`desktop/Makefile`** — `linux-full` / `windows-full` (build to its own
   `build/linux/full/` + `build/windows-full/` dirs so the two modes never clobber each
   other) + `smoke-full` (xvfb boot + `curl /login` + a real `POST /mcp/call`).
8. **Docs** — the scope, this session, a debugging entry for the snap quirk, two new build
   READMEs (`build/linux/full/`, `build/windows-full/`), and the thin-shell README now points
   at the full mode.

## How the UI picks its transport (the subtle bit)

`invoke.ts` had `if (inTauri()) return tauriInvoke(...)`. A full-stack webview IS in Tauri
(`__TAURI_INTERNALS__` is always injected), so that branch would win and the webview would
keep hitting the 5 IPC commands, not the loopback gateway. The flip: **an explicit
`VITE_GATEWAY_URL` wins over Tauri IPC**. The full build bakes one in (build.sh); the thin
build leaves it unset so IPC wins. The browser path is unchanged (it always had
`VITE_GATEWAY_URL` or the default). No UI feature flag, no per-verb branching.

Rejected alternative: mirror every gateway verb as a `#[tauri::command]` (the "desktop
command-layer gap" the packaging scope tracks). A dead-end of per-verb hand-mirroring that
drifts from the gateway. Mounting the gateway in-process reuses the **entire** HTTP surface
the UI is built and tested against, with zero UI changes beyond the priority flip.

## Tests (real store/bus/gateway/caps, rule 9)

- **Optional-dep seam:** `cargo build` + `cargo test -p lazybones-shell` (no feature, no
  webkit) — 2/2 green. The property that keeps every other CI lane webkit-free holds.
- **`full` compile + link:** `cargo check --features full` + `cargo build --features full
  --release` — clean. webkit is present on this box so the release ELF links.
- **`full.rs` unit:** 2/2 (loopback addr valid + distinct from dev 8080; env fallback).
- **The headline (`ui/src-tauri/tests/full_loopback_test.rs`):** a NON-windowed boot of
  `boot_full` on `127.0.0.1:0` + reqwest — `login_then_mcp_call_works_over_the_loopback_gateway`
  (login returns a real signed token for `user:ada`/`acme`; that token drives a real
  `tools.catalog` `POST /mcp/call`, non-empty) + `login_refuses_an_unseeded_user` (the
  mandatory capability/deny: `user:stranger`/`acme` → 403, the wall holds). 2/2 green.
  This is the portable proof — no display, no webkit, no Tauri window.
- **fmt:** `cargo fmt` clean on both the `ui/src-tauri` crate and the `rust/` workspace.

`cargo test --features full` totals **6/6** (2 commands_test + 2 full_loopback + 2 full unit).

## The local-windowed smoke (blocked, documented)

Running the release binary against the host X display proved the Rust boot (`full: seeded 38
core skills` + `full: loopback gateway on http://127.0.0.1:8800`) then crashed at window init
with `symbol lookup error: /snap/core20/current/lib/x86_64-linux-gnu/libpthread.so.0 …
GLIBC_PRIVATE`. NOT a shell/gateway bug — a snap `core20` old-glibc `libpthread` leaks into
the host's webkit/GTK init on this dev box (modern glibc has `libpthread` as a stub in libc).
Does NOT reproduce in the Ubuntu 22.04 build container (no snap). The non-windowed integration
test is the portable proof; `make -C desktop smoke-full` is the canonical windowed proof.
Logged at
[`debugging/desktop/full-binary-snap-libpthread-crash-at-window-init.md`](../../debugging/desktop/full-binary-snap-libpthread-crash-at-window-init.md).

## What's deliberately NOT in this slice (recorded, not gaps)

- **Persistent store / signing key.** A fresh in-memory store + ephemeral key per launch
  (state doesn't survive restart). The seeders are idempotent so each launch still logs in
  cleanly. Persistence is a follow-up.
- **Native sidecars** (federation, control-engine). Need their own binaries + config; run
  `make dev` for them. Core product is fully functional without.
- **Runtime port config.** Fixed `8800` (distinct from dev `8080`). Override needs a matching
  UI rebuild (the URL is baked at build time). Recorded.

## Files

**New:**
- `ui/src-tauri/src/full.rs` — the standalone boot (seeders + gateway + reactors).
- `ui/src-tauri/tests/full_loopback_test.rs` — the non-windowed headline proof.
- `docs/scope/desktop/desktop-standalone-backend-scope.md` — the ask + decisions.
- `desktop/build/linux/full/README.md`, `desktop/build/windows-full/README.md` — the build type docs.
- `docs/debugging/desktop/full-binary-snap-libpthread-crash-at-window-init.md`.

**Edited:**
- `ui/src-tauri/Cargo.toml` (the `full` feature + optional deps + reqwest dev-dep).
- `ui/src-tauri/src/lib.rs`, `ui/src-tauri/src/desktop.rs` (mount the gateway in `run()`).
- `ui/src/lib/ipc/invoke.ts` (transport priority flip).
- `rust/role/gateway/src/server.rs` + `lib.rs` (the `serve_listener` seam).
- `desktop/docker/build.sh`, `desktop/docker/build-windows.sh` (feature + `VITE_GATEWAY_URL`).
- `desktop/Makefile` (`linux-full`, `windows-full`, `smoke-full`).
- `desktop/build/linux/executable/README.md` (points at the full mode).
- `.gitignore` (the new `build/linux/full/` + `build/windows-full/` dirs).
- `docs/debugging/README.md` (the snap-quirk row).
