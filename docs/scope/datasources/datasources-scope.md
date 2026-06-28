# Datasources scope — `federation`, a native extension for external SQL sources

Status: scope (the ask). Promotes to `public/datasources/datasources.md` once shipped.

We want a workspace to **connect to external databases** — MySQL, PostgreSQL/TimescaleDB, and the other
federated sources DataFusion supports — and query them from rules, the AI agent, the UI, and other
extensions, **without breaking "one datastore" (rule 2) or the workspace wall (rule 6)**. The answer is
a **native (Tier-2) `federation` extension** that embeds DataFusion + its connectors as a *library*,
owns the connection pools behind `net:*` + a secret, and exposes one read-first MCP verb,
`federation.query`. SurrealDB stays the platform's authoritative store; external DBs are **federated
sources reached through the gated extension**, never a second authority and never wired into core.

> Read with: `../extensions/reference-extensions-scope.md` (the native-extension doctrine + the `net:*`
> family + the native host-callback this builds on — the `timescale` reference extension is the seed of
> this scope), `../rules/rules-engine-scope.md` (the rule `source(...)` verb that calls
> `federation.query`), `../secrets/secrets-scope.md` (the DSN), `../ingest/ingest-scope.md` (the mirror
> path), `../../vision/0003-iot-dashboard.md` §"An external warehouse", README §3 (rules 2/5), §6.3.

---

## Doctrine: SurrealDB is authority; external DBs are federated sources

This scope **applies** `reference-extensions-scope.md`'s doctrine (a native Tier-2 extension is the
sanctioned escape hatch that may own external resources) to the general "connect to any SQL source"
case — generalizing its `timescale` reference extension into one DataFusion-backed federation engine.
Two roles, kept structurally apart:

1. **SurrealDB = the one datastore + the authority.** It holds *all platform state* — rules, chains,
   jobs, series, caps, workspaces, inbox/outbox. It is reached **natively** (host `data.query`/
   `series.*` verbs), never through DataFusion. It is privileged, not a peer. **Do not register
   SurrealDB as a DataFusion source** — that would fork the authority and bypass `caps`.
2. **MySQL / Timescale / … = federated sources.** Owned by the `federation` extension, reached only
   through `federation.query`, `net:*`-gated, workspace-pinned. They are consumers/sources, never a
   node's persistence layer and never a sync peer (`0003` rejected both explicitly).

To a rule author the two look uniform (`source("series")` vs `source("timescale")` — see
`rules-engine-scope.md`); architecturally they take different, correct paths. That uniform UX over a
correct split is the whole point.

## Goals

- A **native (Tier-2) `federation` extension** (`rust/extensions/federation/`) that **embeds**
  `datafusion` + `datafusion-table-providers` (MySQL, Postgres/Timescale, ODBC, DuckDB) as a library
  and holds the connection pools behind one `Source` trait, one file per source kind.
- **One read-first MCP verb, `federation.query {source, sql}`** → `{columns, rows}`, **workspace-pinned
  by the host** (the workspace predicate/scope is host-set from the token, never `sql`-supplied),
  `caps`-gated (`mcp:federation.query:call`). The model/script supplies a query *shape*; it can never
  name a cross-tenant or unregistered source.
- **`net:*`-gated connections + a mediated secret.** A source connects only to a `host:port` the admin
  approved at install (`net:tls:host:5432`), with its DSN pulled from `lb-secrets` (`secret:federation/*`)
  — never in a rule, the UI, a record, or a log.
- **Datasource registration as a workspace record** (`datasource:{ws}:{name}` → kind + endpoint ref +
  secret ref), with admin CRUD verbs (`datasource.add`/`list`/`remove`/`test`).
- **Two ways to consume external data**, both blessed by `0003`: **federate** (query live via
  `federation.query`) and **mirror** (a durable `lb-jobs` batch pulls a range and `ingest.write`s it
  into the platform series plane for dashboards/cache/offline).

## Non-goals

- **Running Spice.ai (or Cube) as a service.** We embed DataFusion **the crate** (the engine Spice and
  `rubix-cube` both build on); we do **not** supervise the Spice product as an external service nor
  expose a Postgres-wire/BI endpoint. (Cube was considered and rejected — see Intent.)
- **Model serving / text-to-SQL inside this extension.** `ai.*` is the AI-gateway's job
  (`../ai-gateway/`); a rule's `ai.ask` proposes SQL and `federation.query` executes the *re-validated*
  result. The federation extension is data access only.
- **Promoting an external DB to authority or a sync peer.** External data is a downstream source/sink.
  Platform state never persists there (rule 2).
- **A raw SQL handle to callers.** No caller — rule, agent, UI, or other extension — ever gets a DB
  connection; they get `federation.query` (read-first), workspace-pinned. A `federation.write` is a
  separate, later, Ask-gated verb (see Resolved decisions), not in v1.
- **Bridging client-provided/arbitrary sources at call time.** Sources are registered at install/admin
  time (admin-approved `net:*`), not named ad-hoc by a caller — the wall is at registration.

## Intent / approach

**Embed the engine as a library, front it with our verb — the `rubix-cube` lesson.** `rubix-cube`
already embeds DataFusion (its `spice_engine` is its own wrapper over the `datafusion` crate, not the
Spice product) and exposes it only through validated, allowlist-scoped verbs. We do the same, one level
out: the `federation` extension embeds `datafusion` + `datafusion-table-providers`, registers each
external DB as a DataFusion table provider behind a pool, and exposes **only** `federation.query`. The
heavy engine + the drivers + the sockets live in **one supervised, admin-approved process** — never in
core, never as a service with its own API.

**Native Tier-2, because it owns sockets — built on the four platform fixes.** Per
`reference-extensions-scope.md`, a native sidecar owning an external connection is the escape hatch.
This extension uses the same fixes the reference set defined: **fix 1** (the native host-callback, so a
mirror job can call `ingest.write`), **fix 2** (the `net:*` family, enforced pre-connect), and the
**secret store** for the DSN. It is the `timescale` reference extension, generalized to N source kinds
behind one engine.

**Workspace-pinned at the host, not trusted to the source.** `federation.query` authorizes workspace-
first then `mcp:federation.query:call`; the host resolves `{source}` to a *registered* datasource in
the **caller's** workspace (a name the caller can't forge into another tenant's) and applies the read
through the extension's pool for that source. DataFusion's own permissions are **not** our security
layer — `caps` + the registration wall are. A SELECT-only validator (ported from `rubix-cube`'s SQL
validator) re-checks the `sql` before execution; v1 is read-only, so a write/DDL is rejected outright.

**Federate vs mirror — name both, pick per intent (`0003`).** *Federate* (`federation.query`) reads the
external DB live — for fresh/ad-hoc/interactive needs. *Mirror* is a durable, resumable `lb-jobs` batch
(the §6.10 batch-as-job rule) that reads an external range and `ingest.write`s it into the platform
series plane — for dashboards, caching, offline, and joining with platform data at SurrealDB speed. Same
extension owns the connection for both; the difference is copy-in vs query-through. This is exactly the
`0003` "external warehouse without breaking one datastore" pattern (egress mirror = the dual).

**Rejected — Cube as the datasource/semantic layer.** Cube (headless BI) was considered for "nicer
ecosystem." Rejected for v1: its value is its **non-MCP API surface** (SQL-wire/REST/GraphQL for BI
tools) + its own security context — adopting it either throws away that value (if wrapped behind MCP) or
punches a tenant-wall hole enforced by a third-party runtime (if its endpoints are exposed). DataFusion-
the-crate behind `federation.query` gives the federation value with the wall intact. A future
"expose a gated BI endpoint" extension can revisit Cube deliberately; it is not smuggled in here.

**Rejected — DataFusion in a core crate.** Tempting (it's a Rust lib), but it pulls a heavy dependency
and owns sockets/pools, which belong in a supervised, admin-approved, `net:*`-gated process — not in the
symmetric node binary every device runs (rule 1). Core stays lean; federation is an installable extension.

## How it fits the core

- **Tenancy / isolation:** `datasource:{ws}:{name}` is workspace-keyed; `federation.query`'s `{source}`
  resolves only within the caller's workspace (host-set, un-spoofable). ws-B can neither name nor read a
  ws-A datasource; ws-B's `federation` instance reaches none of ws-A's endpoints. The native callback's
  `ws` (for a mirror job's `ingest.write`) is host-set, never sidecar-supplied. Mandatory isolation test
  across store + MCP + the `net:*` boundary.
- **Capabilities:** `federation.query` gated `mcp:federation.query:call`; `datasource.add`/`remove`/
  `test`/`list` gated per-verb (admin-only for mutate). At **connect time** the supervisor enforces
  `net:*` (`requested ∩ admin_approved`) — a source whose endpoint the grant omits is refused, opaque,
  even with the binary present. The deny path is the headline (port the reference-extension deny test).
- **Placement:** `either`, by config — but a workspace can reach external sources only where the
  `federation` extension is installed/approved. Symmetric: the extension binary is the same; *which*
  endpoints it may open is config (the grant). No `if cloud`.
- **MCP surface (§6.1 — judged):**
  - **Query (the core add):** `federation.query {source, sql}` → `{columns, rows}`. Read-first,
    workspace-pinned, SELECT-only validated. Bounded result (a row cap); an unbounded export is a
    **mirror job**, not this call.
  - **CRUD (admin):** `datasource.add` (register a source: kind, endpoint, secret ref — admin-approves
    the `net:*` + `secret:*` at install), `datasource.remove`. Each its own verb + cap.
  - **Get / list:** `datasource.list {}` (registered sources, no secrets), `datasource.test {source}`
    (a real connectivity probe — green/red in the UI).
  - **Live feed:** N/A for a query (it returns rows). A mirror job's progress is the **job's** feed
    (`lb-jobs`), and the resulting series stream over the **existing `GET /series/{s}/stream` SSE** — no
    new watch verb (the bridge writes series, the dashboard streams them, per `0003`).
  - **Batch → a job:** `federation.mirror {source, query, target_series, range}` enqueues a **durable,
    resumable `lb-jobs`** batch (returns a job id) that reads the external range and `ingest.write`s
    samples. A long query never blocks a tool handler (§6.1).
- **Data (SurrealDB):** `datasource:{ws}:{name}` (kind + endpoint ref + secret ref) is the only platform
  record — workspace-walled, the one datastore. The **external** rows are the extension's, behind MCP,
  **never** platform state. Mirrored samples land in the existing **series** plane (they join the one
  datastore as ingest, not as a federated read).
- **Bus (Zenoh):** none directly from a query. A mirror job's `ingest.write` publishes series motion
  (`publish_sample`) so dashboards update live; must-deliver effects (none here) would use the outbox.
- **Sync / authority:** SurrealDB stays the source of truth on every node (rule 2). A federated read is
  live and node-local; a mirror job is authoritative on its hosting node and **resumes mid-range** on
  restart (the `lb-jobs` checkpoint/resume — the `0003` "resumable migration job"). The external DB is
  never a sync peer.
- **Secrets:** the DSN/connection string is `secret:federation/{source}` in `lb-secrets`, pulled by the
  supervisor and handed to the pool — never to a rule, the page, a record, or a log (port the
  reference-extension secret-mediation discipline; the source client lives behind one trait in one file).
- **SDK/WIT impact:** the **native host-callback** (a mirror job calling `ingest.write`) is the
  forever-shaped child→host boundary `reference-extensions-scope.md` fix 1 defines — this extension is a
  consumer of it, not a new ABI. `net:*` is the auth-caps grammar addition (already scoped there).

## Example flow

A KFC admin connects the chain's existing Timescale warehouse and a nightly report reads it.

1. **Register.** Admin → Datasources → Add → `TimescaleDB`, host `tsdb.acme:5432`, paste the DSN. The
   install asks the admin to approve `net:tls:tsdb.acme:5432` + `secret:federation/tsdb`. Save stores
   `granted = requested ∩ approved` and `datasource:acme:timescale`. `datasource.test` runs a real probe
   → green.
2. **Supervise.** The supervisor spawns/holds the `federation` sidecar; at connect time it checks
   `net:tls:tsdb.acme:5432` is in the grant (else refuse, opaque), pulls the DSN via
   `secret:federation/tsdb`, and opens the pool behind the `TimescaleSource` trait.
3. **Federate (live).** A rule step runs `source("timescale").query("SELECT store, avg(temp) t FROM
   readings WHERE ts > now() - interval '1 day' GROUP BY store").filter("t > 5.0")`. `lb-rules` collects
   the grid via `federation.query {source:"timescale", sql:…}`; the host authorizes
   `mcp:federation.query:call` workspace-first, resolves `timescale` in `acme`, validates SELECT-only,
   runs it on the pool, returns `{columns, rows}`. The rule alerts on hot stores.
4. **Mirror (cache).** For the dashboard, a `federation.mirror {source:"timescale", query, target_series:
   "cooler.temp", range:"-30d"}` enqueues an `lb-jobs` batch that pulls the range and `ingest.write`s it
   (via the native callback, `caller ∩ grant`, ws host-set) into the series plane; the dashboard's
   `GET /series/cooler.temp/stream` SSE shows it — fast, offline-capable, no live external dependency.
   A node restart mid-mirror resumes from the checkpoint.
5. **Deny path:** registering without `net:tls:tsdb.acme:5432` → step 2 refuses the connect (opaque,
   sidecar degraded). A ws-B caller naming `source:"timescale"` resolves nothing in ws-B → denied. A
   `federation.query` with a non-SELECT `sql` is rejected by the validator before execution.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks** for our own stack: the
**real supervisor**, real store, real caps, the real `net:*` enforcement, a real `lb-jobs` queue for the
mirror. The external DB itself is the **one sanctioned fake-boundary** (a true external, behind the one
`Source` trait, one file — §0): tests run against a **real spawned MySQL/Postgres** (a container, as
`pnpm test:gateway` spawns a real node) seeded with real rows — not an in-process re-implementation.

- **Capability-deny (§2.1):** `federation.query` denied without `mcp:federation.query:call`;
  `datasource.add`/`remove` denied without the admin cap; **`net:*` deny** — a source whose endpoint the
  grant omits → connect refused even with the binary present (the headline reference-extension deny).
- **Workspace-isolation (§2.2):** ws-B cannot resolve/query a ws-A datasource; ws-B's instance reaches
  no ws-A endpoint; a mirror job's callback `ws` is un-spoofable — across store + MCP + `net:*`.
- **SELECT-only / pin enforcement:** a `federation.query` with INSERT/UPDATE/DDL is rejected (read-first
  v1); the workspace scope is host-applied (a `sql` that tries to widen beyond the registered source
  fails); port `rubix-cube`'s validator tests.
- **Offline / restart (§2.3):** kill the sidecar mid-connection → respawn → reconnects, **no platform
  state lost** (a series sample mirrored before the kill is intact); a **mirror job resumes mid-range**
  after a node restart and does not double-write (the `lb-jobs` checkpoint + ingest dedup
  `(series,producer,seq)`). The external reconnection is the sidecar's; the durable truth is SurrealDB.
- **Happy round-trips:** `datasource.add` → `test` green → `federation.query` returns seeded rows;
  `federation.mirror` pulls a seeded range → series → SSE shows it; a **rule** reads `source("…")` end to
  end (ties to `rules-engine-scope.md`).
- **Secret mediation:** the DSN never appears in a record, a log, the page, or a `federation.query`
  result (a redaction assertion).
- **Frontend (real gateway):** the Datasources admin page (`add`/`test`/`list`) over the bridge
  (`*.gateway.test.tsx`) + a dashboard reading mirrored series — against a real spawned node + real DB.

## Resolved decisions

No open questions — these are the long-term answers the build follows.

- **Engine → embed `datafusion` + `datafusion-table-providers` as a library in the extension.** Not the
  Spice product as a service, not Cube. Rationale: the federation value with our verb in front and the
  wall intact — the `rubix-cube` lesson (embed, don't bolt on a service).
- **Placement of the engine → the `federation` native Tier-2 extension, never core.** It owns sockets +
  a heavy dep; that belongs in a supervised, admin-approved, `net:*`-gated process, not the symmetric
  node binary (rule 1). Core links no DB driver.
- **Verb → one read-first `federation.query`, workspace-pinned by the host.** SELECT-only validated
  (port `rubix-cube`'s validator). A `federation.write` is a **separate, later, Ask-gated** verb (the
  per-tool-call Ask gate, `agent-run-scope.md` Part 2), not in v1 — read access first.
- **Source resolution → a registered `datasource:{ws}:{name}` record; callers name the alias, never a
  raw endpoint.** The wall is at admin registration (the `net:*` + secret approval), so a caller can't
  reach an unapproved endpoint by spelling a DSN.
- **Two consume modes → federate (`federation.query`) + mirror (`federation.mirror` = an `lb-jobs`
  job).** Live query for freshness; durable resumable mirror into the series plane for dashboards/cache/
  offline. The `0003` external-warehouse pattern, made concrete. SurrealDB stays authority either way.
- **SurrealDB is never a DataFusion source.** Platform data is read natively (`data.query`/`series.*`);
  registering it as a federated source would fork the authority and bypass `caps`. Hard line.
- **Secret + connection → behind one `Source` trait, one file per kind; DSN in `lb-secrets`.** The
  reference-extension discipline (the GitHub/Timescale "true external behind one trait" carve-out).
- **Result bound → a row cap on `federation.query`; unbounded reads are a mirror job.** No blocking
  large read in a tool handler (§6.1).
- **Spice / Cube revisit path noted, not built.** If a customer needs their BI stack (Tableau/Superset)
  to point at the data, a *future* "gated external BI endpoint" extension can wrap a Spice/Cube SQL-wire
  surface deliberately, with its own tenancy mapping + tests — out of scope here, recorded so it isn't
  smuggled in.

## Related

- `../extensions/reference-extensions-scope.md` — the native-extension doctrine, the `net:*` family, the
  native host-callback (fix 1), and the `timescale` reference extension this generalizes.
- `../rules/rules-engine-scope.md` — the rule `source(...)` verb that collects via `federation.query`;
  the uniform-surface/two-paths split.
- `../rules/rule-chains-scope.md` — a chain step that reads an external source inside its rule.
- `../secrets/secrets-scope.md` — the DSN mediation; `../ingest/ingest-scope.md` — the mirror target
  (the lazybones-native series plane, the answer to "Timescale" for *platform* data).
- `../jobs/jobs-scope.md` — the resumable mirror job; `../ai-gateway/ai-gateway-scope.md` — where a
  rule's `ai.ask` proposes the SQL `federation.query` then re-validates and runs.
- `../../vision/0003-iot-dashboard.md` §"An external warehouse (e.g. TimescaleDB)" — the doctrine this
  implements; README `§3` (rules 2/5/6), `§6.3` (two tiers), `§6.10` (jobs/batch).
- Source: `rust/rubix-cube/` (the embedded-DataFusion + SQL-validator pattern reused; MIT/Apache-2.0).
