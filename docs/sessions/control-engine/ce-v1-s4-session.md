# Session — control-engine v1: slice S4 (appliance registry + routed hop)

Status: **done, green.** The `ce_appliance` registry, the three registry verbs, appliance→base
resolution, and the two-node routed `control-engine.tree` all land — plus the generic core work
they required. Branch `ce-v1` (fresh worktree off master; PR to master).

Parent scope: [`control-engine-scope.md`](../../../rust/extensions/control-engine/docs/control-engine-scope.md).
Slice doc (acceptance criteria):
[S4](../../../rust/extensions/control-engine/docs/slice-4-appliance-registry-routing.md).
Prior: [`ce-v1-s1-s3`](ce-v1-s1-s3-session.md).

Core routing work: [`native-routing-registry-session.md`](native-routing-registry-session.md).

---

## What shipped

### 1. CORE (generic, CE-ignorant): native sidecars are first-class in the MCP routing registry

**The gap (Open Question #1, corrected in-slice).** The kickoff premise was "the routed hop is already
generic — no core change." That is true for *routing* (`mcp/src/call/dispatch.rs` routes by ext id) but
NOT for *serving*: `lb_mcp::serve_call` only dispatched against the **wasm** `Registry`, and native
Tier-2 sidecars (like `control-engine`) were not in that registry at all — they were reached only by a
direct host→child `call_sidecar`. So a native ext was **unreachable over the cross-node routed hop**, and
the S4 exit gate (two-node routed `ce.tree`) could not pass without a core addition. This was surfaced to
the user, who chose the **best-long-term** fix (no hack).

**The fix (a small trait, no per-tier branches, no CE strings).** Abstract the local-call target behind
a `LocalDispatch` trait (`lb_runtime`), implemented by both the wasm `Instance` and a new host
`SidecarDispatch` adapter. `lb_mcp::Registry`'s `Hosted` now holds `Arc<Mutex<dyn LocalDispatch>>`, so
`resolve`/`dispatch`/`serve_call`/the catalog treat wasm and native **uniformly**. `install_native`
registers the native ext into the MCP registry via the adapter, so `serve_ext` + `serve_call` answer a
routed call against a native child with zero Tier knowledge. Workspace stays structural: the adapter
resolves `SidecarMap.get(ws, ext_id)` per call (ws recovered from the routed key via `Incoming::ws()`).
Supervised restart stays on the typed lifecycle path (`call_sidecar`); the adapter surfaces a transport
fault to the routed caller (which is exactly S4's "offline fail-loud").

Files: `runtime/src/dispatch.rs` (new trait) · `mcp/src/registry.rs` · `mcp/src/call/dispatch.rs` ·
`mcp/src/serve.rs` · `bus/src/query.rs` (`Incoming::ws()`) · `host/src/serve.rs` ·
`host/src/native/call.rs` (new adapter + shared `call_once_or_restart`) · `host/src/native/tool.rs`
(shares the retry) · `host/src/native/install.rs` (registers the adapter). Proof:
`host/tests/native_routing_test.rs` (a native sidecar reachable over the routed hop — generic).

### 2. CORE (generic): the `store.write` / `store.delete` MCP verbs

There was no generic store-**write** MCP verb (only read-only `store.query`/`store.schema`). Added
`crates/host/src/store_mutate/` — two host-native verbs gated **per table** by the
`store:<table>:<action>` grammar (mirrors `rules_save`): the outer `mcp:store.<verb>:call` gate at the
dispatcher, then the `store:<table>:write` gate inside (delete gated under the same `write` action, like
`rules_delete` — the grammar has no distinct delete action). Wired into `is_host_native` +
`call_tool` dispatch + `lib.rs`. This is generic, CE-ignorant platform infra: the CE `ce_appliance`
registry is its first caller (it requests `store:ce_appliance:write`), and **nothing in core mentions
`ce_appliance`**. Tests: `host/tests/store_mutate_test.rs` (round-trip · deny-per-verb before any
write · ws-isolation + cross-ws write refused).

### 3. CE (the extension): the `ce_appliance` registry + resolution

- **lib+bin split** (ROS-sidecar precedent) so the integration tests drive the verbs against a real
  gateway. `main.rs` is now a thin wrapper over `serve::serve`.
- **`HostCtx`** (`src/host.rs`, the ROS idiom): the `lb-sidecar-client` callback transport + the
  sidecar's own grant, with a per-verb `require()` self-check (the inbound `native.call` carries no
  caller identity — the fine `mcp:control-engine.appliance.*:call` gate is the sidecar's job).
- **Record + store** (`src/appliance/`): the `Appliance` model (`{id,name,mode,node,base,secret_ref?,ts}`,
  table `ce_appliance`) and its `store.*`-callback CRUD (`put`/`get`/`list`/`remove`). Sorted host-side
  (SurrealDB `ORDER BY` needs the idiom in the selection — see the debug entry).
- **Verbs** (`src/tools/appliance/{add,list,remove}.rs`, one file each): `add` validates
  `id`/`node`/`base` (http(s) origin) + `mode`, upserts; `list` reads ws-walled; `remove` deletes the
  record only (no CE reach).
- **Resolution** (`src/resolve.rs`): a graph verb's `appliance` selector → the CE **base**. Empty →
  canonical local; a known record → its base; an unknown/other-ws id → **not-found** (the isolation
  wall, no existence leak). Real-engine dev tier (no gateway) falls back to a literal base only when the
  registry store itself is unreachable — never when a real gateway says "absent" (so no isolation leak).
  **Local-vs-remote is the host's job, not the sidecar's** (symmetric nodes): any call that reaches the
  sidecar is for an appliance this node owns; the host router forwarded a remote appliance to its owner.
- **Manifest**: requests `mcp:store.{write,query,delete}:call` + `store:ce_appliance:{read,write}` + the
  three `control-engine.appliance.*` tools.

---

## Tests (all green — real infra, real seed; no mocks beyond the one `ce_fake`)

- `host/tests/store_mutate_test.rs` — 4: write round-trip + idempotent delete · deny-per-verb (no
  record written / erased) · ws-B write invisible to ws-A + cross-ws write refused.
- `control-engine/tests/appliance_registry_test.rs` — 6 (real spawned gateway + node + store + caps):
  add→list→resolve→remove round-trip · deny before write (self-check) · deny by the host store gate ·
  remove deny (record survives) · **ws-A appliance invisible to ws-B** (list empty · resolve not-found ·
  remove no-op) · **stateless** (a fresh `HostCtx` still answers — registry reread from SurrealDB).
- `host/tests/control_engine_appliance_routing_test.rs` — 1: the **two-node routed `control-engine.tree`
  driven by an `ce_appliance` record** (A → B over Zenoh, B's native sidecar returns its seeded graph) +
  **offline fail-loud** (drop B → prompt loud `Extension` error, **no outbox rows queued**).
- `host/tests/native_routing_test.rs` — 1 (core): a native sidecar reachable over the routed hop.
- `control-engine` crate units — 3: dispatch deny-before-call + verbatim DTOs.
- Regression: `cross_node_routing_test` (wasm hop) + `control_engine_test` (S3 direct + hot-restart) stay
  green. `cargo test --workspace` green (paste below); `cargo fmt`; sanity-grep empty.

```
# LB_CE_FAKE=1 cargo test --workspace --no-fail-fast -- --test-threads=4
TOTAL passed=1273 failed=0 ignored=1     (exit 0)

# isolated confirmations
control-engine (crate)                     : 3 passed  (dispatch deny-before-call + verbatim DTOs)
host store_mutate_test                      : 4 passed
host native_routing_test                    : 1 passed  (native ext over the routed hop — core)
host control_engine_test                    : 1 passed, 1 ignored (S3 direct + hot-restart)
host control_engine_appliance_routing_test  : 1 passed  (two-node routed ce.tree + offline fail-loud)
control-engine appliance_registry_test      : 6 passed  (CRUD · resolve · deny-per-verb · ws-iso · stateless)
cross_node_routing_test                     : 3 passed  (wasm hop, regression; 3/3 isolated)

# real-engine tier (opt-in, against live ce-studio on :7979)
CE_ENGINE_URL=127.0.0.1:7979 cargo test -p lb-host --test control_engine_test -- --ignored
control_engine_against_real_ce_studio       : 1 passed

cargo fmt        : clean
sanity-grep      : empty over crates/{host,mcp,caps,runtime,bus}/src + role/*/src (control-engine|ce_appliance|rubix-ce|…)
```

> Note on the workspace run: `--test-threads=4` (not the default) sidesteps the **pre-existing**
> `cross_node_routing` Zenoh-discovery flake that only trips under maximum parallel load (documented in
> STATUS.md; fails identically on clean HEAD). The test passes 3/3 in isolation and here.

---

## Open questions (resolved in-slice)

- **OQ#1 (routed hop carries ext tools end to end?)** — Routing is generic (by ext id) but *serving* was
  wasm-only; a native ext was unreachable cross-node. Fixed as a **generic** router change (the
  `LocalDispatch` trait; native sidecars first-class in the registry) — NOT CE code. Slice doc's "no core
  change" premise corrected.
- **OQ#2 (`mode:"local"` node implicit or explicit?)** — **explicit always** (`node` is required on
  `add`). One resolution path, no special case.
- **Slice doc's stale isolation premise ("tamper the claim → denied on B")** — corrected: node B does NOT
  re-verify a token; the routed key carries the ws structurally. So the enforceable isolation is that a
  ws-B principal lands on ws-B's key/namespace and finds not-found (tested), not a claim-tamper rejection
  the stack does not perform.

## Follow-ups (deferred, additive)

- S5 write verbs + `secret_ref` mediation on the appliance record.
- S6 `ce.watch` COV; `appliance.remove` disarming a live watch.
- A discovery layer that reads `ce_appliance` records to populate the remote-routing entry (S4 stands it
  in with `register_remote_extension` in the test).
