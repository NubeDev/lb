# Ingest scope — series normalize: write-time filters + tier methods (store less, on purpose)

Status: **SHIPPED 2026-07-26** — promoted to `doc-site/content/public/ingest/ingest.md`;
session: [`sessions/ingest/series-normalize-session.md`](../../sessions/ingest/series-normalize-session.md).
Written 2026-07-26. Builds directly on the shipped retention plane
([`series-retention-scope.md`](series-retention-scope.md), issue #58) and depends on the
retention reactor from [`series-sample-cap-scope.md`](series-sample-cap-scope.md) (issue #65) —
**which was found already merged on `master` and wired at node boot**, and was proven live on a real
product node during this slice. Without a driver, none of this runs; the driver runs.

> **Implemented as written, with one deliberate deviation** — see "Shipped deviations" at the foot.

We want an operator to control **what gets stored, not just how long it lives**. Today a series
stores every accepted sample; retention later bounds it by time (`raw_for_ms`) and count
(`max_samples`). This scope adds the two remaining knobs: (1) **write-time filters** — per-prefix
predicates that decide *store or don't store* as samples commit (deadband, min-interval, range,
mute); (2) a **`method` per rollup tier** — so a downsampled tier reads as a plain series of one
value per bucket boundary (`13:00, 13:15, 13:30 → avg | min | max | last | first | nearest | sum |
count`). Together: a producer can poll every 2 seconds while the store settles at one row per 15
minutes, and a dashboard reads either the recent raw window or the resampled grid through the same
`series.read`.

## Goals

- **One policy, one prefix-match, one admin cap.** Both new knobs are additive fields on the
  existing `series_retention` policy (`series.retention.set`), governed by the shipped
  longest-prefix-wins rule. No new verb family, no new cap. (Rejected: a parallel
  `series.normalize.*` verb family — two prefix-matchers over the same series would need their own
  precedence story and a second admin surface for what is one question: "what does this prefix
  keep?")
- **Filters run at commit, never at accept.** The staging append stays index-free and cheap
  (drain-backpressure invariant); predicates run inside `commit_batch` where the batch is already
  in hand. A filtered sample is **counted, never silent** (`filtered` per-reason counters on the
  commit/GC pass results).
- **A closed, exact set of tier methods.** `avg` (from the stored exact `sum/num_count`), `min`,
  `max`, `sum`, `count`, `last`, `first`, `nearest` (the sample closest to the bucket boundary —
  the "snap to 13:15" ask). Chosen per tier; `series.read {mode:"buckets"}` returns that method's
  value as *the* value column (the full stat row stays available for callers that want it all).
- **More options, bounded by exactness.** Every method must be exactly re-aggregable from stored
  per-bucket stats or a kept representative sample — that is the line that keeps this storage
  policy, not a compute plane.

## Non-goals

- **No stream processing / expression transforms in core.** Interpolation, unit conversion, rhai/
  script transforms, cross-series math — that is compute, banned by the parent scope. The
  extensible path is a **flow or extension** that reads a series and writes a *derived* series
  through the normal gated verbs. Core only filters and aggregates exactly.
- **No per-sample transform-on-write (scale/offset).** Producers/extensions own value shaping
  (modbus already has affine `transform` + COV at source). The host's job is bounding its own disc,
  not fixing producers' units.
- **No rate limiting of *acceptance*.** Still the parent scope's open question. Filters bound what
  is *retained*; accepted-then-filtered samples still cost staging throughput.
- **No new table.** `series_retention` gains fields; `series_rollup` gains the
  `first/first_ts/nearest/nearest_ts` columns; nothing else.

## Intent / approach

**The policy grows two blocks:**

```jsonc
series.retention.set {
  prefix: "modbus.",
  raw_for_ms: 900000,          // raw lives 15 min (existing)
  max_samples: 100000,         // existing (#65)
  filter: {                    // NEW — write-time, applied in commit order
    drop: false,               // true = accept-but-store-nothing mute
    min_interval_ms: 0,        // ≥1 stored sample per N ms per (series, producer)
    deadband: {abs: 0.5},      // or {pct: 1.0}; drop if |v - last committed| < delta
    range: {min: -40, max: 120, mode: "drop"}  // or "clamp" (clamped samples counted too)
  },
  tiers: [
    {width_ms: 900000, keep_for_ms: 0, method: "avg"}   // NEW field: method
  ]
}
```

- **Filter evaluation order** (cheap → stateful): `drop` → `range` → `min_interval_ms` →
  `deadband`. Non-numeric payloads skip the numeric predicates (`range`, `deadband`) untouched —
  a filter never silently eats an event/object series.
- **Deadband/min-interval state** is per `(series, producer)` — the last *committed* value + ts —
  held on the existing `series_meta` row (one field pair, updated in the same commit transaction).
  Never compare across producers (`seq`/ordering lesson,
  `debugging/ingest/latest-pinned-to-pre-restart-sample.md`).
- **`method` on a tier** selects the value column at read time; the GC keeps storing the full
  exact stat row. `first/nearest` need their representative kept at fold time (new rollup columns);
  `avg` = `sum / num_count` (exact, never mean-of-means); everything else already exists.
- **Both range modes ship** (`drop` and `clamp`) — the user asked for options; the default is
  `drop` (a −9999 sensor error clamped to −40 is a plausible-looking lie; dropped-and-counted is
  honest).

**Rejected alternatives:**

- *Filter at the producer/extension only.* The host cannot trust producers to bound the host's
  disc; source-side filtering (modbus COV) remains welcome as the first line, this is the enforced
  second.
- *Filter at staging append.* Re-couples producer rate to store queries (deadband needs a read) —
  the exact coupling drain-backpressure removed.
- *A derived "normalized" output series.* Doubles storage (the opposite of the goal) and breaks
  the `(series, producer, seq)` identity.
- *Open-ended aggregation (percentiles, stddev at arbitrary read widths).* p50/p95 are not exactly
  re-aggregable from bucket stats; admitting them makes rollups approximate. Deferred until someone
  needs them enough to accept "approximate" being written on the tin.

## How it fits the core

- **Tenancy / isolation:** policies, filters, meta-state, rollups all workspace-scoped (`query_ws`)
  as today. Mandatory isolation test.
- **Capabilities:** no new verb → no new cap. The additive fields ride
  `mcp:series.retention.set:call` (admin-tier). Deny test: the existing gate covers the new fields.
- **Placement:** `either`; runs where the ingest role runs (commit path + retention reactor).
  Config, not a code branch.
- **MCP surface:** `series.retention.set/list` gain the fields; `series.read {mode:"buckets"}`
  gains an optional `method` override (defaults to the tier's); commit/GC passes gain `filtered` /
  method counters. All additive — every existing caller unaffected (serde defaults preserve
  meaning: absent `filter` = store everything, absent `method` = today's full-stat row).
- **Data (SurrealDB):** `series_retention` (+`filter`, +`method` per tier), `series_meta`
  (+last-committed pair), `series_rollup` (+`first/first_ts/nearest/nearest_ts`). One datastore;
  no new table.
- **Bus:** N/A — the live Zenoh stream is untouched; a dashboard watching motion still sees every
  published sample even when the store filters it. (Feature, not bug: live ≠ stored. Say so in the
  public doc.)
- **State vs motion / stateless extensions / symmetric nodes:** unchanged.
- **SDK/WIT impact:** none — host-native verbs only. Flag loudly if that changes.
- **One responsibility per file:** `ingest/src/filter.rs` (predicates), `commit.rs` wires it,
  `rollup.rs` gains the two column pairs, `bucket.rs` gains method selection, `retention.rs` gains
  the fields. Nothing near 400 lines.
- **Skill doc:** N/A — additive fields on existing verbs; if a retention-administration skill ever
  exists it gains fields, not a page.

## Example flow

A modbus point polled every 2 s (43,200 samples/day raw, ~30 MB/day/point unbounded):

1. Admin (or the modbus extension at boot, holding the cap) sets the policy above for `modbus.`.
2. Producer writes every 2 s. `deadband {abs: 0.5}` drops the ~90% of samples where the value
   didn't move; `range` drops the −9999 error frames, counted.
3. Dashboard "live" panel reads `series.latest` + the last-15-min raw window — always the freshest
   real values, fast (small indexed range scan).
4. The retention reactor (#65) ticks every 5 min: raw older than 15 min folds into 15-min buckets
   at exact boundaries, raw evicts. Store settles at **96 rows/day/point**.
5. Dashboard "history" panel reads `{mode:"buckets", width_ms: 900000}` → one `avg` value at
   13:00, 13:15, 13:30… A state-type point uses `method:"last"` on its own longer prefix
   (longest-prefix-wins) and reads as a step chart.

## Testing plan

Real store, no mocks (testing §0). Mandatory: **capability deny** (new fields refused without the
admin cap) and **workspace isolation** (ws-B policy filters/folds nothing in ws-A). Slice-specific:

- Each filter predicate: stores what passes, drops what doesn't, **counters match exactly**; a
  non-numeric payload passes numeric predicates untouched; deadband state survives a node restart
  (it's on `series_meta`, prove it) and is per-producer (two producers, independent deadbands —
  seed with `sample_at()`, independent `ts`/`seq`).
- `clamp` stores the bound value and counts it; `drop` stores nothing and counts it.
- Each method: fold known samples, assert the bucket value (esp. `nearest` picks the
  boundary-closest sample and `avg` is exact across a two-pass re-aggregation — no mean-of-means).
- Filters compose with cap + time horizon; idempotent second GC pass; `mode:"buckets"` read
  returns the tier's method value over evicted history.
- **The product test** (per `verify-in-product-not-suite`): boot a real node, real producer at
  2 s cadence, policy set, nobody calling verbs — assert the stored row count plateaus at the grid
  rate and the dashboard reads stay fast. Revert-check every regression test.

## Risks & hard problems

- **Deadband turns a read into every commit batch.** The last-committed lookup rides `series_meta`
  (already read at commit for the cardinality cap) — keep it in that same query or the commit path
  gains a per-series round-trip behind the global session mutex. Measure.
- **Filtered ≠ lost must-deliver.** A `qos: must-deliver` sample that the *operator's own policy*
  drops is delivered-then-filtered — state it in the public doc or it reads as a broken promise
  (same class as cap-eviction).
- **The #65 dependency.** All of this is decorative until the retention reactor is proven live.
  This scope must not ship "green in the suite" against a driver that never ticks — the exact
  drain-backpressure failure class.
- **Clamp lies plausibly.** A clamped value is indistinguishable from a real reading at the bound.
  Default `drop`; document the trade.
- **Ordering.** Any "oldest/nearest/first/last" is by `(ts, seq-within-producer)` — never raw
  `seq` across producers. Already burned once; the multi-producer test is mandatory.

## Open questions

**None — all decided (2026-07-26); implement as stated, do not re-open:**

- **`min_interval_ms` keeps the *first* sample of each interval.** Simplest, stateless beyond the
  existing last-committed pair, and deterministic under re-drain (the first accepted commit wins;
  a "keep last" would rewrite a committed row per interval).
- **Filtered counters are per-pass, per-prefix, per-reason** (`filtered: {range, deadband,
  min_interval, muted, clamped}` on the commit/GC pass results). Per-series granularity ships with
  the "series.list shows sizes" observability slice, not here.
- **A `method` the tier didn't store is a clear `BadInput`** ("tier folded without `nearest`; set
  the method on the tier and let new buckets accumulate"). Never approximate silently.
- **Extensions stand policy like any principal**: request `mcp:series.retention.set:call` +
  `:list:call`, upsert **only-if-absent** at boot (an admin's row is authoritative), degrade to a
  logged warning if the cap is denied. No special treatment (rule 10).

## Shipped deviations (2026-07-26)

**Two rollup columns, not four: no `nearest`/`nearest_ts`.** This scope specified
`+first/first_ts/nearest/nearest_ts` on `series_rollup`. Only `first`/`first_ts` shipped.

Within a bucket `[t, t+w)` every sample is at or after `t`, so "the sample closest to the bucket
boundary" *is* the first sample — a stored `nearest` column would have been a byte-for-byte
duplicate of `first`, in a slice whose entire purpose is storing less, and `nearest` would have been
a method that returns exactly what `first` returns. The useful "snap to 13:15" semantic reaches
**across** the boundary: the sample nearest a grid point in absolute time may sit just *before* it.
So `nearest` is computed at read time from two representatives already on the row — this bucket's
`first` (nearest from above) and the previous bucket's `last` (nearest from below), nearer wins,
ties to `first`. Exact, no extra storage, and genuinely distinct from `first`
(`series_method_test::nearest_snaps_to_the_grid_across_a_bucket_boundary` proves both).

*Rejected:* storing the columns as specified — duplication plus a no-op method.

**One addition the scope did not name:** `ingest.write` now returns a `filtered` object when its
inline drain discarded anything. Acceptance is deliberately unfiltered, so without it a producer
sees `accepted: 4`, finds two rows, and has nothing on the wire explaining the gap. Present only
when non-zero, so every existing caller's reply shape is unchanged.

**One regression found and fixed while building this** (never shipped):
[`debugging/ingest/filtered-batch-stops-the-drain-loop.md`](../../debugging/ingest/filtered-batch-stops-the-drain-loop.md)
— the drain loop's `committed == 0` stop condition stopped meaning "staging is empty" the moment a
third outcome (dequeued-but-stored-nowhere) existed.

## Related

- [`series-retention-scope.md`](series-retention-scope.md) — the shipped machinery this extends.
- [`series-sample-cap-scope.md`](series-sample-cap-scope.md) — the reactor/driver + count bound
  (issue #65) this **depends on**.
- [`ingest-scope.md`](ingest-scope.md) — the parent; "no analytics engine" non-goal is the wall
  this scope deliberately stays inside.
- [`drain-backpressure-scope.md`](drain-backpressure-scope.md) — why filters live at commit.
- `debugging/ingest/latest-pinned-to-pre-restart-sample.md` — the ordering axis.
- `../datasources/series-decimation-scope.md` — bucketed reads that gain the `method` column.
- README **§6.1** (time-series model), **§3** rules 2/3/6.
- `rubix-ai-extensions/extensions/modbus/docs/scope/` — the first consumer (poll-to-grid).
