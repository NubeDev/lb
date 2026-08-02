# Datasources scope — `federation.profile`, a durable per-source discovery profile

Status: scope (the ask). Promotes to `public/datasources/datasources.md` once shipped.

We want a **persisted, workspace-scoped discovery profile per datasource** — per table: the
columns and their kinds, real foreign keys, per-text-column cardinality + top distinct values,
per-numeric-column min/max and per-group range spans, and the detected time/value/metric/place
shape inputs — computed **server-side in one bounded pass** and stored as a
`datasource_profile:{ws}:{source}` record, read back in one cheap call. Today every "smart"
consumer (rubix-ai's Quick Chart, agents writing SQL, the panel wizard) re-derives this live from
the browser: N× `SELECT DISTINCT … LIMIT 200` + N× `GROUP BY` range scans per open, ~20 s on a
wide table, uncached server-side, and re-paid by every user on every page load.
`datasource-samples-scope.md` explicitly deferred this: *"No statistics/profiling (row counts,
min/max, cardinality). Useful later."* Later is now.

> Read with: `datasources-scope.md` (the parent federation extension),
> `datasource-samples-scope.md` (the sibling snapshot verb whose pipeline this reuses),
> `schema-designer-scope.md` (the `db_schema:{ws}:{name}` record — the persisted-record template),
> `../embeddings/embeddings-scope.md` (the derived-index pipeline shape this copies),
> `../caching/response-cache-scope.md` (why the moka cache is the wrong home for this).

**Owning repo: `NubeDev/lb`** (host verbs + record + reactor + sidecar profiling pass), shipped
additively and tagged `node-v*`; rubix-ai bumps the pin and adopts in
`rubix-ai/docs/scope/frontend/dashboard/quick-chart-discovery-scope.md`. Others get the benefit:
any lb consumer, agent, or extension can read the profile.

---

## Goals

- **One profiling pass, sidecar-side.** A new `federation.profile {source, tables?}` computation:
  for each table (bounded), read the schema + FKs (the `federation.sample` reads), then **one
  scan per table** computing per column — text: distinct count (capped) + top-K values (default
  60, matching the metric-cardinality ceiling); numeric: min/max, null fraction; timestamp-ish:
  min/max range. Grouped range spans (per text column × the table's value column) computed in the
  same engine, not in the browser. This replaces ~2×N browser round-trips with one sidecar pass
  next to the data.
- **A durable derived record.** Result persisted as `datasource_profile:{ws}:{source}` —
  `{source, profiled_at, tables: [{name, row_estimate, columns: [{name, type, kind,
  distinct?, values?, min?, max?, null_frac?}], foreign_keys, group_ranges?}], truncated,
  version}`. Derived data, always rebuildable; wiping it loses nothing (the embeddings doctrine).
- **One cheap read verb.** `federation.profile_get {source}` returns the stored record (or
  `NotFound` when never profiled) — instant, no external DB touch. This is what UIs and agents
  call on the hot path.
- **Freshness without inline cost.** A `react_to_profiles` reactor (the `react_to_reminders` /
  `relay_outbox` altitude): a bounded, indexed scan for profiles older than `refresh_after_secs`
  (default: 24 h) re-enqueues a profiling pass as an `lb-jobs` job. Plus an explicit
  `federation.profile_refresh {source}` for force-rebuild (the `docs.reindex` shape). First
  profile is produced on datasource **register/update** (enqueue, not inline) and on the first
  `profile_get` miss if the caller passes `{compute_if_missing: true}`.
- **Zero new read privilege.** `profile_get` and `profile` ride the existing
  `mcp:federation.query:call` cap (the `federation.schema`/`sample` precedent — a profile is
  strictly less than what the read cap can already `SELECT`). `profile_refresh` gets its own cap
  `mcp:federation.profile_refresh:call` because it spends external-DB work on demand.
- **Prompt-ready.** The record is bounded (caps below) so it can travel as an agent context
  **ref** within the context-basket fences (refs-not-bodies, 8 KB body budget) — the sanctioned
  path for "give the model the shape of this source".

## Non-goals

- **Not the response cache.** The moka page-cache is in-memory, restart-cold, 60 s-TTL, and
  blind to external writers — all wrong for hours-fresh durable metadata. No new cache class.
- **Not histograms/sketches/full statistics.** Top-K values, min/max, cardinality, range spans —
  the inputs chart/SQL inference actually uses. HLL/t-digest are a later verb if ever needed.
- **No drift watching.** We refresh on a clock and on demand; we do not subscribe to or poll the
  external DB for schema change (the `schema-designer-scope.md` "no schema sync" stance).
- **No PII policy beyond the sample denylist.** The `datasource-samples` column-name denylist
  (`password|secret|token|api_key|hash`) applies to profiled `values` too; nothing smarter here.
- **Not a query planner/statistics feed for DataFusion.** The engine keeps its own statistics
  path; this record is for callers, not the planner.

## Intent / approach

Copy the embeddings pipeline shape onto datasources: **an explicit batch job for bulk/rebuild +
a reactor for scheduled freshness, producing an idempotent derived record**, never computed
inline on the read path.

- **Sidecar** (`rust/extensions/federation/src/profile.rs`): the pass. Per table, one
  `describe_table` + `foreign_keys` (existing reads), then per-column aggregates emitted as a
  small number of engine queries (`SELECT col, COUNT(*) … GROUP BY col LIMIT K` for text;
  one `SELECT MIN/MAX/COUNT` row for numerics — batched per table where the dialect allows).
  Bounded: ≤ 25 tables/pass, ≤ 60 values/column, ≤ 200 distinct counted before capping to
  `"60+"`, cells truncated ~256 chars, `truncated: true` when cut — the `federation.sample`
  bounding stance. Never catalog SQL through `federation.query` (unplannable — see
  `../../debugging/datasources/discovery-via-information-schema-sql-unplannable.md`); always the
  provider schema reads.
- **Host** (`rust/crates/host/src/federation/profile.rs` — verbs + descriptors;
  `profile_store.rs` — the record CRUD): `federation.profile` authorizes → resolves the alias in
  the caller's ws → `net:*` check → mediates DSN → one sidecar call → **upserts the record** →
  returns it. `profile_get` is a pure store read. `profile_refresh` enqueues an `lb-jobs` job
  (kind `datasource_profile`) so a slow source never blocks the caller; the job runs the same
  host path.
- **Reactor** (`react_to_profiles`): stateless durable scan on the tick, ws-isolated like every
  sibling reactor. **Scan cost must be indexed and bounded** — select only
  `profiled_at < now - refresh_after_secs` with a LIMIT, never a full-table rescan
  (`../../debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md` is the cautionary tale).
  Off by default; enabled + tuned via `BootConfig`.
- **BootConfig seam:** `profile: Option<ProfileConfig { enabled, refresh_after_secs,
  max_tables, max_values }>` — role = config, no branches. Standalone env `LB_PROFILE=1` etc.
  read only at the binary boundary. Compile-time cargo feature `datasource-profile`
  (OFF by default, the `page-cache` precedent); CI compiles + boots with it off.

**Alternatives rejected:**
1. *Cache the browser's probe queries in the gateway response cache.* Fixes warm repeats only;
   first-touch still pays N round-trips, restarts go cold, and the cache scope itself flags that
   external writers never invalidate it. Also leaves every consumer re-implementing the probe.
2. *Profile inline inside `federation.sample` on every call.* The embeddings scope's rejected
   "embed inline in `put_doc`" trap: it moves the 20 s into a different caller instead of
   deleting it. Sample stays cheap; profile is the precomputed sibling.
3. *A rubix-ai-only cache layer.* Violates the family direction — the host/agent/extension
   consumers upstream would each rebuild it; discovery is core mediation-path material.

## How it fits the core

- **Tenancy / isolation:** record id is `datasource_profile:{ws}:{source}`; alias resolves in
  the caller's workspace; ws-B reading ws-A's profile → `NotFound`. The reactor honours ws
  isolation (a ws-B tick never touches ws-A records). Isolation test mandatory.
- **Capabilities:** reads under existing `mcp:federation.query:call` (opaque `Denied`);
  `profile_refresh` under its own cap. Deny tests mandatory for both.
- **Placement:** either role — sidecar call + store write, both role-agnostic (symmetric nodes).
- **MCP surface:** `profile_get` (get), `federation.profile` (compute+upsert, bounded
  synchronous), `profile_refresh` (enqueue → job id). Tool-catalog rows + real arg schemas
  (`x-lb entity: datasource` on `source`) ship alongside, per `tool-catalog-scope.md`.
- **Data (SurrealDB):** one new record table, indexed on `{ws, profiled_at}` for the reactor
  scan. Derived + rebuildable; no migration concerns.
- **Motion / jobs:** refresh rides `lb-jobs` (kind `datasource_profile`) — durable, resumable,
  attempts-bounded. No new queue.
- **Secrets:** DSN mediated host-side, never stored in or returned with the profile.
- **Rule 10:** the record and verbs are generic over every `Source` kind; no kind- or
  extension-specific branch. A kind that can't answer an aggregate returns nulls, never errors.
- **SDK/WIT impact:** none — Tier-2 native protocol, additive verbs; `Source` gains no new
  required method (profiling composes existing reads + engine queries).
- **FILE-LAYOUT:** one responsibility per file as listed above; match arms in
  `federation/tool.rs` + sidecar `main.rs`; reactor in its own `react_to_profiles.rs`.
- **Skill doc:** the implementing session adds a live `profile_get` run to
  `docs/skills/datasources/SKILL.md`.

## Example flow

1. Admin registers `warehouse` (sqlite) in ws `acme`; registration enqueues a
   `datasource_profile` job.
2. The job runs: host resolves + mediates → one sidecar `federation.profile` pass → 6 tables
   profiled (columns, FKs, per-text top-60 values + cardinality, numeric min/max, group range
   spans vs the detected value column) → `datasource_profile:acme:warehouse` upserted,
   `profiled_at` stamped.
3. A user opens rubix-ai's Quick Chart → the UI calls `profile_get {source: "warehouse"}` →
   the record returns in one store read → `detectMetricShape` runs on it locally → step 3
   renders **instantly**; the 20 s probe fan-out never fires.
4. An agent asked to "chart energy by site" receives the profile as a context ref and writes a
   correct grouped query first try.
5. 24 h later `react_to_profiles` finds `profiled_at` stale, enqueues a refresh; a schema change
   on the source appears in the profile within a day, or immediately after an admin hits
   `profile_refresh`.

## Testing plan

Per `scope/testing/testing-scope.md` — real store (`mem://`), real sidecar, seeded SQLite, no fakes:

- **Capability deny (mandatory):** no `mcp:federation.query:call` → opaque `Denied` on
  `profile`/`profile_get`; no `mcp:federation.profile_refresh:call` → `Denied` on refresh.
- **Workspace isolation (mandatory):** ws-B `profile_get` of ws-A's source → `NotFound`;
  reactor tick in a two-ws store only refreshes each ws's own records.
- **E2E happy path:** seed the demo SQLite (two tables, FK, mixed text/numeric/time columns,
  > 60 distinct in one column) → `federation.profile` → assert record shape: top-60 + capped
  cardinality flag, numeric min/max, FK present, group ranges vs the value column, no DSN
  anywhere.
- **Bounds:** 30-table source → 25 + `truncated: true`; long cells truncated; denylisted column
  (`password`) emits `«redacted»` values.
- **Idempotence:** re-profile of an unchanged source upserts the same record (stable ordering).
- **Reactor:** stale record → job enqueued exactly once (no duplicate enqueue while a job is
  in-flight); fresh record → no work; scan is LIMIT-bounded (assert query shape or row-touch
  count).
- **Feature-off build:** `datasource-profile` off compiles, boots, and the verbs are absent from
  the catalog.
- Offline/sync, hot-reload: N/A.

## Risks & hard problems

- **Profiling cost on the external DB.** Top-K + min/max per column is cheap on indexed columns
  and a full scan on others; a huge unindexed table could hurt the source. Mitigations: per-pass
  table/column caps, `row_estimate` first (cheap catalog read where available) with a skip
  threshold, the job (not inline) execution path, and off-by-default reactor.
- **Duplicate/overlapping refreshes.** Reactor tick + manual refresh + register-time enqueue can
  race; the job claim (`status='queued'` atomic claim) plus an in-flight guard on the record
  (`profiling_since`) must make enqueue idempotent.
- **Record size.** 25 tables × wide columns × 60 values can exceed the context-basket body
  budget; the record must carry per-table sub-objects so a caller can ref one table's slice, and
  a worst-case record should stay well under ~100 KB (verify in the bounds test).
- **Reactor CPU on small boxes.** The Pi-pegging incident is the standing hazard — the scan must
  be index-backed and the default tick lazy (minutes, not seconds).

## Open questions

- Should `profile_get` fall back to computing when missing (`compute_if_missing`) or stay a pure
  read and let callers enqueue? Recommend: pure read + an explicit flag, so the hot path can
  never block 20 s by surprise.
- `row_estimate`: catalog-based (fast, approximate, per-dialect) vs `COUNT(*)` (exact, possibly
  slow)? Recommend catalog where the kind supports it, else omit.
- Should the reactor's `refresh_after_secs` be per-datasource (a field on the `datasource`
  record) or global BootConfig? Recommend global first, per-record when asked for.

## Related

- `datasource-samples-scope.md` — the sibling snapshot verb; its "no statistics/profiling"
  non-goal is exactly this scope.
- `schema-designer-scope.md` — the `db_schema:{ws}:{name}` persisted-record template.
- `../embeddings/embeddings-scope.md` — the batch-job + reactor derived-index pipeline copied here.
- `../caching/response-cache-scope.md`, `../caching/dashboard-query-acceleration-scope.md` — why
  this is a record, not a cache class.
- `../jobs/jobs-scope.md` — the durable queue the refresh rides.
- `../../debugging/datasources/discovery-via-information-schema-sql-unplannable.md` — why the
  pass uses provider schema reads, never catalog SQL.
- `../../debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md` — the reactor scan-cost hazard.
- Downstream consumer: rubix-ai
  `docs/scope/frontend/dashboard/quick-chart-discovery-scope.md`.
