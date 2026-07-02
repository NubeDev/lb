---
name: store-read
description: >-
  Read the Lazybones SurrealDB store safely — a bounded, workspace-walled, SELECT-only query surface
  plus schema/table/graph browse verbs. Run a read-only SurrealQL SELECT (`store.query`), read the
  schema (`store.schema`), and browse tables/rows/edges (`store.tables`/`store.scan`/`store.graph`).
  Use when a task says "query the store", "run a SELECT", "read the DB schema", "browse tables/rows",
  "read the graph", "inspect records", or "call store.* verbs". This is the platform-native read path
  (authoritative SurrealDB) — distinct from `federation.query` (external datasources). Every query is
  parse-allowlisted (single SELECT, row/time bound) and workspace-walled at the host.
---

# Reading the store (`store.*`, the native read surface)

SurrealDB is the one datastore and the authority (rule 2). This surface exposes it for **reads only**,
safely: a `store.query` is a single parse-allowlisted `SELECT`, bounded (row cap + timeout) and
workspace-walled at the host, so a caller can read no more than a direct query in their own workspace
could. It's the platform-native path used by dashboards, rules, and the DB-view UI; external databases
go through `federation.query` instead (the datasources skill).

The crates are `rust/crates/host/src/store_query/` (query + schema) and `.../dbview/` (tables/scan/
graph). Two call styles: dedicated `/store/…` REST routes and the `/mcp/call` bridge.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Capabilities: `mcp:store.query:call`, `store.schema:call`, `store.tables:call`, `store.scan:call`,
`store.graph:call`. Workspace-first; denial is opaque.

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| Read-only SELECT | `POST /store/query` | `{"tool":"store.query","args":{"sql":"…","vars":{…}?}}` | `sql`, `vars?` |
| Schema (tables+cols) | `GET /store/schema` | `{"tool":"store.schema","args":{}}` | — |
| List tables | `GET /store/tables` | `{"tool":"store.tables","args":{}}` | — |
| Scan a table (paged) | `GET /store/tables/{table}/rows?limit=&cursor=` | `{"tool":"store.scan","args":{"table":"…","limit":?,"cursor":"…"?}}` | `table`, `limit?` (50), `cursor?` |
| Read the graph | `GET /store/graph` | `{"tool":"store.graph","args":{"table":"…"?}}` | `table?` |

- **`store.query`** runs **one `SELECT`** — parse-allowlisted (no INSERT/UPDATE/DELETE/DDL, no multi-
  statement), row-capped, and timeout-bounded. `vars` is a JSON object of `$`-bound bindings
  (injection-safe — never string-interpolate). It can even `SELECT` from an inline array (no table
  needed), handy for seeding demos.
- **`store.scan`** pages a table: `limit` (default 50) + an opaque `cursor` returned for the next page.
- **`store.graph`** reads edges (optionally scoped to `table`) for a graph view.

```bash
# a bounded SELECT with a bound var
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"store.query","args":{"sql":"SELECT id, ts FROM channel_message WHERE channel = $c ORDER BY ts DESC LIMIT 20","vars":{"c":"general"}}}'

# synthetic rows without a table (real rows from an inline array)
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"store.query","args":{"sql":"SELECT * FROM [{host:\"a\",cpu:12},{host:\"b\",cpu:20}]"}}'

# browse: schema → tables → a page of rows
curl -s http://127.0.0.1:8080/store/schema -H "authorization: Bearer $TOKEN"
curl -s "http://127.0.0.1:8080/store/tables/channel_message/rows?limit=25" -H "authorization: Bearer $TOKEN"
```

## 3. Native vs federated (which read path)

- **`store.query` → platform data** (this surface) — SurrealDB, native and **authoritative**. Use it
  for anything in the platform store: channel messages, dashboards, prefs, series, tags.
- **`federation.query` → external data** (datasources skill) — Postgres/Timescale behind the gated
  `federation` extension. SurrealDB is **never** a federated DataFusion source (the hard line).

A saved PRQL query (`query.*`, the query skill) compiles to and dispatches to whichever of these fits
the target — one authoring surface over both correct paths.

## Gotchas

- **SELECT-only, single statement, bounded** — a write, DDL, or multi-statement is rejected before
  execution; results are row-capped and time-bounded. This is a read surface, not a mutation path.
- **Use `vars`, never string-interpolation** — bindings go through the real parameter path (injection-
  safe). Building SQL by concatenation defeats the gate.
- **Workspace-walled at the host** — a query physically sees only the caller's workspace namespace; a
  ws-B caller can't read ws-A rows regardless of the SQL.
- **`store.scan` is paged** — pass the returned `cursor` for the next page; don't try to pull a whole
  large table in one call (that's what the bound prevents).
- **Denials are opaque** — a missing cap and an empty result look the same.
- **For external DBs use `federation.query`** — `store.*` is platform-only.

## Related

- External reads: `docs/skills/datasources/SKILL.md` (`federation.query`), and one authoring surface
  over both: `docs/skills/query/SKILL.md` (PRQL `query.*`).
- Dashboards bind cells to `store.query`: `docs/skills/dashboard-mcp/SKILL.md` §4.
- Scope: `docs/scope/store/`. README §3 (one datastore, capability-first), §6.1 (the store model),
  §7 (workspace wall).
- Source: `rust/crates/host/src/store_query/`, `rust/crates/host/src/dbview/`; routes in
  `rust/role/gateway/src/server.rs`.
