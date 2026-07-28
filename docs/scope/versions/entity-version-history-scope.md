# Versions scope — generic entity version history + restore

Status: scope (the ask). Promotes to `doc-site/content/public/versions/` once shipped.

Every save of a dashboard, flow, or rule silently destroys the previous record. Flows carry a
`version: u32` counter, but it is run-pinning only — the prior graph is overwritten on save
(`crates/host/src/flows/save.rs:33-47`) and nothing can list or restore it. Dashboards and rules
have no version at all. The ask: keep the **last N (default 20, adjustable) full snapshots of an
entity per save**, per entity, in the store, with `versions.list` / `versions.get` /
`versions.restore` verbs — one **generic** subsystem that covers dashboards, flows, and rules on
day one and any future kind by adding a row to a plan table, never per-entity code. Flows adopt it
too: their counter stays (runs pin it), but durable history + restore come from here.

**Owning repo: `NubeDev/lb` (this repo).** Ships in `lb-node`, released as a `node-v*` tag;
`NubeIO/rubix-ai` then bumps its pin and builds the UI (its
`docs/scope/frontend/version-history-scope.md`).

## Goals

- A capped, per-entity ring of full after-image snapshots: newest `N` versions of each
  dashboard / flow / rule, written automatically on every successful save — no per-entity save
  code touched, no opt-in from callers.
- `versions.list { kind, id }` (metadata, newest-first), `versions.get { kind, id, version_id }`
  (the snapshot), `versions.restore { kind, id, version_id }` (make an old version the live
  record again), `versions.config.get/set` (the adjustable cap, admin).
- Restore is a **forward action**: it re-dispatches the entity's own `*.save` verb with the
  snapshot as input, so it inherits every existing validator (flows' DAG/config/cron checks,
  dashboard bounds/view checks), the caps wall, audit, cache invalidation, and undo capture.
  Never a raw store write.
- Restore works after delete: the ring outlives the entity, so restoring the last snapshot
  recreates a deleted dashboard/flow/rule.
- Generic by construction (rule 10): kinds are data rows in one plan table
  `(kind → table, save_tool, id extraction)`; adding a kind is adding a row.

## Non-goals

- **Not undo.** The undo journal (`scope/undo/undo-scope.md`) is a per-actor, linear,
  conditional-restore stack; this is per-entity, addressable, restore-anything history. They
  compose (a restore is itself undoable) but share no storage.
- No diffs, labels, comments, or "named releases" on versions — metadata is
  `{ actor, ts, tool, entity_rev }`; a diff UI is a host/frontend concern over `versions.get`.
- No branching / merge / time-travel queries across entities.
- No document/asset content versioning (per-asset concern, per undo-scope).
- No capture on `*.delete` — the ring already holds the last saved states; a tombstone row adds
  nothing.
- v1 kinds are core-owned (dashboard, flow, rule). Extension-declared kinds (via manifest) are a
  named follow-up; the plan-table seam is shaped for it.

## Intent / approach

Three small pieces in `crates/host/src/versions/` (verb-per-file, FILE-LAYOUT), no new crate:

1. **Capture** — a `versions_capture` sibling to `undo_capture`, hooked at the same depth-0
   dispatch chokepoint in `crates/host/src/tool_call.rs` (~:402, the "shared seam"). A pure
   `plan.rs` maps a successful mutating call to `Captured { kind, table, id }` or `NotCaptured`:
   `dashboard.save` / `flows.save` / `rules.save` in v1 (ids from the call's `input.id`, the same
   derivation `undo_capture/plan.rs` proves). Capture writes the **after**-image (read back via
   `read_versioned` to stamp `entity_rev`) — undo journals the *before*, history keeps the
   *after*, because "restore version 7" means "what it looked like after that save".
2. **Storage** — one `lb_store::capped_insert` per capture into table `entity_version`, with
   `cap_key = "{kind}:{id}"` and ULID row ids. That primitive (`crates/store/src/capped.rs:91`)
   already gives transactional insert+trim-to-newest-N, FIFO ordering without a clock, and a
   per-key lock against snapshot-isolation over-growth — it was built to be reused exactly like
   this. Record shape:
   `{ kind, entity_id, entity_rev, entity_version?, tool, actor, ts, snapshot }`
   (`entity_version` carries flows' `u32` counter when the kind has one). **Dedupe:** if the
   snapshot hash equals the ring head's, skip — a no-op save must not burn a ring slot.
3. **Verbs** — `versions.list` returns metadata only (never N full snapshots in one response);
   `versions.get` returns one snapshot; `versions.restore` re-dispatches the kind's `save_tool`
   at depth+1 through `call_tool` with the snapshot as args. The restore's own save is then
   captured normally, so restoring v7 appends a new head equal to v7 — history stays append-only
   and re-restorable.

**The adjustable cap.** `pub const DEFAULT_VERSION_CAP: usize = 20` (the house pattern —
`DEFAULT_FINISHED_RUN_CAP`, undo's `DEFAULT_DEPTH_CAP`), overridable per workspace by a
`versions_config` store record `{ cap, per_kind: { <kind>: cap } }` via admin-gated
`versions.config.get/set`, node-clamped to `1..=100`. Resolution: per-kind override → workspace
cap → const. A lowered cap applies on next capture (capped_insert trims to the cap it is handed);
no reaper job.

**Rejected alternatives.** (a) Per-entity version code in each save path — three copies today,
N copies tomorrow, and the exact thing the generic chokepoint exists to avoid. (b) Widening the
undo journal into history — its scope doc explicitly disclaims version history; keys, semantics,
and retention are all wrong (per-actor stack vs per-entity ring). (c) Raw store write on restore —
bypasses validators, caps, audit, cache invalidation, and undo; undo-scope already litigated and
rejected out-of-band restores. (d) SurrealDB TTL / reaper retention — age-based or overshooting;
`capped.rs`'s module doc records why the transactional ring wins.

## How it fits

- **Isolation/tenancy** — `entity_version` rows are ordinary workspace-scoped store records;
  `capped_insert` is ws-scoped; the ring is invisible cross-workspace. Sync ships them like any
  record; restore-conflict semantics inherit from the save verb (a forward action, per undo-scope).
- **Capabilities & the deny path** — dotted verbs so wildcards work (the `undo`/`redo`
  ungrantable-verb trap from `undo-exposure-scope.md`). `versions.list` rides the viewer
  `mcp:*.list:call` wildcard; `versions.get` and `versions.restore` get explicit `member` grants
  in `authz/builtin_roles.rs`. **No escalation:** `versions.restore` requires the caller to hold
  the cap for the kind's underlying `save_tool` — checked before re-dispatch, same pattern as
  undo. The named deny: a viewer holding `versions.list` but not `mcp:dashboard.save:call` is
  refused `versions.restore` with the standard cap error. `versions.config.set` is
  `ADMIN_ONLY_CAPS`.
- **Placement / symmetric nodes** — capture and restore run at the authoritative node inside
  normal dispatch; no role branch, no new config on `BootConfig`.
- **API/MCP surface** — get-list + one action verb. Registered in all three places: rich
  descriptors with real JSON Schemas in `tools/descriptor.rs`, `HOST_TOOLS` rows in
  `system/catalog.rs`, a `versions.` prefix arm in `tool_call.rs`'s `run_host_verb`. A dedicated
  gateway route file `role/gateway/src/routes/versions.rs` (house pattern, mirroring
  `history.rs`).
- **Data** — one new table, ringed at N per entity; snapshots are the entity's own JSON under the
  store's `{ data, rev }` envelope. **Prerequisite: the structural `Secret<T>`
  never-in-a-snapshot guard** that `undo-exposure-scope.md` names as the gate for widening any
  captured floor — it lands with (or before) this scope and covers both undo's journal and this
  table. v1 kinds are safe today (rule bodies reference secret *names*), but the guard is
  structural, not per-kind review.
- **Motion** — none. Capture is a store write on the dispatch path; restore emits whatever the
  save verb already emits. Cache invalidation falls out for free: the re-dispatched save verb is
  already mapped in `cache/policy.rs`.
- **Undo interaction** — a restore's save is undo-captured like any depth-0 dashboard save
  (dashboards are in the allowlist today; flows/rules restores become undoable when undo's floor
  widens — independent, no coupling). Note the restore dispatch runs at depth+1 under the
  `versions.restore` call: the depth-0 wrap must treat the restore as the user action, capturing
  the entity write it performs.
- **Rule 10 / no mocks** — the plan table names core kinds only, generically; tests boot the real
  `mem://` store and dispatch real verbs.

## Example flow

1. ada edits the "Plant Room" dashboard and hits save → `dashboard.save` dispatches, succeeds.
2. At depth 0, `versions_capture` classifies the call → `Captured { kind: "dashboard", ... }`,
   reads the record back (`entity_rev = 41`), hashes it (≠ head), and `capped_insert`s
   `{ kind, entity_id, entity_rev: 41, tool: "dashboard.save", actor: "ada@acme.com", ts,
   snapshot }` with `cap_key = "dashboard:plant-room"`, cap 20. The oldest ring row is trimmed in
   the same transaction.
3. Three saves later the layout is wrecked. `versions.list { kind: "dashboard", id: "plant-room" }`
   → 20 metadata rows newest-first.
4. `versions.restore { kind: "dashboard", id: "plant-room", version_id: <ulid of rev 41> }` —
   the host checks ada holds `mcp:dashboard.save:call`, loads the snapshot, re-dispatches
   `dashboard.save` with it. Validators run, caches invalidate, audit logs, undo captures.
5. The restore's save is itself captured → the ring head is now a copy of rev 41. Ctrl+Z would
   undo the restore.
6. A flow follows the same path via `flows.save` — restore bumps `flow.version` (the counter
   keeps its run-pinning meaning; history rows carry which counter value each snapshot had). A
   rule likewise via `rules.save`.
7. ada deletes a rule by mistake → `versions.list { kind: "rule", id }` still answers →
   `versions.restore` of the head recreates it.

## Testing plan

Real infra (`mem://` store, real dispatch), seeded data. Mandatory categories:

- **Capability-deny** — `versions.restore` refused for a caller lacking the kind's save cap
  (holding `versions.restore` alone); `versions.config.set` refused for non-admin;
  `versions.get` refused without grant.
- **Workspace-isolation** — rings written in ws A are invisible to `versions.list` in ws B; a
  cross-ws `versions.restore` by id is refused, not applied.
- **Offline/sync** — a version row created at the edge syncs like any store record; restore at
  the hub of an edge-authored snapshot is a plain forward save.
- Unit: `plan.rs` classification (save→Captured with correct kind/id; delete/list/other→
  NotCaptured); cap resolution (per-kind > workspace > const, clamped); dedupe (identical
  snapshot hash skips).
- Integration: 25 saves → exactly 20 ring rows, newest-first, oldest trimmed; concurrent saves
  of one entity never over-grow the ring (the capped key-lock); restore roundtrip per kind —
  dashboard/flow/rule content byte-equal to the snapshot after restore; restore re-runs
  validators (a snapshot made invalid by a since-tightened check is *refused*, not written);
  restore-after-delete recreates the entity; a restore appends a new head; undo after a
  dashboard restore returns the pre-restore record; `cache/policy.rs` invalidation observed
  after restore.
- Catalog: `versions.*` present in `system.tools`; descriptors validate args (bad `kind` → typed
  error, not a store miss).

## Risks & hard problems

- **Snapshot size × N × entities.** A large dashboard JSON ×20 is real bytes. Accepted: rings
  are hard-capped, dedupe skips no-ops, and `versions.list` never ships snapshots. If a kind
  grows pathological, the per-kind cap override is the lever; compression is a later,
  compatible optimization inside the record.
- **The secret guard is a genuine prerequisite** — shipping snapshots before the structural
  `Secret<T>` guard re-opens the exact hole undo-exposure fenced. Sequenced first in the build.
- **Capture failure must not fail the save.** Same posture as undo capture: the user's save
  succeeds; a failed capture is logged loudly and the ring simply misses a version — never a
  taint that silently disables history without trace.
- **Restore vs concurrent edit** is last-writer-wins by design (it is a plain save). That is the
  correct product semantic here — unlike undo, which refuses on drift — and the appended head
  means nothing is ever lost by racing.

## Open questions

Two raised by the build + live verification (2026-07-28), neither blocking what shipped:

- **Should `versions.list` return a per-entity `can_restore`?** `dashboard.save` is owner-gated
  *underneath* its capability, while the no-escalation check re-demands only the capability — so a
  member restoring a colleague's board passes both versions gates and is refused by ownership at the
  nested save. The refusal is correct and non-silent, but a client cannot predict it without
  modelling per-kind ownership, which is exactly the knowledge the generic seam keeps out of clients.
  The node already knows the answer. **Recommendation: add it**, as an additive field.
- **Should the ring row carry the kind's `hash_ignore`?** The client diff currently shows save
  metadata (a dashboard's `updated_ts`) beside real changes, because only the node knows which fields
  are stamped by the act of saving. Returning them (e.g. `meta_fields`) would let a diff sort them
  last without any per-kind client knowledge. Small, additive, cosmetic.

Otherwise none — the user asked for a decision-complete scope. Decisions that would otherwise be
questions, made above: after-image (not before) snapshots; no capture on delete; delete keeps
the ring (enables recreate); restore = re-dispatch save (never raw write); LWW on restore vs
undo's refuse-on-drift; cap = const 20 + per-workspace/per-kind admin override clamped 1..=100;
dedupe by snapshot hash; metadata-only `list`; dotted verb names; extension kinds deferred but
seam shaped.

## Related

- `scope/undo/undo-scope.md` — the sibling projection; its "not version history" non-goal is
  this scope. `scope/undo/undo-exposure-scope.md` — the `Secret<T>` guard prerequisite and the
  grantable-verb trap.
- `crates/store/src/capped.rs` — the storage primitive (module doc = the retention design).
- `scope/flows/flows-scope.md` Decision 1 — `flow.version` stays run-pinning; unchanged here.
- Downstream: `NubeIO/rubix-ai` → `docs/scope/frontend/version-history-scope.md` (the UI), which
  bumps to the `node-v*` tag this ships in.
