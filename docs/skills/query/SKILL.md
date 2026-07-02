---
name: query
description: >-
  Author, save, and run Lazybones queries over the node gateway. Write once in PRQL (or `raw` for a
  dialect-specific escape hatch), save it as an editable `query:{ws}:{id}` record, and run it against
  the platform store OR a registered external datasource through one MCP verb family (`query.save/get/
  list/delete/run/compile`). Use when a task says "save a query", "run a PRQL query", "compile PRQL",
  "query Postgres/the store by name", "edit a saved query", or "reuse a query from a rule". PRQL is the
  authoring layer; the host compiles to the target's dialect and dispatches to the engines that already
  exist (`store.query` / `federation.query`) — no new engine, no new authority.
---

# Saved queries (`query.*`, PRQL → the existing engines)

Author a query **once in PRQL**, save it as an **editable named record**, and run it against any
target — the SurrealDB-native store or a registered external datasource — through one MCP verb family.
PRQL is the human-friendly authoring layer; the host compiles it to the right dialect at run time and
**dispatches to the query engines that already exist**: `store.query` for the platform,
`federation.query` for an external datasource (see the `datasources` skill). This adds **no new query
engine and no new authority** — SurrealDB stays the one datastore; external DBs stay federated behind
the gated `federation` extension.

- The pure compiler: `rust/crates/prql/` (`lb-prql`, wraps `prqlc`, zero I/O).
- The host service: `rust/crates/host/src/query/` (one verb per file) — owns the saved record and the
  compile→dispatch.
- The record: `query:{ws}:{id}` → `{id, name, description, lang, text, target, params, removed, ts}`.

**`query.*` is reached only over the `POST /mcp/call` bridge** (no dedicated REST routes — unlike
`datasource.*`/`prefs.*`). Workspace + principal come from the token (the hard wall); each verb
authorizes first, denials are opaque.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Capabilities — one per verb: `mcp:query.save:call`, `query.get`, `query.list`, `query.delete`,
`query.run`, `query.compile`. **`query.run` COMPOSES, never widens (rule 5):** the caller needs
`mcp:query.run:call` **AND** the underlying target cap — `mcp:store.query:call` for `target:"platform"`
or `mcp:federation.query:call` for a `datasource:` target. Holding `query.run` alone, without the
target cap, is denied (the headline no-widening rule). `query.compile` is pure (no data access) and
needs only its own cap.

## 2. The verbs (all via `POST /mcp/call`)

| Verb | Args | Result |
|---|---|---|
| `query.save` | `id, name?, description?, lang, text, target, params?, ts` | `{id}` (upsert by id — create or edit) |
| `query.get` | `id` | the full record (re-open in an editor) |
| `query.list` | — | `{queries:[{id,name,target,lang,ts}]}` (flat roster, no text/results) |
| `query.delete` | `id, ts` | `{ok:true}` (soft — `removed=true`) |
| `query.compile` | `lang, text, target` | `{sql}` or a typed error — **dry-run, no execution, no target cap** |
| `query.run` | `{id}` OR `{lang, text, target, params?}`, `vars?`, `ts` | `{columns, rows}` |

- **`lang`** is `"prql"` (default, portable) or `"raw"` (target-native escape hatch: raw SurrealQL for
  `platform`, raw SQL for a datasource — for what PRQL can't express).
- **`target`** is `"platform"` (→ `store.query`, SurrealDB-native) or `"datasource:<name>"`
  (→ `federation.query`, external), where `<name>` is a datasource registered in the **caller's**
  workspace (un-spoofable, host-resolved).
- **`ts`** is a caller-supplied logical timestamp (determinism, README §3) — pass a real value.
- **`vars`** is an object of `$`-bound bindings for a parameterized query (injection-safe, mirrors
  `store.query`'s `vars`).

## 3. Compile-preview → save → run → re-edit

```bash
BASE=http://127.0.0.1:8080/mcp/call
auth=(-H "authorization: Bearer $TOKEN" -H 'content-type: application/json')

# 1. compile-preview (no execution, no data access) — feeds the editor's live validate
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"query.compile","args":{
  "lang":"prql","target":"datasource:warehouse",
  "text":"from orders | filter status == \"paid\" | group store (aggregate { rev = sum amount }) | sort {-rev}"}}'
# → {"sql":"SELECT store, SUM(amount) AS rev FROM orders WHERE status = 'paid' GROUP BY store ORDER BY rev DESC"}

# 2. save (upsert by id) — now appears in query.list
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"query.save","args":{
  "id":"store-revenue","name":"Store revenue","lang":"prql","target":"datasource:warehouse",
  "text":"from orders | filter status == \"paid\" | group store (aggregate { rev = sum amount }) | sort {-rev}",
  "ts":1719800000000}}'

# 3. run — compiles for the target dialect, authorizes query.run AND the target cap, dispatches
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"query.run","args":{"id":"store-revenue","ts":1719800001000}}'
# → {"columns":["store","rev"],"rows":[["north",980400],…]}

# 4. re-open, edit, save the SAME id in place (the re-edit ask)
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"query.get","args":{"id":"store-revenue"}}'
```

**Platform target** — a saved query with `target:"platform"` compiles PRQL → `sql.generic` and runs
through the existing `store.query` read-only parse-allowlist (single `SELECT`, 10k-row / 5s bound). A
SurrealQL-only feature (graph traversal, `FETCH`) uses `lang:"raw"` against `target:"platform"`.

**Ad-hoc run** — pass `{lang, text, target}` inline instead of `{id}` for an unsaved one-shot.

## 4. Reuse from a rule

A rule references a saved query by name: `source("query:store-revenue")` resolves to `query.run
{id:"store-revenue"}`, collecting the grid — a shared, centrally-edited data definition instead of SQL
duplicated per rule. The rule runs under `caller ∩ grant`; `query.run` re-checks the target cap inside
the collect. (See `docs/scope/rules/rules-engine-scope.md`.)

## Gotchas

- **`query.run` composes, never widens** — you need `query.run` **and** the target cap
  (`store.query` for platform, `federation.query` for a datasource). `query.run` alone → denied.
- **`query.*` is MCP-only** — no dedicated gateway REST routes.
- **PRQL → SurrealQL is a subset (Phase 1)** — only `from/filter/select/aggregate/sort/take` map
  cleanly; anything else is rejected by the `store.query` gate. Drop to `lang:"raw"` SurrealQL for the
  rest. PRQL → external SQL (DataFusion/Postgres) is the clean, full path.
- **Read-first / SELECT-only** — PRQL can't express DML, and a `raw` write is rejected by the engine
  gate. Results are row-capped by the underlying engine; nothing is stored by `query.run`.
- **Params are injection-safe** — `vars` binds through the engine's real parameter path. On the
  **datasource** path the `federation.query` sidecar has no bind-param path yet, so a parameterized
  datasource query is a **typed error** (loud, never string-interpolated) until it grows one.
- **Workspace wall at resolution** — `id` and a `datasource:` target resolve only within the caller's
  workspace; ws-B can neither `get`/`run` a ws-A query nor reach a ws-A datasource.
- **`id` is a kebab-case slug** unique per workspace (the record key); `name` is the editable display
  label. `save` upserts by `id`; edits overwrite in place (no version history in v1).
- **`ts` defaults to 0** on the bridge if omitted — pass a real monotone value.

## Related

- Scope + shipped doc: `docs/scope/query/prql-query-scope.md`,
  `docs/sessions/query/prql-query-session.md`, `docs/public/query/query.md`.
- The datasource engine a `datasource:` target dispatches to: `docs/skills/datasources/SKILL.md`,
  `docs/scope/datasources/datasources-scope.md` (+ the hard line: SurrealDB is never a DataFusion
  *source*).
- The `store.query` read-only gate the platform target reuses: `rust/crates/host/src/store_query/`.
- Rules `source("query:<name>")` seam: `docs/scope/rules/rules-engine-scope.md`.
- Rules 2/5/6/7 + §6.3: `README.md` §3. Upstream: PRQL / `prqlc` (embedded in `lb-prql`).
