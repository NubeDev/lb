# Store scope — a node disk budget (bound bytes, not just rows)

Status: scope (the ask), issue [#122](https://github.com/NubeDev/lb/issues/122). Promotes to
`doc-site/content/public/store/` once shipped.

Everything that bounds growth on this platform bounds **rows**. Series retention evicts on a
time horizon and a FIFO `max_samples`; staging is capped at 100k; jobs, flow runs, entity
versions, insight occurrences and the telemetry ring all ride the shared `capped_insert`
primitive. Not one of them knows how many **bytes** are on disk, and nothing anywhere compares
the store's footprint against the space the machine actually has. The single byte-level signal
in the tree is `LOG_ADVISORY_BYTES` — a hardcoded 256 MiB constant in
`crates/host/src/store_admin/status.rs:18` that logs a warning and does nothing else. So an
operator deploying a node to a Pi with an 8 GB card has no way to say "never exceed 4 GB", and
the first symptom of getting it wrong is a full filesystem, which on an append-only engine is
also the hardest state to recover from. This scope makes the budget **configurable, measured,
and enforced**: one `LB_STORE_MAX_BYTES` allowance, a reactor that acts on it instead of
merely warning, and defaults that are bounded rather than infinite.

## Goals

- **An operator can state the allowance.** `LB_STORE_MAX_BYTES` (via `BootConfig`) sets the
  node's disk budget; the existing advisory threshold derives from it instead of a constant.
- **Crossing the budget does something — or says why it can't.** At the soft mark the node
  reclaims what it already knows how to reclaim (compaction), and when reclamation stops paying
  it stops pausing writes and says the allowance is too small. Never an hourly write outage for
  no reclaimed bytes.
- **Reclamation respects the engine.** On an append-only store a delete *adds* bytes until the
  next compaction. Every reclamation path is ordered so eviction is followed by a compaction
  that can actually run.
- **Bounded by default.** A node with no retention policy configured must not keep series
  forever. `DEFAULT_MAX_SAMPLES` stops being advisory (the "release 2" already named in
  `series-sample-cap-scope.md`), and `ingest_dead_letter` gains the retention it never had.
- **Observable before painful.** `store.status` reports budget, usage, headroom and free disk —
  the operator sees the trend, not just the cliff.
- **Symmetric.** The budget and its cadence are config, never a code branch (rule 1). An
  embedder fills the field; only the binary reads env.

## Non-goals

- **Refusing writes at a hard ceiling.** A write-refusal wall needs a defined failure contract
  for every producer (ingest, jobs, the outbox) and is its own scope — see decision 3.
  This scope reclaims and warns; it does not yet say no.
- **Per-workspace quotas.** The budget is node-scoped. Fair-sharing a node's disk between
  tenants is a real ask and a different one; nothing here forecloses it.
- **Changing the engine or the compaction mechanism.** SurrealKV only (rule 2);
  `online-compaction-scope.md` owns *how* a pass runs, this scope owns *when*.
- **Retention/rollup semantics.** Shipped. This scope changes defaults and adds a driver; it
  does not change what eviction means.
- **Log-file rotation.** Logs go to stderr (`crates/telemetry/src/config.rs`); rotation belongs
  to systemd/docker, not the node.

## Intent / approach

Three slices, each independently shippable, in strict order — the value is front-loaded and
the risk is back-loaded.

**Slice 1 — the budget is a number the operator picks.** Add
`BootConfig::store_budget_bytes: Option<u64>`, parsed from `LB_STORE_MAX_BYTES` in
`node/src/config.rs::from_env` following the `LB_MAX_EXTENSION_UPLOAD_BYTES` pattern exactly
(warn-and-fall-back on a malformed value, never panic in boot config). Thread it into
`store_admin::status` so `LOG_ADVISORY_BYTES` becomes the *default* when unset rather than the
only possible value. Extend `StoreStatusReport` with `budget_bytes`, `free_disk_bytes` and a
headroom figure. `None` ⇒ today's exact behaviour, so this slice is pure addition.

**Slice 2 — the reactor acts.** `store_admin/reactor.rs` currently warns past the threshold and
stops there (`reactor.rs:38`); auto-trigger was explicitly deferred in
`online-compaction-scope.md` OQ5 pending **field measurement of the pause cost**. That
measurement still does not exist — the only figure on record is `duration_ms: 22` on a 58 KB
test log (`sessions/store/online-compaction-session.md:209`), which says nothing about a pass at
budget scale. **So slice 2 opens with that measurement, and the deferral stands until it lands.**
If the pause at multi-GB scale is worse than the disk pressure it relieves, the correct outcome
is to ship slices 1 and 3 and leave the trigger manual — that is a success, not a failure.

Given a measurement that permits it: at a **soft mark (default 80% of budget)** the reactor
enqueues the existing `store.compact` job — the same job an operator triggers today, no new
mechanism — with a minimum interval between passes. Given the measured 26–65× commit-log bloat,
compaction alone should resolve the large majority of budget crossings.

The minimum interval alone is **not** a sufficient guard. A store whose *live set* exceeds the
soft mark re-crosses it after every pass and re-enqueues on the next eligible tick — a recurring
write outage (each pass quiesces all writes) with zero reclaimed bytes. The driver therefore
needs a **convergence condition**: when a pass returns `after_bytes > 0.9 × before_bytes`,
compaction is not the problem. Stop auto-enqueueing and log the "budget too small for this
workload" line **at the soft mark** — do not wait for the hard mark to say the useful thing.
Auto-enqueueing resumes on its own once a pass is productive again.

**Slice 3 — bounded defaults.** Two changes with real payoff, and one deliberate omission.

Flip `DEFAULT_MAX_SAMPLES` from advisory to enforced for series with no explicit policy
(`cap.rs:196` currently only warns), and give `ingest_dead_letter` a retention pass in the
existing GC. These are the whole value of the slice: they convert "unbounded by default" into
"bounded by default", which is the difference between a node that can blow any budget and one
that cannot.

**The enforcement path must distinguish "no policy record" from "a policy record saying
`max_samples: 0`".** Today `over_cap_warning` uses `max_samples == 0` to mean *unpolicied*
(`cap.rs:196`), but the shipped warning already promises operators that `max_samples: 0` is the
explicit **opt-out** from the coming default. Both readings cannot survive the flip. Resolution:
policy-record **existence** decides. No record ⇒ `DEFAULT_MAX_SAMPLES` applies; a record with
`max_samples: 0` ⇒ genuinely unbounded, honoured as written.

**Omitted deliberately: an early-execution tightening pass at the hard mark.** An earlier draft
had the 95% mark run configured retention policies ahead of their tick. Retention already ticks
every 300s, so it buys at most five minutes on a disk that fills over days — near-zero payoff for
real machinery. Worse, on an append-only engine it is actively harmful (see below). The hard mark
therefore does exactly two things: trigger a compaction, and log loudly.

**The append-only ordering rule (applies to every reclamation path).** On SurrealKV a delete is a
**tombstone appended to the log**. Eviction does not free bytes — it *adds* them, and the space
is only recovered by a subsequent compaction. This is the same property that makes row-count
estimation useless above, and it inverts the naive remedy: deleting rows when the node is nearly
out of budget makes the immediate situation worse. So **any pass that evicts must be followed
immediately by a compaction that is exempt from the minimum interval.** Without that exemption
the failure sequence is real: 80% → compact → still growing → 95% → evict (log grows) →
compaction blocked by the one-hour interval → budget blown. The exemption is what makes the hard
mark safe, and it is why the hard mark does not evict at all in the shape above — the routine
retention tick already does that, and its tombstones are cleaned up by the next scheduled pass.

**Alternative rejected: a filesystem-level quota** (a dedicated partition, or a cgroup/ZFS
dataset limit). It genuinely bounds bytes and costs us no code — but the node learns about the
limit as `ENOSPC` in the middle of a write, which on an append-only log is precisely the
scenario with the worst recovery story. An in-process budget can act at 80% while it still has
room to compact. Operators should do both; only one of them is ours to build.

**Alternative also rejected: estimating bytes from row counts.** Cheap, and wrong by the same
26–65× the bloat measurement found. The budget must read the actual directory size.

## How it fits the core

- **Tenancy / isolation:** the budget is node-scoped and operates below the namespace wall — it
  stats a directory and enqueues an existing job; it never reads a record as any principal. The
  tightening pass in slice 3 runs per-workspace through the existing GC path, which is already
  workspace-scoped. The proof is the isolation suites passing unmodified.
- **Capabilities:** no new verbs. `store.status` stays `store:status:read`, `store.compact`
  stays admin-gated `store:compact:run`. The reactor mints no principal — node maintenance, the
  same posture as `spawn_retention_reactors` and today's store-admin reactor.
- **Placement:** either. Budget, marks and cadence are `BootConfig`; whether the reactor runs at
  all is the existing `BootConfig::reactors` (rule 1). An edge node on an SD card and a cloud
  node on a volume differ only in the number.
- **MCP surface (§6.1):** *get* — `store.status` gains fields; no new tool. *CRUD* — N/A, the
  budget is boot config, not a record (an operator who could raise the ceiling via an MCP call
  could evade it). *Live feed* — N/A, disk fills slowly; poll status. *Batch* — reuses the
  existing `store.compact` job.
- **Data (SurrealDB):** no schema change. Slice 3's dead-letter retention reuses the existing
  `series_retention` machinery rather than adding a table.
- **Bus (Zenoh):** none. The advisory is telemetry (tracing); nothing here is must-deliver.
- **Sync / authority:** node-local by definition — disk belongs to one node.
- **Secrets:** N/A.
- **SDK/WIT impact:** none. `BootConfig` is `#[non_exhaustive]`, so the new field is additive
  for every embedder.
- **Skill doc:** yes — the implementing session extends `skills/store-compact/SKILL.md` with the
  budget flow (set the allowance → read headroom → observe an auto-pass), grounded in a live
  run. A new skill is not warranted; this is the same drivable surface.

## Example flow

1. An operator deploys to a Pi with an 8 GB card and sets `LB_STORE_MAX_BYTES=4294967296`.
2. Ingest runs at 1s cadence for days. Retention keeps rows bounded; the commit log grows with
   superseded versions and tombstones.
3. `store.status` reports `log_bytes`, `budget_bytes`, headroom and free disk. The trend is
   visible days before it matters.
4. The log crosses the 80% soft mark. The reactor enqueues `store.compact`; the pass rewrites
   the log down to the live set; headroom returns to normal. The operator finds a log line and a
   completed job, not an outage.
5. Meanwhile the routine 300s retention tick evicts stale rows. Each eviction *appends a
   tombstone* — the log grows slightly. Those bytes come back at the next compaction, which is
   the only thing that ever frees space on this engine.
6. On a node where compaction stops reclaiming — a pass returns `after_bytes > 0.9 ×
   before_bytes`, meaning the live set itself is the budget — the driver stops auto-enqueueing
   and logs "budget too small for this workload" **at the soft mark**. No hourly write pause for
   no benefit. The allowance is genuinely too small, which is information, not a failure to act.
7. If the hard mark is crossed anyway, it triggers one compaction **exempt from the minimum
   interval** and logs loudly. It does not evict — on an append-only engine that would add bytes
   at the worst possible moment.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`):
- **Workspace isolation:** the full isolation suites pass **unmodified**. Slice 3 touches the
  per-workspace GC path, so this is the gate, not a formality.
- **Capability deny:** unchanged verbs, but re-assert — no `store:status:read` ⇒ no budget read;
  no `store:compact:run` ⇒ no manual trigger. The reactor's own pass is node maintenance and
  bypasses no wall it should not.
- **Hot-reload:** extensions survive a budget-triggered pass exactly as they survive an
  operator-triggered one (they hold `Arc<Store>`).

Key cases (real store, real bytes, real ingest path — no mocks, rule 9):
- *Env parse:* unset ⇒ today's 256 MiB advisory and no auto-trigger; set ⇒ marks derive from it;
  malformed ⇒ warn + fall back, never panic.
- *Soft mark:* seed through the real ingest path until the log crosses 80%; assert exactly one
  `store.compact` job is enqueued, it completes, and `after_bytes` ≪ `before_bytes`.
- *No thrash:* a store that stays over budget after a pass does not enqueue on every tick —
  pin the minimum interval.
- *Convergence:* a store whose **live set** exceeds the soft mark compacts **once**, sees
  `after_bytes > 0.9 × before_bytes`, and then stops auto-enqueueing while logging the
  budget-too-small line. Assert no second job over many ticks — this is the write-outage-forever
  regression, and it is the test most likely to be omitted.
- *Quiet store:* below the soft mark ⇒ no pass, no warning, no job. Prove the driver does not
  tick on an idle node (the `dev-node-cpu-job-scan` lesson).
- *Bounded default:* a series with no policy record stops at `DEFAULT_MAX_SAMPLES` instead of
  growing forever. **State that this test fails with the change reverted.**
- *Opt-out is distinguishable:* a series with a policy record of `max_samples: 0` grows past
  `DEFAULT_MAX_SAMPLES` untouched, while a series with **no record** is capped. Both cases in
  one test — this is the ambiguity the flip introduces.
- *Dead-letter retention:* an `ingest_dead_letter` table seeded past the horizon is pruned;
  entries inside it survive.
- *Eviction grows the log:* assert directly that a retention pass **increases** `log_bytes` and
  that a following compaction reclaims it. This pins the append-only property the ordering rule
  depends on, so a future change cannot quietly invalidate it.
- *Hard-mark exemption:* a hard-mark crossing inside the minimum interval still compacts —
  prove the exemption fires and the interval does not block it.
- *Pause measurement (slice 2 gate, not a pass/fail test):* record `duration_ms` for a pass on a
  log at budget scale (GB, not the 58 KB of the existing session log) and write the number into
  the session doc. Slice 2's auto-trigger is approved by that number or it is not approved.

## Risks & hard problems

- **Auto-compaction pauses writes.** This is the deferral being reversed, so it deserves the
  scrutiny. A pass takes the session mutex and quiesces writes; on a large log that pause is
  visible. Threshold-driven (not clock-driven) plus a minimum interval keeps it rare, but the
  implementing session must measure the pause at budget scale and document it. If the pause is
  worse than the disk pressure, ship slice 1 and 3 and leave the trigger manual — say so
  plainly.
- **Bounded-by-default is a behaviour change on live nodes.** A node relying on the current
  keep-forever default will start evicting after an upgrade. This needs a loud release note and
  an explicit opt-out (`max_samples = 0` already means unbounded — the escape hatch exists, it
  just has to be documented rather than discovered).
- **Compaction needs free space to run.** Rewriting a log requires room for the new one. A
  budget set close to the physical disk can put the node in a state where the remedy will not
  fit. The soft mark must leave headroom for a pass, and the status report must surface free
  disk — not just budget usage — so this is visible.
- **The budget measures the store, not the node.** `log_bytes` sums `clog/*.clog`
  (`store/src/status.rs:41`), which measurement shows is 99.9% of the store directory (decision
  4) — so the number is honest *about the store*. It does not cover extension artifacts, native
  sidecar binaries or OS logs sharing the filesystem. A 4 GB budget therefore bounds the store,
  not the partition, and `free_disk_bytes` in `store.status` is what keeps that distinction
  visible. Say it plainly in the operator-facing docs; a promise that is quietly broader than
  the truth is worse than a narrow one stated clearly.

- **A node-global job in a per-workspace queue.** `store.compact` compacts the whole node
  (`compact(&node.store)`), but `lb_jobs` records are per-workspace and `drain_compact_jobs`
  drains per-workspace (`reactor.rs:52`). One budget crossing must produce exactly one job in
  `BootConfig::workspace` (decision 8) — a per-workspace fan-out would quiesce every write on the
  node N times for one crossing, and that is a bug, not a design option.

## Decisions

Every question this scope raised is answered here. Build to these; do not re-litigate them
mid-implementation. If one turns out to be wrong, change it deliberately and record why in the
session doc — but start from a decision, not a question.

1. **Marks are a percentage of budget, and inert when the budget is unset.** Soft mark 80%,
   hard mark 95%, both as percentages of `store_budget_bytes`. When the budget is `None` there
   are no marks at all — the node keeps today's flat 256 MiB advisory warning and never
   auto-triggers anything. Percent reads naturally against an operator-chosen allowance, and
   tying the marks to the budget's existence is what makes slice 1 purely additive.

2. **The default budget when unset is none.** No auto-derivation from filesystem size. On a
   shared volume "80% of the disk" is a number the operator never chose and cannot predict, and
   silently acquiring a write-pausing behaviour on upgrade is exactly the surprise this scope
   exists to prevent. Unset means today's behaviour, forever.

3. **A hard ceiling never refuses writes — not in this scope, and not as a follow-up until
   field data demands it.** Refusal needs a failure contract for every producer (ingest, jobs,
   outbox) and turns a disk problem into a data-loss problem. This scope reclaims and reports.
   Revisit only if production shows reclamation is genuinely insufficient.

4. **`log_bytes` is the right measure, and the budget's promise is honest — measured.** On two
   real store directories the `clog` tree is **99.9%** of the total: 803,878 of 804,636 bytes,
   and 30,485,397 of 30,485,867 bytes. The only sibling is a sub-KB `manifest`. So no widening
   of the stat is needed. Do add `manifest` to the sum for exactness (it is one cheap stat), and
   state plainly in `store.status` docs that the budget covers the **store directory**, not
   extension artifacts, sidecar binaries or OS logs — that is what `free_disk_bytes` is for.

5. **The minimum interval between auto-passes is one hour, and the hard mark is exempt.** The
   exemption is not a tuning knob: it is what makes eviction safe on an append-only engine (see
   the ordering rule in slice 3). One hour is the starting value; the pause measurement in
   slice 2 may justify changing it, and changing it is a config edit, not a redesign.

6. **The productive-reclaim threshold is `after_bytes > 0.9 × before_bytes` ⇒ stop
   auto-enqueueing.** Ship that number. It only has to separate "compaction reclaimed
   essentially nothing" from the measured 26–65× bloat case, and any value in 0.85–0.95 does
   that. The principle — stop pausing writes when passes stop paying — is fixed; the constant is
   a named `const` so tuning it later is a one-line change with a test.

7. **The dead-letter table gets its own horizon, defaulting to 30 days.** Dead letters are
   diagnostic: they are worth keeping longer than the data that produced them, and they are
   small. A separate horizon also means tightening series retention never silently destroys the
   evidence needed to debug why records were dead-lettered.

8. **Reactor-minted compact jobs land in one deterministic workspace: the node's configured
   default (`BootConfig::workspace`).** Never a fan-out — one crossing, one job, because the
   pass is node-global and each one quiesces every write on the node. `requested_by` is the
   literal string `"system:store-budget"`, distinct from any real principal, so an operator
   reading the job record sees immediately that the budget driver triggered the pause rather
   than a person. If the default workspace is somehow absent, log and skip rather than guessing.

9. **`max_samples` semantics: policy-record existence decides.** No record ⇒ `DEFAULT_MAX_SAMPLES`
   applies. A record with `max_samples: 0` ⇒ genuinely unbounded, honoured as written. This is
   the promise the shipped `over_cap_warning` text already made to operators (`cap.rs:196`), so
   it is the reading that must survive the flip.

10. **Slice 3 ships no early-execution tightening pass.** The routine 300s retention tick already
    does the eviction; running it early buys at most five minutes on a disk that fills over days,
    and on an append-only engine it adds bytes at the worst moment. The hard mark compacts and
    logs. This is a deliberate deletion from an earlier draft, not an oversight.

## Related

- `scope/store/online-compaction-scope.md` — the pass this scope drives; OQ5 there is the
  deferral slice 2 reverses. Read it first.
- `scope/store/persistent-backend-scope.md` — the SurrealKV posture (no native max-size option,
  which is why this is an application-layer budget).
- `scope/store/session-concurrency-scope.md` — the mutex an auto-triggered pass takes.
- `scope/ingest/series-retention-scope.md`, `scope/ingest/series-sample-cap-scope.md` — the
  row-bounding half; slice 3 ships the "release 2" bounded-by-default flip named there.
- `scope/ingest/drain-backpressure-scope.md` — "a request pays for its own work, never the
  backlog"; a budget-triggered pass pays for the backlog explicitly and observably.
- `crates/host/src/store_admin/status.rs` — `LOG_ADVISORY_BYTES`, the constant this replaces.
- `crates/host/src/store_admin/reactor.rs` — the warn-only driver slice 2 gives teeth.
- `node/src/config.rs` — `BootConfig::from_env`, the one place `LB_*` is read.
