# Slice 4 — the appliance registry + the routed hop

Status: scope slice (S4). Depends on: S3 (the sidecar + read verbs exist). Parent:
`control-engine-scope.md`.

Make `appliance` a real, workspace-walled concept: the `ce_appliance` registry in
SurrealDB, the three registry verbs, and local-vs-remote resolution — a `ce.*` call
naming an appliance owned by another node rides the **existing** routed-MCP-over-Zenoh
hop to that node's identical sidecar. This is the slice where both mandatory test
categories (deny + isolation) land in full, and where the symmetric-nodes claim is
proven with two real in-process nodes.

## Deliverables

- **Record**: `ce_appliance:{ws}:{id}` = `{ id, name, mode: "local"|"appliance",
  node, base, secret_ref?, ts }` — written via the **generic** `store:ce_appliance:*`
  verbs the sidecar requests in its manifest (no host table code, per the
  core-ignorance invariant). `node` is an enrolled machine principal's node id
  (`api-keys` `kind="appliance"` + `edge-trust` — reused as-is, no new enrollment).
- **Verbs** (one file each, `src/tools/appliance/`): `ce.appliance.add`,
  `ce.appliance.list`, `ce.appliance.remove`. `add` validates: `node` exists and is
  enrolled in this workspace; `base` parses as an http(s) origin; id is unique.
  `remove` does NOT reach into the CE — it deletes the record (and S6 will disarm any
  live watch).
- **Resolution** (`src/resolve.rs`): every non-registry `ce.*` verb resolves
  `args.appliance` → the record (workspace-first) → owning `node`:
  - this node → serve locally via the cached `ce-client-rust` client for that record's
    `base`;
  - another node → return the routed-hop indication so the **host router** forwards
    the call over Zenoh (the existing `<ext>.<tool>` cross-node hop — the router
    routes by ext id, never inspects the tool). The sidecar itself never opens a
    Zenoh session; symmetry is the host's job.
  - unknown/other-workspace appliance → **not-found** (indistinguishable from
    non-existent — don't leak existence across the wall).
- Manifest additions: `store:ce_appliance:read`, `store:ce_appliance:write`, the
  registry `[[tools]]` with their own caps (`mcp:control-engine.appliance.add:call`
  etc. — registry writes are admin-ish, distinct from graph writes).

> **Status: SHIPPED (2026-07-02).** Built as three layers — a generic core change (native sidecars
> first-class in the MCP routing registry), the generic `store.write`/`store.delete` verbs, and the CE
> `ce_appliance` registry + `resolve.rs`. See [`ce-v1-s4-session`](../../../../docs/sessions/control-engine/ce-v1-s4-session.md)
> and [`native-routing-registry-session`](../../../../docs/sessions/control-engine/native-routing-registry-session.md).
> Exit gate met. The corrections below (Open Questions + the stale claim-tamper premise) are resolved.

## Testing (this is the isolation slice — all mandatory categories)

- **Two-node routed test** (the `cross_node_routing` pattern): two in-process `Node`s on one real Zenoh
  bus; node B's sidecar binds a `ce_fake` (opt-in real engine); a ws-A caller on node A runs
  `control-engine.tree { appliance }` where the record points at node B → the call routes, executes on
  B, returns the tree.
  - **CORRECTION (stale premise):** the original "asserts the workspace claim is re-checked on B (tamper
    the claim → denied)" does NOT match the shipped stack — node B does **not** re-verify a token on the
    routed hop (`mcp/src/serve.rs`: "It does NOT re-authorize"). The workspace rides the Zenoh key
    structurally: a ws-B principal's call physically lands on `ws/ws-B/...`, which B answers only for
    ws-B. So the enforceable isolation is that a ws-B caller reaches ws-B's namespace and finds
    **not-found** (tested in `appliance_registry_test`: ws-B resolve of a ws-A id → not-found), NOT a
    claim-tamper rejection. The routed happy-path is `control_engine_appliance_routing_test`.
- **Workspace isolation:** appliance registered in ws-A: absent from ws-B's
  `ce.appliance.list`; ws-B `ce.tree` naming it → not-found; ws-B cannot
  `ce.appliance.remove` it.
- **Capability deny:** `ce.appliance.add` without its cap → denied before any store
  write (assert no record).
- **Offline fail-loud:** stop node B; `ce.tree { appliance→B }` errors promptly and
  loudly; assert nothing queued (no outbox rows, no retry) — interactive commands are
  online request/response by decision.
- **Restart/statelessness:** respawn the sidecar; `ce.appliance.list` still answers
  (registry is in SurrealDB, reread on demand — no in-memory registry cache without
  invalidation, keep it simple: read per call until measured).

## Exit gate

Two-node routed `ce.tree` green + the full isolation/deny matrix green in
`cargo test --workspace`.

## Open questions (RESOLVED in-slice)

- **Does the existing routed-MCP hop already carry extension tools end to end?** — Partly. *Routing*
  (`mcp/src/call/dispatch.rs`) is generic (by ext id), BUT *serving* (`lb_mcp::serve_call`) dispatched
  only against the **wasm** registry — native Tier-2 sidecars were not in it and were reachable only by
  a direct `call_sidecar`. So a native ext was **unreachable over the cross-node hop**. Fixed as a
  **generic** router change (core-ignorance held): a `LocalDispatch` trait (`lb_runtime`) + a host
  `SidecarDispatch` adapter make native sidecars first-class in the ONE registry, so `resolve`/
  `dispatch`/`serve_call` are Tier-agnostic. No CE strings in core; sanity-grep clean.
- **`mode: "local"` records: `node` implicit or explicit?** — **Explicit always** (`node` is required on
  `appliance.add`). One resolution path, no special case.
