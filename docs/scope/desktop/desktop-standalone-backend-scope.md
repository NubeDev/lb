# Desktop scope — standalone full-stack build mode (`full`)

Status: scope + build. Promotes to `public/desktop/desktop.md` once shipped.

The Tauri shell (`ui/src-tauri`) ships in a **thin IPC mode** today: it boots the node
in-process but exposes only five `#[tauri::command]` verbs (`channel_*` + `agent_invoke`)
and mints a hardcoded demo principal. The React UI bundled into the webview is built to talk
to the **HTTP/SSE gateway** over `VITE_GATEWAY_URL` — so against the packaged shell, login
and every other gateway verb have nothing to answer them. The shell works for the channel
demo; it does not work as a standalone product. This scope closes that gap with a **second
build mode**, not a rewrite of the first.

## Goals

- A second cargo feature on `lazybones-shell`: **`full`** (implies `desktop`). Built with
  `--features desktop,full`, the binary boots the node, **mounts the SSE/HTTP gateway
  in-process on a loopback port**, runs the boot seeders, opens the window. The webview
  talks to that loopback gateway over HTTP — exactly as the browser does against `make dev`.
  Login, MCP, SSE, the agent catalog, flows, insights: all of it, one binary, no external
  node.
- The existing **thin shell** stays the default (`--features desktop`). The two modes share
  one source, one window wiring, one node — the only branch is `#[cfg(feature = "full")]`.
- The UI picks its transport by **build config**, not a runtime switch: when
  `VITE_GATEWAY_URL` is baked in (the `full` build), the HTTP path wins even inside the
  Tauri webview; when it is unset (the shell build), Tauri IPC wins. One line in
  `invoke.ts`, no feature flag in the UI.
- Makefile entrypoints: `make -C desktop linux-executable` (shell, unchanged) and
  `make -C desktop linux-full` (full stack).
- Same shape on Windows (`windows-full`) — the only OS-provided runtime dep is WebView2.

## Non-goals

- **Persistent store / persisted signing key.** The full mode boots an in-memory node
  (`Node::boot()`) and an ephemeral signing key — a fresh state each launch, like the
  current `NodeHandle::boot`. Persistence (`LB_STORE_PATH` + `signing_key::resolve`) is a
  separate, orthagonal ask; the seeders are idempotent so a fresh boot still logs in
  cleanly. Recorded as a follow-up. **Update:** the store half is reversed by
  [`desktop-persistent-store-scope.md`](desktop-persistent-store-scope.md) — the `full` desktop
  now defaults to a durable per-user store, so user data survives a restart. The signing-key half
  is still open (a restart still re-logs the user in).
- **Native sidecars** (`federation`, `control-engine`). Those need their own sidecar
  binaries + env config (pre-approved endpoints, a running ce-rest). The desktop `full`
  boot does not mount them; a developer who wants them runs `make dev`. The core product
  (identity, channels, dashboards, flows, rules, insights, agent, ingest) is fully
  functional without them. **Update:** the `federation` half of this non-goal is reversed by
  [`desktop-federation-bundle-scope.md`](desktop-federation-bundle-scope.md) — it bundles +
  auto-installs the federation sidecar into `full` so datasources test/query standalone
  (`control-engine` remains deferred). Register-but-can't-test in `full` today is that gap.
- **The `hello` wasm bring-up demo.** It needs the built `.wasm` at a relative path that
  does not exist beside an installed binary. `load_enabled` (re-load previously-published
  extensions from the durable cache) is a no-op on a fresh store and stays.
- **Runtime port config.** The loopback gateway URL is baked into the UI at build time
  (`VITE_GATEWAY_URL=http://127.0.0.1:8800`), so the gateway binds a fixed port. A fixed
  port (`8800`, distinct from the dev `8080` so they don't collide) is the v1 contract;
  changing it is a rebuild. Recorded.
- **Closing the thin-shell IPC command-layer gap** (`assets_*`, `workflow_*`, registry, the
  workspace switcher — `desktop-packaging-scope.md`'s separate ask). Unchanged by this
  scope; the `full` mode makes it moot by routing everything over HTTP.

## Intent / approach

The architecture already permits this (symmetric nodes — "the shell IS a node", §3.1; the
gateway IS a node that also speaks HTTP). The slice is wiring, not invention:

1. **Cargo feature** — `full = ["desktop", "dep:lb-role-gateway", "dep:lb-authz"]` in
   `ui/src-tauri/Cargo.toml`. The headless command-layer build (no feature) and the thin
   shell (`desktop`) are untouched — the optional-dep seam the packaging scope established
   extends cleanly.
2. **`src/full.rs`** (one responsibility: "boot the standalone backend onto a node") —
   `seed_dev_identity` (mirrored from `rust/node/src/main.rs:22`: `user:ada` →
   `workspace-admin` of `acme`, idempotent), the catalog seeders (`seed_core_skills`,
   `seed_agent_definitions`, `seed_personas`, `migrate_active_persona`,
   `grant_default_core_skills`), the four background reactors (`spawn_flow_reactors`,
   `spawn_agent_reactors`, `spawn_approval_reactors`, `spawn_insight_digest_reactors`),
   and `Gateway::new_live(node, SigningKey::generate())` served on `127.0.0.1:8800`. Every
   piece already exists in `lb_host` / `lb_authz` / `lb_role_gateway`; this file composes
   them exactly as `node/src/main.rs`'s gateway branch does.
3. **`desktop.rs::run()`** — boot the `NodeHandle` (shared with the shell mode so the IPC
   commands stay registered and the window wiring is one path), then
   `#[cfg(feature = "full")] full::boot_full(handle.node.clone(), &handle.ws, addr)` before
   opening the window. The serve task is held for the life of the app.
4. **`invoke.ts`** — when `import.meta.env.VITE_GATEWAY_URL` is defined (the full build),
   the HTTP transport wins; else Tauri IPC (the shell build). The browser path is
   unchanged (it always had `VITE_GATEWAY_URL` or the default).
5. **Build wiring** — `build.sh` / `build-windows.sh` accept `LB_SHELL_FEATURES` (default
   `desktop`; set `desktop,full` for full) and, when `full` is on, build the UI with
   `VITE_GATEWAY_URL=http://127.0.0.1:8800`. The Makefile's `linux-full` / `windows-full`
   targets set both.

Alternative rejected: have the shell mirror every gateway verb as a `#[tauri::command]`.
That is the "desktop command-layer gap" the packaging scope tracks — a dead-end of
per-verb hand-mirroring that drifts from the gateway. Mounting the gateway in-process
reuses the entire HTTP surface the UI is built and tested against, with **zero UI changes**
beyond the one-line transport priority.

## How it fits the core

- **Symmetric nodes (rule 1):** the strongest possible statement — the packaged desktop
  binary is a full node with a window attached, reaching its own gateway over loopback
  HTTP. No `if desktop` in any core crate; the only switch is the shell crate's own
  cargo feature, which gates *what gets mounted in the wiring layer*, never behavior.
- **Capability-first (rule 5) / workspace wall (rule 6):** the loopback gateway enforces
  both exactly as `make dev` does — the token minted by `/login` is the wall. The desktop
  gets no special path.
- **No mocks (rule 9):** the full mode boots the real store, real bus, real gateway, real
  caps. Nothing to fake.
- **One responsibility per file (rule 8):** `full.rs` is the one "mount the standalone
  backend" verb; `desktop.rs` stays the window wiring; `state.rs` stays the node handle.

## Testing plan

Per `scope/testing/testing-scope.md`. The headless command-layer suite
(`cargo test -p lazybones-shell`, no feature) MUST stay green — the optional-dep seam is
the property that keeps every other CI lane webkit-free. New:

- **Feature-off build gate:** `cargo build -p lazybones-shell` (no feature) on a webkit-less
  box still compiles — `full` adds no compile dep when off.
- **Feature-on compile gate:** `cargo build -p lazybones-shell --features desktop,full`
  compiles (needs the webkit box for the `desktop` half; the `full` half is pure Rust).
- **Boot smoke (Linux, the headline):** `make -C desktop linux-full` then `xvfb-run` the
  binary; assert (a) the process stays up, (b) `curl http://127.0.0.1:8800/login` with
  `{user:"user:ada",workspace:"acme"}` returns a real signed token, (c) that token drives a
  real `POST /mcp/call`. Real store, real gateway, real caps (rule 9). This is the proof
  that "login works" against the packaged binary.

## Risks & hard problems

- **Port 8800 taken.** Loud bind failure at boot (the serve task errors, the window still
  opens but every call fails). Acceptable for v1; the message must name the port. A real
  fix is a runtime-negotiated port + a Tauri command that hands the URL to the UI (recorded
  follow-up).
- **CORS.** Already permissive in the gateway (`CorsLayer::permissive()` at
  `rust/role/gateway/src/server.rs:343`) — the webview origin (`tauri://localhost`) reaches
  the loopback origin cleanly. No new CORS work.
- **Ephemeral signing key.** A desktop restart issues a new key; stored browser tokens 401
  and the app falls back to the login screen (the existing `requestError` 401 path). Not a
  bug; documented. Persistence is the follow-up.

## Open questions

- Persistence: should the full mode default to a durable store under the user's data dir
  + a persisted signing key, so state survives restart? Leaning: yes, but as a separate
  slice — this one ships the standalone path, not durability.
- Loopback port: fixed `8800`, or runtime-negotiated with a Tauri command feeding the
  chosen URL back into the UI? Leaning: fixed for v1 (baked URL), runtime for v2.

## Related

- `docs/scope/desktop/desktop-packaging-scope.md` — the packaging slice that shipped the
  binary; its "Non-goal: closing the desktop command-layer gap" is the gap this scope
  closes by routing over HTTP instead of mirroring verbs.
- `rust/node/src/main.rs:22-273` — the gateway boot branch this mirrors (`seed_dev_identity`
  + the seeders + `Gateway::new_live` + `serve`).
- `ui/src/lib/ipc/invoke.ts` — the transport seam whose priority this flips.
- README §3 rule 1 (symmetric nodes), §6.13 (Tauri-local vs browser-remote).
