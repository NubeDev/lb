# Datasources scope — series decimation (slice C: charts downsample, they don't page raw points)

Status: **shipped** (2026-07-14, issue #57, `series-plane-readiness`) — **child slice C** of
[`page-chaining-scope.md`](page-chaining-scope.md). See
[`../../sessions/ingest/series-plane-readiness-session.md`](../../sessions/ingest/series-plane-readiness-session.md).
Implementation notes (resolving this doc's open questions): execution is a **chunked fold over the
keyset pager** rather than a SurrealDB `GROUP BY time_bucket` — SurrealDB 2 has no ordered `last`
aggregate, and the fold keeps `last` exact, tolerates non-numeric payloads, and stays O(page) memory
while riding the `(series, ts)` index. Both `width_ms` and `budget` accepted (explicit width wins);
buckets are **sparse**; hard cap 2 000 buckets; LTTB deferred. Where retention GC (issue #58) has
evicted raw history, the read merges the stored rollup tier — see
[`../ingest/series-retention-scope.md`](../ingest/series-retention-scope.md) for why those stored
tiers don't violate this doc's "no materialized rollups" stance (they are the sole surviving copy
post-eviction, not a cache).

A million samples do not belong in a 1000px chart. Slice B made `series.read` page **raw rows**
fast; but a chart that keyset-pages raw points still ships a million rows to draw a thousand pixels —
fast per page, wrong total. This slice adds `mode:"buckets"` to `series.read`: **server-side
time-bucket decimation** to a bounded point budget for the *visible window*, executed as a SurrealDB
`GROUP BY time_bucket(width)` read **where the data lives**. Each bucket returns `{t, min, max, avg,
last}` so **spikes survive** — a plain `avg` smooths away the exact peak a temperature alert cares
about. The window itself pages by a **time cursor** (the same keyset chain from slice A, over buckets
instead of rows), so pan-back loads the adjacent window at O(page). This is the real answer to "a
million points into a chart, fast."

## Goals

- `series.read` accepts `mode:"buckets"` with a **bucket width** (or a target **point budget** the
  server turns into a width for the window) and returns **≤ budget** buckets, never the raw range.
- Each bucket carries `{t, min, max, avg, last}` — enough that a spike **inside** a bucket shows in
  `max` (or a trough in `min`), not averaged away. min/max/last is the safe default.
- The read is a **`GROUP BY time_bucket(width)`** aggregate on the series plane in SurrealDB — computed
  at read time, **not** stored/materialized rollups (rule 2).
- The visible **window** pages by a **time cursor** — the same opaque keyset chain from slice A, keyed
  on bucket time — so pan-back / pan-forward loads adjacent windows at O(page), not an OFFSET scan.
- Same shared contract as slice B: opaque cursor, bounded budget, **existing** `mcp:series.read:call`
  cap re-checked per page, workspace + series from the token — the cursor is a bookmark, not a grant.
- No new capability, no new verb — an additive `mode` field on the read slice B already grew.

## Non-goals

- **The cursor/keyset primitive** — codec, tiebreaker discipline, versioning belong to slice A
  ([`page-cursor-scope.md`](page-cursor-scope.md)); this consumes it over bucket time.
- **Raw-row `mode:"rows"` paging** — that is slice B ([`series-paging-scope.md`](series-paging-scope.md));
  this builds **on** it (same verb, same cursor chain, different aggregation shape).
- **Executing the decimation *on a federated source*** — pushing this same `time_bucket … GROUP BY` down
  to a warehouse (TimescaleDB's native `time_bucket`/continuous aggregates) is slice D
  ([`federation-paging-scope.md`](federation-paging-scope.md)). This slice defines the **contract** (bucket
  shape, budget→width, spike-survival, the time-cursor chain) and executes it for a **native SurrealDB
  series**; slice D executes the *identical contract* by pushdown for a source-resident series. Both are
  first-class fast paths — a deployment that keeps its bulk in Timescale gets the same decimated chart,
  computed at the warehouse (see the note below).
- **The chart component** that calls this and composes it with the live `series.watch` tail — slice E
  ([`page-chaining-ui-scope.md`](page-chaining-ui-scope.md)).
- **Stored/pre-aggregated rollup tables** or a downsampling ingest job — decimation is a read-time
  `GROUP BY`, not a second copy of the data.
- **Total bucket counts / numbered pages** — forward/back window cursoring only (parent non-goal).

## Intent / approach

**Decimate at the source, over the visible window, at read time.** The series plane already holds
samples indexed on `(series, ts)` (slice B's index guarantee). A windowed `GROUP BY time_bucket(width)`
is an index-range scan plus a streaming aggregate: it reads only the window, emits one row per bucket,
and returns a bounded set regardless of how many raw samples the window contains. The client sends a
window's worth of budget; the server picks `width = window_span / budget` (snapped to a sane unit),
runs the aggregate, and returns ≤ budget buckets. Pan-back is just the **next time cursor**: the last
bucket's `t` is the keyset position, and the adjacent window is another O(page) index seek — the exact
keyset chain from slice A, walking bucket boundaries instead of rows.

**One decimation contract, two engines — native or pushed down.** This slice's bucket record, budget→width
derivation, and time-cursor chain are **engine-agnostic**. For a native series the aggregate is a SurrealDB
`GROUP BY time_bucket` (below); for a **series that lives in a warehouse** the *same* aggregate is **pushed
down to the source** by slice D — Timescale runs it via its native `time_bucket()`/continuous aggregates
and returns ~budget bucket rows, so the raw range never crosses the wire. A caller (and the chart in slice
E) asks for `mode:"buckets"` identically; which engine computes it is resolved by where the series lives.
The contract here is the single definition both honor.

**Per-bucket `{t, min, max, avg, last}`, not `avg` alone.** A chart that draws only bucket averages
lies: a 200°C spike lasting three samples inside a one-minute bucket vanishes into the mean, and the
alert that fires on it never sees a line touch the threshold. Returning `min` and `max` per bucket lets
the renderer draw an envelope (or a min/max candlestick) so the spike **survives decimation**; `last`
gives a stable "current value at bucket edge" for step lines; `avg` is the trend. The cost is honest:
**3–4× the payload** of a single value per bucket, so the point budget is set against the *bucket
record*, not a scalar — 1000 pixels is ~1000 buckets is ~4000 numbers, not a million.

**Read-time `GROUP BY`, not stored rollups (rule 2).** We deliberately do **not** materialize
downsampled tiers (the classic timeseries-DB move: 1s → 1m → 1h rollup tables). That is a second
datastore-shaped thing, it goes stale under late-arriving samples, and it forces a width choice at
write time when the right width depends on the reader's window. The window is bounded and the key is
indexed, so the read-time aggregate is already O(window/bucket) — fast enough without a second copy.
One datastore, one truth.

**Rejected — `avg`-only decimation.** Smaller payload, but it hides exactly the spikes this platform's
alerting cares about; a monitoring chart that can't show a peak is worse than useless. **Rejected —
client-side decimation** (ship raw rows, downsample in the browser): it defeats the entire point,
moving a million rows over the wire to throw most away. **Rejected — materialized rollup tiers** (above,
rule 2). The recommended default is **fixed-width time buckets with min/max/avg/last**, computed at read
time, paged by time cursor. Whether a viz can instead request **LTTB** (shape-preserving, better line
fidelity) is the open question below.

## How it fits the core

- **Tenancy / isolation:** the workspace is host-set from the token on **every** window page; the
  `GROUP BY` is scoped `WHERE workspace = $ws AND series = $series` before any bucketing. The time cursor
  carries only a bucket-time position — never the workspace or series. A ws-A cursor replayed under a
  ws-B token buckets nothing (the predicate resolves empty). Mandatory isolation test below.
- **Capabilities:** **no new cap.** The bucketed read is gated by the existing `mcp:series.read:call`,
  re-authorized **per window page** (workspace-first, then the cap) exactly as `mode:"rows"`. A mid-pan
  revoke denies the next window. Deny path unchanged. Mandatory deny test below.
- **Placement:** `either`, no `if cloud`. The same `GROUP BY time_bucket` runs against a local edge
  series or a mirrored cloud series; which series exist is config (grants), not a branch.
- **MCP surface (§6.1):** additive — `series.read` grows `mode:"buckets"` + `bucket`/`budget` fields
  alongside slice B's `{limit, cursor, direction}`. It is a **get/list-shaped read** returning
  `{buckets, next_cursor, prev_cursor}`. No CRUD (decimation writes nothing), no new live feed (the
  forward tail stays `series.watch`), no new verb. A long whole-range decimated export stays a
  **mirror/export job** (§6.10), not a client window-loop.
- **Data (SurrealDB):** no new table, no stored aggregate. The change is a **read-time `GROUP BY
  time_bucket(width)`** over the existing series records, riding slice B's `(series, ts)` index for the
  window range scan. State plane only.
- **Bus (Zenoh):** none. State-vs-motion (rule 3): windows page *backward through committed state* in
  SurrealDB; the live forward edge is `series.watch` (slice E composes the two — it is not this slice).
- **Sync / authority:** reads committed local state; no new authority. Offline pages whatever the local
  node holds, same as any series read.
- **Secrets:** none.
- **SDK/WIT impact:** additive fields on the existing `series.read` MCP verb — no new verb, no new
  host-callback, no ABI break. A guest already calling `series.read` gains decimation by sending
  `mode:"buckets"`.

## Example flow

A dashboard temperature chart shows the last 6 hours over ~1000px; a sensor spiked to 200°C for three
samples 90 minutes ago.

1. The chart calls `series.read` with `{series, mode:"buckets", window:{from,to} , budget:1000}` under a
   token carrying ws + the `mcp:series.read:call` grant.
2. Host re-authorizes **workspace-first**, then the cap. Passing.
3. Host derives `width = 6h / 1000 ≈ 21.6s`, snapped to `30s` (≈720 buckets ≤ budget).
4. Host runs `SELECT time_bucket(ts, 30s) AS t, min(v), max(v), math::mean(v) AS avg, last(v) AS last
   FROM series WHERE workspace=$ws AND series=$series AND ts >= $from AND ts < $to GROUP BY t ORDER BY t`
   — an index-range scan + streaming aggregate over the window.
5. The bucket covering the spike returns `max: 200` even though its `avg` is ~24; the renderer draws the
   min/max envelope and the peak **is on screen**.
6. Response: `{buckets:[…≤720…], next_cursor: <bucket-time keyset>, prev_cursor: <…>}`.
7. The user pans back 6 hours: the chart echoes `prev_cursor`; host re-authorizes (per-page), seeks the
   adjacent window by bucket time (O(page)), returns the previous window's buckets. No OFFSET, flat
   latency at any pan depth.

## Testing plan

Per `scope/testing/testing-scope.md`. **No mocks** — a real `mem://` SurrealDB store **seeded with a
large real series** (e.g. 1M samples across the window) that includes a **deliberate spike inside a
single bucket**. Real host, real capability check.

Mandatory categories:

- **Capability deny** — a token **without** `mcp:series.read:call` gets denied on the bucketed read,
  same as `mode:"rows"`; and a **mid-pan revoke** denies the next window page.
- **Workspace isolation** — a series seeded in ws-A returns **no buckets** under a ws-B token; a ws-A
  time cursor replayed under ws-B resolves nothing (workspace-first predicate, cursor carries no ws).

Slice-specific (decimation correctness):

- **Bounded budget** — a window of 1M samples with `budget:1000` returns **≤ 1000 buckets**, never the
  raw range; width derivation respects the budget.
- **Spikes survive** — the seeded spike appears in the covering bucket's **`max`** (and a seeded trough
  in `min`); assert the bucket's `avg` is far below the spike, proving `avg` alone would have hidden it.
- **Per-bucket shape** — `{t, min, max, avg, last}` all present and correct against a hand-verified small
  bucket (min ≤ avg ≤ max; `last` equals the chronologically last sample in the bucket).
- **Window paging by time cursor** — pages N adjacent windows via `next_cursor`/`prev_cursor`, asserts
  contiguous non-overlapping bucket time ranges (no skipped/duplicated bucket at a window seam), and
  `next_cursor == null` at end-of-range.
- **Index-backed / flat latency** (integration) — the deep-window page is not materially slower than the
  first (rides the `(series, ts)` index range scan, no OFFSET).

## Risks & hard problems

- **`avg`-only regression** — the single easiest thing to get wrong; the spike test is the guard. Ship
  min/max/last as the default and keep the payload budget aware of the 3–4× cost.
- **Bucket-boundary seam correctness** — a keyset over bucket time must not skip or double a bucket at a
  window edge (half-open interval discipline `[from, to)`; the cursor position is the boundary). This is
  the bucket-time analogue of slice A's tiebreaker — get the interval half-openness exactly right.
- **`time_bucket` availability / semantics** — confirm SurrealDB's bucketing function name, epoch
  alignment, and whether widths must snap to whole units; buckets must align to a stable epoch so
  adjacent windows tile without drift.
- **Empty buckets** — a gap in the series (no samples in a bucket) — decide **sparse** (omit empty
  buckets, recommended; the renderer interpolates/gaps) vs **dense** (emit a null bucket per width).
  Sparse keeps the read cheap; state it and test it.
- **Width vs budget round-off** — deriving width from a target budget can slightly over/undershoot the
  bucket count; clamp to ≤ budget and document the snapping so the client's budget is a true ceiling.
- **Aggregate cost on a cold window** — the `GROUP BY` is O(window samples); a pathologically wide
  window with a tiny budget still scans every sample. The window is bounded by the caller, but document
  the max window span before it must become an export job (§6.10).

## Open questions

- **Fixed time-bucket min/max/avg/last (recommended default) vs LTTB.** LTTB (Largest-Triangle-Three-
  Buckets) is shape-preserving and gives visibly better line-chart fidelity, but it is a point-selecting
  algorithm (returns chosen real samples, no min/max envelope) rather than an aggregate — harder to
  express as a single SurrealDB `GROUP BY` and it changes the bucket record shape. **Per-viz choice or
  one server default?** Likely the viz's `fieldConfig` picks the decimation method (see
  [`../frontend/dashboard/viz/field-config-scope.md`](../frontend/dashboard/viz/field-config-scope.md)),
  with fixed min/max/avg/last as the safe default when unset. Resolve during implementation.
- **Budget vs explicit width** — does the read take a target **point budget** (server derives width,
  recommended for the "fill 1000px" caller) or an explicit **bucket width** (caller controls, needed for
  aligned multi-series overlay), or **both**? Pick the primary; state the other's behavior.
- **Sparse vs dense empty buckets** (see risk) — recommend sparse; confirm the renderer contract with
  slice E.
- **Default & max budget**, and the max window span before the read must become an export job.
- **Aggregate set** — is `{min, max, avg, last}` enough, or does a viz need `first`/`count`/`sum`? Keep
  the default tight; grow only with a caller.

## Related

- [`page-chaining-scope.md`](page-chaining-scope.md) — the **parent**: doctrine + the one shared contract.
- [`page-cursor-scope.md`](page-cursor-scope.md) — **slice A**: the opaque cursor codec + keyset
  primitive this consumes over bucket time.
- [`series-paging-scope.md`](series-paging-scope.md) — **slice B**: `mode:"rows"` raw paging; this slice
  **builds on** it (same verb, same cursor chain).
- [`federation-paging-scope.md`](federation-paging-scope.md) — **slice D**: the decimation equivalent for
  a federated source (pushdown `GROUP BY`); referenced, not built here.
- [`page-chaining-ui-scope.md`](page-chaining-ui-scope.md) — **slice E**: the chart caller that requests
  buckets and composes them with the live `series.watch` tail.
- [`../ingest/ingest-scope.md`](../ingest/ingest-scope.md) — the `series` plane + `series.read` this grows
  ([`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs)).
- [`../frontend/dashboard/viz/`](../frontend/dashboard/viz/) — the viz `fieldConfig` that may pick the
  decimation method (LTTB vs fixed bucket).
- README `§3` (rules 2/3/5/6), `§6.1` (API shape).
