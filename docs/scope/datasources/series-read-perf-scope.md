# Datasources scope — series-read performance (pushdown decimation + batched latest)

Status: **implemented + released + consumed downstream** (2026-07-21). Ingest + host suites green
(session log `docs/sessions/datasources/series-read-perf-session.md`); **released as `node-v0.5.0`**
(minor bump — new `series.latest_many` verb, no wire break); **ems consumed it** — pins bumped to
`node-v0.5.0` and `fetchLatest` collapsed to one `series.latest_many` call (ems companion scope now
marked implemented). Live-latency timing on a real node is the one remaining verification (the
O(buckets) *shape* is test-pinned by `pushdown_is_o_buckets_not_o_rows`). Promotes to
`doc-site/content/public/datasources/` once timed.
Owning repo: **lb** (this repo). Downstream embedders (ems) consume it by bumping the `node-v*`
tag once shipped — see the ems companion scope `ems/docs/scope/series-ingest/series-fetch-fast-path-scope.md`
(the next slice of ems's `series-ingest-readiness-scope.md`, which already anticipates "ems bumps pin +
adapts `fetch-history.ts`").

A dashboard that reads the series plane is currently slow in exactly the two places a dashboard
reads it, and both are fixable without changing a single wire contract. This scope closes the gap
between what the series-decimation scope **promised** (a read-time SurrealDB `GROUP BY`) and what it
**shipped** (a chunked in-Rust fold), and adds the one missing read verb — a plural `series.latest`
— that turns a fleet snapshot from N round-trips into one. Both are additive: no new capability, and
the bucket fix keeps the existing `series.read` wire shape byte-for-byte.

## Goals

- **Bucketed `series.read` (`mode:"buckets"`) becomes O(buckets), not O(raw rows).** Push the
  decimation into a SurrealDB `GROUP BY` on a computed bucket key — the execution the
  [`series-decimation-scope`](series-decimation-scope.md) always intended — so a window backed by
  N raw samples returns ≤ budget buckets **without transferring N rows into the host**. Target: the
  measured 2.9 s single-series 24 h/10 k-sample read drops to low tens of ms; latency scales with
  the bucket count, not the sample count.
- **Preserve every semantic of the current fold, exactly.** Per-bucket `{t, min, max, avg, last,
  count}`; `min/max/avg` over numeric payloads only; `last` is the payload of the chronologically
  last sample by `(ts, seq)`; non-numeric payloads still `count` and can be `last` but never
  perturb the numeric aggregates; empty buckets omitted (sparse); `MAX_BUCKETS` cap unchanged.
- **The stored-rollup merge (post-GC history) keeps working** — the retention tier
  ([`series-retention-scope`](../ingest/series-retention-scope.md)) still fills buckets that raw no
  longer covers, re-aggregated exactly from `sum`/`count`. The pushdown replaces only the **raw**
  fold; the rollup merge composes on top unchanged.
- **Add `series.latest_many { series: [String] }` → one round-trip for a multi-series snapshot.**
  A fleet "now" view (client board KPIs, per-point freshness) currently fans out one
  `series.latest` per series; each pays a full authorize + query round-trip (~290 ms floor
  measured). One verb, one query, one authorize → the fan-out collapses.
- **No new capability, no wire break.** `series.read` grows nothing; `series.latest_many` reuses
  the existing `mcp:series.latest:call` grant, checked once for the batch.

## Non-goals

- **Changing the bucket wire shape or the budget→width derivation.** `{t, min, max, avg, last,
  count}`, sparse buckets, `MAX_BUCKETS`, and `effective_width` are unchanged — this is an
  execution swap behind an identical contract. LTTB stays deferred (decimation scope OQ).
- **The time-cursor window chain.** Forward/back window paging belongs to
  [`series-decimation-scope`](series-decimation-scope.md) / the cursor primitive; this scope changes
  how one window is aggregated, not how windows chain.
- **Federated pushdown** (Timescale `time_bucket`) — that is the federation slice; this executes the
  native-SurrealDB engine only.
- **A plural `series.read`** (many series' trends in one call). The trend path is already concurrent
  and per-series bounded; batching *reads* is a larger contract (per-series windows/budgets) with no
  demonstrated caller. `series.latest_many` is scoped because the snapshot fan-out is the measured
  pain; a plural trend read is explicitly deferred until a caller needs it.
- **Materialized read-time rollup tables.** Rule 2 stands — decimation stays a read-time aggregate.
  The only stored tier remains GC's post-eviction rollup (the sole surviving copy, not a cache).

## Intent / approach

**Two SurrealDB queries per bucketed window, both index-range scans, both O(buckets) out.** The
current handler ([`crates/ingest/src/bucket.rs`](../../../rust/crates/ingest/src/bucket.rs)) pages
every raw row of the window into the host and folds them into a `BTreeMap<bucket, Acc>` in Rust —
so a 10 k-sample window moves 10 k rows over the store boundary to emit ~240 buckets. Replace the
fold with a grouped aggregate computed **where the data lives**:

- **Query N (numeric aggregates):** `SELECT math::floor((time::millis(ts) - $from)/$width) AS b,
  count() AS num_count, math::min(payload) AS min, math::max(payload) AS max, math::sum(payload) AS
  sum FROM series WHERE series=$s AND type::is::number(payload) AND ts>=… AND ts<… GROUP BY b`.
  SurrealDB's `math::*` aggregate only numeric values; the `type::is::number(payload)` predicate
  makes `num_count` the numeric count, so `avg = sum/num_count` is exact.
- **Query L (count + exact last):** `SELECT b, count() AS count, array::last(p) AS last, array::last(t)
  AS last_ts FROM (SELECT math::floor((time::millis(ts)-$from)/$width) AS b, payload AS p,
  time::millis(ts) AS t, seq FROM series WHERE series=$s AND ts>=… AND ts<… ORDER BY t ASC, seq ASC)
  GROUP BY b`. The ordered subquery makes `array::last` the chronologically last payload by
  `(ts, seq)` — the exact `last` the fold guaranteed, including a non-numeric `last`. `count()` here
  is the **total** sample count (numeric + non-numeric).

Merge the two by bucket index `b` in the host (a `BTreeMap<b, …>` join — O(buckets), not O(rows)),
map `b`→`t = from + b*width`, then run the **existing** rollup merge for the post-GC tail. The host
still owns width/budget derivation (`effective_width`) and the final `Bucket` shape.

**Why the original scope's blocker was a false negative.** The decimation scope shipped the fold
because "SurrealDB 2 has no ordered `last` aggregate" and because a naive `GROUP BY` can't tolerate
non-numeric payloads. Both are now disproven against the pinned engine (SurrealDB 2.6.5, verified
with a throwaway probe on the real `mem://` store): `array::last()` over an ordered subquery **is**
an exact ordered `last`, and `math::min/max/sum` **skip** non-numeric values natively (so a mixed
bucket aggregates its numbers and still counts/carries its non-numbers). The two-query split is what
buys both properties in one pushed-down read — the single missing insight in 2026-07-14's
implementation. This scope treats the fold as a **performance regression against its own scope's
stated goal**, not a design choice, and finishes the job the decimation scope set out to do.

**`series.latest_many` — one query for the whole snapshot.** `series.latest` is one authorize + one
`ORDER BY ts DESC, seq DESC LIMIT 1` per series ([`crates/ingest/src/latest.rs`](../../../rust/crates/ingest/src/latest.rs)).
A snapshot of K series is K of those. Add `latest_many(store, ws, &[String]) -> Vec<(String,
Option<Sample>)>`: one `WHERE series IN $names` scan grouped to the top sample per series
(`ORDER BY ts DESC, seq DESC` then first-per-group), returned as a name→sample map. One round-trip,
one authorize. The wire result is `{ latest: { "<name>": Sample|null, … } }` — a null entry means
"no committed sample yet" (never an error), mirroring single `series.latest`'s null contract, and
every requested name appears (absent series → null) so the caller needs no reconciliation.

**Rejected — client-side or host-side caching of latest values.** A device-shadow cache is durable
state in a stateless plane (rule: stateless extensions), goes stale under late samples, and adds an
invalidation problem the query doesn't have. The query is already one indexed seek per series;
batching removes the per-call *round-trip* floor without any cache. **Rejected — a SurrealDB view /
continuous aggregate for buckets** (materialized): rule 2, and it re-introduces the write-time width
choice the read-time aggregate exists to avoid. **Rejected — widening `series.read` with a `series:
[String]` array** to also batch trends: no measured caller, and per-series windows/budgets make the
contract materially bigger; deferred to a real need.

## How it fits the core

- **Tenancy / isolation:** unchanged and load-bearing. Both bucket queries and `latest_many` are
  `store.query_ws(ws, …)` — the workspace is host-set from the token and scopes the namespace before
  any `GROUP BY`/`IN`. A ws-B token can only ever aggregate ws-B series; a `series IN $names` list
  from ws-A resolves to nothing under ws-B. Mandatory isolation test below.
- **Capabilities:** **no new cap.** Bucketed `series.read` stays on `mcp:series.read:call`;
  `series.latest_many` reuses `mcp:series.latest:call`, authorized **once** for the batch (the batch
  is one logical read of one series-latest surface, not K grants). A principal without the grant is
  denied the whole batch — it cannot read one series' latest it couldn't read singly. Deny test below.
- **Placement:** `either`, no `if cloud`. Both are plain workspace-scoped SurrealDB reads; they run
  identically on an edge node or the cloud head-end. Which series exist is grants/config, not a branch.
- **MCP surface (§6.1):** get/list-shaped reads only.
  - `series.read` — **unchanged surface**, faster engine. No new field, no new verb, same
    `{buckets, …}` response. A guest calling it today gets the speed-up for free.
  - `series.latest_many` — **new list-shaped read verb** taking `{ series: [String] }`, returning
    `{ latest: { name: Sample|null } }`. Bounded, always-fast (one indexed query over a caller-sized
    name list), so it is a **synchronous** read, not a job — the bound is the caller's `series[]`
    length, which is the rendered point set, not the raw sample count. No CRUD (reads write nothing),
    no new live feed (the forward tail stays `series.watch`).
- **Data (SurrealDB):** no new table, no stored aggregate. Bucketing becomes a read-time `GROUP BY`
  over the existing `series` table riding the `(series, ts)` index (`series_ts_idx`) for the window;
  `latest_many` is one indexed `WHERE series IN …` read. State plane only.
- **Bus (Zenoh):** none. State-vs-motion holds — these page committed state; the live edge stays
  `series.watch`.
- **Sync / authority:** reads committed local state; no new authority. Offline reads whatever the
  local node holds, same as any series read.
- **Secrets:** none.
- **No mocks:** both are proven against a real `mem://` SurrealDB store seeded via the real
  `write`+`commit_batch` path (the probe already did this for the query shapes). No fake store.
- **SDK/WIT impact:** `series.read` — none (additive-nothing). `series.latest_many` — one new MCP
  verb name in the catalog + dispatch; no host-callback, no ABI break. Flag: guests gain the batch
  verb by name; existing `series.latest` callers are untouched.
- **Skill doc:** **N/A.** This adds no new agent-/API-drivable *surface* — `series.read` is unchanged,
  and `series.latest_many` is a mechanical batch of the already-documented `series.latest` contract
  (the [`../mcp/ems-provisioning-verb-shapes-scope.md`](../mcp/ems-provisioning-verb-shapes-scope.md)
  verb-shapes doc records its wire shape). No `skills/<name>/SKILL.md` is owed.
- **One responsibility per file:** the bucket pushdown is a new store method beside the fold in
  `bucket.rs` (or a sibling `bucket_query.rs` if `bucket.rs` nears the 400-line gate); `latest_many`
  is a function beside `latest` in `latest.rs`; the host wrappers mirror the existing
  `series_*_value` in `crates/host/src/ingest/read.rs`; dispatch/catalog get one arm/entry each.

## Example flow

**A — a meter trend board (the 2.9 s case).** A `TrendWidget` reads 24 h of one series (~10 k raw
samples) at budget 240.

1. Caller: `series.read {series, mode:"buckets", from, to, budget:240}` under a token with
   `mcp:series.read:call`.
2. Host authorizes workspace-first, then the cap. Passing. Derives `width = span/240` via
   `effective_width` (unchanged).
3. Host issues **Query N** and **Query L** (above), both `query_ws(ws, …)`, both scanning only the
   `[from, to)` range on `series_ts_idx`. SurrealDB returns ~240 numeric-aggregate rows and ~240
   count/last rows — **not** 10 k raw rows.
4. Host joins the two by bucket index (O(240)), computes `avg = sum/num_count`, maps `b`→`t`, then
   merges the stored rollup tier for any post-GC part of the window (unchanged).
5. Response: the identical `{buckets:[…≤240…]}` the fold produced — same numbers, same spikes in
   `max`, same sparse gaps — in low tens of ms instead of 2.9 s.

**B — a client board snapshot (the fan-out case).** A `ClientDashboardPage` shows one KPI per point
across a site's meters — say 30 series.

1. Caller: `series.latest_many {series:[…30 names…]}` under a token with `mcp:series.latest:call`.
2. Host authorizes once. Runs one `WHERE series IN $names` top-per-series query.
3. Response: `{ latest: { "<name>": {payload, ts, …} | null, … } }` — all 30 in one ~single-digit-ms
   round-trip, versus 30 × ~290 ms fanned out.

## Testing plan

Per [`scope/testing/testing-scope.md`](../testing/testing-scope.md). **No mocks** — real `mem://`
SurrealDB, seeded a large real series via `write`+`commit_batch` (the probe harness in
`crates/ingest/tests/series_plane_test.rs` is the model). Real host, real capability check.

Mandatory categories:

- **Capability deny** — a token without `mcp:series.read:call` is denied the bucketed read
  (unchanged); a token without `mcp:series.latest:call` is denied the whole `series.latest_many`
  batch. A mid-read revoke denies the next call.
- **Workspace isolation** — a series seeded in ws-A returns **no buckets** and **null latest** under
  a ws-B token; a `series IN [ws-A names]` batch under ws-B resolves every name to null (the
  namespace predicate is workspace-first; the name list carries no grant).

Slice-specific (correctness = the pushdown is behaviourally identical to the fold):

- **Fold-parity, exhaustive** — the headline test. Seed a series with a mixed window (numeric +
  non-numeric payloads, a deliberate in-bucket spike, two producers sharing a `ts` broken by `seq`,
  an empty bucket gap). Assert the **new pushdown** returns a `Vec<Bucket>` **byte-identical** to the
  **old fold** for the same query: same `t` set (sparse gaps preserved), `min ≤ avg ≤ max`, `avg =
  sum/num_count` exact, `count` = total incl. non-numeric, `last` = payload at max `(ts, seq)`
  incl. a non-numeric `last`, spike present in the covering bucket's `max` while its `avg` sits far
  below. (Keep the fold available as the oracle for this test even after the handler switches.)
- **Rollup merge intact** — the existing `retention_gc_rolls_up_then_evicts_and_buckets_merge_rollups`
  test passes unchanged: post-GC buckets still fill from the stored tier, exact.
- **`series.latest_many` shape** — every requested name present; a series with samples returns its
  chronologically-newest (`ts` then `seq`) sample; an unknown/empty series returns `null`; a
  non-numeric latest is returned verbatim. Result equals mapping single `series.latest` over the
  same names (parity oracle).
- **Performance (integration, the regression guard)** — seed ≥10 k samples in one window; assert the
  bucketed read completes well under a fixed budget (e.g. ≤200 ms on the dev box) AND that its
  latency is flat as the sample count grows 10×/100× at fixed budget (O(buckets), not O(rows)). This
  is the test whose absence let the fold ship against the scope's own goal — it makes the perf
  contract executable, not aspirational.

## Risks & hard problems

- **Silent semantic drift from the fold.** The whole risk surface is "the GROUP BY is subtly not the
  fold." The fold-parity test (running both on the same seed and diffing bucket-for-bucket) is the
  guard; it must cover non-numeric payloads, the `(ts, seq)` last tiebreaker, sparse gaps, and the
  numeric-vs-total count split — the exact corners the two-query split exists to preserve.
- **`array::last` ordering guarantee.** `array::last` returns the last element *as grouped*; the
  ordered subquery (`ORDER BY t ASC, seq ASC`) is what makes that chronological. If a future
  SurrealDB bump reorders grouped input, `last` silently regresses — pin the behaviour with the
  parity test and a comment citing the engine version verified (2.6.5).
- **Bucket-index vs floor alignment.** The fold floors `ts` to `width` boundaries; the pushdown keys
  on `floor((ts-from)/width)`. These must produce the **same** bucket set for the same window — the
  `from`-relative index and the absolute floor differ by a constant offset that the `b`→`t` mapping
  must invert exactly (`t = from + b*width`, and `from` itself need not be width-aligned). Test a
  window whose `from` is **not** width-aligned to catch an off-by-one seam.
- **Two queries vs one transaction.** Query N and Query L are two reads of the same committed window;
  a concurrent commit between them could in principle show a sample in L's count but not N's
  aggregate. Mitigate by reading both in one `query_ws` multi-statement call (one snapshot) — verify
  `query_ws` returns both result sets from a single `.query(a; b)` and `take(0)/take(1)` them.
- **`type::is::number` vs the fold's `as_f64`.** The fold counts a payload as numeric via serde
  `as_f64()`; the query via `type::is::number(payload)`. A JSON number that is a string-encoded
  number (`"12.3"`) is numeric to neither — but confirm booleans and integer-vs-float agree between
  the two, or the parity test will (correctly) fail. Align the predicate to `as_f64`'s exact notion.

## Open questions

None — this scope is written to have none. The three decisions that would otherwise be open are
**resolved here**:

1. *Pushdown vs keep the fold* → **pushdown**, because the decimation scope's stated goal was the
   `GROUP BY` and its only cited blocker (ordered `last`, non-numeric tolerance) is disproven against
   the pinned engine (verified 2026-07-21 on SurrealDB 2.6.5).
2. *One query or two* → **two, in one `query_ws` snapshot** (numeric aggregates; count+ordered-last),
   because a single statement cannot both skip non-numerics for `math::*` and carry a non-numeric
   `last` — the split is the minimal shape that preserves every fold semantic.
3. *`series.latest_many` result shape* → **a `{ latest: { name: Sample|null } }` map with every
   requested name present**, so the caller does no reconciliation and a missing series is an explicit
   `null` (parity with single-latest's null-not-error contract).

## Related

- [`series-decimation-scope.md`](series-decimation-scope.md) — the parent contract this **finishes**:
  it defined `mode:"buckets"` and *intended* the read-time `GROUP BY`; this scope supplies the
  execution its implementation deferred to an in-Rust fold.
- [`series-paging-scope.md`](series-paging-scope.md) — the raw-row `mode:"rows"` slice + the
  `(series, ts)` index both this scope's queries ride.
- [`page-chaining-scope.md`](page-chaining-scope.md) — the parent doctrine + the shared cursor/budget
  contract (unchanged here).
- [`../ingest/series-retention-scope.md`](../ingest/series-retention-scope.md) — the stored rollup
  tier the bucket merge composes with (untouched by the raw-fold swap).
- [`../ingest/ingest-scope.md`](../ingest/ingest-scope.md) — the `series` plane, `series.read`,
  `series.latest` these grow ([`crates/host/src/ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs),
  [`crates/ingest/src/bucket.rs`](../../../rust/crates/ingest/src/bucket.rs),
  [`crates/ingest/src/latest.rs`](../../../rust/crates/ingest/src/latest.rs)).
- ems companion: `ems/docs/scope/series-ingest/series-fetch-fast-path-scope.md` — the downstream
  consumer (collapse the `series.latest` pool to `series.latest_many`; inherit the bucket speed-up on
  a tag bump), the next slice of ems's `series-ingest-readiness-scope.md`.
- [`../mcp/ems-provisioning-verb-shapes-scope.md`](../mcp/ems-provisioning-verb-shapes-scope.md) —
  the closed `series.latest`/`series.read` wire-shape contract (issues #48/#60). Its Open Questions
  pre-declare that a **new** `series.latest`-family verb "is a new lb-core scope, not a correction to
  this one" — this scope **is** that new scope for `series.latest_many`; the shapes doc's table is
  left untouched.
- README `§3` (rules 2 One-datastore / 3 State-vs-motion / 5 Capability-first / 6 Workspace-hard-wall),
  `§6.1` (Data store — SurrealDB).
