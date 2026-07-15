# Store scope — online commit-log compaction (bound a long-running node)

Status: scope (the ask). Promotes to `doc-site/content/public/store/` once shipped.

The store's two bounding mechanisms each cover half the problem and the halves don't meet.
**Boot-time compaction** (`crates/store/src/open.rs::compact_log`) rewrites the SurrealKV
commit log down to its live set — but only at `Store::open`. **Retention GC**
(`spawn_retention_reactors`, 5-min cadence) evicts stale rows while the node runs — but on an
append-only engine every eviction is a *tombstone appended to the log*, not bytes reclaimed.
So a long-running node is bounded in **rows** and unbounded in **bytes**: measured, ~24h of
1s-cadence ingest produced a **1.5 GB log over a ~23 MB live set (~65× bloat)** and a **13–14s
boot** that replays all of it. Today the only way to reclaim the log is to restart the node.
This scope closes the loop: the log stays bounded *while the node runs*, and its growth is
observable before it becomes a boot-time surprise.

## Goals

- **Bounded log bytes on a running node.** A node ingesting for a week must not accumulate a
  commit log proportional to its write history. Disk and next-boot cost track the live set.
- **Observable before painful.** Log size (bytes, segment count) is readable via a verb and
  logged as an advisory warning past a threshold — the same visibility-first posture as the
  sample-cap over-cap warnings.
- **No new failure modes.** Compaction is best-effort: a failed or interrupted pass leaves the
  log valid (the boot path already holds this contract; the runtime path must too).
- **Symmetric.** Which nodes run it and how often is `BootConfig`, never a code branch (rule 1).

## Non-goals

- **Boot-time compaction** — shipped (`open.rs`, `compaction_test.rs`). This scope adds the
  runtime half; it does not touch the boot half except to share its implementation.
- **Retention/rollup semantics** — shipped (`series-retention-scope.md`,
  `series-sample-cap-scope.md`). This scope reclaims the bytes those evictions leave behind;
  it does not change what gets evicted.
- **The global session mutex** — `session-concurrency-scope.md` stands. If the chosen
  mechanism needs a write quiesce, it *uses* the mutex; it does not redesign it.
- **Changing the engine.** SurrealDB on SurrealKV only (rule 2).
- **Fixing write amplification at the source.** If the 65× bloat turns out to be an
  ingest-path UPSERT pattern, that is its own scope; compaction is correct regardless.

## Intent / approach

The hard constraint, already documented in `open.rs`: **surrealdb 2.x exposes no path to
SurrealKV's `Store::compact()`**. Boot compaction works by opening a *second, exclusive*
`surrealkv::Store` on the directory **before** SurrealDB opens it. At runtime SurrealDB holds
the engine open, so that trick is unavailable as-is. Three candidate shapes, in the order to
try them — **the spike decides, before any code**:

1. **Live-handle compaction (spike first).** Determine whether `surrealkv::Store::compact()`
   is safe to invoke concurrently with an open surrealdb handle on the same directory —
   either via a second `Store` instance (likely refused: the engine locks its dir) or via an
   upstream surrealdb API we've missed at the pinned version. If upstream surrealdb has (or
   will accept) a `compact()` passthrough, that is the *right* fix and everything below is
   scaffolding. File the upstream issue either way; don't block on it.
2. **Swap-the-handle maintenance pass (the likely answer).** A maintenance job that: takes the
   store's session mutex (writes quiesce — the mutex is load-bearing here, for once as an
   asset), drops the surrealdb handle, runs the existing `compact_log` (shared with boot),
   reopens, and swaps the new handle in. Requires the `Surreal<Db>` handle to live behind a
   swappable cell (`ArcSwap`/`RwLock`) instead of a plain field — a small, auditable change to
   `Store`, but it touches the crate the session-concurrency scope says to treat carefully:
   **spike-before-code, and the isolation suites must pass unmodified.**
3. **Supervised restart-to-compact (the honest fallback).** If neither of the above is safe:
   a threshold-crossing log emits the advisory warning, and compaction remains a restart
   concern — but a *managed* one: the `store.compact` verb returns `{requires_restart: true,
   log_bytes, live_estimate}` so an operator (or the supervisor, later) restarts knowingly
   instead of discovering a 14s boot by surprise. Observability ships even in this outcome.

Whichever shape wins, **observability ships first and unconditionally**: a `store.status`
read surface (log bytes, segment count, last-compaction result) and the threshold warning.
That slice is cheap, independent, and turns the next "why is the node slow" from an afternoon
into one verb call.

**Alternative rejected:** a fast periodic compaction tick. Compaction is whole-log I/O
(`open.rs` keeps it on `spawn_blocking` for exactly this reason); running it on a minutes
cadence would trade unbounded disk for a periodic I/O storm. The driver must be
**threshold-driven** (log bytes vs. live estimate), not clock-driven — same lesson as
`debugging/agent/dev-node-cpu-job-scan.md`.

## How it fits the core

- **Tenancy / isolation:** compaction operates below the namespace wall — it rewrites log
  bytes, never reads records as any principal. The wall is unaffected, and the proof is that
  the existing workspace-isolation suites (incl. `concurrent_ns_test`) pass **unmodified**.
- **Capabilities:** `store.status` is read-gated (`store:status:read`); triggering a pass is
  admin-gated (`store:compact:run`). Deny path: an ungranted caller gets the standard caps
  refusal; there is no anonymous trigger. The background driver itself mints no principal —
  it executes node maintenance, the same posture as `spawn_retention_reactors`.
- **Placement:** either. Cadence/threshold and whether the reactor runs at all are
  `BootConfig::reactors` config (rule 1).
- **MCP surface (§6.1):** *get* — `store.status` (one snapshot; no list, there is one store
  per node). *Batch/long-running* — `store.compact` **must be a job**: it is whole-log I/O
  with no upper bound; the verb enqueues and returns a job id, progress via the job feed.
  No CRUD, no live feed — log size changes slowly; poll `store.status`, don't stream it. N/A
  verbs stated per SCOPE-WRITTING §6.1.
- **Data (SurrealDB):** no schema change. Last-compaction outcome is either derived live from
  the manifest or stored as one node-scoped status record.
- **Bus (Zenoh):** none required. The threshold warning is telemetry (tracing), not a bus
  message; nothing must-deliver here.
- **Sync / authority:** node-local by definition — a commit log belongs to one node.
- **Secrets:** N/A.
- **SDK/WIT impact:** none. No extension-visible surface changes.
- **Skill doc:** yes — this ships a drivable surface (`store.status`, `store.compact` job).
  The implementing session writes `skills/store-compact/SKILL.md` grounded in a live run
  (check status → trigger → watch the job → confirm shrink).

## Example flow

1. A node ingests at 1s cadence for days. Retention GC keeps rows bounded; the log grows.
2. The threshold driver sees `log_bytes` cross the configured multiple of the live estimate
   and logs the advisory warning (visible in `store.status` too).
3. An operator (or the driver itself, if shape 1/2 landed) enqueues `store.compact` → job id.
4. Writes quiesce behind the session mutex; the handle is swapped out; `compact_log` rewrites
   the log to the live set; the handle is swapped back in. Queued writes proceed — they waited
   on the mutex exactly as they would behind any long transaction.
5. `store.status` shows the new size; the job completes with `{before_bytes, after_bytes}`.
6. Next boot replays only the live set: ~1s, not 14s.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`):
- **Workspace isolation:** the full isolation suites, `concurrent_ns_test` included, pass
  **unmodified** after any `Store` handle change. This is the non-negotiable gate.
- **Capability deny:** a caller without `store:compact:run` cannot trigger a pass; without
  `store:status:read` cannot read status.
- **Hot-reload / restart:** extensions and reactors survive a compaction pass exactly as they
  survive any slow store moment (they hold `Arc<Store>`, not raw handles — verify).

Key cases (real store, real bytes — no mocks, rule 9):
- *Shrink:* seed + evict through the real retention path until the log holds mostly dead
  bytes; run the pass; assert `after_bytes` ≪ `before_bytes` and every surviving record reads
  back intact across all workspaces.
- *Crash mid-pass:* kill the process during compaction (extend `store/tests/crash_test.rs`);
  reopen must succeed on either the old or new log, never a corrupt one.
- *Writes during the pass:* concurrent writers block on the mutex and land after the swap —
  none lost, none duplicated.
- *Threshold driver:* below threshold → no pass, no warning; above → warning, and (shape 1/2)
  a pass. Prove the driver does **not** tick on a quiet store.
- *Boot dividend:* boot a copy of a compacted vs. uncompacted large store; assert the
  time-to-listen bound. **State that the shrink test fails with the fix reverted.**

## Risks & hard problems

- **The engine may not allow it.** Shape 1 may be impossible and shape 2 may hit a SurrealKV
  directory lock even after dropping the surrealdb handle (async drop timing). The spike must
  prove the swap is race-free before any code lands. If both fail, ship shape 3 and say so
  plainly — a documented bound beats a clever hack (same posture as the boot-compaction doc).
- **The handle swap touches sacred ground.** `Store` is the workspace wall's foundation; a
  swappable handle must not open a window where a query runs against a half-open engine.
  The mutex-held swap is designed to prevent exactly that — prove it under concurrency.
- **Compaction cost itself.** Whole-log I/O while the node serves. `spawn_blocking` +
  threshold-driven keeps it rare and off the async workers, but a pass on a huge log will
  visibly pause writes. Measure and document the pause; don't hide it.
- **The 65× bloat may be a bug wearing compaction's clothes.** If segment inspection shows
  the log is mostly superseded ingest UPSERTs, the write path deserves its own scope —
  compaction then treats the symptom well but someone must own the cause.

## Open questions — RESOLVED 2026-07-15 (see `sessions/store/online-compaction-session.md`)

1. **Second handle:** surrealkv 0.9.3 has NO directory lock — a concurrent open *succeeds and
   then loses data* (live-handle writes post-compact: 10/10 lost). Shape 1 rejected on
   demonstrated loss, not a lock. Upstream 2.6.5 source checked: only *index* compaction
   exists, no log-compact passthrough.
2. **Drop-release:** asynchronous (router drain + `kvs.shutdown()` after the drop), 74–240 ms
   observed over 10 rounds. Shape 2 detects release by fd-poll and SKIPS the pass on timeout —
   race-free by refusing to race.
3. **Threshold:** absolute bytes, 256 MiB default (`LOG_ADVISORY_BYTES`). No cheap live-set
   estimate exists yet; segment count is too coarse at 512 MiB segments. Revisit if a cheap
   live estimate appears.
4. **What's in the bloat:** superseded versions + tombstones from ordinary churn — measured:
   12,000 real ingest-path samples with 92% retention-evicted → 17.7 MB log over a 680 KB
   live set (26×), fully recovered by compaction. No write-amplification bug found. Story
   closed, no follow-up scope.
5. **Driver:** operator-triggered for release 1; the reactor only warns past the threshold.
   Auto-trigger is a follow-up once the pause cost is measured in the field.

**Spike ambush (bigger than the scope):** compaction at the pinned engine version silently ate
every write made after a previous compaction — the SHIPPED boot path destroyed data from the
third boot onward. Root cause + fix (`compact_log`'s merge-completion rule) in
`debugging/store/compaction-merge-eats-next-sessions-writes.md`. Shape 3 (restart-to-compact)
would NOT have been a safe fallback until that fix; it is now unnecessary — shape 2 shipped.

## Related

- `crates/store/src/open.rs` — `compact_log` (the boot half; the runtime pass reuses it) and
  the doc comment recording that surrealdb 2.x exposes no compaction path.
- `scope/store/session-concurrency-scope.md` — the mutex the swap leans on. Read before code.
- `scope/store/persistent-backend-scope.md` — the SurrealKV posture this builds on.
- `scope/ingest/series-retention-scope.md`, `scope/ingest/series-sample-cap-scope.md` — the
  row-bounding half (shipped, incl. `spawn_retention_reactors`); this scope is its byte half.
- `scope/ingest/drain-backpressure-scope.md` — "a request pays for its own work, never the
  backlog": the compaction job pays for the backlog *explicitly and observably*, which is the
  compliant way to violate it.
- `store/tests/compaction_test.rs`, `store/tests/crash_test.rs` — the tests to extend.
