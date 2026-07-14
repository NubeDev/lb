# viz Grafana-parity backend — P3: the import pin as code

**Scope:** `docs/scope/viz/grafana-parity-backend-scope.md` §Phasing P3.
**Date:** 2026-07-14. **Status:** SHIPPED (uncommitted working tree). 29/29 green.

## What P3 asked for

> The `__inputs` resolver + v1/v2 detector + the ported migration subset as a **small dep-light
> crate beside lb-viz** (e.g. `crates/grafana-map`, no host-crate dependency), so the standalone
> converter workspace consumes it as a plain git dep instead of vendoring. The `dashboard.import`
> verb calls the same crate.

Both open questions in the scope are now decided:
- **Crate name/placement:** `rust/crates/grafana-map` — a new workspace member. Constraint met:
  **no host dependency**. Deps are `serde_json` + `thiserror` only (dep-light by design, mirroring
  lb-viz's "no I/O, no store, no bus" posture).
- (The `timezone` on-record question was P1's; unchanged here.)

## What shipped

New crate `grafana-map`, one responsibility per file (FILE-LAYOUT):

| file | responsibility |
|---|---|
| `src/lib.rs` | public `pin()` entry + `PinError`/`PinReport`; orders detect → migrate → resolve |
| `src/detect.rs` | v1/v2/snapshot discriminator (`Shape`) |
| `src/inputs.rs` | `__inputs` resolver — Grafana's `dash_template_evaluator.go` ported |
| `src/migrate/mod.rs` | ordered v33 migration subset + `MigrateReport` |
| `src/migrate/datasource_ref.rs` | v33 datasource-string → `{uid}` ref |
| `src/migrate/panel_type.rs` | `graph`→`timeseries`, `singlestat`→`stat`/`gauge` |
| `tests/import_pin_test.rs` | integration over real export fixtures |
| `tests/fixtures/prom_v30_export.json` | real pre-v33 Prometheus export (`__inputs`, string ds, graph+singlestat, row, template var, `__requires`) |
| `tests/fixtures/v2beta1_export.json` | v2 app-platform export (rejected) |

`pin(root, input_values)` mutates the export in place and returns a `PinReport` (migration steps
applied + degradation notice, `__inputs` resolution: unresolved / auto-filled / `__requires`).

## The pipeline order (and why)

`detect → migrate_v1 → resolve_inputs`. Detect first so v2/snapshot are rejected before we touch
anything. **Migrate before resolve** so a `${DS_*}` token first gets wrapped by the datasource-ref
step into `{"uid": "${DS_*}"}`, then the `__inputs` resolver substitutes the token *inside* the
fresh ref → `{"uid": "fed-prom-uid"}`. Proven by `full_pin_migrates_then_resolves_inputs` and the
fixture test.

## Honest pins (what is ported vs. bounded)

- **`__inputs` resolution is name-keyed, no prefix magic** — exactly Grafana's evaluator. The token
  is the input's `name` verbatim; there is no `DS_`/`VAR_` special-casing. `pluginId == "__expr__"`
  inputs auto-fill to `"__expr__"` without a caller value. An unresolved input is **reported and the
  `${...}` left verbatim** — never blanked (a later template var might own it). `pin` still
  `Ok`-returns with an unresolved input; the caller checks `report.is_clean()` / `inputs.unresolved`.
- **All three envelopes stripped** (`__inputs`, `__requires`, `__elements`) — our deliberate delta
  from Grafana's backend (which strips only `__inputs`). `__requires` is captured into the report
  first (informational); `__elements` library panels are the mapper's job upstream — this crate only
  strips the envelope.
- **Migration is a floor, not the full chain.** Only the two interchange-critical steps are ported:
  datasource-string→ref (structural half only — see below) and the three panel-type renames.
  `MigrateReport.degraded` fires for `schemaVersion` < 21 (predates the ported floor) or missing
  (applied blind). Everything else Grafana's `DashboardMigrator` does is intentionally NOT run —
  running the un-ported chain silently would be the dishonest move.
- **datasource-ref: structural half only.** A string datasource becomes `{"uid": <string>}`. We do
  **not** fill `type`, because the name→type map is Grafana's live datasource list, which this
  dep-light crate has no access to. The `dashboard.import` verb fills `type` when it resolves the uid
  against the caller's federation datasource. Special uids (`-- Mixed --`, `__expr__`) wrap the same
  way and the mapper degrades them per-target.
- **panel-type renames** are `type`-only. `graph`→`timeseries`; `singlestat`/
  `grafana-singlestat-panel`→`stat`, or `gauge` when `gauge.show == true`. Options/fieldConfig
  rewriting is the lb-viz/mapper side's job. Unknown/newer types left verbatim (carried-opaque).
- **v2 rejection** triggers on `apiVersion: dashboard.grafana.app/*` OR top-level `elements`+`layout`.
  A real v2beta1 export nests `elements`/`layout` under `spec`, so the `apiVersion` is the actual
  discriminator on a genuine export — the shape check is the belt-and-suspenders for a spec-unwrapped
  blob. Snapshots rejected via top-level `snapshot`.

## Rule-10 / core-purity check

The crate branches on **panel types and datasource shapes** (Grafana's own vocabulary), never on an
extension id, datasource name, or role. No host/store/bus dep, no `if cloud`. Grafana JSON is
interchange throughout (serde_json Values) — never stored. Consumed by the standalone converter (git
dep) and `dashboard.import` (import-export-scope) identically.

## Tests (29/29)

- `cargo test -p grafana-map`: 27 unit (detect 5, inputs 5, datasource_ref 4, panel_type 5,
  migrate/mod 3, lib 5) + 2 integration over real fixtures. `cargo clippy -p grafana-map` clean.
- Fixture coverage matches the scope's testing plan: a real 13.x-shaped export with `__inputs`
  (name-keyed resolution + all three envelopes stripped), a pre-v33 export with string datasource
  AND `graph`/`singlestat` panels (→ ref + `timeseries`/`stat`/`gauge`), and a v2beta1 export
  (rejected with a `classic`-pointing message).

## Not done / follow-ups

- **`type` fill on datasource refs** is deferred to `dashboard.import` (it owns the federation
  datasource list). This crate cannot and does not guess it.
- **The `dashboard.import`/`export` verbs** (import-export-scope) are the real consumer and are not
  built here — P3 delivers the pin they call. "Lands with whichever consumer builds first" per scope;
  the crate is ready.
- **`__elements` library-panel mapping** to `panel:{id}` + `panelRef` cells is the mapper's job — the
  pin strips the envelope; it does not synthesize the panel cells.
- Grafana reference clone (`~/code/go/grafana`) is **not present** in this environment; the evaluator
  and migrator behavior were ported from the scope doc's pinned descriptions + prior P1/P2 knowledge,
  not re-read from source this session. Worth a re-verify pass against the real
  `dash_template_evaluator.go` + `DashboardMigrator.ts` if the clone returns.

## Git

Left untouched per standing instruction (user commits). Uncommitted new files: the `grafana-map`
crate + its `Cargo.toml` workspace-member line. Nothing else in the tree touched by this session.
