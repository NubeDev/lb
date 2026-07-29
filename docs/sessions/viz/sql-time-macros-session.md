# Session — ONE Grafana SQL macro layer, query-time and engine-aware (viz sql-time-macros scope)

Date: 2026-07-29. Scope: [`../../scope/viz/sql-time-macros-scope.md`](../../scope/viz/sql-time-macros-scope.md)
(all decisions were final in the scope). Downstream consumer: rubix-ai Quick Chart builder
(`rubix-ai docs/scope/frontend/dashboard/quick-chart-builder-scope.md`), built in the same working
session against a local `[patch]`.

## What shipped

The Grafana **function** macros — `$__timeFilter(col)`, `$__timeGroup(col,'<dur>')`,
`$__timeGroupAlias`, `$__time(col)`, `$__timeFrom()`, `$__timeTo()` — now expand **at query time,
per source `kind`, in the federation child**. The import-time translator is **deleted**; lb has
exactly one macro layer again.

- **New `federation/src/sql_macros.rs`** (pure, unit-tested in place): the `$__` byte scanner
  (balanced parens + quote-aware comma split — the discipline ported verbatim from the retired
  translator), the per-engine expansion table keyed on the child's own `kind` vocabulary
  (`postgres` → `date_bin`, `timescale` → `time_bucket`, `sqlite` → epoch-ms integer floor,
  `mysql` → `FROM_UNIXTIME(FLOOR(UNIX_TIMESTAMP(..)))` — table entry ready, the kind itself still
  has no connector), and the interval grammar (`'5m'`, `"30s"`, `1500ms`, bare int = ms,
  `'$__interval'` resolved from the attached window). Errors are NAMED, never silent SQL breakage:
  `unsupported macro $__foo`, `time macro $__x needs the render window … pass "resolution"`,
  `unexpanded value macro $__timeFrom …`, `bad interval …`, the fill-arg refusal, and
  `no SQL time-macro expansion for source kind "x"`. Un-macro'd SQL returns **byte-identical**.
- **Hook in `federation/src/main.rs::federation_query`**, BEFORE `run_query_cached`: the result-cache
  key hashes the RAW input (raw sql + `resolution` both participate) and expansion is a pure function
  of exactly those, so the key stays honest and two widths can never collide on one entry.
- **Host attach (`resolution`)**: `viz.query`'s `federation.query` arm now calls the new
  `attach_resolution` (`host/src/viz/resolution.rs`) after the value pass — attaching
  `resolution: {from_ms, to_ms, width_ms}` **only when the sql still carries a `$__` token**, so a
  macro-free target's child args (and its result-cache key) stay byte-for-byte. Threaded through the
  enumeration seam: `host/src/federation/tool.rs` (explicit parse) →
  `host/src/federation/query.rs` (signature + the `json!`); the other three `federation_query`
  callers (query.run, mirror, channel query worker) pass `None`.
- **Deleted `host/src/dashboard/grafana/macros.rs`** (295 lines) + its `mod` + the `bind.rs` call:
  `dashboard.import` now stores target SQL **verbatim, macros included** — a migrated Grafana SQL
  panel is live and zoom-coarsening, not a frozen Postgres translation. The `macro` DegradedItem
  kind is retired (documented in `grafana/mod.rs`); an unsupported macro surfaces as a named error
  at first render instead of an import notice. Translator test cases re-homed as expansion-table
  cases in `sql_macros.rs`.

### One genuine issue the scope did not anticipate

The shipped value pass (`host/src/viz/macros.rs`) replaced bare `$__timeFrom`/`$__timeTo` with a
plain `String::replace`, which would corrupt the Grafana **function forms** `$__timeFrom()` /
`$__timeTo()` into `1699…()` before they ever reached the child. The two passes could not compose
without a fix, and the scope says the value pass is "unchanged". Decision (best long-term, recorded
here): the value pass gains one private helper `replace_bare` that skips a token followed by `(` —
the bare token stays the host's value macro, the call form is the child's function macro. Behavior
for every previously-valid input is identical (new unit test
`function_forms_pass_through_bare_tokens_substitute` pins the composition).

Also noted while implementing: the child `kind` vocabulary is `sqlite | postgres | timescale`
(`source/mod.rs::connect`) — there is **no `datafusion` kind** (DataFusion is an execution
*strategy*, only taken for `information_schema` queries, which a macro'd chart query never is), so
expansion-by-kind IS the executing dialect for every reachable macro'd query. The v1
timestamp-column assumption per engine is documented in the module header; for sqlite it is
**epoch-ms INTEGER** (the house convention — series export, the demo fixtures).

## Tests (all green, real infra, no mocks)

- `cargo test -p federation --bin federation sql_macros` — 7 unit tests: the engine × macro
  expansion table, interval forms, nested-paren columns, missing-resolution named error,
  unsupported/unexpanded named errors, byte-identity, unknown-kind error.
- `cargo test -p lb-host --lib` — 368 passed (includes the new value-pass composition test, the
  `attach_resolution` behavior, and the rewritten bind pass-through test
  `grafana_macros_pass_through_verbatim_at_bind`).
- **New `host/tests/viz_sql_time_macros_test.rs`** (real `Node::boot()` + real federation sidecar +
  real on-disk SQLite): 3 tests —
  `function_macros_expand_per_engine_and_recoarsen_with_the_budget` (the Quick Chart emission
  bucketed at 1s → 100 rows, re-coarsened to ≤10 by `maxDataPoints:10`, the seeded spike surviving
  in `max`, the half-open `$__timeFilter` window), `named_errors_missing_resolution_and_
  unsupported_macro` (direct call without `resolution` names the field + fix; WITH an explicit
  `resolution` it executes; `$__unixEpochFilter` names the token), and
  `macro_target_deny_and_workspace_isolation_hold` (the two MANDATORY cases through macro'd
  targets). `test result: ok. 3 passed` (11.3s).
- Neighbors re-run green: `viz_resolution_macros_test` (2), `viz_query_test` (17),
  `dashboard_test` (3), `dashboard_query_options_test` (12), `federation_sqlite_test` (2),
  `cargo test -p federation` (44 + integration bins). `cargo fmt --all --check` clean.

## Release note

Per the scope's release discipline: the translator deletion and the query-time expansion are ONE
slice — tag the next `node-v*` from this state so no build exists with zero macro handling;
rubix-ai then bumps its pin once (its Quick Chart datasource-track time bucketing is gated on it).
Until then the rubix-ai checkout runs against this tree via its local `[patch]`.
