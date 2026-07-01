# Datasources scope — page-chaining (keyset cursor paging for large timeseries)

Status: scope (the ask). Promotes to `public/datasources/datasources.md` once shipped.

We want reads over **large timeseries** — a series with millions of samples, a federated warehouse table,
a raw data-console scan — to **load a page at a time, fast, and chain to the next page with a cursor**,
instead of pulling the whole range into memory or paying an `OFFSET` scan that gets slower every page.
Today `series.read` returns an unbounded `Vec<Sample>` for a `seq` range ([`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs)); a big series
either OOMs the call or stalls the dashboard. This scope defines **one paging contract** — an opaque
**keyset cursor** + a bounded `limit`, returning `{rows, next_cursor}` — and settles *which engine runs
it*: **SurrealDB pages the platform state plane (the fast path); DataFusion/federation only pages by
pushing the keyset predicate down to the real source, and anything that must load at dashboard speed is
mirrored into the series plane and paged there.**

> Read with: `datasources-scope.md` (the `federation` extension + the **federate-vs-mirror** doctrine this
> extends), `../ingest/ingest-scope.md` (the `series` plane + `series.read`/`series.latest`/`series.watch`
> this modifies), `../query/prql-query-scope.md` (`query.run` over `store.query`/`federation.query` — a
> saved query pages the same way), `../frontend/data-console-scope.md` + `../frontend/dashboard/viz/`
> (the two callers: a raw table and a chart), README §3 (rules 2/3/5/6), §6.1 (API shape), §6.10 (jobs).

---

## The decision this scope exists to make: DataFusion or SurrealDB?

**Neither is "the pager." Paging is a contract; the engine that owns the data runs it.** Two roles, same
split as `datasources-scope.md`:

1. **SurrealDB = the paging engine for anything that must load fast.** It holds the platform state plane —
   `series`, store tables — with an index on the natural order key (`(series, seq)` / `(ts, id)`). A
   **keyset** page (`WHERE key < cursor ORDER BY key DESC LIMIT n`) is an index seek: O(page), not
   O(offset). Every dashboard chart, every data-console table, every rule backfill pages here.
2. **DataFusion/federation = the analytics + federation engine, not a low-latency pager.** It is superb at
   scan/join/aggregate across external sources, but it materializes/streams result sets and depends on the
   **connector's predicate pushdown** to page cheaply. So `federation.query` pages **only** by pushing the
   keyset predicate down to the underlying source (Timescale/Postgres do; a CSV/DuckDB table may not). A
   source that can't push down is **not** live-paged for a dashboard — it is **mirrored** (the existing
   `federation.mirror` `lb-jobs` batch) into the series plane, then paged by SurrealDB at index speed.

So the answer to "fast page loads over lots of timeseries": **page the series plane in SurrealDB with a
keyset cursor.** DataFusion never becomes the thing between the user and a fast chart — that would tie every
page to a re-run federated query and the vagaries of pushdown. This is the federate-vs-mirror doctrine
applied to paging: *federate for fresh/ad-hoc (pushdown-paged); mirror for fast/repeated (keyset-paged).*

**Rejected — DataFusion as the uniform pager over both native and external.** Tempting for one code path,
but it would demote SurrealDB's index-backed series read to a DataFusion table scan, add a query-planner
hop to every page, and make dashboard latency hostage to connector pushdown. Core stays lean; the fast path
stays native. **Rejected — `LIMIT/OFFSET` paging.** O(offset) scan-and-discard degrades on exactly the big
series this is for, and it's unstable under head-appends (a new sample shifts every page). Keyset is
append-stable and O(page) — the right primitive for append-mostly timeseries.

## Goals

- **One paging contract, engine-agnostic at the MCP surface:** a read takes `limit` + an opaque `cursor`
  (a `before`/`after` bookmark) and returns `{rows, next_cursor}` (and `prev_cursor` where bidirectional).
  `next_cursor == null` means end-of-range. The client **chains** pages by echoing the cursor back — it
  never constructs one.
- **Keyset, not offset.** The cursor encodes the **position key** (the last row's composite sort key, e.g.
  `(ts, seq)` — a unique tiebreaker so no row is skipped/duplicated on ties), not a row number. Stable
  under concurrent head-appends (the timeseries norm).
- **Additive to the existing read verbs — no new capability.** `series.read`, `store.query`,
  `federation.query`, and `query.run` grow `limit`/`cursor` params and a `next_cursor` in the result.
  Paging is a *parameter of a read you can already do*, gated by the **same** read cap (`mcp:series.read:call`
  et al.). No `page` verb, no `paging:*` grant.
- **A chart doesn't page raw points — it downsamples.** For a viz over a huge series, the fast-load answer
  is **server-side time-bucket decimation** to a bounded point budget for the visible window, with the
  *window* itself chained by a time cursor. Tabular callers page raw rows; chart callers page decimated
  buckets. Same cursor shape, two `mode`s.
- **The cursor is a bookmark, not a capability.** Every page **re-authorizes** workspace-first then the read
  cap; the workspace and series come from the **token/request**, never decoded from the cursor. A ws-A
  cursor replayed in ws-B resolves nothing and is denied — the wall is the token, re-checked per page.

## Non-goals

- **Total counts / "page 42 of 1000".** Counting a giant series is O(series) and defeats the point. Page-
  chaining is forward/back cursoring; the UI shows "load more"/infinite scroll, not a numbered pager. If a
  bounded count is ever needed it's a separate, explicitly-capped read.
- **Unbounded export as a paged call.** Pulling a whole multi-million-row range is a **mirror/export
  `lb-jobs` job** (returns a job id, resumable — `federation.mirror`, §6.10), not a client looping
  `next_cursor` thousands of times. Paging serves *interactive* windows; bulk movement is a job. (Stated
  bound: a `limit` cap per page and a soft page-count ceiling in the client.)
- **DataFusion as the primary pager** (rejected above) and **offset paging** (rejected above).
- **Making a federated non-pushdown source load fast live.** Its answer is *mirror then page*, not a
  cleverer live cursor. We don't paper a slow source with pagination.
- **A new time-series database.** Rule 2 holds — no Timescale/Influx *inside* the platform; the series plane
  in SurrealDB is the fast store, and Timescale (etc.) is a federated/mirrored source (`datasources-scope.md`).

## Intent / approach

**A cursor codec + a keyset predicate builder, shared by every read that pages.** Two small,
single-responsibility pieces (FILE-LAYOUT): a `cursor` file that encodes/decodes the opaque bookmark
(base64 of the composite position key + sort direction + a `mode` tag), and a `keyset` file that turns
`(cursor, limit, direction)` into the `WHERE key ⋛ cursor ORDER BY key LIMIT n+1` predicate (fetch `n+1` to
know if a `next_cursor` exists without a count). `series.read` runs this against the `series` index;
`store.query`/`federation.query` compose it into the SQL (SurrealDB predicate, or a DataFusion pushdown
filter for a federated source). One contract, three call sites, no engine branch at the surface.

**Downsampling is a read `mode`, executed where the data lives.** For `mode:"buckets"`, the series read
groups by a time bucket (`time_bucket(width)` — SurrealDB `GROUP BY` on the series plane; the DataFusion
equivalent for a federated source) and returns per-bucket `{t, min, max, avg, last}` so spikes survive the
decimation (a plain `avg` hides the exact peak a temperature alert cares about). The point budget (buckets
per window) is bounded by the request; the *window* pages by time cursor. This is the real "million points
into a 1000px chart, fast" answer — and it's the same keyset chain, just over buckets instead of rows.

**Why the cursor needs no signing.** A cursor carries only a position key; it grants nothing. Because every
page re-runs the full workspace-first + capability check with the *token's* ws (the cursor's contents are
never trusted as scope), a tampered or replayed cursor can at worst point at a different position *within
what the caller may already read* — never across the wall. So: plain base64 keyset, no HMAC. (Recorded as
an open question in case we want tamper-evidence for audit rather than security.)

**Live tail stays motion; paging is the historical state read (rule 3).** `series.watch` (Zenoh) is the
live forward edge; page-chaining walks *backward through committed state* in SurrealDB. They compose
exactly as the dashboard wants: **subscribe forward for new samples, page backward to backfill history** —
motion and state each doing their own job, never one faking the other.

## How it fits the core

- **Tenancy / isolation:** the workspace is host-set from the token on **every** page; the cursor never
  supplies it. A ws-A `next_cursor` used in ws-B resolves no series and is denied. The keyset predicate is
  applied *inside* the ws-scoped store namespace, so a cursor can't walk past the wall even within one call.
  Mandatory isolation test: ws-B replays a ws-A cursor → empty/deny, across `series.read` + `store.query` +
  `federation.query`.
- **Capabilities:** **no new cap.** Each paged read keeps its existing gate (`mcp:series.read:call`,
  `mcp:store.query:call`, `mcp:federation.query:call`, `mcp:query.run:call`), authorized workspace-first
  per page. The deny path is unchanged and re-asserted per page (a mid-chain grant revocation denies the
  next page — tested).
- **Placement:** `either`, no `if cloud`. The same keyset code runs on an edge node paging its local series
  and on a cloud node paging a mirrored warehouse. Which *sources* exist is config (the datasource grants),
  not a code branch.
- **MCP surface (§6.1 — judged):**
  - **Get / list (the core change):** `series.read` grows `{limit, cursor, direction, mode}` and returns
    `{rows|buckets, next_cursor, prev_cursor}`. `store.query`/`federation.query`/`query.run` grow the same
    `{limit, cursor}` and echo `next_cursor`. All are **reads** — same verbs, same caps, additive fields.
  - **CRUD:** N/A — paging adds no writes.
  - **Live feed:** unchanged — `series.watch` (Zenoh) is the forward tail; paging is the backward
    historical read. Named together so callers compose them, not conflate them.
  - **Batch → a job:** an unbounded pull is **not** a page loop — it stays `federation.mirror` /
    an export `lb-jobs` job (resumable, returns a job id, §6.10). Paging explicitly serves interactive
    windows only; the bound is a per-page `limit` cap.
- **Data (SurrealDB):** no new table. The change is an **index** guarantee on the paging key
  (`series` already keys `(series, seq)`; store tables page on their PK/`ts`) and a keyset predicate. The
  series plane stays the one datastore; external rows page in the extension via pushdown (or are mirrored
  in first). Bucket decimation is a `GROUP BY` read, not stored aggregates.
- **Bus (Zenoh):** none from a page (it's a state read). The composed live tail uses the existing
  `series.watch`/`publish_sample` motion; no new subject.
- **Sync / authority:** SurrealDB is authority on every node (rule 2). A page is a node-local index read of
  committed state; keyset stability means a page taken before a sync merge stays valid after (new samples
  append past the cursor, they don't renumber it). A federated live page is fresh-but-node-local; the
  mirror path is the durable/offline one.
- **Secrets:** N/A — paging touches no secret; the federated DSN mediation is unchanged (`datasources-scope.md`).
- **SDK/WIT impact:** additive fields on existing MCP read verbs — no new verb, no new host-callback, no ABI
  break. A guest/extension that already calls `series.read` gains paging for free by sending `limit`/`cursor`.

## Example flow

A dashboard chart and a data-console table both load fast over a 5-million-sample cooler-temp series.

1. **Chart, first window.** The viz cell calls `series.read {series:"cooler.temp", direction:"back",
   limit:500, mode:"buckets", bucket:"1m", window:"-6h"}`. Host authorizes `mcp:series.read:call`
   workspace-first, resolves the ws from the token, runs a keyset `GROUP BY time_bucket('1m')` on the
   `(series, ts)` index → 360 buckets `{t,min,max,avg,last}`, returns them + `next_cursor` (the oldest
   bucket's key). ~one index seek, no full scan, spikes preserved (min/max).
2. **Chart, pan back.** User scrolls left; the cell re-calls with `cursor = next_cursor`. Same auth, keyset
   seek continues *older* from the bookmark — O(page), identical latency to page 1. Chained, not offset.
3. **Live edge.** Meanwhile the cell holds a `series.watch` subscription; new samples arrive as **motion**
   and append at the right edge. Backfill = paging (state), live = watch (motion) — composed, rule 3 intact.
4. **Table (raw rows).** The data-console opens the same series in `mode:"rows"`, `limit:100`. It pages raw
   samples with a `(ts, seq)` keyset cursor; "load more" echoes `next_cursor`. No count, no offset, stable
   as new samples land.
5. **Federated source.** An analyst pages a Timescale table via `federation.query {source:"tsdb",
   sql:"SELECT … ORDER BY ts", limit:100, cursor}` — the keyset predicate is **pushed down** to Timescale,
   so each page is an indexed read *on the warehouse*. The same table exposed over a non-pushdown connector
   is instead **mirrored** (`federation.mirror`) into `cooler.temp` and paged as in steps 1–4.
6. **Deny / isolation:** a ws-B caller replays ws-A's `next_cursor` → resolves nothing in ws-B → denied,
   opaque. Revoking `mcp:series.read:call` mid-chain → the next page is denied. Neither leaks the wall.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks** (CLAUDE §9): a **real**
`mem://` store **seeded with a large real series** (tens of thousands of samples — enough that offset would
visibly lose to keyset), the real host caps check, the real gateway; the federated case uses the one
sanctioned external-boundary (a **real spawned** Postgres/Timescale container, seeded, behind the `Source`
trait — `datasources-scope.md`), not an in-process re-implementation.

- **Capability-deny (§2.1):** each paged read denied without its existing read cap; a grant **revoked
  mid-chain** denies the next page (the cursor is not a bypass).
- **Workspace-isolation (§2.2):** ws-B replaying a ws-A cursor gets empty/deny on `series.read`,
  `store.query`, and `federation.query`; the keyset predicate cannot walk past the ws namespace even within
  one call. The cursor carries no ws authority (assert it's ignored as scope).
- **Keyset correctness:** paging a seeded N-sample series with `limit=k` yields **every** sample exactly
  once, in order, no gaps/dupes — including across a **tie** on the sort key (the `(ts, seq)` tiebreaker),
  and with **concurrent head-appends** during the walk (older pages unaffected — the append-stability
  property offset lacks). `next_cursor == null` exactly at end-of-range.
- **Decimation:** `mode:"buckets"` returns ≤ the requested point budget, preserves per-bucket **min/max**
  (a seeded spike inside a bucket shows in `max`, not smoothed away by `avg`), and pages windows by time
  cursor.
- **Performance (the whole point):** page-1 latency and per-page latency are **flat as depth grows**
  (assert page 1 ≈ page 500 within a band), and bounded memory (a page never materializes the full range) —
  vs a control offset read that degrades. This is the regression that proves "fast page loads."
- **Federation pushdown vs mirror:** against the real container, a keyset `federation.query` **pushes the
  predicate down** (assert via the connector/plan, not a full-scan-then-slice); a non-pushdown source is
  proven to require the **mirror** path (documented, tested that mirror→series pages fast).
- **Frontend (real gateway):** the data-console table and a dashboard viz cell page + "load more" over the
  bridge (`*.gateway.test.tsx`) against a real spawned node + seeded series; live `series.watch` composes
  with backward paging without double-rendering the seam.

## Risks & hard problems

- **Tiebreaker discipline.** A keyset on a **non-unique** sort key (two samples at the same `ts`) skips or
  repeats rows unless the cursor includes a unique tiebreaker (`seq`/`id`). Every paged read must sort on a
  **unique composite**; a viz-only `ts` sort is a latent bug. This is the single easiest thing to get wrong.
- **Federation pushdown is per-connector.** `datafusion-table-providers` push predicates for SQL sources but
  not every provider (file/DuckDB). Silent fallback to full-scan-then-limit reintroduces the O(offset)
  problem *and* hides it. Mitigation: detect/assert pushdown; where absent, **refuse to live-page** and
  route to mirror — never quietly serve a slow page as if it were fast.
- **Cursor stability across schema/order changes.** A cursor encodes a specific sort key; if a later read
  changes the order or the key's type, an old cursor is meaningless. Version the cursor (`mode`+key layout)
  and reject/ignore an incompatible one cleanly (restart the chain) rather than mis-seek.
- **Decimation semantics.** `avg`-only buckets hide spikes an alert cares about; min/max/last per bucket is
  the safe default, but it's 3–4× the payload of a single value — the point budget must account for it.
  Whether LTTB (shape-preserving) beats fixed time-buckets is viz-dependent (open question).
- **Deep back-pages under retention/compaction.** If old samples are pruned mid-chain, a `next_cursor`
  pointing into pruned range must terminate cleanly (end-of-range), not error.

## Open questions

- **Cursor encoding:** plain base64 keyset (recommended — every page re-authorizes, so the cursor grants
  nothing) vs HMAC-signed for **tamper-evidence in audit** (not security). Do we need the audit property?
- **One shape or two verbs:** extend `series.read` with `mode:"rows"|"buckets"` (recommended — additive, no
  new cap) vs a distinct `series.query` for the aggregated/decimated read. Decide before the first client.
- **Bidirectional paging:** ship `direction:"back"` only (backfill/infinite-scroll-up, the dashboard need)
  first, or `prev_cursor`/`after` from day one for a table that scrolls both ways?
- **Decimation algorithm:** fixed time-bucket min/max/avg/last (recommended default) vs LTTB (shape-optimal
  for line charts) — per-viz choice, or one server default? Does viz's `fieldConfig` pick it?
- **Default & max `limit`** per read, and the client's soft page-count ceiling before it must offer "export
  as a job" instead of more pages.
- **Where the keyset/cursor code lives:** a shared `lb-ingest`/`lb-store` helper vs per-verb — must satisfy
  FILE-LAYOUT (one `cursor` file + one `keyset` file, reused), decide the crate.

## Related

- `datasources-scope.md` — the `federation` extension + **federate-vs-mirror** doctrine this extends to
  paging (federate = pushdown-paged; mirror = keyset-paged in the series plane); `federation.query`/
  `federation.mirror`, `datasource.*`.
- `../ingest/ingest-scope.md` — the `series` plane and `series.read`/`series.latest`/`series.watch` this
  modifies; the `(series, seq)` key the keyset seeks on. Code: [`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs).
- `../query/prql-query-scope.md` — `query.run` over `store.query`/`federation.query` pages via the same
  contract; a saved query gains `limit`/`cursor` for free.
- `../frontend/data-console-scope.md` — the raw-table caller (rows mode); `../frontend/dashboard/` +
  `../frontend/dashboard/viz/` — the chart caller (buckets/decimation mode) and `fieldConfig` unit render.
- README `§3` (rules 2 one-datastore, 3 state-vs-motion, 5 caps-first, 6 workspace wall), `§6.1` (API
  shape — get/list vs batch-as-job), `§6.10` (jobs — the export/mirror alternative to a page loop).
</content>
</invoke>
