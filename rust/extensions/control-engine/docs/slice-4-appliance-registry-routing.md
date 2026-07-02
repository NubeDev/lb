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

## Testing (this is the isolation slice — all mandatory categories)

- **Two-node routed test** (the `cross_node_routing` / `offline_sync` pattern): two
  in-process `Node`s on one real Zenoh bus; node B's sidecar binds a `ce_fake` (or the
  real engine, opt-in); a ws-A caller on node A runs `ce.tree { appliance }` where the
  record points at node B → the call routes, executes on B, returns the tree. Same
  test asserts the **workspace claim is re-checked on B** (tamper the claim → denied).
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

## Open questions (resolve in-slice)

- Does the existing routed-MCP hop already carry extension tools end to end (it routes
  by ext id), or does anything assume core-only tools? Verify against the shipped
  router before writing any code — if a gap exists it is a **generic** router fix, not
  CE code (core-ignorance invariant).
- `mode: "local"` records: is `node` implicit (the installing node) or explicit?
  Default: explicit always — one resolution path, no special case.
