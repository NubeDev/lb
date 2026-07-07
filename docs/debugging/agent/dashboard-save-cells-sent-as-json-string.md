# `dashboard.save` failed five turns straight — the model sent `cells` as a JSON-encoded string

**Area:** agent tool ergonomics (dashboard MCP bridge)
**Date:** 2026-07-05
**Symptom:** Live, the widget-builder run burned five consecutive turns on
`dashboard.save … bad input: cells: invalid type: string "[{\"i\": …}]"` — the model kept passing
the `cells` array as a JSON-*encoded string* and the raw serde error never taught it otherwise. A
later run failed once more on `cells: missing field widget_type`, and `dashboard.share` failed on
`now` arriving as a numeric string.

## Root cause

Three ergonomic gaps at the same seam:

1. `dashboard.save` (and `.share`) were advertised **name-only** — no arg schema — so the model
   guessed encodings (the same failure class as the earlier `federation.schema` gap).
2. The bridge decoded structured args strictly: the very common model shape "stringified JSON" was
   a hard type error with no steering.
3. `Cell.widget_type` was the ONLY v-specific field without `#[serde(default)]` — a v3
   `view`-addressed cell (exactly what `dashboard.catalog` teaches) failed to deserialize.

## Fix

- Real descriptors: `dashboard.save` (`{id, title, cells: array, variables?, now}`,
  `dashboard/save.rs`) and `dashboard.share` (`{id, visibility enum, team?, now}`,
  `dashboard/share.rs`), registered in `tools/descriptor.rs` — the schema validator now also gives
  a clean ``arg `cells` must be array`` steer pre-handler.
- Bridge leniency with authority intact (`dashboard/tool.rs`): `typed_arg` decodes a JSON-encoded
  string form (all of save's validators still run on the decoded value); `u64_arg` accepts a
  numeric string; both steer explicitly when they can't.
- `Cell.widget_type` is `#[serde(default)]` like every other version-specific field.

## Regression tests

`dashboard/tool.rs` unit tests (`typed_arg_*`, 3) + the dashboard suites green.

**Verified live:** the retest run's `dashboard.save` succeeded (one attempt after one
`missing field widget_type` steer pre-fix; post-fix a v3 cell with no `widget_type` saves clean).
