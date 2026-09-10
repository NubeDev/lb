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

> **A series with NO policy record is capped at 100,000 raw samples.** Absence of a record is not
> "unbounded" — the explicit opt-out is a record carrying `max_samples: 0`. This is a behaviour
> change that deletes history on existing nodes: read [Upgrading](../upgrading/upgrading.md) before
> you upgrade.

### Filters — store less, on purpose

Predicates run **when a batch commits**, in a fixed cheap-to-stateful order: `drop` → `range` →
`min_interval_ms` → `deadband`.

- **Non-numeric payloads skip the numeric predicates.** A filter written for the analog points under
  a prefix never silently eats a string/object event series that shares it.
- **State is per `(series, producer)`** — the last *committed* value and timestamp — and it is
  durable, so a node restart does not re-open a deadband. Two producers on one series filter
  independently.
- **`min_interval_ms` keeps the FIRST sample of each interval.** Deterministic under a re-push.
- **`range` defaults to `drop`.** `clamp` is available, but a −9999 sensor error clamped to −40 is
  indistinguishable from a real reading at the bound — dropped-and-counted is the honest default.
- **Nothing is discarded silently.** Every drop is counted per reason (`muted`, `range`,
  `min_interval`, `deadband`) plus `clamped` for samples stored at a bound. The counts ride back on
  `ingest.write` (when non-zero) and on each GC pass.

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

## What `ingest.write` guarantees

**One sample costs one write.** `ingest.write` commits your samples straight into the indexed
`series` tables and returns. There is no landing table, no background worker, and no queue between
the call and the store.

**Your write is never billed for anyone else's backlog**, because there is no backlog to be billed
for. The call does exactly your own work: your samples, in batches of at most **256**, each batch
one transaction.

**Your write is readable the moment it returns.** The ack comes after the transaction commits, so
the sample is visible to your very next `series.latest` / `series.read`.

**Exactly-once still holds.** A crash before the commit rolls the whole batch back, and the
producer never saw an ack. A `must-deliver` producer re-pushes, and the UPSERT keyed on
`[series, producer, seq]` absorbs the repeat exactly once.

### Why there is no staging table any more

Earlier releases appended each sample to a durable `ingest_staging` table and had a background
worker move it into `series` later. Storing one sample therefore cost **three** writes to the same
database: the staging insert, the `series` insert at commit, and the staging delete tombstone.

The justification was that staging was a cheap unindexed landing zone, so a burst would land
somewhere cheap and the expensive indexed write would happen off the burst. Both halves were wrong:

- **The landing zone was not cheap.** It was a table in the same database, so a staged sample paid
  the same write-ahead-log append and the same memtable insert as a committed one, plus a tombstone
  when it left. Staging did not defer work. It added work, to the store that was already loaded.
- **The write it deferred was not expensive.** The engine underneath is an LSM tree, where a
  secondary index entry is just another key-value pair appended to the same memtable and log. There
  is no index page to read, lock or rewrite.

Measured on a real store, 200,000 samples cost **115,398 ms and 11.87 MB** through staging, against
**3,752 ms and 4.02 MB** committed directly. That is 31 times faster for a third of the bytes.

Staging was also described as backpressure. It was not. When the store was too loaded to take a
write, staging responded by writing to that same store two extra times. Real backpressure is a
buffer somewhere the store is not — a producer's own memory, for instance — and that belongs to the
producer, not to the node.
