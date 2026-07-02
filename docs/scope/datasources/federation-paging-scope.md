# Datasources scope — federation paging (pushdown-page or route to mirror)

Status: scope (the ask) — **child slice D** of [`page-chaining-scope.md`](page-chaining-scope.md).
Promotes to `public/datasources/datasources.md` once shipped.

We want the read verbs over **external and saved sources** — `federation.query` (live external SQL via
DataFusion), `store.query` (native SurrealDB tables), and `query.run` (saved PRQL,
[`../query/prql-query-scope.md`](../query/prql-query-scope.md)) — to **page with the same
`{limit, cursor}` contract** the series plane uses, and to page **fast**. The catch is that a federated
source only pages cheaply when the keyset predicate is **pushed down** to the underlying database; a
source that can't push down would fall back to a full scan and O(offset) discard — the exact cost this
whole feature exists to kill. So this slice's job is to **detect pushdown, page live when it's real, and
refuse to live-page when it isn't** — routing a non-pushdown source to the existing **mirror** path
([`series-paging-scope.md`](series-paging-scope.md) then keyset-pages it in the series plane at index
speed). This is the [`datasources-scope.md`](datasources-scope.md) **federate-vs-mirror** doctrine applied
to paging.

> Read with: [`page-chaining-scope.md`](page-chaining-scope.md) (the parent doctrine + the one shared
> contract), [`datasources-scope.md`](datasources-scope.md) (the `federation` extension + federate-vs-mirror
> this builds on — **not** re-scoped here), [`page-cursor-scope.md`](page-cursor-scope.md) (slice A — the
> cursor/keyset primitive this consumes), [`series-paging-scope.md`](series-paging-scope.md) (slice B — the
> mirror's keyset-paged target), [`../query/prql-query-scope.md`](../query/prql-query-scope.md) (`query.run`
> pages the same way), [`../ingest/ingest-scope.md`](../ingest/ingest-scope.md) (the mirror's `ingest.write`
> target), [`../jobs/jobs-scope.md`](../jobs/jobs-scope.md) (the resumable mirror job). README `§3`
> (rule 2), `§6.1` (API shape), `§6.10` (jobs).

---

## Goals

- Grow **`federation.query`**, **`store.query`**, and **`query.run`** with the shared `{limit, cursor,
  direction}` params, echoing `{rows, next_cursor}` — the same opaque-keyset contract slices A/B define, so
  a caller (and slice E) pages every read the same way regardless of source.
- **`store.query` pages like the series plane** — native SurrealDB tables keyset-page over their order key
  (`WHERE key < cursor ORDER BY key DESC LIMIT n`), O(page), reusing slice A's predicate directly. This is
  the easy, always-fast case.
- **`federation.query` pages *only* by pushdown.** The keyset predicate + `LIMIT` must be **pushed down**
  to the underlying source (Timescale/Postgres connectors do it; a CSV/DuckDB/ODBC provider may not). We
  **assert pushdown from the DataFusion plan / connector capability** before serving a live page — never
  full-scan-then-slice in the engine.
- **Decimation pushes down too — the primary chart path for a warehouse-resident series.** A
  `mode:"buckets"` read (slice C) over a federated source pushes the **`time_bucket(width) … min/max/avg/
  last GROUP BY`** aggregation **down to the source**, so a chart over a 5M-row Timescale hypertable
  aggregates *at Timescale* (its native `time_bucket` / continuous-aggregate strength) and only ~1000
  buckets cross the wire. This is a first-class fast path, not a fallback — for a Timescale-primary
  deployment it is *the* read that makes dashboards fast. Same pushdown discipline: assert the aggregate
  executes at the source, or route to mirror.
- **Refuse to live-page a non-pushdown source; route it to a bounded mirror.** When pushdown can't be
  proven, the verb returns a **structured "not live-pageable → mirror" outcome** (with the
  `federation.mirror` invocation shape), not a silently slow page. The caller mirrors a **specific bounded
  window** into the series plane, where slice B keyset-pages it at index speed. Mirror is a deliberate
  cache of a named window — **never a bulk copy of the whole warehouse** (a Timescale-primary deployment
  keeps its bulk external, by design; pushdown, not mirror, is its everyday path).
- **`query.run` inherits its engine's answer** — a saved PRQL compiled to a native `store` read pages like
  `store.query`; one compiled to a `federation` source pages only under the same pushdown rule.
- **Additive, no new surface.** No new cap, no new verb — each verb keeps its existing gate
  (`mcp:federation.query:call` / `mcp:store.query:call` / `mcp:query.run:call`), re-checked per page.

## Non-goals

- **The cursor/keyset primitive internals** — the codec, tiebreaker discipline, and versioning are slice A
  ([`page-cursor-scope.md`](page-cursor-scope.md)); this slice **consumes** them.
- **The decimation *contract*** (bucket record shape `{t,min,max,avg,last}`, budget→width derivation,
  spike-survival, the time-cursor chain) — that is slice C
  ([`series-decimation-scope.md`](series-decimation-scope.md)). This slice owns only **executing** that
  contract by **pushing the aggregate down** to a federated source; C owns the same read computed natively
  in SurrealDB. **Native `rows` paging** is slice B ([`series-paging-scope.md`](series-paging-scope.md));
  the mirror *targets* B, this slice does not re-page the series plane.
- **The frontend** — the data-console/dashboard callers are slice E
  ([`page-chaining-ui-scope.md`](page-chaining-ui-scope.md)).
- **Re-scoping the `federation` extension** — DataFusion embedding, the `Source` trait, `net:*`,
  registration, SELECT-only validation, and the `federation.mirror` job all live in
  [`datasources-scope.md`](datasources-scope.md); this slice adds *paging* on top of them and references
  them as the base.
- **Making a non-pushdown source live-page fast** (parent non-goal) — its answer is *mirror then page*, not
  a cleverer live cursor. **Numbered totals** and **unbounded export as a page loop** stay parent non-goals
  (export is a mirror/`lb-jobs` job).
- **Cross-source keyset over a join** — a `federation.query` that joins two providers is not guaranteed
  pushdown-pageable; if pushdown can't be proven for the composed plan, it takes the mirror route like any
  other non-pushdown query. No special live-join pager in v1.

## Intent / approach

**One contract, three engines, but the pager is the engine that owns the data.** This applies the parent's
core decision (paging is a contract; whoever owns the rows runs it) to the datasources verbs:

- **`store.query` (SurrealDB) — always the fast path.** Native tables carry an index on their order key;
  slice A's keyset predicate is an index seek, O(page), flat latency at any depth. `store.query` just grows
  the shared params and delegates to the same keyset code the series plane uses. Nothing external, nothing
  to detect.
- **`federation.query` (DataFusion) — pushdown-paged or not paged at all.** DataFusion materializes/streams
  result sets and pages cheaply **only** when the connector accepts the keyset predicate + `LIMIT` as a
  pushdown. So the verb, before serving a page, **inspects the plan/connector**: is the `WHERE key <
  cursor … LIMIT n` executed *at the source* (a `TableProviderFilterPushDown::Exact` / connector-reported
  pushdown), or would DataFusion scan the whole table and slice locally? If pushed down → serve the live
  keyset page and echo `next_cursor`. If **not** → do **not** serve a page; return the mirror route.
- **`query.run` (saved PRQL) — inherits the compiled engine's rule.** PRQL compiles to a `store` or a
  `federation` read; whichever engine it lands on decides paging by the two rules above.

**Decimation is an aggregate pushdown, same discipline as the keyset.** A `mode:"buckets"` federated read
injects the slice-C aggregate — `time_bucket($width, ts) AS t, min(v), max(v), avg(v), last(v) … GROUP BY
t ORDER BY t` — into the `federation.query` plan and **inspects whether the group-by + aggregate execute
at the source**. Against Timescale this maps to its native `time_bucket()` (or a continuous aggregate),
so the warehouse returns ~`budget` bucket rows and the platform ships those, not the raw range — the
single most important read for a Timescale-primary dashboard. If a connector can't push the aggregate
down (it would stream the full range into DataFusion to bucket locally), the read routes to mirror exactly
as a non-pushdown keyset does: never scan-then-aggregate behind a fast-looking response.

**Detect pushdown from the plan, never from a full scan.** The headline discipline of this slice: pushdown
is **asserted structurally** — from DataFusion's optimized `ExecutionPlan` (the keyset filter appears as a
source-level predicate, not a `FilterExec` above a full `TableScan`) and/or the connector's declared
capability — **before** any rows move. We never run the query, notice it was slow, and call that "paged."
A scan-then-slice fallback is the one thing this slice exists to forbid: it reintroduces O(offset) **and**
hides it behind a page-shaped response.

**Refuse-to-live-page is a routed answer, not an error.** A non-pushdown `federation.query` returns a
structured outcome — `{ pageable: false, route: "mirror", mirror: { source, query, suggested_target_series,
range } }` — so the caller (or slice E's UI) can enqueue the existing `federation.mirror` `lb-jobs` batch
([`datasources-scope.md`](datasources-scope.md) §MCP surface). The mirror pulls the range once and
`ingest.write`s it into the series plane ([`../ingest/ingest-scope.md`](../ingest/ingest-scope.md)); from
then on slice B keyset-pages it at index speed, offline-capable, no live external dependency. This *is* the
federate-vs-mirror doctrine as paging: **federate = pushdown-paged (fresh/ad-hoc); mirror = keyset-paged in
the series plane (fast/repeated).**

**Additive on existing verbs — the cursor is a bookmark, not a capability.** No new cap, no new verb: each
verb keeps its gate and grows `{limit, cursor}`. Every page **re-authorizes workspace-first then the
verb's existing cap**; the workspace is host-pinned from the token, and per
[`datasources-scope.md`](datasources-scope.md) `{source}` resolves only to a **registered**
`datasource:{ws}:{name}` in the *caller's* workspace. A cursor (opaque keyset, no workspace/source encoded)
and a `sql`/PRQL body therefore **cannot forge a cross-tenant or unregistered source** — the wall is the
token + the registration record, re-checked per page.

**Rejected — silent full-scan fallback ("just page it locally").** Serving a non-pushdown source by
scanning and slicing in DataFusion would make it *look* paged while costing O(offset) and getting slower
every page — the precise failure this feature kills, now hidden behind a green response. We refuse it: the
honest answer to "this source can't live-page" is *mirror it*, not *scan it quietly*.

**Rejected — a fourth "smart pager" that scans a source once and caches pages.** That is a mirror with no
durability, no resume, and no workspace-walled home — it re-invents `federation.mirror` badly. The series
plane is the sanctioned cache; use it.

## How it fits the core

- **Tenancy / isolation (rule 6):** the workspace is host-pinned from the token on **every** page; the
  cursor never supplies it, and `{source}` resolves only to a registered `datasource:{ws}:{name}` in the
  caller's workspace. A ws-B caller **replaying a ws-A cursor** resolves nothing; a ws-B caller **naming a
  ws-A datasource** is denied at resolution; ws-B's `federation` instance opens no ws-A endpoint. Mandatory
  isolation test across **store + MCP + the `net:*` boundary** (the mirror job's callback `ws` is host-set,
  un-spoofable, per [`datasources-scope.md`](datasources-scope.md)).
- **Capabilities (rule 5):** **no new cap.** Each verb keeps its existing gate —
  `mcp:federation.query:call`, `mcp:store.query:call`, `mcp:query.run:call` — re-checked per page (a
  mid-chain revoke denies the next page). At connect time the `net:*` grant still gates the external
  endpoint; a page over a source whose endpoint the grant omits is refused, opaque. The deny path is
  unchanged from [`datasources-scope.md`](datasources-scope.md).
- **Placement:** `either`, no `if cloud`. The same pushdown-detect + keyset code pages a local edge source
  and a cloud warehouse; *which* datasources exist is config (registration + grants), not a branch. The
  mirror runs wherever `lb-jobs` runs.
- **One datastore (rule 2):** external DBs stay **federated sources, never a second authority or sync
  peer**. `store.query` keyset uses an **index guarantee** on the order key (no new table). A non-pushdown
  source is not promoted to a live pager — it is copied into the **existing series plane** as ingest. No
  new persistence layer, no external authority.
- **MCP surface (§6.1):** additive `{limit, cursor}` on three existing **get/list-shaped** reads — no CRUD,
  no new verb, no new live-feed. The forward tail stays `series.watch` (unchanged). A **non-pushdown page
  routes to the existing batch `federation.mirror` job** (returns a job id, §6.10) — a long/unbounded pull
  is never a blocking page loop in a tool handler.
- **State vs motion (rule 3):** paging walks *backward through committed state* (native rows, or the
  mirrored series in SurrealDB). The live forward edge stays `series.watch` (Zenoh); callers compose them,
  never conflate them. A live `federation.query` page is a bounded read, not a feed.
- **Secrets:** unchanged — the DSN stays `secret:federation/{source}` in `lb-secrets`, pulled by the
  supervisor ([`datasources-scope.md`](datasources-scope.md)); a cursor or paged result never carries it (a
  redaction assertion, as in the base).
- **SDK/WIT impact:** additive fields on existing MCP read verbs — no new verb, no new host-callback, no ABI
  break. A guest already calling `federation.query`/`store.query`/`query.run` gains paging by sending
  `limit`/`cursor`; the mirror route reuses the existing native `ingest.write` callback.

## Example flow

A KFC dashboard pages two external sources — one that pushes down, one that can't.

1. **`store.query` (native, always fast).** A rule pages a native table:
   `store.query { table:"alarms", limit:200, direction:"back" }`. The host authorizes
   `mcp:store.query:call` workspace-first, runs slice A's keyset over the indexed order key, returns
   `{ rows, next_cursor }`. The next page sends the cursor back — an index seek, flat latency.
2. **`federation.query` — pushdown, live-paged.** The Timescale warehouse is registered
   (`datasource:acme:timescale`, [`datasources-scope.md`](datasources-scope.md) step 1–2). A call
   `federation.query { source:"timescale", sql:"SELECT ts, store, temp FROM readings ORDER BY ts DESC",
   limit:500, cursor }` authorizes `mcp:federation.query:call`, resolves `timescale` in `acme`, validates
   SELECT-only, and **injects the keyset predicate** (`ts < cursor.ts …`). The host **inspects the
   optimized plan**: the Timescale connector reports the filter + `LIMIT` as an **exact pushdown** (executed
   at the source). It serves a live page and echoes `next_cursor`. Fast, fresh, no copy.
3. **`federation.query` — no pushdown, routed to mirror.** A second source is a **DuckDB CSV** provider that
   cannot push the keyset predicate down. The same call shape comes in; the host injects the predicate,
   inspects the plan, and finds DataFusion would **full-scan then filter locally** (a `FilterExec` over a
   full `TableScan`, connector reports `Unsupported`). It **refuses to live-page** and returns
   `{ pageable:false, route:"mirror", mirror:{ source:"csv_warehouse", query, suggested_target_series:
   "warehouse.temp", range:"-30d" } }`.
4. **Mirror → series → fast keyset.** The caller enqueues `federation.mirror { source:"csv_warehouse",
   query, target_series:"warehouse.temp", range:"-30d" }` — a durable, resumable `lb-jobs` batch
   ([`../jobs/jobs-scope.md`](../jobs/jobs-scope.md)) that pulls the range once and `ingest.write`s it into
   the series plane (native callback, `ws` host-set). From then on the dashboard pages **`series.read`**
   (slice B) over `warehouse.temp` at index speed — the non-pushdown source now loads fast, offline-capable.
5. **`federation.query` — decimated chart, pushed down (the Timescale-primary dashboard read).** The
   temperature chart calls `federation.query { source:"timescale", …, mode:"buckets", window:"-6h",
   budget:1000 }`. The host injects `time_bucket('30s', ts) AS t, min(temp), max(temp), avg(temp),
   last(temp) … GROUP BY t`, inspects the plan — the aggregate executes **at Timescale** (native
   `time_bucket`) — and ships ~720 bucket rows. A 90-minutes-ago spike survives in the bucket `max`. The
   raw 5M rows never leave the warehouse; the dashboard is fast and the bulk stays in Timescale by design.
6. **`query.run` (saved PRQL).** A saved query compiled to `store` pages like step 1; one compiled to the
   `timescale` source pages like step 2/5; one compiled to the CSV source routes like step 3.
7. **Deny / isolation paths.** ws-B calling any verb **without its cap** → denied. ws-B **replaying a ws-A
   `federation.query` cursor** → the cursor is an opaque keyset, the workspace/source come from ws-B's
   token/request → resolves nothing. ws-B naming `source:"timescale"` → not a registered datasource in ws-B
   → denied at resolution, opaque. A source whose `net:*` endpoint the grant omits → connect refused even
   with the page request valid.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks for our own stack:** the
**real host, real caps, real `net:*` enforcement, a real `lb-jobs` queue for the mirror**, real store, real
MCP. The **external DB is the one sanctioned fake-boundary** — tests run against a **real spawned
Postgres/Timescale container** (and a non-pushdown provider — a real DuckDB/CSV source) seeded with **real
rows**, behind the one `Source` trait ([`datasources-scope.md`](datasources-scope.md) testing plan), **not
an in-process re-implementation**.

- **Capability-deny (§2.1) — per verb:** `federation.query` denied without `mcp:federation.query:call`;
  `store.query` without `mcp:store.query:call`; `query.run` without `mcp:query.run:call` — page requests
  denied like any call. Plus the base **`net:*` deny**: a page over a source whose endpoint the grant omits
  → connect refused, opaque.
- **Workspace-isolation (§2.2) — across store + MCP + `net:*`:** ws-B **replaying a ws-A cursor** resolves
  nothing (the cursor carries no workspace); ws-B **naming a ws-A datasource** is denied at resolution;
  ws-B's `federation` instance reaches **no** ws-A endpoint; a mirror job's callback `ws` is un-spoofable.
- **Pushdown-vs-mirror (the headline):**
  - A keyset `federation.query` against the **real Timescale/Postgres** container **pushes the predicate
    down** — assert via the **DataFusion plan / connector capability** (the keyset filter + `LIMIT` execute
    at the source; no `FilterExec` over a full `TableScan`), **not** by observing it was fast and **not**
    full-scan-then-slice. Prove the page is O(page): row-count read from the source stays bounded across
    deep pages.
  - A **non-pushdown source** (the real DuckDB/CSV provider) is **proven to require the mirror path** — the
    verb returns `{ pageable:false, route:"mirror" }` and **serves no live page**; it never silently
    full-scans.
  - **Decimation pushdown (the Timescale chart path):** a `mode:"buckets"` `federation.query` against the
    real Timescale container **pushes the `time_bucket … GROUP BY` down** — assert via the plan/connector
    that the aggregate runs at the source (the container returns ~`budget` rows, not the raw range), and
    that a seeded **spike survives** in the bucket `max`. Prove only bucket rows cross the boundary (bounded
    payload independent of raw-range size). A connector that can't push the aggregate down is proven to
    route to mirror, not stream-then-aggregate locally.
  - **Mirror → series pages fast:** the routed `federation.mirror` pulls the **bounded seeded window** into
    the series plane, then **slice B keyset-pages it at index speed** (flat latency at depth) — the
    round-trip a non-pushdown source takes. (Assert the mirror is the named window, not a whole-table copy.)
- **Cursor stability / re-auth:** a mid-chain **cap revoke** denies the next page; an **incompatible/older
  cursor** (slice A versioning) is rejected cleanly (restart the chain), never mis-seeks.
- **Secret mediation:** the DSN never appears in a page result, a cursor, a record, or a log (redaction
  assertion, per the base).
- **Happy round-trips:** `store.query` chains N pages over seeded native rows; `federation.query` chains N
  pushdown pages over the Timescale container; `query.run` pages a saved PRQL over both a native and a
  federated compile target.

## Risks & hard problems

- **Silent full-scan fallback is the whole risk of this slice.** If pushdown detection is wrong-permissive,
  a non-pushdown source is served as a page — O(offset), and *hidden* behind a page-shaped response. The
  detection must be **structural (plan/connector), fail-closed** (unknown pushdown → route to mirror, never
  live-page), and regression-tested against a real non-pushdown provider.
- **Pushdown is per-connector and per-query.** The same connector may push down a simple `WHERE ts <
  cursor` but not the same predicate under a join or a function-wrapped key. Detection is on the **composed
  optimized plan for this exact query**, not a static "Timescale = pageable" flag.
- **Keyset semantics on an external order key.** The external table's order key needs a **unique
  composite** (slice A's tiebreaker) or the keyset skips/repeats rows across pages; a source whose
  `ORDER BY` isn't a stable unique key can't be pushdown-paged correctly and must route to mirror (the
  mirror assigns a stable `seq` in the series plane).
- **DataFusion plan-shape coupling.** Asserting pushdown reads DataFusion internals (`ExecutionPlan` /
  `TableProviderFilterPushDown`); a `datafusion` upgrade can change the plan shape and break detection. Pin
  the check to the connector's declared capability where possible and cover it with the real-container test
  so an upgrade fails loudly.
- **Mirror latency vs "fresh."** Routing to mirror trades freshness for speed; a caller wanting *live*
  non-pushdown data can't have both. The scope's answer is explicit (federate = fresh, mirror = fast); the
  UI (slice E) must surface "this source was mirrored — showing cached-to-`ts`," not pretend it's live.

## Open questions

- **Refusal shape:** the exact structured `{ pageable:false, route:"mirror", mirror:{…} }` payload — does
  the verb **suggest** the `federation.mirror` args (source, query, target series, range) or just signal
  "not pageable" and leave the caller to compose the mirror? → resolve during build (recommended: suggest,
  so slice E can offer a one-click "mirror this").
- **Auto-mirror vs explicit:** should a repeated non-pushdown page **auto-enqueue** the mirror job, or
  always require an explicit caller/admin action (cost + `net:*` egress)? → recommend explicit in v1
  (egress is admin-approved), revisit an auto-mirror policy later.
- **Pushdown assertion source of truth:** the DataFusion optimized `ExecutionPlan` inspection vs the
  connector's declared `supports_filter_pushdown` capability vs both. → resolve in build; prefer connector
  capability + a plan assertion in the test as the guard.
- **Default & max `limit`** per federated verb (a live pushdown page can still be heavy at the source), and
  the page-count ceiling before the caller must offer "mirror / export as a job." → coordinate with
  slices B/E.
- **`query.run` mixed-engine PRQL:** a saved query that composes a native `store` read with a federated
  read — does it page at all in v1, or route to mirror wholesale? → recommend: if any leg is non-pushdown,
  route to mirror; no partial live paging in v1.

## Related

- [`page-chaining-scope.md`](page-chaining-scope.md) — **parent**: the doctrine + the one shared contract.
- [`datasources-scope.md`](datasources-scope.md) — the `federation` extension + **federate-vs-mirror**
  doctrine this builds on (DataFusion, `Source` trait, `net:*`, registration, `federation.mirror`).
- [`page-cursor-scope.md`](page-cursor-scope.md) — **slice A**, the cursor/keyset primitive this consumes.
- [`series-paging-scope.md`](series-paging-scope.md) — **slice B**, the series plane the mirror route pages
  at index speed.
- [`series-decimation-scope.md`](series-decimation-scope.md) — **slice C**, `mode:"buckets"` (reference).
- [`page-chaining-ui-scope.md`](page-chaining-ui-scope.md) — **slice E**, the callers (reference).
- [`../query/prql-query-scope.md`](../query/prql-query-scope.md) — `query.run` pages via the same contract.
- [`../ingest/ingest-scope.md`](../ingest/ingest-scope.md) — the mirror's `ingest.write` target (series
  plane).
- [`../jobs/jobs-scope.md`](../jobs/jobs-scope.md) — the durable, resumable `federation.mirror` `lb-jobs`
  batch.
- README `§3` (rule 2 — one datastore), `§6.1` (API shape), `§6.10` (jobs/batch).
