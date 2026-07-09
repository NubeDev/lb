# Datasources scope — schema designer + the federation write plane

Status: scope (the ask). Promotes to `doc-site/content/public/datasources/` once shipped.

We want to **design a database schema visually** (tables, columns, PKs, FKs/relationships on a
canvas) inside the Datasources UI, store that design as a workspace record, **apply it to an external
database** (Postgres first) as a migration, and then **write data to that database** — per-message
from a flow, and in bulk as a durable job. Today the `federation` extension is read-only
(`federation.query`/`schema`/`sample` + the inbound `federation.mirror`); this scope adds its
deliberately-deferred **write plane** (`datasources-scope.md` → Resolved decisions: "a
`federation.write` is a separate, later, Ask-gated verb") plus the schema-as-a-record layer the UI
and the flow node both consume. SurrealDB stays the sole platform authority; the external DB remains
a downstream federated source/**sink** — never a second authority.

> Read with: [`datasources-scope.md`](./datasources-scope.md) (the federation extension this
> extends), `../extensions/reference-extensions-scope.md` (native Tier-2 doctrine, `net:*`),
> `../jobs/jobs-scope.md` (the export job), `../flows/extension-nodes-scope.md` +
> `../flows/flows-scope.md` (how a flow reaches a verb), `../ingest/ingest-scope.md` (the inbound
> mirror this scope is the dual of).

## Goals

- **Schema as a workspace record.** `db_schema:{ws}:{name}` — the designed schema document:
  `{dialect, tables: [{name, columns: [{name, type, nullable, default?}], pk: [cols]}], fks:
  [{from: {table, cols}, to: {table, cols}, on_delete?}], layout: {table → {x,y}}}`. The **record
  is the product**; the canvas is one editor of it. Canvas geometry rides the record (the
  `Node.position` precedent from flows), so the picture survives reload.
- **CRUD verbs** for the record: `dbschema.save` / `dbschema.get` / `dbschema.list` /
  `dbschema.delete`, each its own cap. (Named `dbschema.*`, not `schema.*`, to keep clear of the
  shipped **introspection** verb `federation.schema {source, table}` — that one reads a *live*
  source's catalog; these manage the *designed* document.)
- **A visual schema-designer page** in the Datasources UI: a `@xyflow/react` canvas of editable
  table nodes (inline column rows: name / type / nullable / PK) with **FK edges drawn by dragging**
  column-handle → column-handle, dagre auto-layout, a side panel for the selected table, and an
  **import** action that seeds the canvas from a live source via the existing `federation.schema`.
- **`federation.migrate {source, schema}`** — diff the `db_schema` record against the live source
  catalog and apply `CREATE TABLE` / additive `ALTER` DDL. Admin-only, **Ask-gated** per call (DDL
  never rides the general write verb). Returns the planned statements first (`dry_run: true` is the
  default); applying is the explicit second step.
- **`federation.write {source, table, rows, key?}`** — a bounded (row-capped) INSERT/UPSERT into a
  registered source, validated write-only (no DDL, no DELETE in v1), idempotent when `key` names the
  conflict columns. This is the verb a **flow's generic `tool` node** calls — no bespoke "postgres
  node", no socket outside the extension.
- **`federation.export {source, from, table, range?}`** — the dual of `federation.mirror`: a durable
  **`lb-jobs`** batch that reads platform data (`from: {series}` or `{query}`) and bulk-writes it to
  the external table, checkpointed/resumable, upsert-keyed so a resume never double-inserts.
- **Flow-node UX:** the `tool` node's config form (JSON-Schema, existing descriptor path) for
  `federation.write` offers a **datasource picker** and a **table picker fed from `dbschema.*` /
  `federation.schema`**, so mapping `${payload.x}` → column is guided by the designed shape.

## Non-goals

- **No new flow node type.** The engine's generic `tool` node + its config form is the whole flow
  surface (rule 10 — core never names "postgres"). If the config-form UX needs a richer widget,
  that's a descriptor-form finding, not a new node kind.
- **No DELETE / TRUNCATE / arbitrary SQL writes in v1.** `federation.write` is INSERT/UPSERT only;
  `federation.migrate` is CREATE + additive ALTER only (no `DROP TABLE`, no column drops — a
  destructive migration is a **named future verb** with its own Ask gate, not a flag on this one).
- **No embedding ChartDB.** ChartDB is the UX reference for the editor (it is exactly this tool)
  but it is **AGPL-3.0 — look, don't lift**. Code may be copied only from Apache-2.0 tabularis
  (its read-only `SchemaDiagram`/`SchemaTableNode` skeleton) or written fresh.
- **No schema *sync*.** The record is desired-state applied on demand; we do not watch the external
  DB for drift or auto-migrate. `migrate`'s dry-run diff is the drift *report*; acting on it stays
  a human/agent decision.
- **No SurrealDB-side DDL.** This designs *external* relational schemas. Platform data stays the
  schemaless/series/tag model; do not grow this into a SurrealDB table designer.
- **Postgres (+ the shipped SQLite kind for tests) first.** MySQL/other dialect DDL generation is
  additive later work behind the same `Source` trait.

## Intent / approach

**The record, not the picture, is the contract.** Everything downstream — the migrate diff, the
flow-node column picker, an AI agent asked to "add a `status` column" — consumes the
`db_schema:{ws}:{name}` JSON. The canvas is a nice editor over it; `dbschema.save` is callable by
the agent just as well as by the UI (MCP is the universal contract, rule 7).

**Writes go through the federation extension, never around it.** The tempting shortcut — a flow
"postgres sink" node owning its own connection — is rejected: it would put a driver, a socket, and
a DSN in the flow runtime, bypass `net:*` + secret mediation, and give per-row writes with no
idempotency story. Instead the extension that already owns the pools grows three verbs
(`write`/`migrate`/`export`), each host-mediated exactly like `federation.query`: workspace-pinned
source resolution, caps, `net:*` pre-connect, validator in front. One owner for the wall.

**DDL generation is server-side, in the extension.** The browser never builds SQL. `migrate` ports
the shipped SELECT-only validator's discipline to a **write validator** (allow-list of generated
statement shapes) and generates dialect DDL behind the existing per-kind `Source` trait
(`source/postgres.rs` gains `plan_ddl`/`apply_ddl`; the designer works offline from any source —
you can design first, register the DB later).

**Bulk is a job; per-message is a verb** (§6.1). `federation.export` mirrors `federation.mirror`'s
shape verbatim: enqueue an `lb-jobs` batch, return the job id, checkpoint per chunk, resume
mid-range on restart, dedupe by upsert `key`. `federation.write` stays synchronous with a hard row
cap (start: 1 000 rows) — past the cap the answer is "use export", returned as a typed error.

**Designer build: lift the Apache-2.0 viewer, add editing.** Start from tabularis's
`SchemaDiagram.tsx` + `SchemaTableNode.tsx` (~740 lines, `@xyflow/react` + dagre — the same React
Flow the flows canvas already uses, no new dependency). Make the table node editable, add the
column-handle FK drag, a table side panel, and wire load/save to `dbschema.*`. *Rejected:* an npm
"schema designer" component — nothing maintained, embeddable, and license-clean exists (ChartDB and
drawDB are full AGPL/standalone apps); owning ~1–2k lines on a stack we already ship is cheaper
than extracting theirs.

## How it fits the core

- **Tenancy / isolation:** `db_schema:{ws}:{name}` is workspace-keyed; `dbschema.*` resolve only in
  the caller's workspace. `federation.write`/`migrate`/`export` resolve `{source}` through the
  existing workspace-pinned `datasource:{ws}:{name}` path — ws-B can neither name a ws-A schema nor
  write to a ws-A source. The export job's callback `ws` is host-set (the mirror precedent).
- **Capabilities:** `mcp:dbschema.save|get|list|delete:call` (mutate = admin-tier, read = member);
  `mcp:federation.write:call`; `mcp:federation.migrate:call` (admin) **plus the per-call Ask gate**
  (`agent-run-scope.md` Part 2) — a migrate is never silent; `mcp:federation.export:call`. Deny is
  opaque. `net:*` enforcement at connect is unchanged (the write path adds no new endpoint class).
- **Placement:** `either` — the verbs run where the `federation` extension is installed/approved,
  the designer page is plain UI. No `if cloud`.
- **MCP surface (§6.1 — judged):**
  - **CRUD:** `dbschema.save` (upsert the record), `dbschema.delete`; `federation.write` (the
    bounded data write); `federation.migrate` (DDL, dry-run default, Ask-gated apply).
  - **Get / list:** `dbschema.get {name}`, `dbschema.list {}` (names + table counts, no layouts).
  - **Live feed:** N/A — the record changes at human speed; the designer refetches on save. An
    export job's progress is the **job's** existing status/feed, not a new watch verb.
  - **Batch → a job:** `federation.export` returns a job id (long-running by nature).
    `federation.write` is the explicitly-bounded synchronous case (row cap named above).
- **Data (SurrealDB):** one new record type, `db_schema:{ws}:{name}` — state, one datastore. The
  external tables written are the extension's concern behind MCP, never platform state.
- **Bus (Zenoh):** none new. Export-job progress rides the jobs feed.
- **Sync / authority:** SurrealDB authoritative for the schema record; the external DB is a sink.
  An export job resumes from its checkpoint after a node restart (the `lb-jobs` contract); a
  half-applied `migrate` is prevented by running its statements in one transaction where the
  dialect allows (Postgres DDL is transactional — use it) and reported per-statement where not.
- **Secrets:** unchanged — the DSN stays `secret:federation/{source}`, mediated by the supervisor;
  the write verbs add no new secret surface. DDL/rows never appear in logs beyond counts.
- **SDK/WIT impact:** none — three new JSON-RPC verbs on the existing sidecar protocol
  (`main.rs` dispatch), host verb files alongside `crates/host/src/federation/` (one per verb,
  FILE-LAYOUT). Flag loudly if the sidecar protocol itself needs a change; it shouldn't.
- **Skill doc:** extend the existing **`docs/skills/datasources/SKILL.md`** — this is a drivable
  surface (design a schema via `dbschema.save`, migrate, write, export over the gateway). The
  implementing session must add the new verbs there, grounded in a live run.

## Example flow

An admin designs an `orders` schema, applies it to the registered `pg-main` Postgres, and a flow
streams data into it:

1. **Design.** Datasources → Schemas → New. On the canvas the admin adds `customers` and `orders`
   table nodes, fills columns inline, marks PKs, drags `orders.customer_id` → `customers.id` to
   create the FK edge. Save → `dbschema.save {name: "shop", …}` → `db_schema:acme:shop` (layout
   included). Alternatively **Import from source** seeds the canvas via `federation.schema`.
2. **Migrate (dry-run).** "Apply to pg-main" → `federation.migrate {source: "pg-main", schema:
   "shop"}` returns the planned DDL (`CREATE TABLE customers …; CREATE TABLE orders …; ALTER …
   ADD CONSTRAINT fk_…`). The UI shows the statements.
3. **Migrate (apply).** Confirm → the same verb with `dry_run: false`, through the **Ask gate**;
   the extension applies the DDL in one transaction on the pool. `federation.schema` now shows the
   live tables matching the design.
4. **Flow writes.** In the flows editor: trigger (series event) → `change`/`select` reshape →
   generic `tool` node, verb `federation.write`. Its config form's pickers (datasource `pg-main`,
   table `orders` from the `shop` schema) guide the column mapping; `key: ["id"]` makes redelivery
   an upsert. Each firing writes its rows under the caller's cap.
5. **Backfill.** `federation.export {source: "pg-main", from: {series: "shop.orders_raw"},
   table: "orders", range: "-90d", key: ["id"]}` → a job id; the job chunks through the range,
   checkpointing. A node restart mid-export resumes from the checkpoint; upsert on `id` means no
   duplicates. Progress on the job feed.
6. **Deny paths.** No `mcp:federation.write:call` → opaque deny, nothing written. A ws-B caller
   naming `pg-main` or `shop` resolves nothing. `federation.write` smuggling `DROP TABLE` in a
   value → rejected by the write validator. `migrate` without the Ask approval → not applied.

## Testing plan

Per `scope/testing/testing-scope.md` — real store, real caps, real supervisor, real spawned
gateway for UI tests; the external DB is the one sanctioned true-external boundary, exercised as a
**real spawned Postgres container + the shipped file-backed SQLite kind** (the
`sqlite-datasource-demo` no-Docker precedent) — never an in-process fake.

- **Capability-deny (mandatory):** each new verb denied without its cap (nothing written/applied —
  assert on the live catalog/rows); `dbschema.save` denied to a non-admin; `migrate` without Ask
  approval does not apply.
- **Workspace-isolation (mandatory):** ws-B cannot `dbschema.get/list` ws-A schemas; ws-B cannot
  `federation.write`/`migrate`/`export` against a ws-A source; the export job's `ws` is
  un-spoofable. Across store + MCP.
- **Validator:** `federation.write` rejects DDL/DELETE/multi-statement; `migrate` generates only
  the allow-listed statement shapes; the SELECT-only validator still rejects writes arriving via
  `federation.query` (no regression).
- **Migrate correctness:** design → dry-run plan matches → apply → `federation.schema` reflects it;
  re-running migrate on an unchanged schema plans **zero** statements (diff idempotence); an
  additive column change plans exactly one `ALTER`; a destructive change (dropped column) is
  **refused** with a clear error, not silently planned.
- **Write/export round-trip + restart:** `federation.write` rows land (read back via
  `federation.query`); redelivery with `key` upserts (row count stable); `federation.export` over a
  seeded series lands the range; **kill the node mid-export**, restart, assert the job resumes from
  its checkpoint and the final table has no duplicates.
- **Row cap:** an over-cap `federation.write` returns the typed "use export" error, writes nothing.
- **Frontend (real gateway):** designer canvas save/load round-trips `db_schema` (layout included);
  FK drag produces the record's `fks` entry; import-from-source seeds nodes from a live SQLite
  source; the flow `tool`-node config pickers list the schema's tables — all `*.gateway.test.tsx`
  against a spawned node.
- **Regression:** any bug → `docs/debugging/datasources/<symptom>.md` + a regression test.

## Risks & hard problems

1. **Migration diffing is the underestimated core.** Type-mapping (record type → dialect type and
   *back* from the live catalog for the diff) is where correctness lives; a naive string compare
   plans spurious ALTERs forever (e.g. `varchar` vs `character varying`). Normalize types through
   one per-dialect mapping table in the `Source` trait and test diff-idempotence explicitly.
2. **Destructive-change ergonomics.** Refusing drops (v1) is safe but users will rename a column
   and be confused that migrate refuses. The refusal error must say *what* to do (rename = add new
   + backfill + later destructive verb). Get this copy right or the feature feels broken.
3. **Ask-gate plumbing for a UI-initiated migrate.** The Ask gate was designed for agent tool
   calls; a human clicking "Apply" needs an equivalent explicit-confirm path that satisfies the
   same audit trail without double-prompting. Decide the seam before building the button.
4. **Write-path abuse.** `federation.write` from a hot flow is a per-firing external write — a
   misconfigured trigger becomes a write storm at someone's production Postgres. Reuse the
   per-workspace/principal rate-limit posture from ingest on this verb; name the limit.
5. **Designer scope creep.** Indexes, checks, enums, partitions, views… the canvas can grow
   forever. v1 record = tables/columns/PK/FK/nullable/default, full stop; everything else is an
   explicit follow-up on the record's versioned shape (`v: 1` field from day one).
6. **License hygiene.** ChartDB is AGPL — enforce "reference only" in review; copied code must
   trace to tabularis (Apache-2.0) or be original. Note provenance in the session doc.

## Open questions

1. **`dbschema` ownership tier** — admin-only mutate, or member-designable with admin-only
   `migrate`? (Lean: member save, admin migrate — designing is harmless, applying is not.)
2. **Where the record's dialect lives** — per-schema (`dialect: postgres`) or resolved at migrate
   time from the target source's kind? (Lean: types stored dialect-neutral, mapped per-kind at
   plan time — one schema, many targets.)
3. **`federation.write` batching inside a flow** — should the flow author be steered to put a
   `batch` node (count-mode) before the tool node, or should the verb micro-batch internally?
   (Lean: document the `batch`-node pattern; keep the verb dumb.)
4. **Export `from: {query}`** — v1 series-only, or also an arbitrary platform `data.query`?
   (Lean: series first; query-sourced export once a real caller needs it.)
5. **Ask-gate UI seam** (Risk 3) — reuse the agent Ask approval record, or a dedicated
   confirm-with-audit route on the gateway?

## Related

- [`datasources-scope.md`](./datasources-scope.md) — the federation extension + the deferred-write
  decision this scope executes; [`sqlite-datasource-demo-scope.md`](./sqlite-datasource-demo-scope.md)
  (the no-Docker test source), [`datasource-samples-scope.md`](./datasource-samples-scope.md)
  (`federation.sample`, the read-side sibling of the flow-node picker).
- `../jobs/jobs-scope.md` — the export job; `../ingest/ingest-scope.md` — `federation.mirror`'s
  landing plane (the inbound dual).
- `../flows/flows-scope.md` + `../flows/node-descriptor-scope.md` — the generic `tool` node and its
  JSON-Schema config form the picker extends; `../flows/extension-nodes-scope.md` — why there is no
  bespoke IO node.
- `../extensions/reference-extensions-scope.md` — native Tier-2 doctrine, `net:*`, secret mediation;
  `../secrets/secrets-scope.md`; `../auth-caps/` (the Ask gate via `agent-run-scope.md` Part 2).
- `docs/skills/datasources/SKILL.md` — the skill the implementing session extends with the new verbs.
- Code: `rust/extensions/federation/src/` (sidecar verbs + `source/` trait),
  `rust/crates/host/src/federation/` (host mediation, one verb per file),
  `ui/src/features/` datasources pages + the flows canvas (the React Flow precedent).
- External reference: tabularis (Apache-2.0 — `SchemaDiagram`/`SchemaTableNode` lift),
  ChartDB (AGPL-3.0 — **UX reference only**), README §3 (rules 2/5/6/10), §6.1, §6.10.
