# Datasources scope — `federation.sample`, one AI-ready snapshot of a source

Status: scope (the ask). Promotes to `public/datasources/datasources.md` once shipped.

We want **one call** that returns everything an AI needs to write SQL against a registered
datasource: the tables, their columns/types, the relationships between them, and a `LIMIT 10`
sample of real rows per table — as a single, bounded, prompt-ready JSON snapshot. Today an agent
must fan out N+1 `federation.schema` calls (one list + one per table), gets **no relationship
metadata at all** (the ERD infers joins UI-side from column naming —
`ui/src/features/datasources/erd/schemaToFlow.ts`), and has no sanctioned way to see example
values, so it guesses at formats (date encodings, enum-ish strings, id shapes) and writes wrong
queries. `federation.sample {source}` closes all three gaps in one round trip.

> Read with: `datasources-scope.md` (the parent — the `federation` extension, `federation.query`/
> `federation.schema`, the gated pipeline this verb reuses verbatim), `sqlite-datasource-demo-scope.md`
> (the shipped `sqlite` kind this will be demoed against), `../testing/testing-scope.md`.

---

## Goals

- **One MCP verb, `federation.sample {source, tables?, limit?}`** → a single JSON snapshot:
  per table its columns (name/type/nullable), its foreign keys, and up to `limit` (default 10)
  real rows; plus a top-level `relationships` list. Callable by the agent, the UI, and rules
  exactly like `federation.query` (rule 7: MCP is the contract).
- **Real relationships where the source knows them.** SQLite (`PRAGMA foreign_key_list`) and
  Postgres (catalog FK read) expose true foreign keys; the sidecar reads them best-effort via a
  new `Source::foreign_keys(table)` method (empty list for kinds that can't answer — the AI can
  still infer from column names, same as the ERD does).
- **Bounded and prompt-sized.** Hard caps host-side: `limit ≤ 50` rows/table, at most 25 tables
  per call (deterministic order, `truncated: true` flagged when cut), and long cell values
  truncated (~256 chars) in the sidecar. The snapshot must reliably fit in a model prompt.
- **Zero new privilege.** Sampling = the same read privilege as a live query; authorized under
  the existing `mcp:federation.query:call` cap, same as `federation.schema` chose (and for the
  same reason: no new grant/rollout).

## Non-goals

- **Not a paging/export API.** `limit` is capped low by design; bulk reads stay on
  `federation.query` + the paging scopes, bulk copies on `federation.mirror`.
- **No statistics/profiling** (row counts, min/max, cardinality). Useful later; a sample of
  real rows is the 80% for SQL-writing and keeps the call one cheap pass.
- **No automatic PII masking.** Redaction beyond a column-name denylist (below) is out of scope;
  the cap gate is the access control, same as `federation.query` (which can already read every row).
- **Replacing the ERD's inference.** The ERD keeps its naming heuristic as fallback; a follow-up
  can prefer real FKs from this verb.

## Intent / approach

A sibling of `federation.schema`, not a new pipeline. Host side
(`rust/crates/host/src/federation/sample.rs`): authorize under the read cap → `resolve` the
source in the caller's workspace → `enforce_endpoint` (`net:*`) → `mediate_dsn` → **one**
`call_sidecar(…, "federation.sample", …)` → return the sidecar's JSON. Sidecar side
(`rust/extensions/federation/src/sample.rs`): `list_tables`, then per table read the provider
schema (the `describe_table` read), `foreign_keys(table)` (new trait method, per-kind impl in
`source/sqlite.rs` / `source/postgres.rs`), and run `SELECT * FROM t LIMIT n` through the
existing engine path. One sidecar call — not N host↔sidecar round trips — because the pools and
the engine already live there.

Result shape (columns-and-rows per table, matching `federation.query`'s frame style):

```json
{
  "source": "warehouse",
  "tables": [
    {
      "name": "orders",
      "columns": [{ "name": "id", "type": "Int64", "nullable": false }, …],
      "foreign_keys": [{ "column": "customer_id", "ref_table": "customers", "ref_column": "id" }],
      "rows": { "columns": ["id", "customer_id", …], "values": [[1, 42, …], …] },
      "row_limit": 10
    }
  ],
  "relationships": [{ "from": "orders.customer_id", "to": "customers.id", "kind": "foreign_key" }],
  "truncated": false
}
```

**Alternative rejected:** having the agent (or UI) compose this from `federation.schema` +
per-table `federation.query … LIMIT 10` calls. It burns one agent turn per table, re-runs the
full gate/DSN mediation N+1 times, still yields no FK metadata, and every caller re-invents the
bounding/truncation. One verb, one bounded pass, one contract.

## How it fits the core

- **Tenancy / isolation:** the source alias resolves **in the caller's workspace** via the same
  `resolve()`; a cross-workspace alias is `NotFound`. Isolation test mandatory.
- **Capabilities:** gated by existing `mcp:federation.query:call`; deny is the same opaque
  `Denied`. Deny test mandatory. No new cap (mirrors `federation.schema`'s decision).
- **Placement:** either role — it's a sidecar call like every federation verb (rule 1).
- **MCP surface (§6.1):** one read verb (a composite **get**), no CRUD/feed/batch. Synchronous is
  correct because the call is hard-bounded (≤25 tables × ≤50 rows, one pooled connection); it can
  never "run long" past the caps, so no job. A palette/agent `ToolDescriptor` with a real arg
  schema (`{source, tables?, limit?}`, `x-lb entity: datasource` on `source`) ships alongside,
  like `schema_descriptor()` — a name-only row leaves the model guessing arg names.
- **Data (SurrealDB):** none written; reads only the `datasource:{ws}:{name}` record. External
  rows pass through, never persisted (state stays state).
- **Bus (Zenoh):** N/A — request/response over the existing sidecar seam.
- **Secrets:** DSN mediated host-side under the federation extension's grant, never returned —
  unchanged from `federation.schema`.
- **Stateless extension / symmetric nodes / one datastore:** unchanged; the sidecar keeps only
  its existing pools.
- **SDK/WIT impact:** none — Tier-2 native protocol, additive verb + additive `Source` trait
  method (default impl returns `[]`, so third-party kinds don't break).
- **FILE-LAYOUT:** one new file per responsibility — host `federation/sample.rs` (verb +
  descriptor), sidecar `sample.rs`, per-kind `foreign_keys` in the existing source files; a new
  match arm each in `federation/tool.rs` and sidecar `main.rs`.
- **Skill doc:** drivable surface → the implementing session updates
  `docs/skills/datasources/SKILL.md` with a live `federation.sample` run (the "give the AI
  context" recipe is exactly what the skill should teach).

## Example flow

1. Admin has registered `warehouse` (sqlite kind) in workspace `acme`; endpoint approved,
   DSN in `lb-secrets`.
2. The agent (or the Datasource UI's "Copy AI context" button, a follow-up) calls
   `federation.sample {source: "warehouse"}`.
3. Host: cap check (`mcp:federation.query:call`) → resolve `datasource:acme:warehouse` →
   `net:*` check → mediate DSN → one sidecar call.
4. Sidecar: lists 6 tables; per table reads columns, `PRAGMA foreign_key_list`, and
   `SELECT * LIMIT 10`; truncates a 4 KB `notes` value to 256 chars; returns the snapshot.
5. The caller pastes/feeds the JSON into the model prompt; the model writes a correct join
   (`orders.customer_id = customers.id`) on the first try because the FK and sample values
   are in front of it.

## Testing plan

Per `scope/testing/testing-scope.md` — real store, real sidecar, seeded records, no fakes:

- **Capability deny (mandatory):** a principal without `mcp:federation.query:call` gets the
  opaque `Denied`.
- **Workspace isolation (mandatory):** ws-B caller sampling ws-A's alias → `NotFound`.
- **E2E happy path:** extend `federation_sqlite_test.rs` — seed a SQLite file with two tables +
  a real FK + >10 rows; assert tables, columns, the FK in `relationships`, exactly 10 rows,
  and no DSN anywhere in the output.
- **Bounds:** `limit` capped at 50; a 30-table source returns 25 + `truncated: true`; a long
  text cell comes back truncated.
- **Best-effort catalog:** one unreadable/empty table doesn't fail the snapshot (mirrors
  `info_schema.rs`'s stance).
- **`tables` filter:** `{tables: ["orders"]}` samples only that table.
- Offline/sync and hot-reload: N/A (stateless request/response verb).

## Risks & hard problems

- **Sample rows can carry sensitive values.** Anyone with the read cap can already `SELECT`
  them, so this adds reach, not privilege — but a *snapshot destined for an AI prompt* travels
  further than a query result. Mitigation in-scope: a small column-name denylist
  (`password|secret|token|api_key|hash`) whose cells are emitted as `"«redacted»"`. Anything
  smarter is an open question.
- **Snapshot size.** A wide table (200 columns) × 10 rows can still be big despite caps;
  cell truncation is the backstop, but verify a worst-case snapshot stays well under ~100 KB.
- **FK reads are per-dialect.** `PRAGMA` vs `pg_catalog` — keep each inside its `Source` impl;
  a kind that can't answer returns `[]`, never an error.

## Open questions

- Should the denylist be admin-configurable per datasource (a field on the `datasource` record)
  or a fixed built-in list to start? Recommend: fixed list first, record field when asked for.
- `ORDER BY` for the sample: first-N in natural order is cheapest but can be unrepresentative
  (all one tenant's rows). Recommend: natural order for v1; note `TABLESAMPLE`/random as follow-up.
- Should the ERD (`SchemaErd`) switch to real FKs from this verb when available? Recommend yes,
  as a separate UI slice.

## Related

- `datasources-scope.md` — the parent extension, `federation.query`/`schema` pipeline.
- `sqlite-datasource-demo-scope.md` — the demo source to exercise this against.
- `rust/crates/host/src/federation/schema.rs` — the sibling verb this mirrors.
- `rust/extensions/federation/src/info_schema.rs` — the existing best-effort catalog stance.
- `ui/src/features/datasources/erd/schemaToFlow.ts` — today's inferred-only relationships.
- `docs/skills/datasources/SKILL.md` — updated on ship with a live run.
- README §3 (rules 5–7, 9), §6.5.
