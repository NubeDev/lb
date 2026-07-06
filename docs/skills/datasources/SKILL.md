---
name: datasources
description: >-
  Manage Lazybones external SQL datasources over the node gateway via the `federation` extension —
  register/remove/list/test a Postgres or SQLite source, run a SELECT-only `federation.query`, browse
  a source's tables/columns with `federation.schema`, snapshot tables + foreign keys + sample rows
  for an AI prompt with `federation.sample`, and mirror an external range into the platform
  series plane with `federation.mirror`. Use when a task says "connect an external database",
  "add/register a datasource", "query Postgres/Timescale over the API", "test a datasource",
  "federate", "mirror an external table", "give the AI a sample of a datasource", or "call
  federation/datasource verbs over REST/MCP". SurrealDB
  stays the platform authority; external DBs are federated sources reached only through this gated,
  `net:*`-bounded, secret-mediated extension.
---

# Managing external SQL datasources (the `federation` extension)

External SQL sources (Postgres / Timescale / a SQLite file) are **not** wired into core — that would
break "one datastore" (rule 2). Instead a **native (Tier-2) `federation` extension** embeds
DataFusion + its connectors as a *library*, owns the connection pools behind one `Source` trait, and
exposes external data through a small set of gated MCP verbs. SurrealDB stays the platform authority;
an external DB is a *federated source*, reached only through this supervised, `net:*`-bounded,
secret-mediated process.

- **The extension** — `rust/extensions/federation/` (`extension.toml` + `src/`): a JSON-RPC stdio
  sidecar embedding `datafusion` + `datafusion-table-providers` behind `source/{postgres,sqlite}.rs`,
  with a SELECT-only validator (`src/validate.rs`). Heavy DB drivers live ONLY here — core links none.
- **The host service** — `rust/crates/host/src/federation/` (one verb per file): resolves the source
  in the caller's workspace, enforces `net:*` pre-connect, mediates the DSN out of `lb-secrets`,
  routes to the supervised sidecar, and re-validates SELECT-only.

Two equivalent call styles (as with `dashboard-mcp` / `channels-inbox-outbox`):

1. **Dedicated REST routes** — the first-party admin page's surface (`/datasources…`).
2. **The universal MCP bridge** — `POST /mcp/call {tool, args}` for ANY verb by dotted name
   (`federation.query`, `federation.schema`, `datasource.list`, …). This is rule 7.

Both derive the **workspace + principal from the bearer token** — never from the body (the hard wall,
README §6/§7). Every verb is capability-gated server-side; a denial is **opaque**.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Send it as `Authorization: Bearer $TOKEN` on every call. Capabilities:

- **Read/query** — `mcp:federation.query:call`, `mcp:federation.schema:call`,
  `mcp:datasource.list:call`, `mcp:datasource.test:call`.
- **Admin (register/remove)** — `mcp:datasource.add:call`, `mcp:datasource.remove:call`. To hand a
  DSN at `add`, the admin also needs `secret:federation/*:write` (the host mediates it into the
  secret store; the record keeps only the ref).
- **The extension's OWN install grant** — `net:tls:*:*:connect` (admin-approved **per endpoint**,
  enforced PRE-CONNECT by the host) and `secret:federation/*:get` (so the host can pull the DSN and
  hand it to the pool). A source whose `host:port` the grant omits is refused — opaquely, even though
  the source is registered.

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| Register source (admin) | `POST /datasources` | `{"tool":"datasource.add","args":{…}}` | `name,kind,endpoint,secret_ref?,dsn?,ts*` |
| Remove source (admin) | `DELETE /datasources/{name}` | `{"tool":"datasource.remove","args":{"name":"…","ts":…}}` | `name,ts*` |
| List sources | `GET /datasources` | `{"tool":"datasource.list","args":{}}` | — |
| Test connectivity | `POST /datasources/{name}/test` | `{"tool":"datasource.test","args":{"source":"…","ts":…}}` | `source,ts*` |
| Query (SELECT-only) | *(MCP only)* | `{"tool":"federation.query","args":{…}}` | `source,sql,ts*` |
| Browse schema | *(MCP only)* | `{"tool":"federation.schema","args":{…}}` | `source,table?,ts*` |
| AI-context snapshot | *(MCP only)* | `{"tool":"federation.sample","args":{…}}` | `source,tables?,limit?,ts*` |
| Mirror → series plane | *(MCP only)* | `{"tool":"federation.mirror","args":{…}}` | `source,query,target_series,job_id,range?,ts*` |

`* ts` — a caller-supplied millisecond logical timestamp (determinism, README §3: no wall-clock
inside a verb). The **dedicated REST routes fill it from the gateway clock**, so their bodies OMIT
it; the **`/mcp/call` path requires you to pass `ts`** in `args`. `federation.query` / `.schema` /
`.mirror` have **no REST route** — they are reached only over `/mcp/call`.

`kind` is `postgres` or `sqlite`. `source` (query/test) and `name` (add/remove) both name a
registered source **within the caller's workspace** — a caller cannot name a cross-tenant source.

## 3. The datasource record

`datasource.add` upserts a `datasource:{ws}:{name}` store record holding **kind + endpoint + a secret
REF — never the DSN**. The DSN, if handed at `add`, is mediated into `lb-secrets` under
`secret:federation/{name}` and dropped from the record. `datasource.list` returns the sources with
the secret **redacted**; the DSN is absent from every list and every query result, and is never
logged.

```bash
# register a Postgres source (DSN mediated into the secret store; record keeps only the ref)
curl -s -X POST http://127.0.0.1:8080/datasources -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "name":"warehouse","kind":"postgres","endpoint":"db.acme.internal:5432",
  "dsn":"postgres://ro:secret@db.acme.internal:5432/analytics"}'

# a real connectivity probe (green/red) — routes through the supervised sidecar
curl -s -X POST http://127.0.0.1:8080/datasources/warehouse/test -H "authorization: Bearer $TOKEN"

# list (DSN redacted)
curl -s http://127.0.0.1:8080/datasources -H "authorization: Bearer $TOKEN" | jq '.datasources'
```

## 4. Query & browse (read-first)

`federation.query` runs a **single SELECT-only statement** (validated host-side *and* in the sidecar
via `sqlparser`; INSERT/UPDATE/DELETE/DDL/multi-statement are rejected before execution) and returns
`{columns, rows}`. `federation.schema` is the no-SQL browse path: omit `table` to list the source's
tables, pass one to describe its columns.

```bash
# SELECT-only query against a registered source
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"federation.query",
  "args":{"source":"warehouse","sql":"SELECT id, region, revenue FROM sales LIMIT 100","ts":1719800000000}}'
# → {"columns":["id","region","revenue"],"rows":[[…],…]}

# browse: list tables, then describe one
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"federation.schema","args":{"source":"warehouse","ts":1719800000000}}'
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"federation.schema","args":{"source":"warehouse","table":"sales","ts":1719800000000}}'
```

`federation.sample` is the **AI-context snapshot** (datasource-samples scope): one call returning,
for every table (or just `tables:[…]`), its columns, its **real foreign keys** (SQLite
`PRAGMA foreign_key_list`, Postgres `information_schema` — best-effort, `[]` where the kind can't
answer), and up to `limit` (default 10, cap 50) sample rows — bounded to 25 tables
(`truncated:true` when cut), long cells truncated, and columns named like
`password`/`secret`/`token`/`api_key` redacted as `«redacted»`. Feed the result to a model before
asking it to write SQL. It rides the **same `mcp:federation.query:call` cap** as query/schema.

```bash
# one AI-ready snapshot: tables + columns + foreign keys + LIMIT-10 rows
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"federation.sample","args":{"source":"warehouse","ts":1719800000000}}'
# → {"source":"warehouse","tables":[{"name":"sales","columns":[…],"foreign_keys":[…],
#     "rows":{"columns":[…],"values":[[…],…]},"row_limit":10}],
#    "relationships":[{"from":"sales.customer_id","to":"customers.id","kind":"foreign_key"}],
#    "truncated":false}
```

Dashboards read a federated source the same way — a cell with `tool:"federation.query"` and
`datasource:{type:"federation","uid":"datasource:<ws>:<name>"}` (see `dashboard-mcp` §4).

## 5. Federate vs mirror

Two distinct paths, kept distinct on purpose:

- **`federation.query`** reads the external DB **live** — nothing lands in the platform store.
- **`federation.mirror`** is a durable **`lb-jobs` batch** that runs `query` against the source and
  `ingest.write`s a bounded range into the platform **series plane** (`target_series`). It
  checkpoints a cursor and **resumes mid-range** on restart; ingest dedup on `(series, producer,
  seq)` prevents double-writing. Use it to pull an external range into native time-series storage
  once, rather than re-federating on every read.

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"federation.mirror",
  "args":{"source":"warehouse","query":"SELECT ts, value FROM metrics ORDER BY ts",
          "target_series":"warehouse.metrics","job_id":"mirror-warehouse-metrics",
          "range":100000,"ts":1719800000000}}'
```

## Doctrine held (why it is shaped this way)

- **SurrealDB is NEVER a DataFusion source** (rule 2) — platform data is read natively; only external
  DBs are federated. Heavy DB drivers link ONLY in the extension crate.
- **Workspace-pinned at the host** (rule 6, §7) — `{source}`/`{name}` resolve only to a
  `datasource:{ws}:{name}` in the caller's workspace; ws-B cannot resolve or query a ws-A source.
- **SELECT-only, read-first** — a write/DDL is rejected before execution. A `federation.write` would
  be a separate, later, Ask-gated verb.
- **Secret mediation** — the DSN flows host→child in the per-call input (not spawn env), lives only
  inside the pool, and never touches a record, a log, a query result, or `datasource.list`.
- **`net:*` per-endpoint, pre-connect** — the admin approves a concrete `host:port`; the host refuses
  a connect the grant omits, opaquely.

## Gotchas

- **Workspace/owner come from the token**, never args. To act in another workspace, `login` into it.
- **`ts` is required on `/mcp/call`** and filled by the gateway on the REST routes.
- **`federation.query` / `.schema` / `.mirror` are MCP-only** — no dedicated REST route (only
  `datasource.add/remove/list/test` have REST routes).
- **SELECT-only** — a non-read statement is refused host-side before the sidecar ever runs.
- **Most denials are opaque** — a missing cap, a missing source, and a `net:*`-omitted endpoint all
  look the same (forbidden/absent). A registered source can still be refused if its endpoint isn't in
  the extension's admin-approved `net:*` grant. **Exception:** a source registered *without* a DSN
  returns a distinct `datasource has no configured connection (add or update its DSN)` — not a
  capability deny — so "no DSN" is not confused with "not allowed".
- **The DSN is never returned** — if you need to change it, re-`add` (upsert) with a new `dsn`. This
  is collision-free by design: every DSN secret is owned by the stable `ext:federation` principal, not
  the admin who ran `add`, so ANY admin can update or remove a source (a store seeded under an older
  bootstrap owner self-heals on the next `add`). A missing federation install still denies (no runtime).
- **`add` needs `secret:federation/*:write`** to hand a DSN; without it, register the `secret_ref`
  and set the secret out of band.
- **Postgres is a feature-gated build** (`--features postgres`, vendored OpenSSL); the default sidecar
  build is SQLite-only — the documented fallback where the TLS connector can't compile.

## Related

- The extension: `rust/extensions/federation/extension.toml` + `src/` (sidecar); the host service:
  `rust/crates/host/src/federation/` (one verb per file).
- Scope + sessions: `docs/scope/datasources/datasources-scope.md`,
  `docs/sessions/datasources/datasources-session.md`, `.../federation-session.md`.
- Reading a federated source in a dashboard: `docs/skills/dashboard-mcp/SKILL.md` §4.
- Design note: `docs/vision/0003-*` (the federation plane); capability/workspace rules: `README.md`
  §3, §6.5, §7; `docs/scope/auth-caps/`.
- Gateway routes: `rust/role/gateway/src/routes/datasources.rs`, `server.rs`.
