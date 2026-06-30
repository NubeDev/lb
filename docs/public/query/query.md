# Query — saved PRQL queries (public)

Status: **Phase 1 shipped (2026-06-30).** Scope: `../../scope/query/prql-query-scope.md`. Session:
`../../sessions/query/prql-query-session.md`.

A workspace **authors a query once, in PRQL, saves it as an editable named record, and runs it against
any source** — the SurrealDB-native store or a registered external datasource — through one MCP verb
family. PRQL is the human-friendly authoring layer; the platform compiles it to the right dialect at
run time and **dispatches to the query engines that already exist** (`store.query` for the platform,
`federation.query` for external). SurrealDB stays the one datastore (rule 2); external DBs stay
federated sources behind the gated `federation` extension. **No new engine, no second authority.**

## Doctrine

PRQL is a *front end* that compiles to a dialect and hands the result to a verb that already enforces
the wall. `query.run` **composes** the target's existing capability — it never widens it (rule 5).

1. **Authoring** is PRQL (or `lang:"raw"` for a dialect-specific escape hatch). One language, learned
   once.
2. **Persistence** is a workspace record `query:{ws}:{id}` — the established saved-artifact pattern
   (`datasource:{ws}:{name}`, `rule:{ws}:{id}`): soft-delete, `ts`, workspace-keyed, capability-gated.
3. **Execution** compiles PRQL→SQL for the target's dialect, then dispatches to the existing engine:
   `store.query` (platform, SurrealQL) or `federation.query` (external). The target's existing
   capability, SELECT-only, and row-cap gates all still apply.

## The MCP surface (`query.*`)

One capability per verb (`mcp:query.<verb>:call`); every verb is workspace-walled and re-authorizes.

| Verb | Input | Returns | Notes |
|---|---|---|---|
| `query.save` | `{id, name?, description?, lang, text, target, params?}` | `{id}` | Upsert by `id` (overwrite in place, no revision history in v1). |
| `query.get` | `{id}` | the full record | For re-opening in the editor. |
| `query.list` | `{}` | `{queries:[{id,name,target,lang,ts}]}` | Flat roster — no text, no result data. |
| `query.delete` | `{id}` | `{ok:true}` | Soft-delete (tombstone); idempotent. |
| `query.compile` | `{lang, text, target}` | `{sql}` | Pure dry-run; own cap only; no data access. |
| `query.run` | `{id}` **or** `{lang, text, target}` (+ `vars?`) | `{columns, rows}` | Compile→dispatch. See no-widening below. |

- **`id`** is a kebab-case slug unique per workspace (the record key); **`name`** is the editable
  display label (mirrors the rules `id`+`name` pattern).
- **`lang`** is `"prql"` (compiles to the target's dialect) or `"raw"` (carries target-native text
  verbatim — raw SurrealQL for platform, raw SQL for a datasource — the escape hatch).
- **`target`** is `"platform"` (→ `store.query`) or `"datasource:<name>"` (→ `federation.query`); the
  datasource must be registered in the **caller's** workspace.

### No-widening (rule 5) — the headline rule

`query.run` requires `mcp:query.run:call` **AND** the underlying target cap:

- platform target → additionally `mcp:store.query:call`;
- datasource target → additionally `mcp:federation.query:call`.

Holding `query.run` **alone** is denied — even with the sidecar present, even before the datasource is
resolved. The check runs before compile/resolution so the deny is unconditional. `query.compile` needs
only its own cap (it never reaches an engine).

### Params (injection-safe)

`$var` binds through the engine's **real parameter path**, never string interpolation:

- **platform** — through `store.query`'s `vars` (SurrealDB `$var` binding). A missing or extra param
  is a typed error.
- **datasource** — the `federation.query` sidecar has no bind-param path in v1, so a parameterized
  datasource query is a typed error (loud, never interpolation) until the sidecar grows one.

## Targets — uniform to the author, correct beneath

- **Platform (`target:"platform"`)** — Phase 1: PRQL compiles to `sql.generic` and runs through the
  existing `store.query` read-only parse-allowlist (single SELECT, 10k-row / 5s bound). The relational
  subset (`from / filter / select / aggregate / sort / take`) maps cleanly; anything outside it is
  rejected by that gate, and the author drops to `lang:"raw"` SurrealQL for it. (Selecting a column
  that is NOT the table name is required — PRQL emits `SELECT *` for a bare `from <table-name>`, which
  pulls SurrealDB's record `id`.)
- **Datasource (`target:"datasource:<name>"`)** — PRQL compiles with the datasource's dialect
  (`postgres`/`mysql`/`duckdb` from `datasource.kind`), then `federation.query` runs it. Every existing
  federation wall (workspace-pin, `net:*`, secret mediation, SELECT-only re-validation, row cap)
  applies unchanged.

## Rules reuse saved queries by name

A rule's `source("query:<name>")` resolves to `query.run {id:<name>}`, so a saved query is a reusable,
centrally-editable data definition rules share — not SQL duplicated per rule. The collect routes
through the ONE MCP contract, so the rule runs under `caller ∩ grant` (the rule principal needs
`query.run` + the target cap — the no-widening rule holds inside rules too).

## Placement

`either`, by config (rule 1). The crate + host service are in the symmetric node binary; whether a
datasource target resolves depends only on whether `federation` is installed and granted (config/role).
The platform target works on any node. No `if cloud`.

## What's NOT in v1 (follow-ons)

- **Phase 2 full PRQL on platform** — DataFusion-as-compute over native `store.query` reads for the
  joins/window-functions/CTEs SurrealQL can't express (scope open question, deferred by decision).
- **Datasource param binding** — pending a `federation.query` sidecar bind-param path.
- **Folders/tags**, **dashboard widget / channel `/query` binding** — follow-on scopes.
- **Writes / DML** — out of scope (PRQL is read; both engines are read-first).