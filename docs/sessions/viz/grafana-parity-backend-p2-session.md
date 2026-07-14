# Viz Grafana-parity backend — P2 (lb-viz tranche 2a + reduce calcs) — session

- Date: 2026-07-14
- Scope: `docs/scope/viz/grafana-parity-backend-scope.md`, Phase **P2**
- Area: `rust/crates/viz/src/transforms/` (6 new transforms), `rust/crates/viz/src/reducer.rs`
  (tranche-2 calcs + the general `pNN`), `rust/crates/viz/Cargo.toml` + workspace deps,
  `rust/crates/host/tests/viz_query_test.rs` (one end-to-end pin)
- Status: **done** — P2 shipped green. P3 (the import pin crate) is next, not started.

## What shipped

Six new transforms, **one responsibility per file** under `crates/viz/src/transforms/`, each pure,
Grafana-verbatim ids/options, each with its own unit tests, all wired through `transforms/mod.rs`
+ the `transform.rs` dispatch:

- `rename_by_regex.rs` — `renameByRegex {regex, renamePattern}`. Anchors the user pattern like
  Grafana's `getRegex` (`^(?:…)$`), `$n` capture substitution, a non-matching name / bad regex /
  empty options degrade to passthrough (never an error).
- `filter_by_ref_id.rs` — `filterByRefId {include}`. Keeps frames whose `ref_id` is in the
  include set; empty/absent include → all frames pass (Grafana's no-op default).
- `convert_field_type.rs` — `convertFieldType {conversions:[{targetField, destinationType, …}]}`.
  Ports number/string/boolean/time coercion. **Time parsing is deliberately narrow** — see below.
- `extract_fields.rs` — `extractFields {source, …}`. Explodes a JSON-object cell into sibling
  fields. **Column order is alphabetical per cell** — see below.
- `labels_to_fields.rs` — `labelsToFields`. Promotes a frame's label map into value fields
  (Grafana's default value-mode).
- `concatenate.rs` — `concatenate`. Merges every input frame's fields into one wide frame,
  padding short columns with `Null` (ragged is honest, never dropped).

**Reducer (`reducer.rs`) tranche-2 calcs**, same pure-`Option` contract (non-numeric/null cells
skipped, empty column → honest `Null`, never a fabricated 0):
`diff`, `diffperc`, `delta`, `step`, `median`, `variance`, `stdDev`, `distinctCount`,
`changeCount`, `allIsZero`, `allIsNull`, plus **the general `pNN` pattern (1–99)** so any imported
percentile computes rather than degrades.

**Deps:** `regex` added to `[workspace.dependencies]` and `regex` + `chrono` to
`crates/viz/Cargo.toml`. The crate stays **pure** — `chrono` is used only for parsing timestamps
in `convertFieldType`, no clock/`now()` reach (the frame-in/frame-out contract is unchanged).

## The honest semantic pins (read these before trusting a fixture)

These are the places where "Grafana parity" is bounded — each is a deliberate, documented cut, not
an oversight:

- **`extractFields` column order is alphabetical per cell.** The extracted sibling fields come from
  a `serde_json::Map`, which sorts its keys — so a `{"b":…, "a":…}` cell yields fields in `a, b`
  order regardless of source order. Grafana preserves insertion order (JS object key order). If a
  fixture needs source-order columns, that's a named follow-up (would require an insertion-ordered
  map); alphabetical is stable and honest for now.
- **`convertFieldType` time parsing is narrow.** It ports **RFC3339** plus **two bare UTC shapes**
  (`%Y-%m-%d %H:%M:%S` and `%Y-%m-%d`, read as UTC) → epoch-ms — **NOT** Grafana's arbitrary
  `dateFormat` (dayjs) grammar. A cell matching none of those converts to `Null` (honest no-value),
  never a guessed epoch. The dayjs format-string grammar is a fixture-demanded follow-up, not
  silently stretched here.
- **`diffperc` is a RATIO, not ×100.** `(last − first) / first`. The client's percent *unit* does
  the ×100 for display — the backend never pre-scales. `first == 0` → honest `Null` (not `inf`),
  matching Grafana leaving `diffperc` unset.
- **`variance`/`stdDev` are POPULATION (÷ n), not sample (÷ n−1)** — Grafana's `calculateStdDev`
  divides by `n`. Pinned to match, not the textbook sample formula.
- **`pNN` is nearest-rank, floor.** `sorted[floor(p/100 · (n−1))]` — no interpolation (Grafana's
  `calculatePercentile`). `p50` over `1..=10` is `sorted[4] = 5`; `p90` is `sorted[8] = 9`. Only
  `p1`…`p99` compute; `p0`/`p100`/`pxx` stay unknown-calc → `Null`.

## Tranche 2b — not built (correctly)

Per the scope, tranche 2b (`groupingToMatrix`, `rowsToFields`, `prepareTimeSeries`, `formatTime`,
`histogram`, …) is added **only as a fixture demands**. No P2 fixture demanded any, so none were
added — an unknown transform id is carried opaque (`viz.query` skip-with-notice), never dropped or
errored. The end-to-end pin proves exactly this bound (`groupingToMatrix` in-pipeline → rows
intact).

## Tests (all green, real store / real node, no mocks)

- `cargo test -p lb-viz`: **77/77 green** — every new transform's unit tests + the tranche-2 calc
  table (`tranche_2_numeric_calcs`, `pnn_percentiles_compute_for_any_1_to_99`, `tranche_2_edges`)
  incl. the null/non-numeric skip discipline.
- `crates/host/tests/viz_query_test.rs::tranche_2a_pipeline_runs_end_to_end` (**the P2 e2e pin**) —
  a `renameByRegex` + `p90` `reduceFields` pipeline runs through the **real** `viz.query` over a
  **real** `store.query` target on really-seeded rows (`1..=10`): the field is regex-renamed
  `payload`→`cpu_load`, then `p90` collapses to one row = `9.0` (the `pNN` pin, over the renamed
  name). Second half proves the tranche bound: an unknown `groupingToMatrix` in the pipeline leaves
  all 10 rows intact (skip-with-notice).
- `cargo test -p lb-host --test viz_query_test`: **13/13 green** (the whole viz.query suite,
  including the P1 time-override e2e — nothing regressed).

### The one real fix this session (the unverified test was red)

The e2e pin arrived from the prior session **unrun** and it was red — but the bug was in the
**test's SQL, not the transforms**. It used `SELECT payload FROM series ORDER BY seq`; SurrealDB
rejects an `ORDER BY` idiom that isn't in the projection (`Missing order idiom 'seq' in statement
selection`). `viz.query` correctly swallows a failed target to an **empty frame** (honest degrade),
so the reduce ran over 0 fields → 0 rows → the assertion failed. Fixed by ordering on the selected
column: `ORDER BY payload` (ordering is irrelevant to `p90` — the reducer sorts internally). No
transform or reducer code changed. This is a nice incidental confirmation that a broken target
degrades cleanly rather than erroring the panel.

## Suite status (honest)

Every suite this diff touches is green (`lb-viz` 77, `viz_query_test` 13). The wider
`cargo test -p lb-host --no-fail-fast` run has **six failing binaries, all pre-existing /
environmental and unrelated to this diff** — none touch viz or dashboard:

- `agent_persona_catalog_test` (6) + `agent_persona_coding_test` (2) — the persona→skill catalog
  gap (`builtin.data-analyst` → missing `core.datasources`, `builtin.workspace-admin` →
  `core.nav`); fails identically at pre-session `9a4b7041` (the P1 session verified the same).
- `cross_node_routing_test` (1) — the known flaky bus-timing test.
- `devkit_e2e_test` (1) — needs a prebuilt `hello_v2` wasm artifact (not built in this checkout).
- `proof_panel_test` (17) — on the known clean-master pre-existing list.
- `store_query_test::schema_reports_tables_and_denies_and_isolates` (1) — a schema-introspection
  expectation (`seq` column). **Verified: fails identically at pre-session `9a4b7041`** (ran it in
  a temp worktree), so pre-existing, not a P2 regression.

## Files (this session)

- Committed already (parallel snapshot `bed36d6` "more updates to viz" / `3ce166e7`): the six
  transform files, `reducer.rs` tranche-2 calcs, `transforms/mod.rs` + `transform.rs` wiring,
  `Cargo.toml` deps. Left as-is (git untouched per session instruction).
- **Uncommitted, this session:** `crates/host/tests/viz_query_test.rs` — the one SQL fix in the
  tranche-2a e2e pin (`ORDER BY seq` → `ORDER BY payload`, both panels).
- Docs: this file + `docs/STATUS.md` (P2 row).

## Next: P3

The import-pin crate (`__inputs` resolver + v1/v2 detector + the ported v33 migration subset) as a
**small dep-light crate beside lb-viz** with **no host dependency** (scope §Phasing 3). Not started.
