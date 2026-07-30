# Store scope — a node disk budget (bound bytes, not just rows)

Status: scope (the ask), issue [#122](https://github.com/NubeDev/lb/issues/122). Promotes to
`doc-site/content/public/store/` once shipped.

Everything that bounds growth on this platform bounds **rows**. Series retention evicts on a
time horizon and a FIFO `max_samples`; staging is capped at 100k; jobs, flow runs, entity
versions, insight occurrences and the telemetry ring all ride the shared `capped_insert`
primitive. Not one of them knows how many **bytes** are on disk, and nothing anywhere compares
the store's footprint against the space the machine actually has. The single byte-level signal
in the tree is `LOG_ADVISORY_BYTES` — a hardcoded 256 MiB constant in
`crates/host/src/store_admin/status.rs:17` that logs a warning and does nothing else. So an
operator deploying a node to a Pi with an 8 GB card has no way to say "never exceed 4 GB", and
the first symptom of getting it wrong is a full filesystem, which on an append-only engine is
also the hardest state to recover from. This scope makes the budget **configurable, measured,
and enforced**: one `LB_STORE_MAX_BYTES` allowance, a reactor that acts on it instead of
merely warning, and defaults that are bounded rather than infinite.

## Goals

- **An operator can state the allowance.** `LB_STORE_MAX_BYTES` (via `BootConfig`) sets the
  node's disk budget; the existing advisory threshold derives from it instead of a constant.
- **Crossing the budget does something.** At the soft mark the node reclaims what it already
  knows how to reclaim — auto-compaction, then retention tightening — rather than logging into
  the void.
- **Bounded by default.** A node with no retention policy configured must not keep series
  forever. `DEFAULT_MAX_SAMPLES` stops being advisory (the "release 2" already named in
  `series-sample-cap-scope.md`), and `ingest_dead_letter` gains the retention it never had.
- **Observable before painful.** `store.status` reports budget, usage, headroom and free disk —
  the operator sees the trend, not just the cliff.
- **Symmetric.** The budget and its cadence are config, never a code branch (rule 1). An
  embedder fills the field; only the binary reads env.

## Non-goals

- **Refusing writes at a hard ceiling.** A write-refusal wall needs a defined failure contract
  for every producer (ingest, jobs, the outbox) and is its own scope — see "Open questions".
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
stops there; auto-trigger was explicitly deferred in `online-compaction-scope.md` OQ5 pending
field measurement of the pause cost. That measurement now exists, and the deferral has outlived
its reason. At a **soft mark (default 80% of budget)** the reactor enqueues the existing
`store.compact` job — the same job an operator triggers today, no new mechanism, with a
minimum interval between passes so a store that is genuinely large does not compact in a loop.
Given the measured 26–65× commit-log bloat, compaction alone is expected to resolve the large
majority of budget crossings.

**Slice 3 — bounded defaults.** Flip `DEFAULT_MAX_SAMPLES` from advisory to enforced for series
with no explicit policy (`cap.rs:196` currently only warns), and give `ingest_dead_letter` a
retention pass in the existing GC. Then, at a **hard mark (default 95%)** where compaction was
not enough, run one bounded tightening pass: the most aggressive *already-configured* retention
policies execute early rather than waiting for their 300s tick. It never invents a policy the
operator did not write, and it never deletes data no policy would eventually have deleted — it
only stops waiting.

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
5. On a node where compaction is not enough, the 95% hard mark fires one tightening pass:
   configured retention policies run early. If the budget is still exceeded afterwards, the node
   says so loudly and unambiguously — the allowance is genuinely too small for the workload,
   which is information, not a failure to act.

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
- *Quiet store:* below the soft mark ⇒ no pass, no warning, no job. Prove the driver does not
  tick on an idle node (the `dev-node-cpu-job-scan` lesson).
- *Bounded default:* a series with no policy stops at `DEFAULT_MAX_SAMPLES` instead of growing
  forever. **State that this test fails with the change reverted.**
- *Dead-letter retention:* an `ingest_dead_letter` table seeded past the horizon is pruned;
  entries inside it survive.
- *Hard mark:* over 95% after compaction ⇒ one tightening pass, and a still-over node logs the
  unambiguous "budget too small" line rather than looping.

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
- **The budget measures the store, not the node.** Extension artifacts, native sidecar binaries
  and OS logs share the filesystem. Reporting free disk alongside budget usage keeps the number
  honest about what it does and does not cover.

## Open questions

1. **Where does the mark live — bytes or percent?** Percent of budget reads naturally but is
   meaningless when the budget is unset. Recommendation: percent of budget, with the marks
   inert whenever `store_budget_bytes` is `None`.
2. **Should the hard mark ever refuse writes?** Out of scope here deliberately. Resolve whether
   the follow-up is worth it once field data shows whether reclamation is ever insufficient.
3. **What is the right default budget when unset — none, or a fraction of the filesystem?**
   Recommendation: none (today's behaviour, purely additive). Auto-deriving from disk size is
   surprising on a shared volume.
4. **Does the dead-letter table want its own horizon, or the workspace's default?** Its own —
   dead letters are diagnostic and worth keeping longer than the data that produced them.
5. **Minimum interval between auto-passes?** Needs the measured pause cost. Start at one hour
   and revisit.

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
