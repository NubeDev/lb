# Session: native (Tier-2) sidecars become first-class in the MCP routing registry

## The ask

Make native Tier-2 sidecars reachable through the ONE `lb_mcp::Registry` routing path
(`resolve`/`dispatch`/`serve_call`) so a routed cross-node call answers against a node's own native
child — with NO per-Tier branch and NO CE/control-engine strings in core. Generic platform infra
(unblocks the control-engine S4 two-node routed `control-engine.tree` exit gate).

## What shipped

- **`LocalDispatch` trait** (`crates/runtime/src/dispatch.rs`): a narrow, object-safe async trait
  `call_tool(&mut self, ws, tool, input_json, ctx)`. Supertrait `Send` (not `Sync` — a wasm
  `Instance` owns a non-`Sync` wasmtime `Store`; `tokio::Mutex<T>` is `Sync` when `T: Send`, so
  `Arc<Mutex<dyn LocalDispatch>>` is `Send + Sync`). Impl for `Instance` (ignores `ws`, forwards to
  `call_tool_with`).
- **Registry Tier-agnostic** (`crates/mcp/src/registry.rs`): `Hosted.instance` is now
  `Arc<Mutex<dyn LocalDispatch>>`. `register`/`register_descriptors` still take a wasm `Instance`
  (box it); added `register_local_dispatch(ext_id, descriptors, Arc<Mutex<dyn LocalDispatch>>)`.
- **dispatch + serve through the trait** (`crates/mcp/src/call/dispatch.rs`,
  `crates/mcp/src/serve.rs`): both call `.call_tool(ws, …)`; try_lock re-entrancy discipline
  preserved. `serve_call` gained a `ws: &str` param.
- **ws recovered on the serving side**: `lb_bus::Incoming::ws()` parses the concrete routed key
  (`ws/{ws}/…`); `crates/host/src/serve.rs` `answer_loop` threads it into `serve_call`. The queryable
  stays `*`-wildcarded (one declaration serves every ws); each `get` targets a concrete ws.
- **Native adapter + shared retry** (`crates/host/src/native/call.rs`): `SidecarDispatch`
  (`Arc<SidecarMap>` + `ext_id`) resolves `get(ws, ext_id)` per call (workspace wall structural),
  re-qualifies the bare tool name to `<ext_id>.<tool>` for the sidecar ABI, and calls via the shared
  `call_once_or_restart` helper. `call_sidecar` (`native/tool.rs`) now uses that same helper (restart
  recovery); the adapter uses a NO-OP recovery.
- **Registered at install** (`native/install.rs`): after `sidecars.insert`, `register_local_dispatch`
  with bare descriptors (strips a `<ext_id>.` manifest prefix). Local `<ext>.<tool>` calls via
  `call_tool` (`tool_call.rs`) now resolve `Target::Local(adapter)` uniformly — no code change there.

## Design choices

- **Launcher/restart in the adapter**: the `Launcher` trait returns `impl Future` (not object-safe),
  and the adapter is stored node-global with no launcher. Chosen: the adapter does a plain call with a
  **no-op recovery** (transport fault surfaces to the routed caller); the typed lifecycle path
  (`call_sidecar`/`restart_native`, which carry the launcher) drives supervised restart. Rejected:
  boxing a `dyn Launcher` (needs an object-safe launcher variant — out of scope). Documented in
  `native/call.rs`.
- **ws-threading for `serve_call`**: recovered from the routed key via `Incoming::ws()` rather than
  adding a field to `CallRequest` — the key already carries the workspace and is the structural wall,
  so the wire envelope stays unchanged.
- **qualified vs bare names**: registry matches BARE names (like wasm); the adapter re-qualifies for
  the sidecar's ABI (its manifest declares tools qualified). Generic — `ext_id + "." + tool`.

## Tests

- New: `crates/host/tests/native_routing_test.rs` — two-node loopback-TCP nodes; native sidecar
  installed on node B, `register_remote_extension` on node A, `serve_ext` on B; a routed
  `control-engine.tree` from A returns B's seeded native graph. (Test files MAY name the ext.)
- `cargo test --workspace` green (exit 0, no FAILED/error lines). Key tests re-run green:
  `native_routing_test` (1), `control_engine_test` (1 + 1 ignored — S3 direct path + hot-restart,
  NOT skipped), `cross_node_routing_test` (3, wasm routed hop unchanged).
- `cargo fmt --check` clean. Sanity-grep for CE strings in core: empty.
