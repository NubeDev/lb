# Ingest scope — a per-tier rollup-row cap: the bound that holds with a dead clock

Status: **implemented, unit-tested (`series_rollup_cap_test.rs`), and driven end-to-end over a live
embedded node (rubix-ai gateway, file-backed store, 2026-08-06) — awaiting release.** The live drive
confirmed set/patch round-tripping, the dead-clock eviction, per-SERIES (not per-prefix) scoping, and
found the `last_pass` projection bug noted under Surfaces. Driven by
**[NubeIO/rubix-ai#84](https://github.com/NubeIO/rubix-ai/issues/84)** (the silent-failure half of
AC 7): a Pi must never fill its disc and brick itself, **even when its wall clock is stopped,
arbitrarily wrong, or jumping backwards** — which is the ordinary state of a Rubix Compute with no
RTC battery and no NTP route.

## The gap this closes

Every bound the retention subsystem had on ROLLUP rows was a time horizon computed as
`now_ms - horizon`:

- a tier's `keep_for_ms` eviction (skipped entirely at `keep_for_ms: 0`, the shipped default);
- the orphan-width drain.

Raw already had a clock-free bound — `max_samples` ([series-sample-cap-scope.md](series-sample-cap-scope.md))
compares a row COUNT to a number, no timestamp anywhere. Rollups had none. So on a node whose
clock is behind its data (observed live twice: 46 min on RC-6 2026-08-04, 53 min on the `.62`
bench node 2026-08-06), every horizon lands before the oldest row, `run_gc` evicts nothing,
**and the rollup tier — the term that actually fills a disc, ~96 rows/day/point at 15-min buckets
— grows without bound while every observable reports a healthy pass.**

Worse, the asymmetry is self-defeating: `cap_pass` FOLDS raw's over-cap window into the tiers
before evicting it, so the one clock-free bound that existed *fed* the table that had none.

`clock_sanity.rs` made the inert pass **detectable** (warnings + `clock_skew_ms`); this scope makes
the store **bounded** regardless. Detection tells an operator to drive to the site; a bound means
they don't have to.

## The change

**`Tier::max_rows: u64`** — a FIFO cap on stored rollup rows, **per series, per tier**. `0`
(the serde default) = unbounded, so every policy row written before the field existed keeps its
exact meaning, including the deliberate "the rollup is kept forever" default.

Semantics, mirroring `max_samples` for raw:

- Eviction orders by the rows' **own `t` axis** — oldest bucket first. No wall-clock value appears
  anywhere in the pass, which is the entire point: "at most N rows" is checkable with a dead clock.
- Over-cap rows are **deleted outright**. A rollup is already the coarsest copy; there is nothing
  further to fold into. That is real data loss and the operator's explicit choice when they set
  `max_rows` — the identical posture `max_samples` takes for uncovered raw.
- No tie-break subtlety, unlike raw: the rollup id is deterministic at `[series, width_ms, t]`, so
  `t` is unique within a tier and "keep the newest N" is exact.
- Runs in `run_gc` AFTER the `keep_for_ms` horizon, so it only evicts what the time bound (when the
  clock works) left behind. The two are independent bounds; a row goes when it violates EITHER.
- **Observable, never silent** (the issue-#65 rule): evictions are reported as
  `GcPass::capped_rollup`, persisted on the `series_gc_pass` record, distinct from
  `evicted_rollup`'s time-horizon count.

### Surfaces

| surface | change |
|---|---|
| `series.retention.set` | none needed — deserializes `Policy` whole; `max_rows` rides in a tier object |
| `series.retention.patch` | tier merge gained a `max_rows` arm (absent preserves, number sets) |
| pack manifests (`retention:`) | `RetentionTier.max_rows`, `#[serde(default)]`, field-for-field mirror |
| `lb_ingest` API | `cap_rollup_rows`, `rollup_count` exported; new file `rollup_cap.rs` |
| `last_pass` projection | `capped_rollup` added to the `SELECT` — see the note below |

#### The projection trap, walked into and caught (2026-08-06)

`capped_rollup` was added to `GcPass`, to `GcPassRecord`, and to `from_pass` — but **not** to the
`SELECT` list in `pass_record::last_pass`. That function projects every column BY NAME, and its own
comment warns that an omitted field "reads back as its serde default forever, with the row on disc
perfectly correct." So the disc row held 45; `series.retention.status` reported `capped_rollup: 0`.

This was invisible to the unit suite because all four original cap tests assert on the `GcPass`
**return value**, which was always right. It surfaced only when driving the verb over a live gateway
and reading the status back. The failure mode is the precise one this feature exists to prevent —
a silent drop on the one surface an operator uses to discover their tier is being capped (issue #65's
observable-never-silent rule) — arriving at the last hop rather than in the eviction itself.

Fixed by adding the column, and pinned by `a_capped_pass_is_observable_on_the_persisted_record`,
which asserts the persisted record carries the same count the pass returned. Verified to fail
(`left: 0, right: 45`) against the unfixed projection.

#### ...and its twin, found only by deploying to a node with existing data (2026-08-06)

Adding the column to the projection exposed a SECOND bug that a fresh install can never show. On
**RC-6**, the first `series.retention.gc` after the upgrade failed outright:

```text
expected a 64-bit unsigned integer, found None
```

`capped_rollup` was declared `#[serde(default)]` only. A `series_gc_pass` row written by a build that
predates the field returns the now-projected column as a **present `NONE`**, not an absent key —
which `default` never sees and `usize` refuses. `Policy::max_samples` documents this exact hazard and
carries `none_as_default` for it: *"`#[serde(default)]` covers an ABSENT key; this covers a PRESENT
null one — the two are different bugs and only one of them survives an upgrade."*

**`run_gc` reads `last_pass` first**, so one stale row broke `series.retention.gc` AND
`series.retention.status` completely on the upgraded node — not a wrong number, a dead pass.

Fixed with `#[serde(default, deserialize_with = "none_as_default")]` (`none_as_default` promoted to
`pub(crate)` and shared). Pinned by `a_pass_row_predating_capped_rollup_still_reads_and_gc_still_runs`,
which seeds a pre-upgrade row with raw SQL — the current struct cannot express its own absence — and
asserts both that it reads back and that `run_gc` completes. Verified to fail without the fix with
the identical error string RC-6 produced.

> **Release requirement:** the `node-v*` tag carrying `Tier::max_rows` MUST include both follow-ups.
> Without the projection fix `capped_rollup` always reads 0; without `none_as_default` the first GC
> after upgrading any node with existing data fails. Neither is reachable from a fresh-install test.

### Sizing (the operator's multiplication)

A rollup row is **385 B on disc, measured** — 10,000 rows of a 15-min tier written to a real
file-backed SurrealKV store, compacted, directory stat'd before and after
(`tests/series_rollup_row_size_test.rs`, an `#[ignore]`d measurement; run it with
`--ignored --nocapture`). That confirms the earlier ~400 B RC-6 estimate to within 4%, so the
numbers below moved only slightly and in the safe direction.

The cap's worst case is `max_rows × series × 385 B`:

| series | `max_rows` | worst case |
|---|---|---|
| 1 | 672 | 0.2 MiB |
| 100 | 672 | 24.7 MiB |
| 1800 | 672 (= `keep_for_ms: 7d`) | **444 MiB** (the earlier estimate said 484 MB) |
| 1800 | 2880 (= 30 d) | 1.86 GiB |

Two caveats the measurement itself surfaced. It is a **clean-tier floor**: the rows were written in
one pass with no interleaved deletes, where a long-lived capped tier continuously evicting its
oldest rows will carry some fragmentation above this. And **compaction reclaimed essentially
nothing** (3,853,575 → 3,853,338 B, 0.006%) because every row was policy-live — the measured form
of this scope's claim that compaction cannot reclaim policy-live rows, and the reason bounding has
to be retention's job rather than the disk budget's.

Pick the number the same way `keep_for_ms` is picked in
`rubix-fleet/docs/rasp-pi/CAPACITY-AND-LIMITS.md` §2, then it holds even when the clock doesn't.

## What this deliberately does NOT do

- **No default cap on rollups.** Unlike raw's `DEFAULT_MAX_SAMPLES`, an unpoliced series has no
  tiers and therefore no rollups; a policed tier with `max_rows: 0` is the operator's written
  intent and is honoured, exactly as `max_samples: 0` is.
- **No clock inference.** An earlier idea — substitute the newest data timestamp for a wrong
  `now_ms` — invents a clock and silently changes every horizon's meaning. A count bound needs no
  clock at all, which is strictly stronger.
- **No write refusal at the disk budget.** `LB_STORE_MAX_BYTES`' hard mark still never refuses a
  write (disk-budget scope decision 3); compaction cannot reclaim policy-live rows, so refusing
  writes there would stop the node without bounding the tier. Bounding is retention's job; this is
  the retention piece that was missing.

## What remains unguaranteed (stated, not discovered)

- **A truly frozen clock** (every new sample stamped the identical `ts`): raw's cap refuses to
  split ts-ties, so raw can creep past `max_samples`; rollups stay bounded (one bucket). The floor
  for that pathology is below the app — a fixed-size partition/filesystem quota under the store.
- **A fresh boot with an empty table and a wrong clock** stays undetectable in-band
  (clock_sanity's documented blind spot; rubix-ai#84 AC 8) — but with `max_rows` + `max_samples`
  set it is now *bounded* even while undetected.
