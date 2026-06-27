# Frontend scope — the workspace data console (DB browser + ingest explorer)

Status: scope (the ask). Promotes to `public/frontend/data-console.md` once shipped. Target stage:
**S9+ collaboration UI** (builds on the shipped S8 data plane — `series.read`/`series.latest`/
`series.find` — and the shipped S9 real-session shell + admin console). Sibling of
`dashboard-scope.md`: the dashboard is the *pretty, curated* presentation of series; this is the
*raw, exploratory* console underneath it.

We want two **workspace-scoped, capability-gated** pages in the shell for a person who is **not good at
SQL** to look at their data and poke at it safely:

1. **A "Data" page (the DB browser)** — pick a table, page through its rows in a plain grid, and flip to
   a **node/edge graph view** (react-flow) that draws the workspace's records and their graph relations
   so relationships are *visible*, not implied by a join nobody can write. Read-only.
2. **An "Ingest" page (the series explorer)** — list/search the workspace's `series`, see each series'
   **latest** value and a table of **recent samples**, and **push a sample by hand** (`ingest.write`)
   from a small form for testing/manual entry. Built directly on the shipped S8 ingest verbs.

Everything runs against a **real node** — real store, real series, real capability checks — seeded with
**real records** through the real write path. No `*.fake.ts`, no mock backend (CLAUDE §9, testing §0).

## Goals

### The Data page (DB browser)

- **A table picker** — list every table in the workspace with its row count, so a non-SQL user sees
  "what's in here" at a glance.
- **A paginated row grid** — select a table, see its rows in a flat, scrollable grid (columns inferred
  from the records), cursor-paged. Read-only; clicking a row expands its full JSON.
- **A graph view (react-flow)** — the same workspace data drawn as **nodes (records) + edges (graph
  relations)**, depth-bounded, so a user can *follow* a relationship (a series → its producer principal,
  a doc → the channel it's linked to) by clicking rather than writing a traversal. Start from a table or
  a single record; expand neighbours on click.
- **No SQL surface.** The user never types a query; the page issues fixed, parameterised reads. (A raw
  query box is an explicit non-goal — see below.)

### The Ingest page (series explorer)

- **A series list / search** — list the workspace's series and filter by tag facet (`kind:temperature`,
  `host:pi-7`) over the existing tag-graph discovery (`series.find`).
- **A series detail** — the **latest** sample (`series.latest`) front-and-centre, plus a table of
  **recent samples** (`series.read` over a bounded range), newest first, payload rendered by type.
- **Manual write** — a small form to push one `Sample` into a series (`ingest.write`) for testing or
  hand entry: series name, payload (typed input), optional labels. The `Explore + manual write` ask.

## Non-goals (the defer-list)

- **No raw SQL / SurrealQL box.** The whole point is "for someone not good at SQL." A free-form query
  console is a separate, higher-privilege tool (it would need query-cost limits, injection review, and a
  much narrower grant) — explicitly deferred, not silently dropped.
- **No writes through the DB browser.** The Data page is **read-only**; editing raw records bypasses
  every domain invariant (membership, validation, dedup) and is how a non-SQL user corrupts state. Edits
  go through the real domain verbs (admin console, etc.), never the raw grid.
- **No new persistence / second datastore.** SurrealDB only; the console reads what is already there.
- **No charting/widgets here.** Live charts, drag-resize tiles, and saved layouts are `dashboard-scope.md`.
  This page is a *table + graph explorer*, not a dashboard.
- **No bulk ingest UI.** Manual write is one sample at a time for testing; the firehose path stays the
  producer/bridge story in `ingest-scope.md`.
- **No device/IoT concepts.** A series is a generic named sequence (the ingest anti-IoT rule holds).

## Intent / approach

**Two pages, two backends, one console.** The Ingest page is almost pure wiring — the host verbs shipped
in S8; they just aren't reachable over the gateway yet, so this slice **exposes the existing
`ingest.*`/`series.*` verbs as gateway routes** and builds the UI on them. The Data page needs a **new,
deliberately small, read-only host surface** (`store.tables` / `store.scan` / `store.graph`) because
there is no generic "list tables / scan rows / read the graph" verb today — every existing read is a
typed domain verb (`get_doc`, `series.read`).

**The Data page's central design tension — and the decision.** A generic raw-table reader **bypasses the
per-record membership gate (gate 3)**. `get_doc` checks workspace + capability *and then* "may this
principal read *this* doc" (owner / shared team / linked channel). A raw `SELECT * FROM doc` answers
"every doc in the workspace" with no gate 3. So the raw browser is **not** a member-level tool — it is a
**workspace-admin lens**, gated by a strong new capability (`mcp:store.scan:call`, granted to the
workspace-admin role only), and it is **read-only**. Two gates still hold hard: the **workspace wall**
(`use_ws` binds the namespace; a ws-B admin physically cannot scan ws-A) and the **capability** (no
grant → opaque `Denied`). What it intentionally relaxes is gate 3, and *only* for an admin who can
already see the whole tenant — recorded here so it's a decision, not an accident.

**Rejected alternatives:**
- *Reuse the existing typed verbs (curated views only).* Rejected as the *primary* path — it can only
  ever show the record types we hand-wrote a viewer for, which defeats "browse what's actually in here."
  The curated verbs stay the *member* experience; the raw browser is the *admin* superset.
- *A raw SurrealQL query box.* Rejected for this slice (non-goal) — too sharp an edge for the stated
  user, and a real injection/cost-limit surface. The fixed parameterised reads cover "look at my data."
- *A second datastore / external DB-admin tool (e.g. Surrealist embedded).* Rejected — violates one
  datastore + capability-first + workspace-wall: an external admin tool reaches around the gate entirely.
- *Writes in the grid.* Rejected (non-goal) — bypasses domain invariants; corruption disguised as
  convenience.

## How it fits the core

- **Tenancy / isolation:** every read binds the workspace namespace first (`store.use_ws(ws)`,
  `crates/store/src/open.rs`) using `principal.ws()` from the token — never a request field. A ws-B admin
  scanning is physically confined to ws-B's namespace. Mandatory isolation test (a ws-B token cannot
  `store.tables`/`store.scan` ws-A, and cannot read/enumerate ws-A series).
- **Capabilities:** new admin-only grants for the raw browser — `mcp:store.tables:call`,
  `mcp:store.scan:call`, `mcp:store.graph:call` — added to the **workspace-admin** role, **not** the
  member role (`role/gateway/src/session/credentials.rs::member_caps`). The ingest page reuses the
  shipped `mcp:series.read:call` / `mcp:series.latest:call` / `mcp:series.find:call` / `mcp:ingest.write:call`.
  Deny path is opaque (`Denied`, no existence signal). **Mandatory deny-test per verb** (HOW-TO-CODE §3
  step 4a).
- **Placement:** `either` — both pages are gateway routes over host verbs; no `if cloud {…}`. The browser
  reaches them over HTTP; the desktop shell over the same MCP contract.
- **MCP surface** (walk all four shapes — SCOPE-WRITTING §6.1):
  - **Get / list (the core of this slice):**
    - `store.tables()` → `[{ table, count }]` for the workspace (admin cap).
    - `store.scan(table, limit, cursor)` → a bounded page of raw rows + a next-cursor (admin cap). Cursor
      on record id; **bounded `limit`** (hard cap server-side) so a huge table never returns unbounded.
    - `store.graph(table?, id?, depth)` → `{ nodes, edges }` for react-flow, **depth- and fan-out-bounded**
      (admin cap). Edges are SurrealDB graph relations (`RELATE` edge records / tag edges).
    - `series.list(prefix)` → series names by prefix. **New small verb** — the scope names it but the S8
      tool dispatch only shipped `read`/`latest`/`find`/`write`; add `series.list` (or fold listing into
      `series.find` with an empty facet set — decide in the session). Reuse `series.find` for tag search.
  - **CRUD (write):** only **`ingest.write`** (already shipped) — surfaced by the manual-write form. The
    Data page has **no write verbs by design** (read-only, see non-goals). State this explicitly so the
    missing writes read as a decision, not a gap.
  - **Live feed (SSE/watch):** **N/A this slice.** Live series motion is the dashboard's job
    (`dashboard-scope.md` subscribes Zenoh); the console is snapshot reads + a manual refresh. If "watch
    this table/series update" is wanted later, it rides the existing bus/SSE path (§6.13), not a poll.
  - **Batch:** **N/A.** Manual write is one sample; the bulk firehose is the producer path (`ingest-scope.md`).
- **Data (SurrealDB):** reads only. `store.tables` via `INFO FOR DB` + a `SELECT count() … GROUP ALL` per
  table; `store.scan` via a parameterised `SELECT * FROM type::table($tb) LIMIT $n START $cursor`
  (validate `$tb` against the table list — same field-id guard as `crates/store/src/list.rs`); `store.graph`
  reads record nodes + their relation edges. No new tables, no writes.
- **Bus (Zenoh):** none this slice (snapshot reads). Live updates deferred to the dashboard's motion path.
- **Sync / authority:** none new — reads committed local state; series already sync as `(table,id)` upserts.
- **Secrets:** none.

## Example flow

**Data page (admin "what's in here"):**
1. A workspace admin opens **Data**. The shell calls `store.tables()` → `[{doc, 12}, {series, 340}, …]`;
   the page lists tables with counts. (A member without `mcp:store.scan:call` never sees the nav entry,
   and a forged call is `403` server-side.)
2. They click **series**. The page calls `store.scan("series", 50, null)` → 50 rows + a next-cursor,
   rendered in a flat grid; clicking a row expands its JSON.
3. They flip to **Graph**. The page calls `store.graph("series", null, 1)` → nodes for the series records
   + edges to their producer principals and tags; react-flow lays it out. Clicking a node calls
   `store.graph(_, id, 1)` to expand that node's neighbours.

**Ingest page (explore + manual write):**
1. A user opens **Ingest**. `series.find([])` (or `series.list("")`) lists the workspace's series;
   typing `kind:temperature` filters via `series.find`.
2. They pick `node.cpu_temp`. `series.latest("node.cpu_temp")` shows the latest value; `series.read(…,
   last N)` fills a recent-samples table, newest first, payload rendered by type.
3. They open **Write sample**, enter series `node.cpu_temp`, payload `61.4`, label `host:pi-7`, submit →
   `ingest.write([{…}])`; the table refreshes and the new sample appears. Without `mcp:ingest.write:call`
   the form's submit is refused server-side (`403`), surfaced as an inline error.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Capability deny — one per verb** (HOW-TO-CODE §3 step 4a): `store.tables`/`store.scan`/`store.graph`
  refused without the admin cap; `ingest.write` refused without `mcp:ingest.write:call`; `series.read`/
  `series.latest`/`series.find` refused without their caps. Forged admin call by a member → `403`
  server-side (mirror `tests/admin_routes_test.rs::forged_admin_call_by_non_admin_is_denied_server_side`).
- **Workspace isolation** (store + MCP): a ws-B token cannot `store.tables`/`store.scan`/`store.graph`
  ws-A (sees only ws-B's namespace), and cannot read or enumerate ws-A series (mirror
  `tests/assets_workflow_routes_test.rs::ws_b_session_cannot_read_ws_a_doc`).
- **No mocks / real infra, seeded data:** Rust gateway route tests spin up the real node on `mem://`,
  seed real records via the real write path (`ingest_write`, `put_doc`), and hit the routes through
  `router(gw).oneshot(...)` with a real token (the `tests/common/mod.rs` harness). Vitest drives the UI
  against a **real in-process gateway seeded with real rows** — **do not add a `data.fake.ts`** (the
  fake is on the retirement list; frontend-scope §"Superseded").
- **Offline/sync, hot-reload:** **N/A** — read-only snapshot console; nothing durable or stateful added.
- **Bound/cost:** `store.scan` honours its server-side `limit` cap; `store.graph` honours its depth/fan-out
  bound (a wide table doesn't return the whole graph). Read-correctness: scan paging returns each row once
  across cursors; `series.read` returns the committed range ordered.

Key UI cases (Vitest): table list renders with counts; selecting a table pages rows; row expand shows
JSON; graph view renders nodes/edges and expands on click; series list filters by facet; latest + recent
samples render by payload type; manual write adds a sample and the table refreshes; deny surfaces as an
inline error, and a member without the cap never sees the Data nav entry.

## Risks & hard problems

- **The membership-gate bypass (the headline risk).** The raw browser intentionally relaxes gate 3 for an
  admin. If `mcp:store.scan:call` ever leaks into the member role, a member reads every record in the
  workspace. Mitigation: admin-only grant, a deny-test asserting the member role lacks it, and a review
  note that this surface is the one place gate 3 is relaxed — keep it admin-only and read-only.
- **Unbounded scans / graph blow-up.** A 1M-row table or a hub node with thousands of edges must not
  return everything. Hard server-side `limit`, cursor paging, and a depth+fan-out bound on `store.graph`
  are load-bearing, not nice-to-have. Easy to underestimate.
- **`store.tables` cost.** `INFO FOR DB` is cheap; a `count()` per table can be expensive on big tables.
  Decide: exact count vs. an estimate vs. lazy/omitted count (open question).
- **Heterogeneous rows in a flat grid.** Records vary in shape; the grid must infer a column union and
  render nested payloads sanely (expand-to-JSON), not assume a fixed schema.
- **react-flow + workspace data shape.** Mapping SurrealDB records/relations onto react-flow's
  node/edge model (ids, edge types, layout) is the fiddliest UI piece; keep the first cut depth-1 and
  click-to-expand rather than auto-laying-out the whole tenant.
- **New dependency.** react-flow (`@xyflow/react`) is a new UI dep — confirm bundle size is acceptable and
  it's only loaded on the Data page (code-split).

## Open questions — RESOLVED (shipped 2026-06-27)

See `sessions/ingest/data-console-session.md` and `public/frontend/data-console.md`.

- **`store.tables` row count:** ✅ **exact `count()` per table.** Cheap at the admin/dev scale this
  targets; a cheaper estimate is a documented follow-up, not a v1 need.
- **`series.list` vs. `series.find([])`:** ✅ **added the dedicated `series.list(prefix)` verb.** Prefix
  listing over the committed `series` table and tag-faceted discovery are different queries; `series.find([])`
  returns nothing by design.
- **Graph scope:** ✅ **started with the `tagged` relation edges** (series→tags) — the relations that
  already exist as edge records. Edge tables are a parameter the host supplies (`lb_tags::TAGGED_TABLE`);
  no synthesised edges. Richer relations (team→member, doc→channel) are added as they ship as edge records.
- **`store.graph` node identity:** ✅ **the SurrealDB record id (`table:id`) directly** — already unique/stable.
- **Pagination contract for `store.scan`:** ✅ **id-cursor** (`<string>id > $after`), stable under
  concurrent writes.
- **Non-admin *read-your-own* mode:** ⏸ **deferred** — shipped admin-only raw; revisit if members want a
  curated, gate-3-enforced browser.

### Shipped note — the graph data-shape caveat

The tag layer tags the *logical* series name (`series:node.cpu_temp`), but the `series` *table* rows have
composite ids (`[series, producer, seq]`). So a graph seeded from `table=series` draws the composite-id
rows as nodes but follows the tag relation only via **per-record expand** (`id=series:node.cpu_temp`, the
click-to-expand path). This is the first-cut graph shape; richer table-level edge joins are a follow-up.

## Related

- `scope/frontend/dashboard-scope.md` — the curated/live presentation of series; this is its raw
  exploratory counterpart (shares the S8 read verbs).
- `scope/frontend/admin-console-scope.md` — the sibling admin surface + the `403`/cap-gated-nav patterns
  and the gateway route tests to mirror.
- `scope/ingest/ingest-scope.md` + `public/ingest/ingest.md` — the `Sample`/`series` model and the
  shipped `ingest.write`/`series.read`/`series.latest`/`series.find` verbs this page surfaces; `series.list`
  is named there.
- `scope/store/` — the SurrealDB record/table model the `store.*` read verbs walk; `crates/store/src/{open,list,read}.rs`
  are the reuse targets (namespace binding + the field-id injection guard).
- README **§3** (workspace wall, capability-first, one datastore), **§6.1** (time-series model),
  **§6.5** (the MCP dispatch chokepoint), **§6.11** (tags = series discovery).
- `scope/testing/testing-scope.md` §0/§2 (no mocks; mandatory deny + isolation tests).
</content>
</invoke>
