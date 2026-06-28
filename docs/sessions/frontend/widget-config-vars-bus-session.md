# Platform fix — generic bus.publish / bus.watch (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/dashboard/widget-config-vars-scope.md ("Platform fix")
- Status: done
- Public: ../../public/frontend/dashboard.md → "Generic bus pub/sub (bus.publish / bus.watch)"
  + ../../public/SCOPE.md
- Tests: rust/crates/host/tests/bus_test.rs (6), rust/role/gateway/tests/bus_routes_test.rs (4)

## Goal

The one missing backend API: a generic, workspace-walled, capability-gated subject pub/sub. Today the bus
is reachable only through series-scoped verbs; the JSON-over-SSE sink, Zenoh-sourced variables, and live
events on non-series subjects need `bus.publish(subject, payload)` (fire-and-forget motion) +
`bus.watch(subject)` (stream). A shared surface, not dashboard-private.

## What shipped

Host `crates/host/src/bus/` (one verb per file, mirroring `ingest`/`series`):
- `authorize.rs` — `authorize_bus` (gates `mcp:bus.publish|watch:call`, opaque deny) + `wall_subject`
  (the workspace-wall guard: namespaces the caller's subject to `ext/{subject}` — the `ws/{id}/` wall is
  added by `lb_bus::ws_key` — and **rejects reserved prefixes** `series/`/`channels/`/`internal/`/`ws/`/
  `presence/` and any `..`/absolute escape). A caller can NEVER name another workspace's subject nor
  impersonate platform motion (rule 6) — structurally, like `series_key`.
- `publish.rs` — `bus_publish` (authorize → wall → `lb_bus::publish`; best-effort, NOT durable).
- `watch.rs` + `subscribe.rs` — `bus_watch` (authorize → wall → `lb_bus::subscribe`) returning a `BusSub`.
- `tool.rs` — `call_bus_tool` (`bus.publish` over `POST /mcp/call` → `{ok:true}`; `bus.watch` is
  stream-only). Wired into `tool_call.rs` (`is_host_native` + dispatch) so the page bridge reaches it.
- `lib.rs` re-exports the verbs.

Gateway `role/gateway/src/routes/bus.rs`:
- `POST /bus/publish` (mirror `/ingest`) — `{ subject, payload }` → `{ok:true}`; gated, walled,
  `403`/`400` on deny/reserved.
- `GET /bus/stream?subject=<s>&token=<jwt>` (generalize `series_stream`) — `401` no token, `403`/`400`
  deny/reserved before any stream body, then `event: message` per published payload. The subject is a
  query param (it contains `/`, so it can't be a single path segment) — auth-first like the series stream.
- Registered in `server.rs`; dev claims gain `mcp:bus.publish|watch:call` (member-level).

## Decisions

- **State vs motion (rule 3):** `bus.publish` is fire-and-forget — `{ok}` means "handed to the bus", never
  "delivered". A must-deliver effect still goes through the outbox. The UI must not fake a "delivered".
- **The wall is structural, not a string check:** `wall_subject` returns a relative key the bus layer
  prefixes with `ws/{id}/`; a reserved-prefix denylist + escape guard make a cross-ws / platform-motion
  subject impossible to express. Tested it bites a real publish.
- **Subject as a query param on the stream:** subjects contain `/`, so a path segment won't match;
  `?subject=` keeps the auth-first 401/403 ordering identical to the series stream.

## Tests + green output

`cargo test -p lb-host --test bus_test` — **6 passed**: `wall_subject` (ext namespacing + reserved/escape
refusal), publish/watch cap-deny (opaque) per verb, a reserved/cross-ws subject refused even WITH the cap,
a publish→watch round-trip in one ws, and **ws-B does not receive ws-A's publish** (the wall holds on a
real Zenoh mesh).

`cargo test -p lb-role-gateway --test bus_routes_test` — **4 passed**: `401` without a token, `403`
without the cap, a reserved subject `400`, and a real **publish over `POST /bus/publish` → received over
the `GET /bus/stream` SSE** on a live socket.

Full `cargo test -p lb-host -p lb-role-gateway` — all green, no regressions.

## Mandatory categories

- **Capability deny (headline):** `bus.publish`/`bus.watch` without the cap → opaque `Denied` (host) /
  `403` (gateway), per verb, direct + via the MCP bridge.
- **Workspace isolation:** ws-B's `bus.watch` does not receive ws-A's publish (real two-ws mesh test); a
  `bus.*` subject naming another ws or a reserved prefix is refused (`BadSubject` / `400`).

## Follow-ups

Extracting to `scope/bus/` is the named residual (lean: keep here until a second non-dashboard caller
lands). Next: Slice 4 wires `bus.watch` into the dashboard's live feed + the refresh control.
