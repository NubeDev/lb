# dashboard — imported Grafana panels bind, save "mapped", and render empty

- Date: 2026-07-16
- Area: `rust/crates/host/src/dashboard/grafana/` (`to_cell.rs`, `import.rs`, the new `bind.rs`)
- Status: **fixed** (viz import-export scope, Phase 4 — the commit path's missing bind stage)

## Symptom

`dashboard.import` reported a clean, successful import — `mappedPanels: 2`, the caller's datasource
binding echoed back in `report.datasources[].mappedTo`, no degraded entry for the mapped panels —
and the record saved and read back intact. Every imported panel then rendered **blank**, forever.
Nothing in the report, the record, or the logs said anything was wrong.

## Verification (before fixing — on the REAL path, no mocks)

Reproduced against a RUNNING node (rubix-ai's embedded lb, gateway `127.0.0.1:8099`) over the real
gateway, importing a real Grafana `schemaVersion: 39` export and binding it to a real seeded sqlite
datasource (`demo-buildings`: 8 sites / 956,160 rows, registered through `datasource.add`).

Feeding the **stored** cell's `sources[]` straight back to `viz.query`:

| target shape | `tool` | args | `viz.query` |
|---|---|---|---|
| what the mapper wrote | `""` | `{rawSql, datasource:{uid}}` | **0 rows** |
| the native shape | `federation.query` | `{source, sql}` | **956160** |

Same SQL, same datasource, same caller. Confirmed: **no imported dashboard has ever rendered data.**
The records are repairable — the mapping is recoverable from the preserved Grafana target — but every
dashboard imported before this fix needs a re-import to become executable.

## Root cause

Two independent gaps, both on the commit path, each sufficient to produce an empty panel:

1. **`tool` was never filled.** `to_cell::targets_to_sources` sets `tool: String::new()`, and its doc
   comment deferred the job: *"the datasource-binding step (post-remap, UI/verb) resolves the concrete
   MCP tool from the bound datasource kind."* **That step did not exist.** `datasources::apply` — the
   only post-remap stage — rewrites the datasource `{uid}` and nothing else. So `tool` stayed `""`
   through save.
2. **The arg names were Grafana's, not ours.** A target's `args` is the original Grafana object
   (`rawSql`, `format`, …). `federation.query` reads `{source, sql}` (`federation/tool.rs`), so even
   with `tool` set, the args carried nothing it could read.

Why it was silent — the failure mode is the real lesson. Both gaps land on deliberate honest-empty
paths that are correct individually and combine into an invisible hole:

- `viz/query.rs::targets()` **skips** a target whose `tool` is empty (an unbound target isn't an
  error — it's just not a query yet).
- `viz/query.rs` maps a dispatch failure (deny / bad input / any tool error) to an **empty frame** —
  deliberately, so a denial is opaque and never a fabricated row.

So an unexecutable target is indistinguishable from a source that legitimately returned no rows. The
import's own report is computed from the panel *mapping* (`view != "json"`), never from whether the
target can actually execute — so it cheerfully said "2 panels mapped."

## Fix

A new `grafana/bind.rs` — the stage `to_cell.rs` promised — run by `import.rs` on **commit only**,
immediately after `verify_mappings` + `datasources::apply`:

- fills `Target.tool` from the bound ref, and
- **adds** `{source, sql}` alongside Grafana's keys (reading `rawSql`/`rawQuery`/`query`/`expr` in
  Grafana's own precedence), never removing them.

Design constraints that shaped it:

- **Rule 10 — no datasource is special-cased.** A bound ref names a datasource *record*;
  `federation.query` resolves that record's `kind`/`dsn` itself (`federation/schema.rs`), so one
  generic mapping covers sqlite/postgres/every future kind with no per-kind branch. Only the two
  reserved non-federation targets (`native` → `store.query`, `series` → `series.read`) are named, by
  the same table that `import::is_reserved_target` already uses.
- **Tenancy cannot widen.** `bind_cells` binds only names the verb already VERIFIED against the
  caller's `datasource.list`. An unmapped ref keeps its Grafana uid, stays unexecutable, and keeps its
  existing `datasources::apply` degraded notice — we never invent a binding the caller didn't choose.
- **Round-trip stays lossless.** `to_grafana` re-emits `args` verbatim as the Grafana target, so the
  original keys had to survive; we only add. Verified: export still emits `rawSql` and the unsupported
  `geomap` still exports as `type: "geomap"`.
- **A bound target with no query degrades honestly** (`kind: "target"`) rather than saving a panel
  that will silently render empty — closing the report's blind spot for that case.

## Verified after the fix (same real path)

- stat panel, from the stored record → `[{"n": 956160}]`.
- timeseries panel → 1 frame, fields `[site, hour, avg_energy]`, **5,768 rows** of real data.
- Export round-trip unchanged (`rawSql` intact, geomap preserved).
- Workspace isolation unchanged: a ws-B import binding a ws-A source → opaque `403 denied`.
- Unmapped datasource still degrades with `tool: ""` and no invented binding.
- `cargo test -p lb-host --lib` → 258 passed; `cargo fmt --all --check` clean.

## Note for the UI half (rubix-ai issue #5)

The report's `mappedPanels` counts panels whose *type* mapped — it is **not** a promise that a panel
will render data. The honest signals for the import UI are `report.degraded[]` (now including
`kind: "target"`) and, for a panel bound to nothing, `sources[].tool == ""`.
