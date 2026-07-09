# `records()` returned positional arrays on the federation path — the `chart` helpers' one-liner promise was a lie there

- **Area:** rules (cage `grid` + the `chart` verb family)
- **Symptom:** The catalog (`crates/rules/src/catalog.rs:132`) advertises `records(grid) -> Array<Map>`,
  the `chart` helpers (`verbs/chart.rs`) require maps, and the rules skill doc (`docs/skills/rules/SKILL.md`
  §6) documents `category(query(...).records(), "name", "value")` as a complete chart-ready rule's last
  line. On the **federation** source path (`demo-buildings`, every sqlite/postgres source) that one-liner
  **failed** with `category: every row must be a record (#{…})` — because `records()` was returning
  positional **arrays**, not maps. A rule author following the docs against the demo dataset hit a wall.
- **Status:** fixed
- **Date:** 2026-07-09

## What was observed (the load-bearing verification)

Two seams feed the cage `Grid`, with **two different row shapes**:

- **platform** (`store.query` / SurrealDB) — rows are JSON **objects** (`{"col": v, …}`).
- **federation** (DataFusion over sqlite/postgres) — `extensions/federation/src/query.rs::shape`
  (lines 97-134) deliberately re-projects Arrow's JSON objects into column-aligned **arrays**
  (`Value::Array(...)`), so the wire shape is `{columns:[...], rows:[[v, ...], ...]}`.

`grid.rs::records()` was a one-liner that forwarded `grid.rows` through `json_to_dynamic` unchanged.
On the platform path that yielded maps; on the federation path it yielded arrays. The cage's own
unit tests (`chart_test.rs`, `grid_test.rs`) run against `RecordingData::platform(...)` which feeds
**object** fixtures — so the chart helpers passed in unit tests and masked the drift. The host-side
render path (`viz/frame.rs::result_to_rows`) had a separate columnar-zip arm that handled federation's
positional shape correctly, so dashboards rendered fine too. The gap lived **only** at the cage
`records()` boundary — exactly where rule authors reach.

The committed buildings regression test (`rules_buildings_examples_test.rs`) was the smoking gun: it
asserted `rows[0][0]` (positional index) and the three example bodies used `r[0]`/`r[1]` access — both
of which silently relied on the broken contract. The prior session doc
(`sessions/rules/buildings-demo-examples-session.md:54`) had even recorded "`.records()` returns
positional arrays" as a "hard-won fact … re-verified by the green test" — a fact about a bug, not a
contract, codified as truth.

## Root cause

`records()` is the seam boundary where the two wire shapes collapse to the one shape every cage
consumer reads. The catalog promised `Array<Map>`; the implementation forwarded whatever the seam
returned. There was no single point that normalized federation's positional rows into maps, so each
consumer either (a) worked by accident on platform data, (b) had its own ad-hoc handling, or
(c) broke loudly on federation data — and the `chart` family is (c). This is the rules-for-widgets
slice-3 gap: the scope *assumed* "last expression = array of row maps" was already the convention,
but on the federation path it wasn't.

## Fix

Collapse the two shapes to maps **at the seam boundary** — one place, every caller wins.

- `crates/rules/src/grid.rs` — `records()` now calls a new `row_to_map(row, columns)`:
  - JSON **object** (platform) → pass through (unchanged behavior).
  - JSON **array** (federation) → zip with `columns` into a map keyed by SELECT aliases in order;
    a trailing cell past `columns.len()` lands under a synthesized `col_<i>` key (honest, never a crash).
  - a bare scalar → single-cell map under the first column name (or `"value"` if columns is empty).
- The cage `chart` helpers, `emit` data, and a plain `for r in rows { r.col }` now work uniformly on
  every source kind. The documented one-liner `timeseries(query(...).records(), "ts")` /
  `category(query(...).records(), "name", "value")` is **true** on federation too.

Downstream of the contract fix, the things that had silently depended on the *broken* shape were
updated to the contract:

- The three existing buildings examples (`buildings_examples.json`) — `r[0]`/`r[1]` → `r.building` /
  `r.kwh_per_m2`.
- The regression test assertions — `rows[0][0]` / `rows[0][1]` → `rows[0].get("building")` /
  `rows[0].get("kwh_per_m2")`, and the `assert_rows` docstring corrected.
- A new fourth example `buildings-intensity-chart` added, whose last line is
  `category(rows, "building", "kwh_per_m2")` — the chart-ready rule the docs promised, now real on the
  demo dataset.

No other caller depended on positional shape (audited): `query_test`, `rules_test`,
`rules_ai_wiring_test`, `federation_sqlite_test`, `ai_fence_test`, `grid_test` either use object-row
fixtures (unaffected — the "object → pass through" branch) or only call `.records().len()` / pass the
result to `emit` (shape-agnostic). The host-side `viz::frame` render path was already correct and is
untouched (separate concern — slice 1's columnar zip).

## Regression

- `crates/rules/tests/grid_test.rs` — two new tests seeded with the **federation** wire shape
  (`RecordingData::federation`, positional-array rows):
  - `records_returns_named_maps_from_federation_positional_rows` — `rows[0].building` resolves (would
    be `Unknown property` on the old positional shape);
  - `category_runs_on_federation_records` — `category(records(), ...)` returns trimmed maps (the
    slice-3 promise, pinned at the unit layer on the federation shape).
- `crates/rules/tests/support/mod.rs` — new `RecordingData::federation(...)` constructor mirroring
  `platform(...)` but marking the source `Federation` kind (so the test composes ANSI SQL and the
  seeded rows carry federation's real positional wire shape).
- `crates/host/tests/rules_buildings_examples_test.rs` — extended to run the new chart body on the
  REAL spawned federation sidecar + REAL `buildings.db`: 8 rows, each trimmed to exactly 2 fields
  (label + value), Riverside on top at 4.68 kWh/m²; the three existing example assertions updated to
  map-key access (still green). This is the end-to-end proof the one-liner works on real federation data.

All green: `cargo test -p lb-rules` (24), `rules_buildings_examples_test`, `query_test`, `rules_test`,
`rules_ai_wiring_test`, `federation_sqlite_test`.

## Lesson

A catalog signature is a contract, not a description of current behavior. `records(grid) -> Array<Map>`
was written as the intended shape, the implementation forwarded whatever the seam returned, and the
unit tests ran against the platform path that happened to match — so the lie held for as long as nobody
ran the documented `chart` one-liner against a federation source. **Two shapes at a seam boundary need
exactly one normalizer, at the boundary** — pushing the mismatch onto every consumer is how a
documented one-liner becomes a wall. And: a "hard-won fact" recorded against a green test can be a fact
about a bug rather than a contract; when the test asserts the broken shape, the test is the drift's
hiding place, not its proof.
