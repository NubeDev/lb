# Session — `Action.argsTemplate` wire rename (flow-bound dashboard controls)

**Date:** 2026-07-22
**Trigger:** NubeIO/rubix-ai#25 — a downstream consumer diagnosed a flow-bound switch/slider dead
end-to-end and traced it to an lb host wire-shape bug. Owning repo is lb (WORKFLOW-LB §2: a host
wire-shape bug is an lb bug).

## The defect

`crates/host/src/dashboard/model.rs` — the dashboard `Action` struct stored its template field as
snake `args_template` with **no `#[serde(rename)]`**, while the entire platform speaks camelCase
`argsTemplate` on the wire: the UI's `Action`, `flowBindingOfAction`, every reminder descriptor
(`crates/host/src/reminder/descriptor.rs`), the `dashboard.pin` envelope, and the sibling
`Target::ref_id`'s `#[serde(rename = "refId")]` in the same file. `Action.args_template` was the one
outlier never renamed. Result: `dashboard.save` didn't recognise the UI's `argsTemplate` → stored
`args_template: null` (binding dropped); `dashboard.get` returned `args_template` → the UI read
`action.argsTemplate` = `undefined` (control never seeded, drag injected an empty template).

## The change

- **`model.rs`** — `Action.args_template` gains `#[serde(default, deserialize_with = "null_default",
  rename = "argsTemplate")]`. Field name + storage unchanged; only the JSON key changes, to the value
  every other producer already emits. `grep '"args_template"'` across `rust/` = **zero wire hits**, so
  the rename can't strand an existing consumer.
- **`pin.rs`** — the `pin_envelope`'s `action` block used to map `argsTemplate`→`args_template` by hand
  (with a comment naming the mismatch). With the rename, `Action` deserializes the camelCase envelope
  directly, so that block now mirrors the `source` deserialize right above it. Retired — one code path.
- **Sweep** — `model.rs` audited for a third un-renamed camelCase-expecting field; none. Every other
  multi-word wire field already carries its rename.

## Tests

- **`model.rs` unit** `action_round_trips_args_template_camel_case` — `argsTemplate` deserializes in and
  serializes back out under the camelCase key; snake `args_template` on the wire is ignored (proves the
  outlier is closed in both directions).
- **`tests/dashboard_flow_control_test.rs`** (new) — real `mem://` store, real `call_dashboard_tool`
  (no mocks, the rule-9 category the shipped UI unit tests skip): `flow_control_args_template_survives_
  save_get` saves a flow-bound slider and asserts each inner template key (`id`/`node`/`port`/`value`)
  survives save→get + the wire-shape guard (key is `argsTemplate`, not `args_template`);
  `non_control_cell_round_trips_without_action` is the additive guard.

## Verified

`cargo test -p lb-host --lib dashboard::model` (10/10) · `--test dashboard_flow_control_test` (2/2) ·
`--test widget_pin_test` (13/13, the retired-map path) · `--test dashboard_query_options_test` (3/3) ·
`--test dashboard_test` (12/12) · `cargo fmt -p lb-host --check` clean. Downstream rubix-ai `cargo build`
green against this checkout via its local `[patch]`.

## Release

Cut a `node-v*` tag carrying this; rubix-ai bumps its `lb-node` pin (currently `node-v0.5.1`) and drops
its dev `[patch]`.
