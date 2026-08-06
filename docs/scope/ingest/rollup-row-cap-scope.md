# Ingest scope — a per-tier rollup-row cap: the bound that holds with a dead clock

Status: **implemented, tested (`series_rollup_cap_test.rs`), awaiting release.** Driven by
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

### Sizing (the operator's multiplication)

A rollup row is ~400 B on disc (estimate from RC-6; pin down under load). The cap's worst case is
`max_rows × series × ~400 B`. For the modbus sizing target (1800 points, 15-min buckets), a
`max_rows: 672` tier — the count-equivalent of `keep_for_ms: 7d` — is ~484 MB worst case. Pick the
number the same way `keep_for_ms` is picked in
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
