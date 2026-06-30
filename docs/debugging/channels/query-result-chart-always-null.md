# Every `query_result` came back `chart:null` even for a clean temporal/numeric shape

- **Area:** channels (`rust/crates/host/src/channel/query_worker.rs` ↔ `chart.rs`)
- **Status:** resolved
- **First seen:** 2026-06-29
- **Resolved:** 2026-06-29
- **Session:** ../../sessions/channels/channels-query-charts-session.md
- **Regression test:** `rust/crates/host/src/channel/query_worker.rs` (`keyed_rows_zips_arrays_into_objects_so_the_picker_plots`) + the end-to-end `rust/role/gateway/tests/gateway_query_test.rs` (`posting_a_query_item_round_trips_a_result_with_columns_rows_and_chart`)

## Symptom

The new happy-path gateway test posted a clean temporal/numeric query
(`SELECT day, signups FROM daily ORDER BY day`) and asserted a non-null line chart in the
`query_result` item. The columns and rows were correct, but `chart` was `null`:

```
auto-plotted line: {"kind":"query_result","source":"warehouse","sql":"…","columns":["day","signups"],
                    "rows":[["2024-01-01",3],["2024-01-02",5],["2024-01-03",7],["2024-01-04",4]]}
  left: Null
 right: "line"
```

In production this means a perfectly plottable result *always* renders table-only — the auto-plot,
the headline feature, never fires. The deny-path round-trip test never reached real rows, so the bug
was invisible until a real query executed.

## Reproduce

```rust
let cols = vec!["day".into(), "signups".into()];
let rows = vec![json!(["2024-01-01", 3]), json!(["2024-01-02", 5]), json!(["2024-01-03", 7])];
assert!(pick_chart(&cols, &rows).is_some()); // FAILS — returns None on array rows
```

## Investigation

`federation.query` returns its result with `rows` as **column-aligned arrays**
(`rows: [[c0, c1, …], …]` — the documented wire shape, see `extensions/federation/src/query.rs`).
The chart picker (`chart.rs::pick_chart` → `infer_column_type`) reads each cell **by column name**:
`row.get(col)`. A JSON *array* has no string keys, so `row.get("day")` is always `None`, every column
inferred as "no signal" → categorical, and the picker fell through to `None`. The worker handed the
sidecar's array rows straight to `pick_chart` without reshaping.

## Root cause

A shape mismatch across an internal seam: `federation.query` emits array rows; `pick_chart` consumes
object rows. The worker glued them together without the zip. (The `viz.query` path already zips
columnar results into named row-objects for exactly this reason — that converter's existence was the
clue.)

## Fix

Zip the (already-capped) array rows into objects keyed by column name *just for the picker*, in
`query_worker.rs::keyed_rows`; keep the compact **array** rows in the persisted payload (the UI maps a
chart series' `field` name back to its column index):

```rust
let keyed = keyed_rows(&columns, &rows);   // [[c0,c1],…] -> [{col0:c0, col1:c1},…]
let chart = pick_chart(&columns, &keyed);
let body  = result_body(&source, &sql, columns, rows, chart, truncated); // rows stay arrays
```

## Verification

- `cargo test -p lb-host --lib query_worker` — `keyed_rows_zips_arrays_into_objects_so_the_picker_plots`
  green (asserts raw arrays do NOT plot, keyed rows DO).
- `cargo test -p lb-role-gateway --test gateway_query_test` — the end-to-end round-trip asserts a real
  `chart.type == "line"`, `chart.x == "day"`, `chart.series[0].field == "signups"` over the real
  gateway + real sqlite sidecar.

## Prevention

The unit test pins both halves of the contract (arrays in → no plot; keyed in → plot), and the
gateway happy-path test now exercises real rows end to end so a future change to the federation wire
shape (or the picker's row contract) fails loudly. Class rule: when one verb's output row shape feeds
another's input, assert the *shape*, not just the values — a values-only test passes on the wrong
shape.
