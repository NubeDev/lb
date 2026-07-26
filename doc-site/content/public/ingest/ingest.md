# Ingest

Remaining asks live in `docs/scope/ingest/` (`ingest-scope.md`, `webhooks-scope.md`).

## Controlling what a series stores

Shipped 2026-07-26. One record — the **retention policy** at a series-name prefix — answers three
independent questions. A series is governed by exactly one policy: the **longest matching prefix**
wins, so `modbus.plant-a.coil-3` can read as a step chart while its analog neighbours under
`modbus.` ride the parent's average.

```jsonc
series.retention.set {
  prefix: "modbus.",
  raw_for_ms: 900000,        // 1. how long raw samples live
  max_samples: 100000,       // 2. how many raw samples are kept (FIFO), 0 = unbounded
  filter: {                  // 3. what is ever stored at all
    drop: false,                                  // accept-but-store-nothing mute
    min_interval_ms: 0,                           // ≥1 stored sample per N ms per (series, producer)
    deadband: {abs: 0.5},                         // or {pct: 1.0}
    range: {min: -40, max: 120, mode: "drop"}     // or "clamp"
  },
  tiers: [
    {width_ms: 900000, keep_for_ms: 0, method: "avg"}
  ]
}
```

Every field is optional and defaults to inert: a policy written before this shipped keeps its exact
meaning.

### Filters — store less, on purpose

Predicates run **when a batch commits**, in a fixed cheap-to-stateful order: `drop` → `range` →
`min_interval_ms` → `deadband`.

- **Non-numeric payloads skip the numeric predicates.** A filter written for the analog points under
  a prefix never silently eats a string/object event series that shares it.
- **State is per `(series, producer)`** — the last *committed* value and timestamp — and it is
  durable, so a node restart does not re-open a deadband. Two producers on one series filter
  independently.
- **`min_interval_ms` keeps the FIRST sample of each interval.** Deterministic under a re-drain.
- **`range` defaults to `drop`.** `clamp` is available, but a −9999 sensor error clamped to −40 is
  indistinguishable from a real reading at the bound — dropped-and-counted is the honest default.
- **Nothing is discarded silently.** Every drop is counted per reason (`muted`, `range`,
  `min_interval`, `deadband`) plus `clamped` for samples stored at a bound. The counts ride back on
  `ingest.write` (when non-zero) and on each drain/GC pass.

> **Delivered-then-filtered, not lost.** Acceptance is deliberately unfiltered — a `must-deliver`
> sample is durably accepted, and only then may the *operator's own policy* decline to store it.
> That is a policy decision made visible in the counters, not a broken delivery promise (same class
> as cap-eviction).

> **Live ≠ stored.** Filters change what the store keeps, never what the bus publishes. A dashboard
> watching the live stream still sees every sample a producer sends, even one the store discards.

### Tier methods — a downsampled tier as one plain value

A rollup tier can name the single `method` it reads as, so `series.read {mode:"buckets"}` returns
one value per bucket boundary (13:00, 13:15, 13:30 …) instead of a stat row you have to pick from:

`avg` · `min` · `max` · `sum` · `count` · `last` · `first` · `nearest`

- **`avg` is exact** — computed from the stored `sum`/`count`, never a mean of means, even when a
  read re-aggregates several tier rows into one wider bucket.
- **`nearest` snaps to the grid.** The value at 13:15 is the sample closest to 13:15 *in absolute
  time* — which may be the one just before the boundary. That makes it genuinely different from
  `first`.
- **`last` is what a coil or state point wants.** Averaging a coil produces a value the point can
  never physically hold.
- **The method applies at whatever width you read at**, not only the tier's own — it describes how
  the series reads, and every method here is exact at any width. So a coil set to `last` stays a step
  chart when a dashboard zooms in. (The tier at exactly the requested width wins; otherwise the
  finest tier that names a method.)
- The method **adds** a `value` column; the full stat row stays on the wire. Omitting it is exactly
  today's behaviour. A per-read `method` argument overrides the tier's, and the response reports
  which method produced the value.
- **A method the tier never stored is a clear error**, never an approximation: buckets folded before
  a method existed carry no representative for `first`/`nearest`, and the request fails saying so.

The closed set is bounded by *exactness*: every method is either re-aggregable from stored bucket
statistics or a kept representative sample. Percentiles and standard deviation are deliberately
absent — they cannot be re-aggregated exactly, and admitting them would quietly turn a storage
policy into an approximate compute engine.

### What it adds up to

A point polled every 2 seconds is 43,200 samples/day. With `raw_for_ms: 900000` and one 15-minute
`avg` tier, the store settles at **96 rows/day** plus a bounded 15–30 minute raw window — so the
"live" panel still reads real recent samples while the "history" panel reads the resampled grid,
both through the same `series.read`. A deadband or `min_interval_ms` thins it further before
anything is written at all.

Retention runs on a **background reactor** (~5 minutes); nobody has to call a verb for a policy to
take effect.

## What `ingest.write` guarantees about latency

Shipped 2026-07-15 — the one part of this surface that is settled and worth stating early, because
producers depend on it.

**Your write is never billed for anyone else's backlog.** A sample you push is durably appended to
staging, and the call commits **at most your own batch** (`ceil(your_samples / 256)`) before
returning. Whatever else is queued in the workspace — another producer's burst, a webhook flood, a
federation mirror — is committed by a **background worker**, not inside your call. One producer's
write latency cannot scale with another producer's staging depth.

**Your write is still readable immediately.** The bounded drain preserves the round-trip: a sample
written over a bridge is visible to the very next `series.latest` / `series.read` over that same
bridge, with no explicit drain. That property is deliberate and tested.

**The bound, stated honestly:** if you write more than 256 samples in one call, that call commits
its own work in batches; if a large backlog sits ahead of you, some of your batch budget may commit
those older rows first (staging drains oldest-first). Either way the cost is bounded, and the
background worker commits the remainder within seconds — nothing is stranded, and exactly-once per
`(series, producer, seq)` holds throughout.

Before this shipped, `ingest.write` drained the entire workspace backlog inside the caller's call: a
single sample behind a 4,671-row backlog took 18.5 seconds, and a producer that timed out left the
backlog in place for the next push to hit again.
