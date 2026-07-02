# ROS driver — Slice 2: the CRUD tree (resource-verbs grammar)

Status: **done** — `cargo build --workspace`, `cargo test -p ros-sidecar`, `cargo fmt` green; the
mandatory capability-deny + workspace-isolation + token-hygiene tests pass against a **real** spawned
gateway (only the ROS box faked behind `RosApi`).

## What shipped

- **`src/host.rs` — `HostCtx`**: the sidecar's one host handle = the `lb-sidecar-client` callback
  client + the sidecar's own grant (`LB_EXT_TOKEN` caps + ws). `require("<verb>")` is the per-verb
  capability self-check; `client()` is the callback for `assets.*`/`secret.*`/`ingest`/`outbox`.
- **`src/shadow.rs`** — the config shadow (`ros` connection records) over the host `assets.*` doc
  store. `RosShadow {uuid,name,base_url,enable,poll_rate,parent}`. Doc id `ros/{uuid}`.
- **`src/resolve.rs`** — `RosApiFactory` (the test seam: `RealFactory` vs a fake), token
  set/get/delete via `secret.*`, and `resolve_api(ros_uuid)` = shadow (base_url) + secret (token) →
  a live `RosApi`.
- **`src/paging.rs`** — the hand-rolled keyset `{items, next_cursor}` envelope (+ unit tests).
- **`src/handlers/`** — `ros.rs` (connection CRUD + `ros.ping`), `network.rs`/`device.rs`/`point.rs`
  (`list`/`get`, proxying the box). Every handler: **cap self-check first**, then shadow/RosApi.
- **`src/lib.rs`** — split lib+bin so `tests/` can drive the handlers; `main.rs` is the thin loop.
- **`extension.toml`** — the resolved capability set (see "Capabilities discovered" below).
- **`tests/crud_test.rs`** — real-gateway CRUD round-trip, cap-deny, workspace-isolation.

## The load-bearing architectural findings (non-obvious; recorded in memory too)

1. **The inbound path to a native sidecar tool carries NO caller identity.** The only route is
   `lb_host::call_sidecar` (gated `mcp:native.call:call`); the sidecar gets `CallParams{tool,input}`
   over the control line — no principal/caps/workspace. Native ext tools are NOT in the runtime
   registry and NOT routed by `call_tool`'s `<ext>.<tool>` branch (wasm-only). So the UI reaches
   `ros.list` via `native.call {ext_id:"ros", tool:"ros.list", …}`.
   - **Consequence — capability:** the fine-grained `mcp:ros.list:call` / `mcp:point.write:call`
     gate is the **sidecar's own job**, against its `LB_EXT_TOKEN` grant (read via
     `lb_auth::claims_unverified` — no key; the host already verified it). This is the scope's
     "each handler does its own capability check," and it's what the deny test drives. (User
     steer: "do whatever is best long term" → self-check + defense-in-depth on the host's coarse
     `mcp:native.call:call`, no ROS-in-core change.)
   - **Consequence — workspace:** isolation is **structural**. The sidecar is spawned per-(ws,ext_id)
     with a fixed `LB_EXT_WS`; every callback authenticates with the ws-scoped token, so ws-A can
     never touch ws-B. The isolation test confirms ws-B sees none of ws-A's connections.

2. **No generic "put a record" host verb exists for a sidecar.** `store.*` is admin read-only;
   `assets.*` (`put_doc`/`get_doc`/`list_docs`/`delete_doc`) IS the workspace-scoped, cap-gated
   key→document store. The config shadow rides it. Low-cardinality config, so `list` = `list_docs`
   (ids) filtered to the `ros/` prefix + `get_doc` each (N+1 is fine; motion goes through `series`).

3. **The gateway maps EVERY `ToolError` (incl. `NotFound`) to an opaque `403`** (`routes/mcp.rs`) —
   the no-existence-oracle contract. So a missing doc reaches the sidecar as `CallError::Denied`,
   indistinguishable from a real refusal. `shadow::get_ros` therefore treats `Denied` as `None`
   (absent): within our own ws the grant covers every `ros/**` doc, so a denial there can only mean
   "absent"; a genuine cap misconfig surfaces at `create`/`list` (which the deny test asserts).

4. **Capability grammar splits on `:` and wildcards per `/` segment.** The doc id must use `/`
   (`ros/{uuid}`), never `:`, or the cap resource `store:doc/ros:{uuid}:read` mis-parses (the `:`
   collides with the action delimiter). `*` matches ONE segment; `**` a trailing run. So:
   `assets.list_docs` needs `store:doc/**:read` (it authorizes against the `*` surface wildcard AND
   each `ros/{uuid}` get needs a two-segment match); write/delete are scoped `store:doc/ros/*`.

## Capabilities discovered (now in `extension.toml`)

Beyond the scope's initial list, the real host gates required:
`mcp:{secret.set,secret.get,secret.delete}:call` + `secret:ros/*/token:{get,write,delete}`;
`mcp:assets.{put_doc,get_doc,list_docs,delete_doc}:call` + `store:doc/**:read`,
`store:doc/ros/*:write`, `store:doc/ros/*:delete`.

## Tests (green)

- `crud_round_trip_and_token_is_never_returned` — create→get→list→tree(net/dev/point)→update→delete,
  asserting the token never appears in `get`/`list`.
- `capability_deny_refuses_before_any_effect` — a grant without `mcp:ros.create:call` is refused and
  leaves NO shadow (a full-grant list is empty after).
- `workspace_isolation_a_cannot_see_b` — ws-B (full grant) sees none of ws-A's connections.
- `paging::tests` — keyset pages walk the whole set without gaps/repeats.

## Deviations / notes

- **One file per resource, not per verb.** FILE-LAYOUT says one file per verb; I grouped a resource's
  `list/get/create/update/delete` per file (each well under 400 lines) to avoid ~25 near-empty files
  that share identical plumbing. Splits per-verb if a file nears the limit as slices 3/4 add verbs.
- `network/device/point . create|update|delete` (write-back to the box) are declared in the manifest
  but not yet wired — the reads are what the UI drill-down needs this slice; writes land with the box
  write-back work.

## Next: Slice 3 — the reusable poller

`src/poller/`: `poller.rs` (loop+schedule+backoff), `source.rs` (Source trait), `sink.rs`
(`ingest.write` via the callback), `gating.rs` (enable AND up the tree). `RosSource` adapts `RosApi`.
`ros.start|stop|status`. The loop/gating/backoff/batching unit-testable with a STUB Source (no box).
