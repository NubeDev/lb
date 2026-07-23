# Session — dashboard Variable gains an `entity` binding, 2026-07-23

## The bug (a silent drop)

`dashboard::model::Variable` had no `entity` field and no serde catch-all. So `dashboard.save`,
`dashboard.get`, and `pack.apply::apply_dashboard` all round-trip variables through `Vec<Variable>`
(`typed_arg`) — and serde silently DROPPED the `entity` object an entity-type variable carries. An
entity var (entity-data-plane Phase D) then resolved **no options**, so a meter/site *template*
dashboard (a `meter` entity var over a pack binding) rendered empty. Same silent-drop class as
`queryOptions` and `argsTemplate` before their fields landed — and the blocker for
`NubeIO/rubix-ai → docs/scope/packs/generated-product-ux-scope.md` Plane 1.

Found by crossing the real wire from the downstream product (rubix-ai): `dashboard.get` returned a
`meter` var with no `entity`, so the UI's `entityVar.ts` had nothing to compile a resolver from.

## The fix

An additive, host-opaque field on `Variable`:

```rust
#[serde(default, deserialize_with = "null_default", skip_serializing_if = "Value::is_null")]
pub entity: Value,
```

Mirrors the sibling opaque `query`/`options` resolver fields — the host stays opaque (the client's
`entityVar.ts` compiles the binding into the same `{tool,args}` resolver `query` carries, re-checked
per call, rule 5). Pre-entity dashboards round-trip byte-clean (empty entity stays off the wire).

## Tests

- `crates/host/src/dashboard/model.rs`: serde round-trip of an entity binding + a skip-if-null guard.
- `crates/host/tests/dashboard_entity_var_test.rs`: a **real-MCP-path** (`call_dashboard_tool`,
  `mem://` store) save → get pin — the wire test the argsTemplate lesson demands, plus a
  non-entity-var guard.

Green: new tests pass; `dashboard_test` / `dashboard_query_options_test` / `dashboard_flow_control_test`
unchanged; `cargo fmt` clean. Released as **`node-v0.9.0`** (minor, additive, no wire break).
