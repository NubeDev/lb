# Caching — optional server-side response cache (session)

- Date: 2026-07-22
- Scope: ../../scope/caching/response-cache-scope.md
- Stage: build (slice 2 — moka + generations + single-flight); slice 1 (asset headers) audited; slice 3 (warm tier) deferred per scope
- Status: in-progress

## Goal
Build the `page-cache`-gated response cache upstream in lb: the read-path middleware after
auth+caps, verb-class policy, canonicalisation, per-`{ws,class}` generation invalidation,
single-flight, `BootConfig.cache`, `cache.stats`/`cache.purge`, all feature-gated with a zero-cost
no-op seam. Exit gate: the feature-off build compiles + the existing suite stays green, and the
correctness suite (deny, ws-isolation, staleness, single-flight, budget) passes on a real embedded node.

## What changed
New module `crates/host/src/cache/` (all files ≤400 lines, one responsibility each):
- `config.rs` — `CacheConfig` (always compiled; plain data — `enabled`, `memory_budget_bytes`,
  `list_ttl_secs`). Re-exported `lb_host::CacheConfig` → `lb_node::CacheConfig`.
- `policy.rs` (feature-on) — the declarative verb-class table: `read_class(verb)` (the v1 allowlist)
  and `dirties(verb)` (write → invalidated classes). The ONE place cacheability is declared.
- `generation.rs` (feature-on) — `Generations`: a `DashMap<(ws,class),u64>`; `current`/`bump`/`bump_all`.
- `live.rs` (feature-on) — `ResponseCache`: `moka::future::Cache` (byte weigher over key+value, global
  TTL, eviction-listener counter), `get_or_compute` (single-flight via `entry().or_try_insert_with`,
  hit/miss counting via `Entry::is_fresh`), canonical key builder (recursive key-sort + sha256),
  `stats_snapshot`, `purge`. Unit tests for canonicalisation (field-order/gen/ws/null-vs-absent).
- `verbs.rs` (feature-on) — `cache.stats` / `cache.purge` handlers.
- `mod.rs` — the cfg-selected `dispatch` seam (on: cache; off: passthrough), `compute_json` (shared),
  the `CacheSlot`/`new_slot` node-field type, `call_cache_tool` (both faces).

Wiring:
- `tool_call.rs`: extracted the 240-line host-verb fan-out into `pub(crate) run_host_verb` so the seam
  can wrap it lazily (single-flight); the `is_host_native` block now ends with
  `return crate::cache::dispatch(node, principal, ws, verb, &input, depth).await;` (after the caps gate
  at line 485 + arg validation). Added `"cache."` to `HOST_NATIVE_PREFIXES` + a `cache.` arm.
- `Node` (`boot.rs`): a `response_cache: CacheSlot` field (a `()` when feature-off — zero cost),
  `install_response_cache` / `response_cache()`.
- `BootConfig.cache: Option<CacheConfig>` (`node/src/config.rs`) + `Default None` + `from_env`
  (`LB_CACHE`/`LB_CACHE_BUDGET_MB`); installed in `builder.rs` right after node boot.
- Cargo: `moka` (optional) + `page-cache` feature on `lb-host`; forwarded by `lb-node/page-cache`.
- Caps: `mcp:cache.stats:call` / `mcp:cache.purge:call` added to `ADMIN_ONLY_CAPS`.
- `ToolError`: `+ Clone` (moka's `Arc<E>` error path).
- `system/catalog.rs`: `cache.stats`/`cache.purge` rows (the coverage assertion).
- `role/gateway/src/lib.rs`: `#![recursion_limit = "512"]` — a pre-existing latent `E0275` (the
  trait solver overflowing the default 128 while proving `Sync` for a deeply-nested rhai AST type in
  a route handler) that only surfaces on a COLD compile. Warm incremental builds dodged it; the fresh
  worktree build of the test suite exposed it. rhai's `sync` feature is unified on, so the type IS
  `Sync` — a depth limit, not a real `!Sync`. Minimal, non-behavioural; unblocks cold builds.

## Decisions & alternatives
- **`viz.query` DEFERRED from the v1 allowlist (the pivotal decision).** The build's first task —
  auditing every allowlist candidate for grant-filtering — found `viz.query` is **subject-filtered**:
  it re-authorizes each panel target under the caller's own grants (`viz/query.rs:240-265`), so a
  denied target collapses to an empty frame and the result varies by caller. Caching it under the
  scope's subject-free `{ws,verb,args,gen}` key would leak a privileged caller's frames to a
  co-workspace caller who lacks the underlying target caps. Per the scope's own rule ("any
  subject-filtered verb drops out **until keyed safely**") and the "when in doubt uncacheable"
  guidance, v1 ships the **5 audit-clean list verbs** (`datasource.list`, `series.list`, `flows.list`,
  `flows.get`, `ext.list`) and defers `viz.query`. The user was consulted and chose this low-risk path
  over the alternative (admit `viz.query` keyed by a capability fingerprint) because a wrong
  fingerprint is a cross-user leak — the highest-stakes failure mode. **Consequence:** the source-picker
  bundle coalescing win ships; the `viz.query` recompute win does not. **Quantisation also defers**
  with `viz.query` (its only consumer). See the scope's updated Decisions.
- **The `cache:read`/`cache:admin` pair mapped onto the existing MCP grammar** (`mcp:cache.stats:call`
  = read, `mcp:cache.purge:call` = admin) rather than a new `Surface::Cache`. The generic host-verb
  dispatch authorizes every verb via `Surface::Mcp`; adding a parallel surface would force the cache
  verbs off the generic path — a rule-10 wrinkle for no gain. Two distinct verb caps give the same
  read-vs-admin split and deny paths. Recorded in the scope Decisions.
- **Policy in a declarative `policy.rs` table, not per-`ToolDescriptor` fields (yet).** The scope's
  ideal is a `cache_class`/`dirties` field on each descriptor so the staleness sweep is mechanical over
  *every* registered verb. v1's allowlist is 5 reads + a handful of writes — small enough to state and
  test by hand — so the table is the pragmatic home. Per-descriptor declaration is the named
  invalidation-hardening follow-up.
- **`ext.list` liveness is TTL-bounded, not generation-invalidated.** Its `running`/`restart_count`
  are process state no MCP write verb dirties; staleness is bounded by the 60 s list TTL (the
  documented operator expectation, like an external sqlite writer). A generation bump on
  install/sidecar-transition is a follow-up.
- **`store.write`/`store.delete` nuke all classes** (coarse but safe): a generic per-table write could
  touch any cached domain, so over-invalidation is the correct v1 trade (correctness > hit rate).
- **Large-future boxing.** Extracting `run_host_verb` (a ~240-line async fn → large future) under the
  seam's extra async layers overflowed the debug worker stack; `Box::pin`-ing the `run_host_verb` call
  in `compute_json` heap-allocates it (the same pattern the viz re-entry uses). See Dead ends.

## Tests
`crates/host/tests/response_cache_test.rs` — real embedded node (`Node::boot`, `mem://`), cache on via
`install_response_cache`, driven through `lb_host::call_tool` (the real gated path), no mocks. Covers:
perf/de-dup (warm re-open = 0 engine dispatch), single-flight (16 concurrent cold reads → 1 miss/15
hits), staleness-after-write (`ingest.write`→`series.list`; `store.write` nuke), workspace-isolation
(+ `cache.purge` scoping), capability-deny (capless caller denied on a warm key; `cache.*` admin
deny), uncacheable-verb-never-hits, budget/eviction. Plus `cache::live` canonicalisation unit tests.

**Test-run status: GREEN.** The `page-cache` lib build (both faces) compiles; the cache unit +
canonicalisation + catalog-coverage tests pass; and the full integration suite passes on a real
embedded node (run in an isolated git worktree at commit `f5405ad9` — see Dead ends for why):

```
$ cargo test -p lb-host --features page-cache --test response_cache_test -- --test-threads=1
running 8 tests
test budget_bounds_the_cache ... ok
test cache_admin_verbs_require_their_caps ... ok
test deny_is_identical_on_warm_and_cold_keys ... ok
test single_flight_coalesces_concurrent_cold_reads ... ok     # 16 concurrent cold reads → 1 miss, 15 hits
test uncacheable_verb_never_hits ... ok
test warm_reopen_runs_zero_engine_dispatches ... ok           # re-open = 0 engine dispatch
test workspace_isolation_and_purge_scope ... ok
test write_invalidates_immediately ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
```

Feature-off `host_catalog_covers_dispatch_prefixes` and feature-on `cache::live::tests` (4) also green.

## Debugging
- Debug-stack overflow from the extracted large future → fixed by `Box::pin` (above). Regression
  coverage: the whole integration suite exercises the boxed path; a stack overflow would abort it.
- No `debugging/<area>/…` entry opened (the overflow was caught + fixed within the session and is
  covered by the suite); note it here per the append-only history norm.

## Public / scope updates
- Scope `docs/scope/caching/response-cache-scope.md` Decisions updated: `viz.query` deferred (audit
  finding) with the keyed-safely follow-up; the `cache:read`/`cache:admin`→MCP-grammar mapping;
  quantisation deferred with `viz.query`.
- Public page `doc-site/content/public/caching/caching.md` flipped from stub → what-shipped.

## Skill docs
`skills/page-cache/SKILL.md` — driving `cache.stats`/`cache.purge` + the on/off knobs. Grounded in the
real verb shapes; the live-run payloads land with the smoke test.

## Dead ends / surprises
- The extension `call/` pipeline (`crates/mcp`) is NOT where the allowlist verbs dispatch — they are
  host-native and funnel through `crates/host/src/tool_call.rs::dispatch_at_depth`. The cache hooks
  there, on the `Node`, not in `role/gateway`.
- A shared working tree with a second live AI session is a real hazard: transient compile breakage in
  untouched crates. The commit-then-worktree isolation is the mitigation.

## Follow-ups
- **`viz.query` keyed-safely** (subject_scoped class = key folds a capability fingerprint; +
  time-bucket quantisation). The whole "headline" win + the quantisation slice.
- Per-`ToolDescriptor` `cache_class`/`dirties` for a mechanical staleness sweep.
- `ext.list` generation bump on install/sidecar transition.
- STATUS.md moved (slice-2 in progress). rubix-ai pin bump is a follow-up gated on the node-v0.5.x tag.
