# Query — saved PRQL queries, Phase 1 (session)

- Date: 2026-06-30
- Scope: ../../scope/query/prql-query-scope.md
- Stage: post-S8 platform (rules + federation plane shipped) — this adds the `query.*` surface over them
- Status: done (backend surface + tests + docs shipped; UI Queries page deferred — see Follow-ups)

## Goal

Ship Phase 1 of the saved-PRQL-query surface end to end and green: a pure `lb-prql` crate wrapping
`prqlc` (compile-only), a host `query.*` MCP family (`save`/`get`/`list`/`delete`/`run`/`compile`)
over a `query:{ws}:{id}` record, the no-widening capability rule, the rules `source("query:<name>")`
seam, and the docs. Phase 2 (DataFusion-over-native-reads for full Surreal PRQL) is explicitly NOT
built — one-line note only.

## What changed

### New crate `lb-prql` (`rust/crates/prql/`)
A pure, zero-I/O wrapper over the pinned `prqlc` (0.13). One responsibility per file:
- `lib.rs` — barrel + `PRQLC_VERSION` const.
- `dialect.rs` — the `Dialect` enum (`Generic`/`Postgres`/`MySql`/`DuckDb`), the map to `prqlc`'s
  dialect, `dialect_for_target` (`"platform"` → `Generic`) and `dialect_for_kind` (datasource kind).
- `compile.rs` — the single `compile(prql, dialect) -> sql` entry point.
- `error.rs` — `PrqlError::{Compile, BadDialect}`.
- `tests/golden_test.rs` — 11 goldens (per-dialect SELECT, aggregate/take, malformed→typed error,
  no signature comment, target/kind maps).

### New host service `query/` (`rust/crates/host/src/query/`)
Sibling to `federation/` and `rules/`, mirroring their patterns exactly:
- `record.rs` — `SavedQuery { id, name, description, lang, text, target, params, tag, removed, ts }`,
  table `query`, `put`/`resolve` (soft-delete collapse), workspace-namespaced.
- `authorize.rs` — `mcp:query.<verb>:call` gate (workspace-first).
- `target.rs` — `QueryTarget::{Platform, Datasource(name)}` + the **no-widening** `underlying_tool()`
  map (platform→`store.query`, datasource→`federation.query`).
- `materialize.rs` — compile `text` to target SQL (PRQL via `lb-prql`, or `raw` verbatim); resolves a
  datasource target's `kind` in the caller's workspace; `validate_params` (datasource params = typed
  error in v1 — no sidecar bind path yet).
- `save.rs` / `get.rs` / `delete.rs` — CRUD; `list.rs` returns a flat roster (no text/result data).
- `compile_verb.rs` — `query.compile` pure dry-run → `{sql}` (own cap, no data access).
- `run.rs` — `query.run {id}` or inline `{lang,text,target}` → `{columns, rows}`. Order: authorize
  `query.run` → resolve → validate → **authorize target cap (no-widening, before compile/resolution)**
  → check params → materialize → dispatch to `store.query`/`federation.query`.
- `descriptors.rs` — `save`/`run`/`compile` descriptors with `x-lb` widgets (`prql` text + `datasource`
  entity picker).
- `tool.rs` — the `query.*` MCP bridge dispatch.

### Wiring
- `rust/crates/host/src/tool_call.rs` — added `query.` to `is_host_native` + a dispatch arm to
  `call_query_tool`.
- `rust/crates/host/src/tools/descriptor.rs` — registered `save`/`run`/`compile` descriptors in
  `host_descriptors()`.
- `rust/crates/host/src/lib.rs` — module + public re-exports.
- `rust/crates/host/Cargo.toml` — added `lb-prql` dep.
- `rust/Cargo.toml` — added `crates/prql` member, `lb-prql` path dep, pinned `prqlc = "0.13"`.

### Rules seam `source("query:<name>")`
- `rust/crates/host/src/rules/seam.rs` — `HostDataSeam` now carries a `queries` set; `resolve` handles
  `query:<id>` (kind from the saved query's target); `collect` intercepts `query:<id>` and routes
  through the ONE MCP contract (`query.run`), so caller ∩ grant (incl. the no-widening target cap) is
  re-checked. Added `workspace_queries` helper.
- `rules/run.rs` + `lib.rs` — thread the queries allowlist through.

### Tests `rust/crates/host/tests/query_test.rs`
10 integration tests on real infra: capability-deny per verb, the two headline no-widening denies
(platform needs `store.query`; datasource needs `federation.query` even with `query.run`),
workspace isolation, compile + malformed-PRQL, read-only gate rejects a `raw` write, injection-safe
param binding (missing/extra typed errors), the save→get→edit→save→run round-trip on real SurrealDB
rows, a rule reading `source("query:<name>")`, and a datasource round-trip against a REAL spawned
Postgres (reusing the federation rig; skips cleanly if Docker is unavailable).

## Decisions & alternatives

- **No-widening check ordered BEFORE compile/resolution.** `query.run` authorizes the target's
  underlying cap before it compiles or resolves the datasource, so the headline deny bites even when
  the datasource is absent or the PRQL is malformed. Rejected: checking it after dispatch (the
  composed verbs re-check anyway) — too implicit; the rule must be legible at the call site.
- **Datasource params = typed error in v1.** The `federation.query` sidecar's JSON-RPC input is
  `{kind,dsn,source,sql}` with no bind-param path. Rather than half-implement or interpolate, a
  parameterized datasource query is a loud `BadInput`. Full platform `$var` binding ships now.
  Rejected: string interpolation (would defeat the read-only gate — the scope's central injection risk).
- **PRQL `select {col}` over `SELECT *` on platform.** PRQL emits `SELECT *` for bare `from t`, which
  pulls SurrealDB's record `id` (an enum-ish type that fails JSON deser). Selecting specific,
  non-table-name columns is the supported relational subset; `lang:"raw"` covers the rest. This is the
  honest Phase-1 subset boundary the scope names.
- **`source("query:<id>")` routes through `query.run`, not a direct service call.** Mirrors how the
  seam already routes federation through `call_tool("federation.query")`; keeps caller ∩ grant + the
  no-widening cap as the single chokepoint.
- **Mirrored, not invented.** `query/` is a structural copy of `federation/` (record/authorize/error/
  tool/descriptor shapes) and `rules/` (id+name saved-record, store CRUD gates), per the brief.

## Tests

Mandatory categories (testing §2): capability-deny (**per verb** + the two headline no-widening
denies), workspace-isolation, and round-trips on REAL backends. No mocks for our own stack: real
embedded SurrealDB, real caps, the real `store.query` parse-allowlist, and the real federation sidecar
against a real spawned Postgres (the one sanctioned external, testing §0). `lb-prql` is pure-unit with
goldens.

Green output:

```
$ cargo test -p lb-prql
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-host --test query_test
test each_verb_denied_without_its_cap ... ok
test compile_returns_sql_and_malformed_is_typed_error ... ok
test raw_platform_write_rejected_by_read_only_gate ... ok
test headline_no_widening_datasource_run_denied_without_federation_cap ... ok
test rule_reads_saved_query_by_name ... ok
test workspace_b_cannot_get_run_delete_ws_a_query ... ok
test params_bind_safely_through_store_query_vars ... ok
test headline_no_widening_run_requires_target_cap ... ok
test round_trip_save_get_edit_save_run_platform ... ok
test round_trip_datasource_real_postgres ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo build --workspace
Finished `dev` profile [unoptimized + debuginfo] target(s)
```

Full `cargo test --workspace --no-fail-fast`: every suite green except the pre-existing flaky
`agent_routed_test::an_edge_invokes_the_hub_agent_over_the_routed_namespace` (a Zenoh discovery
timing race that passes in isolation and is unrelated to this scope).

## Debugging

None — nothing non-trivially broke. Two test-time findings were absorbed inline (not bugs in shipped
code, but worth recording):

- `query.run` initially compiled/resolved the datasource BEFORE the no-widening cap check, so the
  headline datasource deny surfaced as `BadInput("no such datasource")` instead of `Denied`. Fixed by
  reordering (Decision above). The `headline_no_widening_datasource_run_denied_without_federation_cap`
  test is the regression.
- PRQL `select {series, payload}` emitted `SELECT *, payload` (PRQL treats `series` as the table
  name), pulling the record `id` that fails JSON deser. Documented as the subset boundary; tests
  select non-table-name columns.

No `debugging/query/*.md` entry was needed (no shipped-code bug).

## Public / scope updates

- Promoted shipped truth to `public/query/query.md` (replaced the TODO stub).
- Resolved the scope's open questions in `scope/query/prql-query-scope.md` (identity, versioning,
  ad-hoc run, organization, params, downstream binding) and marked Phase 2 deferred.
- Updated `STATUS.md` with the new shipped `query.*` surface.

## Dead ends / surprises

- `prqlc`'s default features build the CLI binary; `default-features = false` shed the heavy CLI deps
  while keeping the library. Pinned at the workspace root (`0.13`).
- The `federation.query` sidecar genuinely has no bind-param path — confirmed by reading
  `extensions/federation/src/main.rs` (input is `{kind,dsn,source,sql}`). This bounded Phase-1
  datasource param support honestly rather than shipping a silent no-op.
- The flaky Zenoh routed test is pre-existing (fails under parallel load, passes in isolation) and
  unrelated; left as-is.

## Follow-ups

- **UI Queries page** — deferred this session (the backend `query.*` surface + tests + docs are the
  must-ship; the page is additive over the descriptors already registered).
- **Phase 2** — full PRQL semantics on the platform target via DataFusion-as-compute over native
  `store.query` reads (scope open question, deferred by decision).
- **Datasource param binding** — extend the `federation.query` sidecar JSON-RPC with a `vars` path,
  then drop the v1 typed-error guard in `materialize::validate_params`.
- **Downstream binding** — saved query as a dashboard widget source / channel `/query` palette entry.
- STATUS.md updated: yes.