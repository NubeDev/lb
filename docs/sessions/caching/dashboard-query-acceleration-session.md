# Caching — dashboard query acceleration (session)

- Date: 2026-07-23
- Scope: ../../scope/caching/dashboard-query-acceleration-scope.md
- Stage: build — all three lb slices (viz.query cache passthrough, `subject_scoped` gateway cache +
  fingerprint + quantiser, `viz.query_batch` fan-in)
- Status: in-progress → green (lb half). Consumer half + pin bump land in rubix-ai.

## Goal / exit gate

Make a warm dashboard open a **hard 10×** by closing the whole caching campaign in lb:
1. `viz.query` carries a top-level, source-blind `cache: {ttl_s}` threaded into every target's args.
2. `viz.query` becomes gateway-cacheable via a `subject_scoped` cache class that folds a **capability
   fingerprint** (not identity/token) into the key + a **time-bucket quantiser** — warm opens skip the
   resolver entirely and N concurrent viewers collapse to one compute.
3. A new `viz.query_batch {panels[], now?, cache?}` fan-in resolves a board's panels concurrently in
   ONE call, per-item partial failure, bounded (cap 64), synchronous — killing the browser connection
   ceiling.

Exit gate: the feature-off build compiles (no-op seam), the existing suite stays green, and the new
correctness suite (deny, ws-isolation, the NEW cross-grant leak test, passthrough/bypass, quantiser,
single-flight, batch partial-failure + cap, and the perf assertion = a warm re-open runs ZERO resolver
dispatches) passes on a real embedded node.

## What changed (all files ≤400 lines, one responsibility — FILE-LAYOUT)

### Slice 1 — cache passthrough (the missing wire)
- `crates/host/src/viz/query.rs` — `viz_query` gains a `cache: Option<&Value>` param; `dispatch_target`
  threads it into EVERY target's args (`map.entry("cache").or_insert_with(...)`) — SOURCE-BLIND, and
  **without overwriting** a caller-supplied per-target `cache` (per-target wins, the top-level is the
  default). The same mechanism `now`/`ts`/`apply_time_override` already use.
- `crates/host/src/viz/tool.rs` — `viz.query` reads the top-level `cache` sibling of `now` and passes it.

### Slice 2 — `subject_scoped` gateway cache
- `crates/host/src/cache/policy.rs` — new `Class::VizSubjectScoped`; `read_class("viz.query")` maps to
  it; added to `ALL_CLASSES` (so `cache.purge` / generic `store.write` nuke it too); new
  `is_subject_scoped(class)` — the class, not a verb name, decides the fingerprinted path (rule 10).
- `crates/host/src/cache/fingerprint.rs` (NEW) — `capability_fingerprint(principal, ws, panel)`: a
  stable SHA-256 over the **sorted set of the panel's target caps the caller HOLDS**, computed with the
  SAME `gate_tool_for` + `authorize_tool` decision the resolver makes per target. No identity, no token.
  Two callers with the same reach share an entry; a caller who would get a different (denied) frame on
  any target produces a different fingerprint → a different key → its own resolve. The one leak boundary,
  in its own reviewed file.
- `crates/host/src/cache/quantise.rs` (NEW) — `quantise_viz_input(input, ttl_s)`: floors the top-level
  `now` and each target's numeric `from`/`to` to the TTL bucket (unit-detected ms vs s), so relative
  "last 1h" opens inside one bucket share the key AND the executed range. End-day-exclusivity survives
  (a boundary floors to itself). Non-windowed / string ranges untouched. Pure — unit-tested.
- `crates/host/src/cache/live.rs` — `get_or_compute_scoped` (same moka single-flight as `get_or_compute`
  but the key also folds the fingerprint) + `build_key_scoped` + the `viz` `class_name` arm.
- `crates/host/src/cache/mod.rs` — the `dispatch` seam branches on `is_subject_scoped(class)` into a new
  `dispatch_subject_scoped`: read `cache.ttl_s` (0/absent ⇒ passthrough = live bypass), quantise, compute
  the fingerprint, then `get_or_compute_scoped` on the quantised input (key and executed range agree).
- `crates/host/src/viz/query.rs` + `viz/mod.rs` — expose `pub(crate) panel_target_tools(panel)`, reused
  by the fingerprint so it folds EXACTLY the caps that gate the panel (kept in lockstep with the
  resolver's target resolution).

### Slice 3 — `viz.query_batch` fan-in
- `crates/host/src/viz/batch.rs` (NEW) — `viz_query_batch`: cap 64 (over-cap ⇒ `BadInput`), resolves each
  panel through the SHARED `crate::cache::dispatch("viz.query")` path (so each panel rides the
  `subject_scoped` gateway cache + fingerprint + quantiser identically — scope open-Q4), concurrently via
  `futures::join_all` bounded by a `Semaphore(16)`, per-item partial failure (a denied panel → opaque
  `{status:"denied"}`, a bad panel → `{status:"error", message}`, siblings resolve).
- `crates/host/src/viz/tool.rs` — the `viz.query_batch` arm (re-checks `mcp:viz.query:call`).
- `crates/host/src/tool_call.rs` — `gate_tool_for("viz.query_batch") => "viz.query"` (rides the existing
  cap; no new privilege — same shape as `series.latest_many`).
- `crates/host/src/system/catalog.rs` — the `viz.query_batch` `HostTool` row (visible to exactly the
  callers who can run `viz.query`, via the catalog's `gate_tool_for` gate).
- `crates/host/Cargo.toml` — `futures.workspace = true` (the bounded fan-in).

## Decisions (scope open questions)

1. **Fingerprint granularity** — the **sorted set of target caps the caller holds** among the panel's
   dispatched tools (scope Q1's recommendation). Minimal and provably the leak boundary: frame content
   varies by caller ONLY via allow/deny per target (within allow, all callers get identical rows), so
   the held-cap set is exactly sufficient. Mutation-checked by the cross-grant test.
2. **Batch cap N = 64** (scope Q2). Over-cap ⇒ `BadInput`; the UI chunks.
3. **Top-level `cache`** sibling of `now` (scope Q3), not `queryOptions.cache`.
4. **Slice 2 wraps the resolver, not the batch verb** (scope Q4): both `viz.query` and each batch panel
   resolve through `crate::cache::dispatch`, so a single-tile refresh hits the same warm path.
5. **Quantiser is two-sided.** The host floors STRUCTURED numeric ranges (`now`, `series.read`
   from/to). A `federation.query` target's time lives in its SQL string — the host never rewrites SQL
   (it doesn't own that vocabulary), so the caller (the rubix-ai UI half) buckets the `$__from/$__to` it
   bakes into SQL to the same `ttl_s`. Both layers then land on one grid: the federation result cache
   and the gateway `subject_scoped` cache share stable keys. Documented so the two halves agree.
6. **Freshness bound = the bucket in the key**, not a per-entry moka `Expiry`. An entry becomes
   unreachable when its bucket rolls (a new key); the global moka TTL is the memory backstop. For the
   conservative UI defaults (30–60 s ≈ the list TTL) a warm re-open inside the bucket is a hit; a larger
   `ttl_s` still bounds staleness by the bucket and simply recomputes if the entry was evicted first
   (correct, just a lower hit rate). Avoids a cache-wide `Expiry` policy for one class.

## Tests (real embedded node, `mem://`, seeded via the real `ingest.write` path — NO mocks)

- Unit: `cache/quantise.rs` (floor now/from/to, ms-vs-s units, end-day-exclusivity, non-windowed
  untouched, two-opens-in-a-bucket equal) — 6 tests.
- Integration: `crates/host/tests/viz_query_acceleration_test.rs` (`--features page-cache`), panels use
  `store.query` targets over the real store (the cache mechanisms are source-blind):
  - `warm_reopen_runs_zero_resolver_dispatch` — the perf assertion (1 miss cold, 1 hit warm, 0 resolves).
  - `quantiser_collapses_opens_within_a_bucket` — same bucket → one compute; next bucket → fresh.
  - `single_flight_collapses_concurrent_cold_opens` — 12 concurrent cold → ONE resolve.
  - `workspace_isolation_and_purge_scoping` — ws A/B never cross; purge A leaves B serving.
  - `cross_grant_caller_never_receives_warm_privileged_frame` — THE leak test, mutation-checked (a
    low-grant caller gets an empty denied frame, never the primed privileged rows).
  - `batch_denied_without_viz_cap` — capability-deny (opaque).
  - `directive_is_source_blind_and_zero_ttl_bypasses` — slice-1 passthrough / bypass parity.
  - `batch_warm_reopen_runs_zero_db_queries` — the batch perf assertion (1 batch resolves N; warm → N
    hits, 0 new resolves).
  - `batch_result_equals_single_query_per_panel` — batch/single parity.
  - `batch_partial_failure_isolates_the_bad_panel` — one bad tile errors, siblings resolve.
  - `batch_over_cap_is_bad_input` — the 64-panel cap.

Green output pasted below.

## Test output (2026-07-23, `--features page-cache`)

```
# cache lib unit tests (quantise + canonicalisation)
cargo test -p lb-host --features page-cache --lib cache::
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 281 filtered out

# the new acceleration integration suite (slices 1–3)
cargo test -p lb-host --features page-cache --test viz_query_acceleration_test
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  warm_reopen_runs_zero_resolver_dispatch ......................... ok
  quantiser_collapses_opens_within_a_bucket ....................... ok
  single_flight_collapses_concurrent_cold_opens ................... ok
  workspace_isolation_and_purge_scoping ........................... ok
  cross_grant_caller_never_receives_warm_privileged_frame ......... ok
  batch_denied_without_viz_cap .................................... ok
  directive_is_source_blind_and_zero_ttl_bypasses ................. ok
  batch_warm_reopen_runs_zero_db_queries .......................... ok
  batch_result_equals_single_query_per_panel ..................... ok
  batch_partial_failure_isolates_the_bad_panel ................... ok
  batch_over_cap_is_bad_input .................................... ok

# regression — no existing suite broke
viz_query_test ............ 17 passed; 0 failed
response_cache_test ....... 8 passed; 0 failed
tools_catalog_test ........ 5 passed; 0 failed
catalog_mcp_test .......... 8 passed; 0 failed
persona_menu_full_catalog_test ... 2 passed; 0 failed

# feature-off (no-op seam) + feature-on both compile; `cargo fmt --all --check` clean.
```

## Follow-ups / notes

- The federation-specific result-cache HIT that slice-1 wires a caller to (the measured 22× on a cold
  node) is exercised by the federation suite + the live rubix-ai Playwright walk; the source-blind
  threading itself is proven here by bypass parity + the gateway cache honouring `ttl_s`.
- Release: cut `node-vX.Y.Z`, then rubix-ai bumps the pin (WORKFLOW-LB §4).
