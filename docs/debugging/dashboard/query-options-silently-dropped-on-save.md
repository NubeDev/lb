# dashboard — `queryOptions` silently dropped on `dashboard.save`

- Date: 2026-07-14
- Area: `rust/crates/host/src/dashboard/` (`model.rs`, the `dashboard.save` tool boundary)
- Status: **fixed** (grafana-parity-backend P1 session)

## Symptom

The editor-parity UI ships a top-level cell field `queryOptions {maxDataPoints, minInterval,
relativeTime}`. A dashboard saved with it read back **without** it — the whole object gone, no
error, no report line. Every save since the editor-parity UI shipped has been losing this user
data.

## Verification (before fixing — the scope's risk note)

Reproduced on the REAL path (no mocks): `call_dashboard_tool("dashboard.save", …)` over a mem://
store with a UI-shaped v3 cell carrying `queryOptions`, then `dashboard.get`. The saved return and
the read-back both came back `Null` for `queryOptions`
(`crates/host/tests/dashboard_query_options_test.rs::ui_shaped_query_options_survive_save_get` —
run against pre-fix code, it fails exactly there).

**Confirmed: shipped user data has been silently lost on every save carrying the field.** The
record cannot be repaired — the data never reached the store.

## Root cause

`Cell` is a **closed** serde struct with no catch-all (`#[serde(flatten)]` map or `unknown_fields`
carrier). The MCP boundary (`dashboard/tool.rs::typed_arg::<Vec<Cell>>`) deserializes the caller's
cells into that struct, so any unknown top-level cell key is dropped by serde **before**
validation, bounds, or the store see it. Unlike `options`/`fieldConfig` (opaque `Value` fields —
carried verbatim), a top-level unknown is simply not part of the struct.

This also bounds the UI's carry-don't-strip guarantee: it holds **inside**
`options`/`fieldConfig`/`custom`, NOT for unknown top-level cell fields — an addition on the UI
side must land a matching model field here first.

## Fix

Typed `QueryOptions` on `Cell` (`queryOptions`, camelCase like `fieldConfig`): the shipped UI trio
plus Grafana's `timeFrom`/`timeShift`/`hideTimeOverride`. Serde-defaulted + null-tolerant +
skip-if-empty (a pre-P1 cell round-trips byte-stable). `viz.query` now applies the time override
when dispatching targets (`viz/time_override.rs`).

## Regression tests

- `dashboard_query_options_test.rs::ui_shaped_query_options_survive_save_get` — the headline pin:
  a UI-shaped cell carrying `queryOptions` survives the real save → get path.
- `model.rs::p1_fields_round_trip` / `p1_fields_default_on_pre_p1_shapes` /
  `query_options_tolerates_partial_shape` — serde round-trip + the additive guard.

## Lesson

A closed serde struct at a tool boundary silently eats every unknown field — when the UI grows a
top-level field, the model must grow it in the same release, and the round-trip test (save → get →
assert the field) is the only thing that catches the gap. "Opaque `Value` fields carry anything"
does not extend to *siblings* of those fields.
