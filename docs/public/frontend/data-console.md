# Workspace data console (DB browser + ingest explorer)

Status: **shipped** (2026-06-27). Two workspace-scoped, capability-gated shell pages for non-SQL users to
look at and poke at their data. Scope: `../../scope/frontend/data-console-scope.md`. Session:
`../../sessions/ingest/data-console-session.md`.

## What shipped

### Data page — the admin, read-only DB browser

A new, deliberately small **read-only** host surface (`crates/host/src/dbview/`, generic store reads in
`crates/store/src/{tables,scan,graph}.rs`), reached over the gateway:

| Verb | Route | Returns |
|---|---|---|
| `store.tables` | `GET /store/tables` | `[{ table, count }]` — exact `count()` per table (the picker) |
| `store.scan` | `GET /store/tables/{table}/rows?limit=&cursor=` | a bounded page `{ rows:[{id,data}], next }`; hard cap 200, **id-cursor** paging |
| `store.graph` | `GET /store/graph?table=&id=&depth=` | `{ nodes, edges }` for react-flow; depth-1, fan-out ≤50 |

The UI (`ui/src/features/data/`): a table picker with counts → a flat, paged row grid (click a row to
expand its full JSON) → a Grid/Graph toggle that lazy-loads a **react-flow** relation view
(`@xyflow/react`, code-split into its own chunk). No SQL box, **no write verbs** — by design.

**The security decision (the headline):** these verbs deliberately relax the per-record membership gate
(gate 3) that typed verbs like `get_doc` enforce — a raw scan answers "every record in the workspace". So
they are **admin-only**: `mcp:store.tables/scan/graph:call` are granted to the workspace-admin role only,
**never** `member_caps`. Two gates still hold hard: the **workspace wall** (`use_ws` binds the namespace —
a ws-B admin physically cannot scan ws-A) and the **capability** (no grant → opaque `403`). A member never
sees the Data nav entry (cap-gated `store.scan`), and a forged call is denied server-side.

### Ingest page — the series explorer

The S8 `ingest.*`/`series.*` verbs, finally reachable over the gateway (member-level caps):

| Verb | Route | |
|---|---|---|
| `ingest.write` | `POST /ingest` | push one sample by hand — producer = the token's principal (un-spoofable); the route **drains** so the sample is visible on the next read |
| `series.list` | `GET /series?prefix=` | **new small verb** — list series names by prefix |
| `series.find` | `POST /series/find` | tag-faceted discovery (`kind:temperature`) |
| `series.latest` | `GET /series/{s}/latest` | the newest committed sample |
| `series.read` | `GET /series/{s}/samples?from=&to=` | a bounded range, ordered |

The UI (`ui/src/features/ingest/`): a series list/search → a detail pane (latest value + recent-samples
table, payload rendered by type) → a manual "write sample" form.

## Guarantees (tested)

- **Capability deny, one per verb** — every verb refused without its cap, server-side, from the token's
  caps (`role/gateway/tests/data_console_routes_test.rs`); the admin-only `store.*` deny proves the gate-3
  relaxation never leaks to a member.
- **Workspace isolation** — a ws-B session sees none of ws-A's tables, rows, graph, or series.
- **No fake backend** — both the Rust route tests and the UI tests run against a **real node**
  (`mem://`), seeded with real rows through the real write path. The UI's real-gateway Vitest harness
  (`role/gateway/src/bin/test_gateway.rs` + `ui/vitest.gateway.config.ts`, `pnpm test:gateway`) is the
  first step of retiring the `*.fake.ts` layer (STATUS Next-up #00).

## Bounds (load-bearing, not nice-to-have)

`store.scan` hard-caps `limit` at 200 with id-cursor paging; `store.graph` bounds seed (50) and per-node
fan-out (50) at depth-1 (click-to-expand for more); `series.list` caps at 500. A million-row table or a
hub node with thousands of edges can never return the whole tenant.

## Non-goals (deferred, not dropped)

No raw SurrealQL box; no writes through the DB browser; no second datastore; no charts/widgets (that's the
dashboard); no bulk-ingest UI; no device/IoT concepts. Live "watch" updates and a member "read-your-own"
curated browser are recorded follow-ups.
