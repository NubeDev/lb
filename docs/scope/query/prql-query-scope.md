# Query scope — saved PRQL queries, one authoring language across every source

Status: scope (the ask). Promotes to `public/query/query.md` once shipped.

We want a workspace to **author a query once, in PRQL, save it as an editable named record, and run it
against any source** — the SurrealDB-native store, a registered external datasource, from a rule, or
from any agent/extension — all through one MCP verb family. PRQL is the human-friendly authoring layer;
the platform compiles it to the right dialect at run time and **dispatches to the query engines that
already exist** (`store.query` for the platform, `federation.query` for external datasources). The save
is the headline: a saved query is a workspace record you can re-open and edit, the way a datasource or
a rule already is — not a one-shot string typed into a box.

> Read with: `../datasources/datasources-scope.md` (the `federation.query` engine + the **hard line**:
> SurrealDB is never a DataFusion source), `../rules/rules-engine-scope.md` (the rule `source(...)` seam
> a saved query plugs into), `../store/` + `rust/crates/host/src/store_query/` (the `store.query`
> read-only SurrealQL gate this reuses), `../channels/channels-query-charts-scope.md` +
> `../frontend/dashboard/viz/` (downstream consumers of a query result), README §3 (rules 2/5/6/7), §6.3.

---

## Doctrine: PRQL is the authoring layer; the engines and the walls are unchanged

This scope adds **no new query engine and no new authority**. SurrealDB stays the one datastore and the
authority (rule 2); external DBs stay federated sources behind the gated `federation` extension. PRQL is
a *front end* that compiles to a dialect and hands the result to a verb that already enforces the wall:

1. **Authoring** is PRQL (or `raw` for a dialect-specific escape hatch). One language, learned once.
2. **Persistence** is a workspace record, `query:{ws}:{id}` — the established saved-artifact pattern
   (`datasource:{ws}:{name}`, `rule:{ws}:{id}`): soft-delete, `ts`, workspace-keyed, capability-gated.
3. **Execution** compiles PRQL→SQL for the target's dialect, then **dispatches to the existing engine**:
   `store.query` (platform, SurrealQL) or `federation.query` (external). The target's existing
   capability, `net:*`, secret, SELECT-only, and row-cap gates all still apply — `query.run` *composes*
   them, it does not widen them (rule 5).

The whole value is the uniform authoring/saving surface over the correct, unchanged split beneath.

## Goals

- A **pure `lb-prql` crate** (`rust/crates/prql/`) that wraps `prqlc` (the official Rust PRQL compiler) —
  compile-only, **zero I/O**, links `prqlc` + `serde` and nothing platform-aware. One verb per file
  (`compile.rs`, the target/dialect map, error types) per FILE-LAYOUT.
- A **host `query` service** (`rust/crates/host/src/query/`, sibling to `federation/` and `rules/`) that
  owns the saved-query record and the dispatch, one verb per file.
- **A saved query is a first-class, editable record:** `query:{ws}:{id}` →
  `{id, name, description, lang: "prql"|"raw", text, target, params, removed, ts}`. Save it, list it,
  re-open it, edit it, run it — the save-and-re-edit ask, met by the same record pattern datasources use.
- **One MCP verb family, `query.*`** — `save` / `get` / `list` / `delete` / `run` / `compile` — each its
  own file + capability, callable the same way by the UI, the agent, a rule, and other extensions
  (rule 7: MCP is the universal contract).
- **Targets are uniform to the author, correct beneath:** `target: "platform"` → `store.query`
  (SurrealDB-native); `target: "datasource:<name>"` → `federation.query` (external). The author writes
  PRQL either way; the host picks the dialect and the engine.
- **Rules reuse the library by name:** a rule's `source("query:<name>")` resolves to `query.run`, so a
  saved query is a reusable, centrally-editable data definition rules share — not SQL duplicated per rule.

## Non-goals

- **A new query engine or a second authority.** We compile to dialects that `store.query` /
  `federation.query` already run. SurrealDB stays the one datastore (rule 2); no new persistence layer.
- **Writes / DML.** PRQL is a read language and both target engines are read-first (SELECT-only). A
  write path is a separate, later, Ask-gated concern — explicitly out of scope (mirrors
  `datasources-scope.md`'s `federation.write` deferral).
- **Registering SurrealDB as a DataFusion *source*.** The datasources hard line holds: platform data is
  read **natively** through `store.query`, never as a federated DataFusion connection. The Phase-2 full-
  semantics path (below) uses DataFusion only as an *in-process compute layer over rows already fetched
  natively* — never as a connection, an authority, or a caps bypass.
- **Text-to-SQL / NL→PRQL.** Generating PRQL from a prompt is the AI-gateway's job (a future
  `ai.ask`→PRQL flow re-validated through `query.compile`); this scope is the save/compile/run surface.
- **Ad-hoc cross-tenant or raw-endpoint targets.** A target resolves only to `"platform"` or a
  `datasource:{ws}:{name}` registered in the **caller's** workspace — the wall is at resolution.

## Intent / approach

**Compile in a pure crate, dispatch in the host — embed the compiler, front it with our verb.** Same
lesson as `federation` (embed DataFusion, expose `federation.query`): `lb-prql` embeds `prqlc` as a
library and exposes one `compile(prql, dialect) -> sql` function with no I/O. The host `query` service
loads the saved record, calls `lb-prql::compile` for the target's dialect, and forwards the SQL to the
engine that already owns the wall. The heavy/edge logic (dialects, validation) stays out of the engines.

**`lang: "prql"` is the default; `lang: "raw"` is the escape hatch.** PRQL compiles to *standard* SQL,
which is a clean fit for `federation.query` (DataFusion is ANSI-ish) but **not** for SurrealQL, which
diverges (record ids, `FETCH` vs `JOIN`, graph edges). So a saved query may set `lang: "raw"` to carry
target-native text verbatim — `raw` SurrealQL for platform, `raw` SQL for a datasource. This gives an
immediate, honest answer for the SurrealQL-only features PRQL can't express, while keeping PRQL the
primary, portable authoring language for everything that maps.

**The platform target — "PRQL for SurrealDB too," in two phases that both keep rule 2.** The user wants
PRQL to work for *all* targets, including the SurrealDB-native path. Done without making SurrealDB a
DataFusion source:
- **Phase 1 — SurrealQL pushdown (subset).** Compile PRQL → `sql.generic`, then run it through the
  **existing `store.query` parse-allowlist gate** (single `SELECT`, 10k-row / 5s bound). The relational
  subset — `from / filter / select / aggregate / sort / take` — maps cleanly to SurrealQL and runs
  natively. Anything outside the subset is rejected by the same gate that already protects `store.query`;
  the author drops to `lang: "raw"` SurrealQL for it. Ships now, no new engine.
- **Phase 2 — full PRQL semantics via DataFusion-as-compute (open question, see below).** For the joins/
  window-functions/CTEs PRQL expresses but SurrealQL can't, compile PRQL → `sql.datafusion`, fetch the
  base tables **natively via `store.query`** (gated, capped), register those result sets as in-memory
  DataFusion tables, and execute the compiled SQL over them. DataFusion is a *pure executor fed by
  native, caps-checked reads* — **never** a SurrealDB connection or authority. This reconciles "full
  PRQL on platform data" with the datasources hard line. Flagged as an open question because it adds a
  compute dependency to the host path and needs a memory/row bound — decide before building Phase 2.

**The datasource target — straight through `federation.query`.** Compile PRQL with the datasource's
dialect (`sql.postgres` / `sql.mysql` / `sql.duckdb`, picked from the `datasource.kind`), then call
`federation.query {source, sql}`. Every existing federation wall (workspace-pin, `net:*`, secret
mediation, SELECT-only re-validation, row cap) applies unchanged — `query.run` adds nothing and removes
nothing.

**Rules plug in by name, not by re-implementation.** `rules-engine-scope.md` already routes
`source("series")`→`store.query`/`series.*` and `source("timescale")`→`federation.query`. We add one
resolution: `source("query:<name>")` → `query.run {id:<name>}`, collecting the grid from the saved
query's result. A rule then composes shared, centrally-edited queries instead of inlining SQL — the
"reference by name via `source()`" decision. The rule still runs under `caller ∩ grant`; `query.run`
re-checks the target cap inside the collect (the established per-source `caps::check`).

**Rejected — a per-feature query box (status quo).** Today SQL is typed ad-hoc into a channel message, a
datasource test, or a rule body, in three dialects, none saved or reusable. Rejected: it duplicates
queries, can't be edited centrally, and forces every author to know the target's raw dialect. One saved
PRQL artifact, compiled per target, is the fix the user asked for.

**Rejected — a brand-new query engine / our own DSL.** PRQL is a mature, typed, composable query
language with an official Rust compiler; inventing a DSL or embedding a second SQL engine would violate
"one datastore" (rule 2) and re-solve a solved problem. We compile to what the engines already run.

## How it fits the core

- **Tenancy / isolation (rule 6):** `query:{ws}:{id}` is workspace-keyed; `query.get`/`run`/`delete`
  resolve only within the caller's workspace. A `target` resolves to `"platform"` or a
  `datasource:{ws}:{name}` in the **caller's** workspace only — un-spoofable, host-set. ws-B can neither
  read nor run a ws-A saved query, and `run` can never reach a ws-A datasource. Mandatory isolation test
  across store + MCP.
- **Capabilities (rule 5):** one cap per verb — `mcp:query.save:call`, `query.get`, `query.list`,
  `query.delete`, `query.run`, `query.compile`. **`query.run` composes, never widens:** the caller needs
  `mcp:query.run:call` **and** the underlying target cap (`mcp:store.query:call` for platform, or
  `mcp:federation.query:call` for a datasource). Holding `query.run` alone, without the target cap, is
  denied — the headline no-widening deny test. `query.compile` is pure (no data access) and needs only
  its own cap.
- **Placement (rule 1):** `either`, by config. The crate + host service are in the symmetric node
  binary; whether a *datasource* target resolves depends only on whether `federation` is installed and
  granted (config/role). No `if cloud`. The platform target works on any node.
- **MCP surface (§6.1 — judged):**
  - **CRUD:** `query.save {id?, name, description?, lang, text, target, params?}` (upsert — create or
    edit by id; the save-and-re-edit core), `query.delete {id}` (soft, `removed=true`). Each its own
    verb + cap.
  - **Get / list:** `query.get {id}` (the full record, for re-opening in the editor), `query.list {}`
    (workspace-scoped roster — name/target/lang/ts, no result data).
  - **Run:** `query.run {id}` (or `{lang, text, target}` for an ad-hoc unsaved run — see open questions)
    → `{columns, rows}` via compile→dispatch. Read-first, row-capped by the underlying engine.
  - **Compile (dry-run):** `query.compile {lang, text, target}` → `{sql}` (or a typed error) **without
    executing** — feeds the editor's live preview/validate, costs no data access, needs no target cap.
  - **Live feed:** N/A — a query returns rows. Live data is the **series SSE** / dashboard path
    downstream (a saved query can *source* a chart; the stream is the chart's, not this verb's).
  - **Batch → a job:** out of scope v1 (reads are bounded by the engine row cap). An unbounded
    export/materialize is the existing `federation.mirror` `lb-jobs` batch, not a new path here.
- **Data (SurrealDB):** `query:{ws}:{id}` (the saved PRQL/raw text + target + params) is the **only**
  new platform record — workspace-walled, the one datastore. Results are never stored by this verb;
  they're returned, or fed to a downstream consumer (channel item, dashboard widget) that owns its own
  persistence.
- **Bus (Zenoh):** none directly. A saved query feeding a dashboard rides the **existing** series SSE
  (motion); this verb is state/read, not motion (rule 3).
- **Sync / authority:** SurrealDB stays the source of truth on every node (rule 2). The saved-query
  record syncs like any workspace record (S3); a *run* is live and node-local. The external DB is never
  a sync peer; the platform target reads native authoritative data.
- **Secrets:** none held here. A datasource target's DSN stays mediated inside `federation`
  (`secret:federation/*`) — `query.run` never sees it (it calls `federation.query`, which mediates).
- **SDK/WIT impact:** none new. An extension/agent reaching `query.*` uses the **existing** host-callback
  (`caller ∩ grant`), exactly as it reaches any host tool. No ABI change.

## Example flow

An analyst writes one PRQL query, saves it, edits it, and a rule reuses it.

1. **Author + compile-preview.** Analyst → Queries → New. Picks `target: "datasource:warehouse"`
   (a registered Postgres), writes PRQL:
   `from orders | filter status == "paid" | group store (aggregate { rev = sum amount }) | sort {-rev}`.
   The editor calls `query.compile {lang:"prql", text, target:"datasource:warehouse"}` → shows the
   compiled `sql.postgres`. Green.
2. **Save.** `query.save {name:"store-revenue", lang:"prql", text, target:"datasource:warehouse"}` →
   `query:{ws}:store-revenue`. It now appears in `query.list`.
3. **Run.** `query.run {id:"store-revenue"}`: the host loads the record, `lb-prql::compile`s it to
   `sql.postgres`, authorizes `mcp:query.run:call` **and** `mcp:federation.query:call`, dispatches
   `federation.query {source:"warehouse", sql}` (which re-validates SELECT-only, pins the workspace,
   pulls the DSN), returns `{columns, rows}`. The analyst plots it / posts it to a channel.
4. **Re-edit later.** `query.get {id:"store-revenue"}` re-opens the PRQL; the analyst adds
   `| filter rev > 1000` and `query.save`s the same id — the record is updated in place (the re-edit ask).
5. **Reuse from a rule.** A rule does `source("query:store-revenue")`; `lb-rules` collects the grid via
   `query.run`, under `caller ∩ grant` (the rule's principal needs `query.run` + the target cap). The
   query is defined once, edited once, consumed by the UI, the agent, and the rule alike.
6. **Platform target.** A second saved query sets `target:"platform"`, PRQL
   `from channel_message | filter ws == $ws | sort {-ts} | take 50`. Phase 1 compiles to `sql.generic`,
   runs it through the `store.query` read-only gate natively. A SurrealQL-only graph traversal instead
   uses `lang:"raw"` against `target:"platform"`.
7. **Deny path:** a caller with `mcp:query.run:call` but **not** `mcp:federation.query:call` runs a
   datasource-target query → **denied** (no widening). A ws-B caller naming `id:"store-revenue"` (a ws-A
   query) resolves nothing → denied. A PRQL that compiles to a non-SELECT or exceeds the bound is
   rejected by the target engine's existing gate.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks** for our own stack: the
**real store**, real caps, the real `store.query` gate, and the **real `federation` sidecar against a
real spawned Postgres** (the one sanctioned fake-boundary, §0 — reuse the datasources test rig). PRQL
compilation is pure and tested with golden in/out pairs.

- **Capability-deny (§2.1):** each `query.*` verb denied without its cap; the **headline no-widening
  test** — `query.run` on a datasource target is denied when the caller lacks `mcp:federation.query:call`
  even *with* `mcp:query.run:call`; likewise platform-target run requires `mcp:store.query:call`.
- **Workspace-isolation (§2.2):** ws-B cannot `get`/`run`/`delete` a ws-A `query:{ws}:{id}`; a `run`
  whose target names a ws-A datasource resolves nothing in ws-B — across store + MCP.
- **Compile correctness (`lb-prql` unit):** golden PRQL→SQL per dialect (`postgres`, `mysql`, `duckdb`,
  `generic`); a malformed PRQL returns a typed error (surfaced by `query.compile`); the SurrealQL-subset
  output passes the `store.query` parse-allowlist (and an out-of-subset one is cleanly rejected there).
- **Read-only / bound enforcement:** PRQL can't express DML, but assert a `lang:"raw"` write is rejected
  by the engine gate (read-first); the engine row cap/timeout bounds the result (no unbounded read here).
- **Params binding:** a parameterized query binds `$var` safely (no string-injection) into both the
  `store.query` `vars` path and the `federation.query` path; a missing/extra param is a typed error.
- **Round-trips (real backends):** `save → get → edit → save → run` returns seeded rows from a **real
  Postgres** (datasource target) and from **real seeded SurrealDB records** (platform target); a **rule**
  reads `source("query:<name>")` end to end (ties to `rules-engine-scope.md`).
- **Frontend (real gateway):** the Queries page (`save`/`list`/`get`/`compile`-preview/`run`) over the
  bridge (`*.gateway.test.tsx`) against a real spawned node + real DB — no `*.fake.ts` (rule 9).

## Risks & hard problems

- **The PRQL→SurrealQL semantic gap is the central one.** PRQL targets standard SQL; SurrealQL is not
  standard SQL. Phase 1 only covers the mapping subset, and the `raw` escape hatch is the honest seam for
  the rest. Over-promising "all of PRQL on SurrealDB" before Phase 2 is the trap — the doc, the UI, and
  `query.compile`'s errors must make the subset boundary legible, not silently mis-compile.
- **`prqlc` version + dialect coverage.** Pin `prqlc`; its dialect targets and output evolve. Golden
  tests freeze the contract; a compiler bump is a reviewed change with re-frozen goldens.
- **Param injection safety.** PRQL has no native bind-param story across all dialects; binding `$var`
  must go through the engines' real parameter paths (`store.query` vars; `federation.query`), never
  string interpolation — an injection here would defeat the read-only gate.
- **Result-shape uniformity.** Both engines return `{columns, rows}` (positional). Keep `query.run`'s
  shape identical to them so downstream consumers (channel charts, dashboard widgets) treat all three
  query verbs uniformly.
- **Phase-2 memory bound.** DataFusion-over-native-reads must cap the base-table rows it pulls via
  `store.query` (reuse the 10k bound) or a large join OOMs the host — decide the bound with the Phase-2
  go-ahead.

## Open questions

> Resolved 2026-06-30 by the Phase-1 build session (`sessions/query/prql-query-session.md` →
> `public/query/query.md`). The Phase-2 item is deferred by decision; the rest are settled.

- **Phase-2 full-Surreal-PRQL mechanism.** DEFERRED. Phase 1 shipped subset-only on the platform
  target (PRQL→`sql.generic` through the existing `store.query` parse-allowlist) + `lang:"raw"` for
  the rest; Phase 2 (DataFusion-as-compute over native `store.query` reads) is explicitly NOT built
  this session. Re-open when full PRQL-on-Surreal semantics are needed.
- **Identity & uniqueness.** DECIDED: `id` is a kebab-case slug unique per workspace (the record
  key); a separate editable `name` is the display label (mirrors the rules `id`+`name` pattern).
  `query.save` upserts by `id`.
- **Edit history / versioning.** DECIDED: overwrite in place (like a datasource). No revision
  history in v1.
- **Ad-hoc run.** DECIDED: `query.run` accepts EITHER `{id}` OR an inline `{lang, text, target}`
  for an unsaved one-shot.
- **Organization.** DECIDED for v1: no folders/tags. `query.list` returns a flat roster
  `(id, name, target, lang, ts)` with no result data and no query text dumped.
- **Params binding (added).** DECIDED for v1: full injection-safe `$var` binding on the platform
  path (through `store.query` `vars`); the datasource path's `federation.query` sidecar has no
  bind-param path yet, so a parameterized datasource query is a typed error (loud, never
  interpolation) until the sidecar grows one.
- **Downstream binding.** DEFERRED: no dashboard/channel binding this session (follow-on scope).

## Related

- `../datasources/datasources-scope.md` — the `federation.query` engine the datasource target dispatches
  to, and the **hard line** (SurrealDB is never a DataFusion *source*) this scope respects.
- `../rules/rules-engine-scope.md` — the `source(...)` seam; this adds `source("query:<name>")`→`query.run`.
- `../store/` + `rust/crates/host/src/store_query/` — the `store.query` read-only SurrealQL gate the
  platform target reuses (Phase 1) and feeds (Phase 2).
- `../channels/channels-query-charts-scope.md`, `../frontend/dashboard/viz/`,
  `../frontend/rules-editor-ux-scope.md` — downstream consumers of a saved query's result.
- `../mcp/` — the universal tool contract `query.*` joins; `../ai-gateway/ai-gateway-scope.md` — the
  future NL→PRQL→`query.compile` path (out of scope here).
- README `§3` (rules 2/5/6/7), `§6.3` (extensions/tiers). Upstream: PRQL + `prqlc`
  (Apache-2.0, https://github.com/PRQL/prql) — embedded as a library in `lb-prql`, the federation lesson.
