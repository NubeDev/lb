# Ingest scope — series statistics and a readable retention status

Status: **BUILT — `series.stats`, `series.retention.status` and `series.producer.health`**, the last
added 2026-07-27 and proven against a real published extension. Released in `node-v0.12.0`.

Retention ships a policy mechanism, a GC pass, and (since issue #65) a reactor that ticks it.
What it does **not** ship is any way to read back what happened. `run_gc` returns a pass
summary; `spawn_retention_reactors` logs it and drops it. `series.retention.list` returns the
*stored policy rows* but never says which row wins for a given series, and nothing anywhere
reports how many samples a series actually holds. The result is a subsystem whose entire
observable surface is `eprintln!` on the node's stdout — which means a downstream product
(rubix-ai) can prove retention works in a test but cannot show an operator that it is working
on their node. This scope adds the two read verbs that close that gap: **`series.stats`** (what
does this series actually hold) and **`series.retention.status`** (what governs it, and when did
GC last run).

This is the read-back half of [`series-sample-cap-scope.md`](series-sample-cap-scope.md). That
scope's own status line — *"testing INCOMPLETE — not shipped… nothing here is proven on a real
node"* — is the same problem stated from the inside: **we cannot easily prove it on a real node
because the node does not report anything.** Making the pass readable makes it assertable, which
is why this scope treats observability as part of the retention feature rather than a nicety.

## Goals

1. **`series.stats`** — for one series: sample count (raw vs rolled-up), first/last sample ts,
   and the set of producers writing to it.
2. **`series.retention.status`** — for one series or prefix: the **effective** policy after
   longest-prefix resolution (with the winning prefix named), plus the last GC pass for the
   workspace: when it ran, how many samples it evicted, how many it rolled up.
2b. **`series.producer.health`** (added 2026-07-27) — for one series: what each of its PRODUCERS
   reports about its own ingest, for the producers that are extensions and choose to say. The host
   answers "who writes this and when" from its own rows; only the producer knows that its link is
   reconnecting or that it has timed out eleven times running.
3. **Persist the pass the reactor already computes** — one upserted record per workspace, so
   "last run" survives a restart and is readable by a UI rather than being a log line.
4. **Resolve the winning policy server-side.** Longest-prefix-wins is host semantics; every
   client that needs the effective policy currently reimplements it (rubix-ai's modbus
   extension has its own `effectivePolicy()`). Two implementations of one rule will drift.

## Non-goals

- **No new retention semantics.** Resolution order, rollup behaviour, and eviction are exactly
  as they are. This scope is strictly additive and read-only.
- **No GC pass history.** Last pass only. A per-pass time series grows without bound, which
  would be an embarrassing shape for the feature whose job is bounding growth. Pass history, if
  ever wanted, is a telemetry/metrics concern.
- **No fleet-wide or all-series aggregate verb.** See Risks — that shape is a performance trap.
- **No UI.** The consuming panel is rubix-ai's
  (`docs/scope/ingest/ingest-observability-scope.md` there).

## Intent / approach

**Two verbs, split along their capability line.** `series.stats` is a data-plane read about
samples; `series.retention.status` is an admin-plane read about policy and GC bookkeeping. A
single combined verb was rejected: it would force one capability to cover both, and it would
prevent a client from degrading per-fact (a caller granted sample reads but not retention
admin should still see counts and freshness).

**`series.stats` is single-subject, by design and by necessity.** It takes one series id and
returns one result. It deliberately offers no "all series" mode: a `count()` per series behind
the store is precisely the pattern that produced the node-wide serialization stall fixed in
`node-v0.11.0`, and a verb that invites a caller to fan it out over a 10k-series workspace is
a regression waiting to be written by a well-meaning UI. Callers that want per-row counts must
come back with a batched verb and a perf argument.

**The GC record is one upserted row per workspace**, written by `run_gc` (not by the reactor)
so that the on-demand `series.retention.gc` verb and the reactor both record their pass through
one path — otherwise a manual GC would silently leave the status stale, and the status would
lie. It lives under the store's reserved namespace alongside the other host-owned bookkeeping.

**An idle pass still stamps the time.** A pass that evicts nothing must update `last_run_ms`.
If it did not, a healthy idle node would show a frozen "last run" and read as a dead reactor —
converting the observability feature into a false alarm generator. This is called out here
because it is the one behaviour that is easy to implement backwards.

## How it fits

- **Workspace is the hard wall.** Both verbs are ws-scoped like every `series.*` verb. The GC
  record is written per-ws; reading it in ws B can never surface ws A's pass. Given the store's
  per-query `USE NS` scoping (node-v0.11.0), the record read goes through the same scoped path
  as every other store op — no new isolation mechanism, and a ws-isolation test is mandatory.
- **Capability-first — name the deny.**
  - `series.stats` → `mcp:series.stats:call`, granted with `series.read` (data-console tier).
  - `series.retention.status` → `mcp:series.retention.status:call`, granted alongside
    `series.retention.list` in `builtin_roles.rs`.
  Both refuse cleanly with the standard capability error. **The refusal must be distinguishable
  from an empty result** — a caller denied the verb and a series with no samples must not look
  alike, or a downstream UI cannot tell "not permitted" from "nothing here", which is the exact
  ambiguity this scope exists to remove.
- **Symmetric nodes.** The record is written by whichever node runs the retention reactor
  (`BootConfig::reactors`) — config, never a code branch. A node that runs no reactors serves
  the verb and reports "no pass recorded on this node", which is honest and correct.
- **Rule 10.** Both verbs are generic over series ids; the core learns nothing about any
  extension. A series written by modbus, a flow, a webhook, or a hand-write is identical here.
- **No mocks.** Tests boot the real `mem://` store, write real samples, run the real `run_gc`,
  and assert the real record and the real counts.
- **One responsibility per file.** New: `crates/ingest/src/stats.rs` (the count/extent query),
  `crates/ingest/src/pass_record.rs` (write/read the last-pass row), and the two host verb
  handlers under `crates/host/src/ingest/`. `run_gc` gains a call to the record writer; the
  reactor loses nothing (it may keep logging).
- **The right API shape.** Both are **get** verbs — single subject, read-only, no pagination.

## Example flow

1. An admin's UI opens a series and calls `series.retention.status { series: "modbus.plant-a.chiller-1.current-l1" }`.
2. The host resolves the stored policy rows by longest prefix: rows exist at `modbus.` and
   (if an operator tuned it) `modbus.plant-a.`. The longer wins.
3. It returns the effective policy **and** `matched_prefix: "modbus."`, so the caller can show
   *inherited from `modbus.`* rather than implying the series has its own row — the distinction
   that makes longest-prefix-wins legible instead of folklore.
4. It attaches the workspace's last GC pass: `{ last_run_ms, evicted, rolled_up }`.
5. The UI calls `series.stats` for the same series and shows raw vs rolled-up counts and the
   last sample age.
6. The operator sees a sawtooth explained — "evicted 132 raw, rolled up 150, 4 minutes ago" —
   instead of inferring it by polling row counts from a shell.

## Testing plan

Mandatory categories:

- **Capability-deny** — each verb refused without its grant; assert the refusal is a capability
  error and **not** an empty success. Both directions (stats granted / retention denied, and
  the reverse) so the per-fact degrade is proven, not assumed.
- **Workspace-isolation** — two seeded workspaces; A's series never appear in B's stats, and
  B's retention status reports B's pass. Assert on a real store, both ws populated.
- **Offline/sync** — N/A (read-only, no outbox).
- **Hot-reload** — N/A.

Key cases:

- Stats over a seeded series: counts, first/last ts, producer set (multi-producer case
  included — a series written by two producers must list both).
- Stats for an unknown/empty series → a valid zero result, never an error.
- Effective-policy resolution: rows at `a.` and `a.b.` → `a.b.c` resolves to `a.b.`, and
  `matched_prefix` says so. No matching row → an explicit "no policy" result, not a fabricated
  default.
- `run_gc` writes the pass record; a second pass **overwrites** it (last-pass-only, asserted).
- **An idle pass (nothing evicted) still updates `last_run_ms`** — revert-check this: making
  the write conditional on `evicted > 0` must turn the test red.
- The on-demand `series.retention.gc` verb updates the same record as the reactor (one path).

## Risks & hard problems

- **Fan-out is the danger.** `series.stats` is cheap for one series and ruinous for ten
  thousand. The verb's single-subject shape is the guard; the doc comment must say why, or a
  future caller will add a `series: []` array and reintroduce the node-wide stall that
  `node-v0.11.0` fixed. Consider asserting the absence of an all-series mode in review.
- ~~**Rolled-up vs raw must be distinguishable in the store** without consulting the policy.~~
  **RESOLVED — they are separate tables** (`series` / `series_rollup`), so the split is
  structural and neither a rollup marker nor a policy lookup is needed. See Decisions 1–2; the
  residual subtlety is per-tier double-counting, which the `tiers` breakdown handles.
- **This scope's parent is unproven.** `series-sample-cap-scope.md` reports incomplete testing
  on a real node. There is a real chance that building the read-back surfaces a defect in the
  retention path itself. That is a *feature* of doing this work — but plan for the possibility
  that release 1 of this scope ends with a bug fix in the parent rather than a clean addition.

## Decisions

Every question this scope opened is resolved below, with the reasoning, as built.

1. **Rollup rows ARE distinguishable from raw rows — they are in two different tables.** Raw
   samples live in `series`, rollups in `series_rollup` (`schema.rs`), so the split needs no
   marker field and no policy lookup. `series.stats` therefore reports raw vs rolled-up in
   release 1 as originally hoped, and the "drop the split" fallback in §Risks is not needed.

2. **Rollup counts are reported PER TIER, not as one total of folded samples.** A rollup row
   exists once per `(series, width_ms, t)` — i.e. once per tier — so the same history is stored
   at every resolution the policy declares. A single "rolled-up samples" number would silently
   double-count a two-tier policy, which is precisely the kind of fabricated-looking figure this
   scope exists to avoid. `stats` returns `rollup_rows` (honest total of STORED ROWS) plus a
   `tiers: [{ width_ms, rows }]` breakdown, ascending by width. The doc comment says so.

3. **`series.retention.status` takes ONE `series` argument that may be a series id OR a bare
   prefix.** No second verb and no mode flag: longest-prefix resolution is the identical
   operation either way (a prefix is just a subject that ends at a boundary), so a settings page
   asking "what governs `modbus.`" and a detail page asking "what governs this series" share one
   code path. `matched_prefix` is echoed in both cases. Splitting them would mint a second
   resolution site — the exact drift goal 4 exists to eliminate.

4. **A missing pass record needs no migration — absent is a valid state.** `last_pass` returns
   `None` and the caller renders "no pass recorded on this node". A node that runs no retention
   reactors (`BootConfig::reactors` off) reports that forever, which is honest rather than an
   error, and keeps the feature symmetric-node-clean.

5. **The pass record includes `duration_ms`.** Measured with a monotonic `Instant` inside
   `run_gc`, so it is real elapsed time and not a function of the caller-injected `now_ms`
   (which stays the logical clock, per determinism §3). It is the earliest signal that GC is
   becoming expensive on a deep workspace — a pass creeping toward its own 300 s period is a
   backlog about to happen.

6. **`GcPassRecord` is its own type, not a re-used `GcPass`.** `GcPass` is `Serialize`-only and
   is the in-process return value; the record adds `last_run_ms`/`duration_ms` and must
   round-trip, so it derives `Deserialize` too. Building the record from the pass
   (`GcPassRecord::new`) keeps one conversion site.

7. **Stored warnings are clipped at `MAX_STORED_WARNINGS` (20), with `warnings_total` carrying
   the true count.** The row is rewritten every reactor tick; one warning per unpoliced series
   would make a hot, unboundedly-wide row on a deep workspace — the same unbounded-growth shape
   this feature is supposed to be embarrassed by. Clipping without `warnings_total` would have
   been dishonest, so both ship.

8. **The record uses the ingest crate's raw `query_ws` + `CONTENT` idiom**, not `lb_store::write`'s
   `{data, rev}` envelope. Every neighbouring series-plane table (`series`, `series_rollup`,
   `series_retention`) is written that way; matching them keeps one read shape across the plane.
   The `rev` the envelope would add has no meaning for a last-write-wins status row.

9. **`series_gc_pass` is registered in BOTH reserved lists** — `lb_store::RESERVED_TABLES` and
   `lb_packs::RESERVED_CORE_TABLES`. It is host-owned bookkeeping, so a pack must never be able
   to write it. (Note in passing: `series_latest` is in neither and is a pre-existing drift — not
   fixed here, but worth an issue.)

10. **First/last sample ts are read as two `ORDER BY ts … LIMIT 1` queries over the `(series, ts)`
    index, not `math::min`/`math::max` aggregates.** The aggregate form needs a subquery-collect
    to be correct against this store and reads worse for no gain; the index already serves the
    ordered limit at both ends. The order key is in the projection because the engine only orders
    by what is selected (`cap.rs` pins the same rule).

11. **`series.retention.status` also returns `default_max_samples`** (`DEFAULT_MAX_SAMPLES`). The
    downstream panel's copy for an ungoverned series says samples are "kept under the host's
    default cap"; without the number that sentence is hand-waving. This makes it nameable. It is
    advisory in this release — unpoliced series are warned about, not evicted.

12. **`mcp:series.stats:call` is also added to the `apikey-read` bundle.** It is the same
    data-plane read tier as `series.read`/`latest`/`find`/`list`, which that bundle already
    carries, so a polling appliance can see its own retention sawtooth without an admin cap.
    `series.retention.status` is deliberately NOT added there — it is admin-plane.

13. **No all-series or array mode was added, and the doc comment on `stats.rs` says why** — a
    `count()` per series behind the store is the shape that produced the `node-v0.11.0` stall.
    The module doc names the fan-out caller it exists to refuse (a UI wanting per-row counts in
    the series library) and points that need at a separate batched verb with its own perf scope.

## Related

- [`series-sample-cap-scope.md`](series-sample-cap-scope.md) — the cap, the GC driver, issue
  #65. This scope is its read-back half.
- [`series-retention-scope.md`](series-retention-scope.md) — the time-based horizon and the
  policy model (issue #58) whose resolution this scope exposes.
- Downstream consumer: `NubeIO/rubix-ai` → `docs/scope/ingest/ingest-observability-scope.md`
  (the Ingest health panel) — the product-side reason this is being asked for.


## Decisions — `series.producer.health` (added 2026-07-27)

14. **Discovery is a TOOL-NAME CONVENTION over the live registry, not `ext.list`.** The downstream
    scope had recorded that `ext.list` "returns `tools: Vec<String>` per installed extension". It
    does not: `ExtRow` (`host/src/ext/row.rs`) has no tool list, and the manifest's `tools` is never
    persisted onto `Install`. The real seam is `node.registry.descriptor_entries()` — the same walk
    `agent::exfil::tainted_tools` uses for `emits_external` — keeping extensions that declare a
    descriptor named `ingest.health`. The registry is also the honest source: it lists what is
    dispatchable *now*, so a declared-but-unloaded extension reads "not reported" rather than
    erroring on a call that cannot land. **No SDK and no manifest mechanism is involved** — an
    extension contributes by declaring one ordinary tool. That was verified, not assumed, before the
    design was committed to.

15. **Producer → extension id is recovered from the IDENTITY GRAMMAR, never from a name.**
    `ingest.write` roots every producer at `{principal.sub()}/{declared}` with at most one separator
    (`root_producer` collapses a declared `/` to `-`, so the depth cannot be forged), and an
    extension's sub is `ext:{id}`. `producer_ext_id` therefore recovers the id by shape;
    `user:ada/gw-alpha` yields `None`, which is a first-class answer. The reader lives in
    `ingest/write.rs` beside the writer ON PURPOSE — these two are one grammar, and split across
    modules a writer and a reader eventually disagree. The same file's tests assert the reader
    inverts the writer for every shape the writer can emit.

16. **The host models three fields and refuses to model a fourth.** `state`, `last_write_ms`,
    `last_accepted` are true of anything that writes samples. Domain facts (timeout runs, poll
    duration, device counts) ride an open `details: [{label, value}]` list carried through verbatim.
    A host field called `consecutive_timeouts` would have encoded "a producer is a polling device"
    into the core — subtler than an `if ext == "modbus"` and just as much a rule-10 leak, because a
    webhook or a flow has no ticks.

17. **The fan-out runs under the CALLER's principal, at depth+1.** `mcp:series.producer.health:call`
    is a data-plane grant beside `series.stats`; it deliberately grants NO reach into an extension
    tool the caller could not already call. Each row re-checks `mcp:{ext}.ingest.health:call`, so a
    forbidden extension reads `denied` (naming the missing cap) while a permitted one beside it
    reports — asserted in both directions, because a single-direction deny test passes just as
    happily against a gate wired to the wrong capability.

18. **Four ways of not-knowing are kept apart, by name.** `not-an-extension` / `not-reported` /
    `denied` / `error` are distinct states, and one producer's failure never fails the read. The
    whole point of the feature is that a refusal must not look like silence and silence must not
    look healthy; a test that merely checked "not Reported" would pass against a verb that collapsed
    them, so each is asserted individually.

19. **The extension is handed its LEAF, not the rooted producer.** It never saw the `ext:<id>/` root
    the host stamped on, and an extension feeding many streams needs to know which one is being
    asked about — otherwise it must guess or answer for all of them.
