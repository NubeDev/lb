# Data console — DB browser + ingest explorer (session)

- Date: 2026-06-27
- Scope: ../../scope/frontend/data-console-scope.md
- Stage: S9+ collaboration UI (builds on the shipped S8 data plane). See STAGES.md.
- Status: **shipped** (built end to end 2026-06-27 — backend + frontend + tests green; see below)

> Topic note: the scope lives under `scope/frontend/` (it's a shell surface, alongside `admin-console`
> and `dashboard`); this session log is filed under `sessions/ingest/` at the requester's ask because
> the Ingest page is the visible half. Keep the cross-links (scope ↔ session ↔ public) intact.

## Goal
Ship two workspace-scoped, capability-gated shell pages for non-SQL users: a **Data** page (admin-gated
raw table browser — paged row grid + react-flow graph of records/relations, read-only) and an **Ingest**
page (series list/search + latest + recent samples + manual `ingest.write`). The Ingest verbs already
shipped in S8 but aren't reachable over the gateway; the Data page needs a new small read-only
`store.tables`/`store.scan`/`store.graph` host surface.

## What changed

**Backend — the new read-only DB-browser surface (`store.*`).** Generic, namespace-bound store reads
(no `lb_tags` dep — `lb_store` stays generic):
- `crates/store/src/tables.rs` — `tables(ws)` = `INFO FOR DB` + a `count() GROUP ALL` per table.
- `crates/store/src/scan.rs` — `scan(ws, table, limit, after)` = `SELECT meta::id(id) AS rid,
  <string>id AS _oid, * OMIT id, in, out … ORDER BY _oid LIMIT n`, **hard-capped** (`MAX_SCAN_LIMIT=200`),
  **id-cursor** paging (`WHERE <string>id > $after`). Returns `{id: "table:…", data: {…}}` per row.
- `crates/store/src/graph.rs` — `graph(ws, table?, id?, edge_tables, depth)` = bounded nodes + relation
  edges for react-flow (`MAX_SEED`/`MAX_FANOUT=50`). Edge tables are a **parameter** (the host passes
  `lb_tags::TAGGED_TABLE`), matched on the edge's denormalized `ent` string.

Host service `crates/host/src/dbview/` (one verb per file): `store_tables_view`/`store_scan_view`/
`store_graph_view` (each `authorize_dbview` first), `error.rs`, `authorize.rs`, MCP bridge `tool.rs`
(`store.tables`/`store.scan`/`store.graph`), `mod.rs`. Wired into `crates/host/src/lib.rs`.

**Backend — `series.list` + exposing the S8 ingest verbs over the gateway.**
- `crates/host/src/ingest/list.rs` — `series_list(ws, prefix)` (gated `mcp:series.list:call`), `SELECT
  series AS name … GROUP BY name`, bounded `MAX_SERIES_LIST=500`. Wired into the ingest `tool.rs` bridge.
- `role/gateway/src/routes/dbview.rs` — `GET /store/tables`, `GET /store/tables/{table}/rows`,
  `GET /store/graph` (admin cap re-checked server-side; no write routes).
- `role/gateway/src/routes/ingest.rs` — `POST /ingest` (writes **then drains** so a manual sample is
  visible on the next read), `GET /series`, `POST /series/find`, `GET /series/{s}/latest`,
  `GET /series/{s}/samples`. Registered in `routes/mod.rs` + `server.rs`.
- `role/gateway/src/session/credentials.rs` — added the member series caps + the **admin-only**
  `mcp:store.tables/scan/graph:call` to the dev principal (the dev principal is a workspace admin).

**Frontend — two pages, real HTTP, no fake.**
- `ui/src/lib/data/{data.types,data.api}.ts` + `ui/src/lib/ingest/{ingest.types,ingest.api}.ts` — api
  clients mirroring the verbs 1:1. Commands wired into `ui/src/lib/ipc/http.ts` (`store_*`, `ingest_write`,
  `series_*`). Caps added to `ui/src/lib/session/admin-caps.ts`.
- `ui/src/features/ingest/` — `IngestView` (series list/search, latest + recent-samples table, manual
  write form) + `useIngest`.
- `ui/src/features/data/` — `DataView` (table picker + counts, paged row grid with row-expand-to-JSON,
  Grid/Graph toggle) + `useData` + `DataGraph` (react-flow, **code-split** via `lazy()` — confirmed a
  separate `DataGraph` chunk in the prod build). `@xyflow/react` added.
- `ui/src/App.tsx` + `ui/src/features/shell/NavRail.tsx` — cap-gated nav: Ingest shows on `series.list`,
  **Data shows only on `store.scan`** (admin-only).

**The real-gateway Vitest harness (unblocks STATUS Next-up #00).** `role/gateway/src/bin/test_gateway.rs`
(boots a real gateway-role node, serves on `$PORT`, prints `LISTENING <addr>`); `ui/src/test/real-gateway.ts`
(globalSetup spawns it, provides the URL); `ui/vitest.gateway.config.ts` + `ui/src/test/setup-gateway.ts`
(+ react-flow jsdom polyfills) + `ui/src/test/gateway-session.ts` (login helper). `pnpm test:gateway`.

## Decisions & alternatives

- **The gate-3 relaxation is admin-only + read-only, on purpose.** `store.*` answers "every record in the
  workspace", bypassing the per-record membership gate `get_doc` enforces. So the caps are granted to the
  workspace-admin role **only** (never `member_caps`), and there are **no write verbs**. Two gates still
  hold hard: the workspace wall (`use_ws`) and the capability. A deny-test asserts a token without the
  admin cap is refused (`store_verbs_denied_without_the_admin_cap_…`).
- **`series.list` vs `series.find([])`** → **added the small `series.list(prefix)` verb.** Prefix listing
  over the committed `series` table and tag-faceted discovery (`series.find`) are different queries; a
  `series.find([])` returns nothing by design (a query must constrain something).
- **`store.tables` row count** → **exact `count()` per table.** Cheap at the admin/dev scale this targets;
  an estimate is a documented follow-up, not a v1 need.
- **scan cursor** → **id-cursor** (`<string>id > $after`), stable under concurrent writes (an offset drifts).
- **graph node id** → the SurrealDB record id (`table:id`) directly (already unique/stable).
- **`lb_store` stays generic** — the graph's edge tables are a parameter the host supplies; no `lb_tags`
  dependency in the store crate (would be a cycle).
- **Frontend tests run against a REAL node, not a fake** (CLAUDE §9). Built the smallest real-gateway
  Vitest harness rather than add a `data.fake.ts`/`ingest.fake.ts`.

## Tests

Backend — `cargo test -p lb-role-gateway --test data_console_routes_test` (real node on `mem://`, seeded
via the real `POST /ingest` write path + `tags_add`):

```
running 7 tests
test ingest_write_without_the_cap_is_denied_server_side ... ok
test store_verbs_denied_without_the_admin_cap_the_gate3_relaxation_stays_admin_only ... ok
test series_find_filters_by_tag_facet ... ok
test ws_b_session_cannot_browse_or_read_ws_a ... ok
test write_then_list_latest_and_read_round_trips_over_the_gateway ... ok
test series_read_list_latest_find_each_denied_without_their_cap ... ok
test tables_scan_graph_round_trip_for_an_admin ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
(Mandatory categories present: **capability deny per verb** — `ingest.write` · `series.read/list/latest/
find` · the three `store.*` admin verbs; **workspace isolation** — ws-B sees no ws-A table/rows/series.)

Frontend — default suite `pnpm test` (fake-backed, unchanged): **21 files / 60 tests pass**, incl. the new
`NavGating.test.ts` (member HIDES Data, admin shows it). Real-gateway suite `pnpm test:gateway`
(`*.gateway.test.tsx` against the spawned `test_gateway` node, seeded with real rows):

```
✓ src/features/ingest/IngestView.gateway.test.tsx (4 tests)
✓ src/features/data/DataView.gateway.test.tsx (3 tests)
Test Files  2 passed (2)   Tests  7 passed (7)
```
`tsc --noEmit` clean; `vite build` green with react-flow code-split into its own `DataGraph` chunk.

## Debugging

No formal `debugging/` entry (nothing shipped broken). Notable in-session gotchas, fixed inline:
- `ORDER BY id` requires the idiom in the projection (the `order-by-needs-selected-idiom` rule) — select
  `<string>id AS _oid` and order by that.
- Series record ids are composite arrays (`[series, producer, seq]`) and tag `out` ids too — render ids
  through a `render_id` helper (string verbatim, structured as JSON), not `String`.
- `series.list` `SELECT VALUE … GROUP BY` mis-projects to `{series: None}`; use `SELECT field AS name`.
- react-flow under jsdom needs a `ResizeObserver` polyfill (added to the gateway setup).

## Public / scope updates

Promoted to `public/frontend/data-console.md`. Scope open questions resolved in the scope doc (see below).

## Dead ends / surprises

- **Graph data-shape mismatch (recorded, not a bug):** the tag layer tags the *logical* series name
  (`series:node.cpu_temp`), but the `series` *table* rows have composite ids. So a graph seeded from
  `table=series` draws the composite-id rows as nodes but finds the tag edges only via **per-record
  expand** (`id=series:node.cpu_temp` — the click-to-expand path). This is the first-cut graph shape; the
  test asserts the edge via the expand path.

## Follow-ups

- Raw SurrealQL box — deferred non-goal (separate higher-privilege tool).
- Live "watch this table/series" updates — ride the existing bus/SSE path (the dashboard's motion job), not a poll.
- Member "read-your-own" curated browser (gate-3-enforced) — deferred; start admin-only raw.
- `store.graph` richer relation set (team→member, doc→channel) as those ship as edge records.
- ~~Migrate the rest of the UI suite onto the real-gateway harness + delete the `*.fake.ts`~~ **DONE
  (2026-06-27, same session, follow-on):** all 14 fakes + the dispatcher deleted; `invoke` throws with
  no real node (no fake fallback); test-only `/_seed/*` routes added to the `test_gateway` bin
  (feature-gated `test-harness`) for surfaces with no public create route (real
  `lb_inbox::record`/`lb_outbox::enqueue`/`lb_assets::record_install` writes — seeding, not faking).
  **All 16 fake-backed suites migrated** to `*.gateway.test.ts[x]` (or unit-tested for `agent`, which
  needs a real model provider). Vitest: **6 default + 18 real-gateway = 70 tests green**. The migration
  surfaced real gaps the fakes hid — missing dev-login caps (`store:doc/*`, `store:skill/*`,
  `mcp:workflow.*`) and an empty-channel bus-key bug in `useWorkflow.start()`, both fixed. See
  STATUS.md #00.

---

## Handoff — copy/paste this into a fresh coding session

```
Read docs/HOW-TO-CODE.md and follow it.

Scope: docs/scope/frontend/data-console-scope.md
Stage: read docs/STATUS.md to confirm where we are (S9+ UI, on the shipped S8 data plane).

Build this slice end to end and COMPLETE — both pages, every verb the scope's MCP surface named,
wired store -> cap -> MCP -> gateway route -> ui/src/lib/.../*.api.ts -> UI, not just the easy subset.

Backend (Rust):
- New read-only host verbs for the Data page: store.tables (list tables + row count),
  store.scan(table, limit, cursor) (bounded, id-cursor paging), store.graph(table?, id?, depth)
  (depth/fan-out bounded nodes+edges for react-flow). One verb per file (FILE-LAYOUT). Reuse
  crates/store/src/{open,list,read}.rs (namespace binding + the field-id injection guard). These
  are ADMIN-ONLY: add mcp:store.tables/scan/graph:call to the workspace-admin role in
  role/gateway/src/session/credentials.rs, NOT to member_caps. They deliberately relax the gate-3
  membership check, so keep them read-only and admin-only.
- Expose the already-shipped ingest verbs over the gateway (they exist in crates/host/src/ingest but
  have no routes yet): ingest.write, series.read, series.latest, series.find — add routes in
  role/gateway/src/routes/ + register in server.rs, mirroring routes/assets.rs. Add series.list(prefix)
  (or fold listing into series.find with empty facets — decide and record).
- Gateway route tests (tests/, mirror common/mod.rs harness + admin_routes_test.rs /
  assets_workflow_routes_test.rs): a capability DENY-TEST PER VERB (member token / no cap -> 403,
  opaque) and WORKSPACE-ISOLATION (ws-B token cannot tables/scan/graph ws-A, cannot read/enumerate
  ws-A series). Real node on mem://, seed real records via the real write path (ingest_write/put_doc).

Frontend (ui/):
- ui/src/features/data/ — the Data page: table picker (counts), paged row grid (row-expand to JSON),
  and a react-flow graph view (add @xyflow/react; code-split / load only on this page). Cap-gated nav
  entry (member without the cap never sees it).
- ui/src/features/ingest/ — the Ingest page: series list/search (series.find facets), series detail
  (series.latest + a recent-samples table from series.read, payload rendered by type), and a manual
  "write sample" form (ingest.write). Inline error on a 403.
- ui/src/lib/data/*.api.ts and ui/src/lib/ingest/*.api.ts mirroring the verb names 1:1; wire the new
  commands in ui/src/lib/ipc/http.ts. Add the two surfaces to the nav/App, cap-gated.
- Vitest for BOTH pages against a REAL in-process gateway seeded with REAL rows. DO NOT add a
  data.fake.ts or ingest.fake.ts — the fake backend is on the retirement list (frontend-scope
  "Superseded", CLAUDE §9). Cover: table list + counts, paging, row-expand, graph render+expand,
  series filter, latest+recent render, manual write refreshes the table, deny -> inline error.

One verb per file, <=400 lines, no utils/helpers/common. No if cloud {...}. Respect FILE-LAYOUT.

Then: fill in docs/sessions/ingest/data-console-session.md (this file) with what changed, decisions
(esp. the gate-3-relaxation), and the PASTED GREEN test output; promote shipped truth to
docs/public/frontend/data-console.md + public/SCOPE.md; resolve the scope's open questions; update
docs/STATUS.md. Cross-link scope <-> session <-> public. If a verb must be deferred, say so as an
explicit scope non-goal — never a silent gap.
```
</content>
