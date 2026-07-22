# Packs scope — pack-seeded series retention (`retention:` block, `Kind::Retention`)

Status: scope (the ask). Promotes to `doc-site/content/public/packs/packs.md` once shipped.

A domain pack can declare **series retention policies** as first-class pack content, so applying the
pack sets the per-workspace `series.retention.*` policy for its time-series prefixes automatically —
the same way `rules:` seeds rules and `channels:` seeds channels. Today retention is live runtime
state a human must set with `series.retention.set` per deployment; a pack that ships a high-rate
producer (EMS's modbus polls write `modbus.*` series continuously) has **no** way to declare "keep
raw for an hour, roll up to 1-minute buckets for a week" — so every deployment accumulates unbounded
raw and every `series.read`/bucket query slows linearly until an operator remembers to run the verb by
hand. This adds a `retention:` block to `pack.yaml` and a `Kind::Retention` apply arm that calls the
existing `set_policy` under the caller's principal, closing that gap with zero new persistence and no
new capability grammar.

## Goals

- **`pack.yaml` gains a `retention:` block** — a list of policy objects, each mirroring the
  `series.retention.set` wire shape exactly (`prefix`, `raw_for_ms`, `max_samples`, `tiers:
  [{width_ms, keep_for_ms}]`). Applying the pack sets each policy in the workspace.
- **One new `Kind::Retention`**, added by the documented closed-`Kind` four-step recipe
  (`pack-core-scope §Workspace-seed kinds`): a `Kind` variant + `as_str()`, a plan arm (id +
  checksum over the block bytes), a dispatch arm that calls `set_policy`, and the manifest serde
  (`deny_unknown_fields`, line-numbered). Nothing else in `apply_plan`/receipt/refusal changes.
- **Rides the existing retention capability** — the dispatch arm calls the SAME `set_policy` the
  `series.retention.set` verb calls, which re-checks `mcp:series.retention.set:call` under the
  caller's principal. No new cap, no pack-specific authority. A caller who can't set a policy by
  hand can't smuggle one via a pack.
- **Idempotent, drift-aware, loud-clobber** — a policy is LWW keyed by `prefix` (its natural id);
  re-applying an unchanged block is a NoOp, a changed block re-applies (listed as
  `retention:<prefix>`), matching the sidebar clobber contract. An operator who hand-edited a
  policy the pack owns sees it clobbered loudly on re-apply, never silently.
- **EMS is the first consumer** — `packs/ems/pack.yaml` gains a `retention:` block for the
  `modbus.` prefix, so a stamped-and-polling EMS meter's series are bounded from first apply. (The
  consumer edit ships in the ems repo per WORKFLOW-LB; this scope delivers the lb capability + the
  pack.yaml block in the rubix-ai donor tree.)

## Non-goals

- **A retention job / scheduler.** This SETS the policy; running GC on a schedule is the separate
  `jobs/job-retention-scope` concern. A pack declares the policy; a node's GC tick (or a manual
  `series.retention.gc`) enforces it. Seeding the policy is the missing declarative half, not the
  enforcement loop.
- **New retention semantics.** `raw_for_ms`/`max_samples`/`tiers` mean exactly what
  `series-retention-scope` defines; this is a new *authoring surface* for the existing policy, not a
  change to what a policy does or how GC reads it.
- **Per-entity or per-series policies.** Retention keys on a name *prefix* (unchanged) — a pack
  declares prefixes, not individual series. A meter-level override is out of scope (no caller).
- **Deleting a policy on pack-uninstall / a `retention: []` teardown.** Packs don't uninstall
  content today (no content-GC across the family); a policy the pack stops declaring is left in
  place, same as a rule the pack stops listing. Teardown is a family-wide future concern, not this
  arm's to invent.
- **Validating the prefix against declared series.** A pack may seed a policy for a prefix no series
  uses yet (the producer extension writes them later) — that's valid, not a warning. `set_policy`
  already tolerates a prefix with no matching series.

## Intent / approach

The closed-`Kind` extension path (`pack-core-scope`) makes this a **data-only** addition — every new
apply arm branches on the KIND, never on a named pack (rule 10 holds). The four steps, concretely:

1. **`Kind::Retention`** in `crates/packs/src/plan.rs` with `as_str() == "retention"` (the receipt
   discriminator, a reader's kind string).
2. **Plan arm** — one planned object **per policy**, keyed by the policy's `prefix` (its natural,
   stable id, like a channel's name), with a checksum over the policy's serialized bytes so a
   changed policy is drift and re-applies while an unchanged one is a NoOp.
3. **Dispatch arm** in `crates/host/src/pack/apply.rs` (`apply_retention`) — find the policy in
   `pack.retention` by prefix, deserialize to the ingest `Policy`, and call `crate::…set_policy`
   (the exact internal function `series.retention.set` dispatches to), which **re-checks
   `mcp:series.retention.set:call`** under the caller's principal. Returns the same warning/error
   shape as `apply_rule`/`apply_channel`.
4. **Manifest serde** — a `retention: Vec<RetentionPolicy>` field on `Manifest`
   (`crates/packs/src/manifest.rs`), `#[serde(default, deny_unknown_fields)]`, with a
   `RetentionPolicy` struct + `RetentionTier` sub-struct whose field names/shape match the
   `series.retention.set` args **byte-for-byte** (`prefix`, `raw_for_ms`, `max_samples`, `tiers`).

**Inline objects, not file refs.** Retention policies are small structured records, so the block is a
list of **inline structs** in `pack.yaml` (the `channels:`/`insights:` model), NOT a file-ref list
(the `rules:`/`dashboards:` model where content lives in a separate file). This keeps a policy
readable in one place and needs no bundle-file plumbing.

**Rejected — a `retention:` file-ref pointing at a `.json` policy file** (the rules model): a
one-line policy in an external file is worse to read and adds bundle plumbing for no gain; inline
matches the data's size. **Rejected — extending the entity binding so a `meter` entity carries a
retention hint**: retention keys on a *series prefix*, which is a producer/ingest concept the entity
layer knows nothing about (rule 10) — the policy belongs beside the other time-series content
(`datasource:`), authored as its own block. **Rejected — seeding the policy from the ems extension's
boot path** instead of the pack: the policy is domain/product config (which prefixes, what horizons),
which is precisely what a PACK owns; putting it in the extension splits one product's declarative
config across two artifacts and hides it from `pack.get`'s receipt.

## How it fits the core

- **Tenancy / isolation:** `set_policy` is `store.query_ws(ws, …)` — the policy row is written in the
  caller's workspace, scoped before any write. A pack applied in ws-A can only ever set ws-A's
  retention; the apply loop already runs every object under the one workspace `pack.apply` resolved.
  Mandatory isolation test: two packs, two workspaces, each sets only its own policy; neither reads
  or clobbers the other's.
- **Capabilities:** **no new cap.** The dispatch arm calls `set_policy`, which authorizes
  `mcp:series.retention.set:call` under the **caller's** principal (the per-object caps wall from
  pack-core §Caps). A `pack.apply` caller lacking that grant gets a partial receipt with
  `retention:<prefix>` denied — recorded, recoverable on grant+re-apply, never silently applied.
  Deny test below.
- **Placement:** `either`, no `if cloud`. A retention policy is a plain workspace record set
  identically on an edge node or the cloud head-end; which prefixes exist is pack config, not a role
  branch.
- **MCP surface (§6.1):** **no new verb.** This is pack *content*, applied through the existing
  `pack.apply` (batch-apply) surface — `pack.validate` (read) previews the plan incl. the new
  `retention:<prefix>` objects; `pack.apply` (admin) writes them; `pack.get` (member read) shows
  them in the receipt. No CRUD verb of its own (the policy CRUD verb `series.retention.set` already
  exists and is unchanged), no live feed (a policy is state, read via `series.retention.list`), no
  new batch job (the apply is a bounded per-object loop the pack engine already owns; each
  `set_policy` is one indexed write).
- **Data (SurrealDB):** no new table. Reuses the existing `series_retention` policy table
  `set_policy` writes. The receipt gains `retention:<prefix>` object rows in the existing receipt
  store. State plane only.
- **Bus (Zenoh):** none. Setting a policy is committed state; GC enforcement (a separate concern)
  emits no motion here.
- **Sync / authority:** writes committed local workspace state; no new authority. A policy set on an
  offline edge node applies locally, same as any pack object.
- **Secrets:** none.
- **No mocks:** the apply test boots the real node (`mem://` store, real gateway, real `pack.apply`)
  and asserts `series.retention.list` returns the seeded policy after apply — no fake pack engine, no
  fake retention store (pack-core's house bar).
- **One responsibility per file:** `apply_retention` is one function beside `apply_rule`/
  `apply_channel` in `crates/host/src/pack/apply.rs`; the `Kind::Retention` variant + plan arm live
  in `crates/packs/src/plan.rs`; the manifest structs in `crates/packs/src/manifest.rs`. No file
  grows a new responsibility beyond the existing per-arm pattern.
- **SDK/WIT impact:** none — pack content is host-side; no plugin boundary, no ABI.
- **Skill doc:** **N/A.** This adds no new agent-/API-drivable *surface* — `pack.apply`/`validate`/
  `get` and `series.retention.set`/`list` are all unchanged and already documented. The `retention:`
  block is a new *authoring* shape in `pack.yaml`, documented in the packs public doc, not a new verb
  a skill drives.

## Example flow

A blank workspace, applying the EMS pack.

1. Operator: `pack.apply { pack: "ems", … }` under a token with `mcp:pack.apply:call` **and**
   `mcp:series.retention.set:call`.
2. The pack engine plans every object; the new `retention:` block yields one object
   `retention:modbus.` with a checksum over its policy bytes.
3. First apply (no prior receipt): `apply_retention` deserializes the `modbus.` policy to the ingest
   `Policy` and calls `set_policy(store, principal, ws, policy)`, which authorizes
   `mcp:series.retention.set:call` under the operator, then writes the policy row in `ws`.
4. Receipt records `retention:modbus.` as applied. `series.retention.list` now returns the policy:
   `{ prefix:"modbus.", raw_for_ms:3_600_000, max_samples:…, tiers:[{width_ms:60_000, keep_for_ms:…}] }`.
5. The modbus extension polls; `modbus.*` series now have a policy, so the next GC tick rolls raw
   older than an hour into 1-minute rollups and evicts it — bucket reads stay fast and bounded from
   day one, with no operator step.
6. A later `pack.apply` with the SAME policy is a NoOp (checksum match); a bumped `raw_for_ms`
   re-applies and the receipt lists `retention:modbus.` as clobbered (loud, LWW).

## Testing plan

Per `scope/testing/testing-scope.md`. No mocks — real `mem://` node, real gateway, real
`pack.apply`, real `set_policy`/`series.retention.list`.

Mandatory categories:

- **Capability deny** — a `pack.apply` caller holding `mcp:pack.apply:call` but NOT
  `mcp:series.retention.set:call` gets a partial receipt: entity/rule objects apply, `retention:<prefix>`
  is denied and listed, no policy is written. Granting the cap + re-apply recovers it (the pack-core
  partial-recovery matrix, exercised for this arm).
- **Workspace isolation** — two packs declaring different `modbus.` policies applied in two
  workspaces set only their own; `series.retention.list` in ws-A never shows ws-B's policy.

Slice-specific:

- **Apply sets the policy** — blank node → `pack.apply` of a fixture pack with a `retention:` block →
  `series.retention.list` returns the exact declared policy (prefix, horizons, tiers). The headline
  test.
- **Idempotent + drift** — re-apply unchanged → NoOp (receipt unchanged, no clobber listed); re-apply
  with a bumped horizon → re-applies, `retention:<prefix>` listed as clobbered, `list` shows the new
  value.
- **Manifest serde** — an unknown field inside a policy (`deny_unknown_fields`) is a line-numbered
  parse error; a `retention:` block with two policies plans two objects; an empty/absent block plans
  none (the `#[serde(default)]` path).
- **Multiple prefixes** — a block with two policies (`modbus.`, `bacnet.`) sets both; each is its own
  receipt object keyed by prefix.

## Risks & hard problems

- **Field-shape drift from `series.retention.set`.** The `RetentionPolicy`/`RetentionTier` manifest
  structs must match the verb's arg shape byte-for-byte, or a policy that validates in the verb fails
  to deserialize from the pack (or worse, silently drops `tiers`). Guard: the apply test asserts the
  round-trip `pack.yaml → set_policy → series.retention.list` equals the value a direct
  `series.retention.set` would store, and the structs are documented as the verb's mirror.
- **Prefix as the object id.** A policy's natural id is its `prefix` string, which can contain a dot
  (`modbus.`) — the receipt object id `retention:modbus.` must survive the receipt store's id
  handling (dotted ids are already load-bearing for series entities, but verify the receipt path).
- **Clobbering an operator-tuned policy.** LWW means a re-apply overwrites a hand-tuned horizon. This
  is the intended sidebar contract (loud clobber, listed) — but the risk is an operator not realizing
  the pack owns the prefix. Mitigate by documenting in the public doc that a pack-declared prefix is
  pack-owned; the loud receipt listing is the signal.
- **No enforcement without a GC tick.** Setting the policy alone changes nothing until GC runs. On a
  node with no scheduled GC, the seeded policy is inert until a manual `series.retention.gc`. This is
  the `job-retention-scope` boundary, but a deployment expecting the pack to "just make it fast" must
  also run GC — call this out in the public doc and the ems consumer note.

## Open questions

None — resolved here:

1. *Inline structs vs a file ref* → **inline** (the `channels:`/`insights:` model), because a policy
   is a small structured record and inline keeps it readable with no bundle plumbing.
2. *Object id* → **the policy `prefix`**, its natural stable key (like a channel's name), so a
   changed policy for a prefix re-applies in place and the receipt reads `retention:<prefix>`.
3. *New cap or reuse* → **reuse `mcp:series.retention.set:call`** via the per-object caps wall — the
   dispatch arm calls the same `set_policy` the public verb does, so no new grammar and the deny path
   is the existing one.
4. *Teardown on un-declare* → **out of scope** (no content-GC in the pack family yet); a policy the
   pack stops declaring is left in place, consistent with rules/dashboards.

## Related

- `scope/packs/pack-core-scope.md` — the closed-`Kind` four-step extension path this follows
  (§Workspace-seed kinds); `Kind::Sidebar` is the shipped sibling this mirrors.
- `scope/packs/pack-entity-binding-scope.md` — the run-once seed-ownership + receipt model the apply
  arm inherits.
- `scope/ingest/series-retention-scope.md` — defines `Policy`/`Tier` semantics + `set_policy` this
  block seeds; the enforcement (GC rollup-then-evict) the policy drives.
- `scope/jobs/job-retention-scope.md` — the scheduled-GC concern this is explicitly NOT (this sets
  the policy; that runs it).
- `scope/datasources/series-read-perf-scope.md` — the read-path perf the bounded raw tier protects
  (an unbounded raw tier is exactly what made bucket reads O(rows); this closes the config gap that
  left it unbounded).
- ems consumer: `NubeIO/ems → packs/ems/pack.yaml` gains the `modbus.` `retention:` block (shipped in
  the ems repo per `docs/WORKFLOW-LB.md`; the rubix-ai donor tree's `packs/ems/pack.yaml` carries it
  here).
- README `§6.10` (jobs/retention), `§6.1` (Data store — SurrealDB), pack-core §Caps (per-object
  authority wall).
