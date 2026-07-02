# Platform enabler — the `native.call` browser bridge (prerequisite for the ROS UI)

Status: **done** — `cargo build --workspace`, `cargo fmt --check`, and
`cargo test -p lb-role-gateway --test native_call_routes_test` (3 tests) all green.

## Result

- **Route:** `POST /native/call` in `role/gateway/src/routes/native.rs`, wired in `routes/mod.rs` +
  `server.rs`. Calls `lb_host::call_sidecar(&node, &OsLauncher, principal, ws, ext_id, tool, input,
  now)`; ws + principal from the token; `403` on denied/not-running/transport, `401` on bad session.
- **Re-export:** `OsLauncher` from `lb_host` (it owns the native tier + supervisor dep), so the gateway
  gets the whole native-tier surface from `lb_host` without reaching into `lb_supervisor`.
- **Tests (real gateway + real spawned echo-sidecar, no mocks):** granted call round-trips the echo +
  the injected workspace; a caller without `mcp:native.call:call` → `403`; a ws-B token cannot reach
  ws-A's sidecar (structural — the child is per-(ws,ext), the call resolves by the token's ws).

This unblocks the full ROS federated UI (CRUD/poll-toggle/point-write controls now have a
browser-reachable path; live values keep riding `series.*`).

## Why (the gap)

The ROS federated UI needs to drive the sidecar's own verbs (`ros.list`, `network.list`, `ros.start`,
`point.write`, …). A federated page reaches the platform ONLY through the host bridge
(`POST /mcp/call` → `lb_host::call_tool`), which dispatches **host-native** verbs (`series.*`,
`ingest.*`, `outbox.*`) and **wasm** `<ext>.<tool>` routing — but NOT `native.call`, the sidecar
dispatch the ROS tools live behind. `call_sidecar` is a `Launcher`-typed entry (it needs the OS
launcher for restart-on-demand), so it was never wired into the string dispatcher. Result: a browser
page cannot reach ANY native-tier sidecar's tools. `fleet-monitor` (also native) sidesteps this by only
reading `series.*` in its UI.

This is the same shape as slice 4's decision: rather than compromise the UI, build the missing platform
primitive. It is a **general** enabler — every native-tier extension UI depends on it, not just ROS.

## What (the route)

`POST /native/call` on the gateway, mirroring `POST /mcp/call`:
- Authenticate the session token → verified principal + workspace (from the token, never the body §7).
- Body `{ext_id, tool, input?}` — the sidecar + its tool + JSON args (a string or a JSON value).
- Call `lb_host::call_sidecar(&node, &OsLauncher, &principal, ws, ext_id, tool, input, now)`.
  - The MCP gate runs inside (`authorize_native` → `mcp:native.call:call`, workspace-first), so a page
    is exactly as denied as a forged call. `OsLauncher` is the production launcher (a unit struct built
    inline, matching `node/src/federation.rs`); it drives the restart-on-demand crash path.
- Return the sidecar's JSON output (parsed), or `403` (denied / not-running / transport — opaque, no
  existence oracle), `401` (bad session).

**Capability:** the coarse `mcp:native.call:call` is what the host gate checks here; the fine-grained
per-verb gate (`mcp:ros.list:call`, …) is the sidecar's own self-check against its `LB_EXT_TOKEN` grant
(slice-2 finding #1). So the browser caller needs `mcp:native.call:call`, and the sidecar independently
enforces the rest — defense in depth, no change to that model.

## Tests (planned)

`role/gateway/tests/` against a REAL gateway + a REAL spawned `echo-sidecar` (the existing native test
fixture — no mocks): a granted `POST /native/call {ext_id:"echo-sidecar", tool:"echo", input}` round-
trips the echo + the injected workspace; a caller WITHOUT `mcp:native.call:call` gets `403`; a ws-B
token cannot reach a sidecar spawned for ws-A (structural — the sidecar is per-(ws,ext), and the call
resolves by the token's ws).

## Follow-up (unblocked by this)

The full ROS federated UI (fleet → connection → network → device → point drill-down, poll toggles via
`ros.start`/`ros.stop` + `*.update {enable}`, `point.write`, and live values via `series.latest`) — the
CRUD/poll/write controls now have a browser-reachable path; live values keep riding `series.*` directly.
