# Session — `records()` returns maps on every path + a chart-ready buildings example

The user's ask: take the existing buildings energy-intensity example rule and make a version that
renders in a chart, against the seeded `demo-buildings` data (`docker/postgres/seed-demo-sqlite.sh`).
Pointed at `docs/scope/frontend/dashboard/rules-for-widgets-scope.md` (the chart-infrastructure plan)
as "past work, BEEN done."

## What I found before writing anything

The chart infrastructure **is** shipped — `verbs/chart.rs` (`timeseries`/`wide`/`category`) exists and
the skill doc documents the one-liner pattern. But on the user's exact data path it would have **failed**:

- `federation.query` returns rows as column-aligned **arrays** (`extensions/federation/src/query.rs::shape`
  re-projects Arrow objects to `Value::Array`); the platform (Surreal) path returns objects.
- `grid.rs::records()` forwarded whatever shape the seam returned — maps on platform, **positional
  arrays on federation**. The catalog promised `Array<Map>`; reality on federation was `Array<Array>`.
- The `chart` helpers require maps (`chart.rs::as_map` errors on a non-record row), so
  `category(query("demo-buildings", ...).records(), ...)` would have errored
  `every row must be a record`. The three existing examples sidestepped this by using `r[0]`/`r[1]`.

The chart-for-widgets scope (slice 3) *assumed* "last expression = array of row maps" was already the
convention — it wasn't, on the federation path. Full diagnosis + fix in
[`../../debugging/rules/records-returns-positionals-on-federation.md`](../../debugging/rules/records-returns-positionals-on-federation.md).

The user chose the principled fix (close the drift at the seam boundary + a chart rule that uses the
documented one-liner) over an in-body workaround.

## What shipped

1. **`records()` honors its `Array<Map>` contract on every source kind** —
   `rust/crates/rules/src/grid.rs` gained `row_to_map(row, columns)` (object → pass through; array →
   zip with `columns` into a map; scalar → single-cell map). `records()` routes every row through it.
   One normalizer at the seam boundary; every cage consumer (the `chart` family, `emit` data, plain
   `r.col` access) wins uniformly.

2. **A new chart-ready example rule** — `buildings-intensity-chart` in
   `rust/crates/host/src/rules/buildings_examples.json`. Same proven intensity query, last line is
   `category(rows, "building", "kwh_per_m2")` — the bar/pie shape a panel draws. Bind a panel to
   `{tool:"rules.run", args:{rule_id:"buildings-intensity-chart"}}` and it renders.

3. **The three existing examples updated to named map access** — `r[0]`/`r[1]` → `r.building` /
   `r.kwh_per_m2` (they had silently depended on the broken positional shape).

4. **`RecordingData::federation(...)` test helper** — `rust/crates/rules/tests/support/mod.rs` mirrors
   `platform(...)` but marks the source `Federation` kind and seeds rows in federation's real positional
   wire shape. So a unit test can now prove the `records()` contract on the federation shape directly,
   without the real sidecar.

## Proof (all green)

- `cargo test -p lb-rules` — 24 passed. Two new tests pin the contract on the federation shape:
  `records_returns_named_maps_from_federation_positional_rows` (`rows[0].building` resolves — would be
  `Unknown property` on the old shape) + `category_runs_on_federation_records` (the chart one-liner on
  federation data, returns trimmed maps).
- `cargo test -p lb-host --test rules_buildings_examples_test` — 1 passed (121s; real spawned
  federation sidecar + real `buildings.db`). The new chart body runs e2e: 8 rows, each trimmed to
  exactly 2 fields (label + value), Riverside Data Center on top at 4.68 kWh/m², 0 findings. The three
  existing example assertions updated to map-key access are still green.
- No blast-radius breakage: `query_test`, `rules_test`, `rules_ai_wiring_test`, `federation_sqlite_test`
  all green (audited every `records()` caller; only the buildings examples + their test depended on
  positional shape, both updated).

## What this does NOT do

- **The host-side render path (`viz::frame::result_to_rows`) is untouched.** It was already correct —
  its columnar-zip arm handles federation's positional `{columns, rows}` shape. This session is
  slice 3 (the cage `records()` boundary), not slice 1 (the viz render path). The
  rules-for-widgets scope's slice 1/2 work (recursive dispatch, `route:false`, the panel wizard E2E)
  remains as the scope describes it.
- **No `insight.*` in the new example.** The user's pasted reference rule used `insight.raise` with
  dedup/severity/tags; the existing `buildings-intensity-alert` example uses `alert(...)`. The new chart
  example is pure shape-trim — chart rows are a rule's `output`, not the insights plane's food (a
  non-goal per the scope).
- **The beginner lesson** the prior handover/session describes is still not written end-to-end; this
  adds one trustworthy-and-tested chart example to step 2 (rules).

## Supersedes a prior session's "fact"

`sessions/rules/buildings-demo-examples-session.md:54` recorded "`.records()` returns positional
arrays … re-verified by the green test" as a hard-won fact. That was a fact about a bug, codified —
the test asserted the broken shape, so it was the drift's hiding place. This session corrects the
contract; the prior session's narrative is unchanged (it accurately describes what was true when
written) but the "fact" is now stale.
