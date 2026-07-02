# Datasources scope — page-chaining (parent: fast paging for large timeseries)

Status: scope (the ask) — **parent/overview**. This is a big feature; it is decomposed into the child
scopes indexed below. Promotes to `public/datasources/datasources.md` once the slices ship.

We want reads over **large timeseries** — a series with millions of samples, a federated warehouse table,
a raw data-console scan — to **load a page at a time, fast, and chain to the next page with a cursor**,
instead of pulling a whole range into memory or paying an `OFFSET` scan that gets slower every page. Today
`series.read` returns an unbounded `Vec<Sample>` for a `seq` range
([`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs)); a big series either OOMs the call or
stalls the dashboard. This parent settles the **doctrine + the one shared contract**; each child scope
builds one slice against it.

> Read with: `datasources-scope.md` (the `federation` extension + the **federate-vs-mirror** doctrine this
> extends), `../ingest/ingest-scope.md` (the `series` plane + `series.read`/`latest`/`watch` we modify),
> `../query/prql-query-scope.md` (`query.run` pages the same way), `../frontend/data-console-scope.md` +
> `../frontend/dashboard/viz/` (the two callers). README §3 (rules 2/3/5/6), §6.1 (API shape), §6.10 (jobs).

---

## The decision this feature turns on: DataFusion or SurrealDB?

**Neither is "the pager." Paging is a contract; the engine that owns the data runs it — and *both*
engines are first-class fast paths.** This platform runs two data homes side by side (some deployments
keep the bulk in SurrealDB; **others keep it in an external warehouse like TimescaleDB** and hold little
in SurrealDB), so paging must be equally fast for either. Same split as `datasources-scope.md`, neither
demoted:

1. **SurrealDB pages its own state plane fast.** It holds `series` + store tables with an index on the
   natural order key (`(series, seq)` / `(ts, id)`). A **keyset** page (`WHERE key < cursor ORDER BY key
   DESC LIMIT n`) is an index seek: **O(page), flat latency at any depth.** Native tables and
   platform-owned series page here.
2. **A federated source pages fast *at the source*, by pushdown.** `federation.query` (DataFusion) pushes
   the keyset predicate + `LIMIT` **down to the underlying database**, which runs it as its own indexed
   range scan. **TimescaleDB/Postgres are the ideal case** — a hypertable range scan is exactly what
   Timescale is built for, and its native `time_bucket()` means even a **decimated chart read aggregates
   at the source** (only ~1000 buckets cross the wire, not a million rows). When the source pushes down,
   this is **just as fast as path 1** and sits directly between the user and the chart — as it should for
   a Timescale-primary deployment.
3. **Mirror is a narrow cache, not the bulk answer.** A source that genuinely *can't* push down (a CSV /
   DuckDB provider) can be **mirrored** — the existing `federation.mirror` `lb-jobs` batch copies a
   **specific, bounded window** into the series plane for offline/edge, then path 1 keyset-pages it. Mirror
   is a deliberate, bounded cache of a window you name — **never a bulk copy of an external warehouse into
   SurrealDB** (a Timescale-primary deployment keeps its bulk in Timescale, by design).

So the answer to "fast page loads over lots of timeseries" is **whichever engine owns the rows pages
them, in place**: SurrealDB keysets its series plane; a pushdown source (Timescale) keysets *and*
decimates at the warehouse. This is the federate-vs-mirror doctrine applied to paging: *federate =
push down to the live source (the primary path for external data); mirror = a bounded window cached in
the series plane (offline/edge, or a non-pushdown source).*

**Rejected — forcing all external data through a mirror into SurrealDB.** That would bulk-copy a
warehouse the deployment deliberately keeps external, doubling storage and going stale — wrong for a
Timescale-primary setup. Pushdown keeps the data where it lives and still pages fast. **Rejected —
DataFusion as the uniform pager over native + external.** It would demote SurrealDB's index-backed series
read to a table scan and add a planner hop to every native page. **Rejected — `LIMIT/OFFSET` paging.**
O(offset) scan-and-discard degrades on exactly the big datasets this is for, and it's unstable under
head-appends (a new row shifts every page). Keyset is append-stable and O(page) — the right primitive for
append-mostly timeseries, native or federated.

## The one shared contract (every slice obeys this)

- **Opaque keyset cursor + bounded `limit`.** A read takes `{limit, cursor, direction}` and returns
  `{rows|buckets, next_cursor, prev_cursor}`. `next_cursor == null` means end-of-range. The client
  **chains** pages by echoing the cursor back — it never constructs one.
- **Keyset, not offset.** The cursor encodes the **position key** — the last row's *unique composite* sort
  key (`(ts, seq)` — a tiebreaker so no row is skipped/duplicated on ties), never a row number.
- **The cursor is a bookmark, not a capability.** Every page **re-authorizes** workspace-first then the
  existing read cap; the workspace and series come from the **token/request**, never decoded from the
  cursor. A ws-A cursor replayed in ws-B resolves nothing — the wall is the token, re-checked per page.
- **Additive — no new capability, no new verb.** Existing read verbs grow `limit`/`cursor` fields; the gate
  stays their current cap (`mcp:series.read:call`, `mcp:store.query:call`, `mcp:federation.query:call`,
  `mcp:query.run:call`).
- **Two `mode`s, one cursor.** `mode:"rows"` pages raw records (tables); `mode:"buckets"` pages
  server-side **decimated** time-buckets (charts). Same chain shape; different payload.
- **State vs motion (rule 3).** Paging walks *backward through committed state* (SurrealDB); the live
  forward edge stays `series.watch` (Zenoh). Callers compose them — subscribe forward, page backward — they
  never conflate them.

## Non-goals (whole feature)

- **Total counts / "page 42 of 1000"** — counting a giant series is O(series); page-chaining is forward/back
  cursoring with "load more", not a numbered pager.
- **Unbounded export as a page loop** — a whole-range pull is a **mirror/export `lb-jobs` job** (resumable,
  returns a job id — §6.10), not a client looping `next_cursor` thousands of times.
- **DataFusion as primary pager** and **offset paging** (both rejected above).
- **Making a non-pushdown federated source load fast *live*** — its answer is *mirror then page*, not a
  cleverer live cursor.
- **A new time-series database** — rule 2 holds; the series plane in SurrealDB is the fast store.

## Slices (child scopes — build in this order)

Each is a separately buildable/testable ask against the shared contract. Dependencies flow downward.

| # | Scope | The ask | Depends on |
|---|---|---|---|
| **A** | [`page-cursor-scope.md`](page-cursor-scope.md) | The foundation: the opaque **cursor codec** + the **keyset predicate** primitive, the tiebreaker discipline, cursor versioning — one `cursor` file + one `keyset` file, reused by every pager. Decides no-signing. | — |
| **B** | [`series-paging-scope.md`](series-paging-scope.md) | The **fast path**: `series.read` grows `{limit, cursor, direction, mode:"rows"}`, keyset over `(series, seq/ts)` in SurrealDB; the index guarantee; compose with `series.watch`. | A |
| **C** | [`series-decimation-scope.md`](series-decimation-scope.md) | **Charts don't page raw points**: `mode:"buckets"` time-bucket min/max/avg/last, a bounded point budget, window paged by time cursor — computed in SurrealDB for native series, or **pushed down** (slice D) for a federated series. | A, B |
| **D** | [`federation-paging-scope.md`](federation-paging-scope.md) | **External sources (the primary path for Timescale-primary deployments)**: `federation.query`/`store.query`/`query.run` keyset **and decimation (`time_bucket` GROUP BY) pushed down** to the source; detect pushdown structurally; a non-pushdown source routes to a **bounded mirror**, never a bulk copy. | A |
| **E** | [`page-chaining-ui-scope.md`](page-chaining-ui-scope.md) | **The two callers**: the data-console table (rows) + a dashboard viz cell (buckets) over the gateway — infinite scroll / "load more", composing backward paging with the live `series.watch` tail. | B, C, D |

Slice A is the keystone: it defines the cursor/keyset shape B, C, D, E all consume. Ship A first; B and D
can then proceed in parallel; C builds on B; E lands last over all of them.

## How it fits the core (feature-level; each slice re-asserts its own)

- **Tenancy / isolation:** the workspace is host-set from the token on **every** page; the cursor never
  supplies it. A ws-A cursor used in ws-B resolves nothing. Every slice carries the mandatory isolation test.
- **Capabilities:** **no new cap** across the feature — each paged read keeps its existing gate, re-checked
  per page (a mid-chain revoke denies the next page). The deny path is unchanged.
- **Placement:** `either`, no `if cloud`. The same keyset code pages a local edge series and a mirrored
  cloud warehouse; which *sources* exist is config (grants), not a branch.
- **One datastore:** no new table, no new store. The change is an **index** guarantee on the paging key + a
  keyset predicate; decimation is a `GROUP BY` read, not stored aggregates. Rule 2 intact.
- **MCP surface (§6.1):** additive `{limit, cursor}` on existing **get/list** reads; **batch/export stays a
  job** (`federation.mirror`); no CRUD, no new live-feed (the tail is the existing `series.watch`).
- **SDK/WIT impact:** additive fields on existing MCP read verbs — no new verb, no new host-callback, no ABI
  break. A guest already calling `series.read` gains paging by sending `limit`/`cursor`.

## Feature-level risks (each slice owns its detail)

- **Tiebreaker discipline** — a keyset on a non-unique sort key skips/repeats rows without a unique
  composite (`(ts, seq)`); the single easiest thing to get wrong (slice A owns it).
- **Federation pushdown is per-connector** — silent full-scan fallback reintroduces O(offset) *and* hides
  it; must detect and route to mirror, never quietly serve a slow page (slice D).
- **Decimation semantics** — `avg`-only buckets hide spikes an alert cares about; min/max/last is the safe
  default at 3–4× payload (slice C).
- **Cursor stability across schema/order changes** — version the cursor; reject an incompatible one cleanly
  (restart the chain) rather than mis-seek (slice A).

## Open questions (feature-level; slice-specific ones live in each child)

- **Cursor encoding:** plain base64 keyset (recommended — every page re-authorizes, so the cursor grants
  nothing) vs HMAC-signed for **audit tamper-evidence** (not security). → resolved in slice A.
- **One shape or two verbs:** extend `series.read` with `mode` (recommended) vs a distinct `series.query`
  for the decimated read. → resolved in slices B/C.
- **Bidirectional paging:** `direction:"back"` only first (the dashboard need) vs `prev_cursor`/`after` from
  day one. → slices B/E.
- **Default & max `limit`** per read, and the client's page-count ceiling before it must offer "export as a
  job". → slices B/E.

## Related

- `datasources-scope.md` — the `federation` extension + **federate-vs-mirror** doctrine this extends.
- `../ingest/ingest-scope.md` — the `series` plane + `series.read`/`latest`/`watch`
  ([`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs)).
- `../query/prql-query-scope.md` — `query.run` pages via the same contract.
- `../frontend/data-console-scope.md` + `../frontend/dashboard/` + `../frontend/dashboard/viz/` — the callers.
- README `§3` (rules 2/3/5/6), `§6.1` (API shape), `§6.10` (jobs).
</content>
