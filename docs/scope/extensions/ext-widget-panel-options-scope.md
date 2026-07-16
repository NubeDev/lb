# Extensions scope — ext widgets as first-class panels (identity + declarative panel options)

Status: **implemented (additive), tests green; awaiting release tags.** `manifest.rs` `Widget` gained
`id`/`options` with parse-time shape+uniqueness validation; `ExtUi`/`ExtRow`/`ExtWidget` relay them;
`dashboard.catalog`'s view key now uses the resolved widget id; `@nube/ext-ui-sdk` exports
`WidgetOptionDef`/`WidgetFieldConfig` and narrows `ctx.fieldConfig` (+ `defineRemote` now returns an
update-in-place `WidgetHandle`). Manifest (21), `ui_decl`, and `widget_catalog_test` (8) pass; SDK
tests pass. Proven downstream against a real rubix-ai node via the `hvac` extension under local
`[patch]`/SDK overrides. Needs the `node-v*` + `ui-v0.9.0` release tags to become real for consumers.
Additive over the shipped widget contract v4 (`@nube/ext-ui-sdk`
`WidgetCtx`), the shipped v3 frames-in data binding
(`../frontend/dashboard/ext-widget-source-binding-scope.md`), and the shipped
`dashboard.catalog` palette (`../frontend/dashboard/widget-catalog-scope.md`).

An extension widget can already BE a dashboard panel (`view: "ext:<ext>/<widget>"`, mounted via
federation, v3 data tiles get shell-resolved `ctx.data` frames + `ctx.fieldConfig`). What it
**cannot** do is *describe itself* to a panel editor: a `[[widget]]` has no stable id (identity is
the slugged display `label`), no way to declare which panel options it understands, and the SDK
types `ctx.options` / `ctx.fieldConfig` as `Record<string, unknown>` / `unknown`. Downstream hosts
(e.g. the rubix-ai Data Studio wizard) therefore cannot offer an honest option surface for an ext
widget the way they do for built-in views (`widget_catalog.json` carries a per-view `options[]`
schema — extensions get none, noted as the "v1 limit" in `dashboard/catalog.rs`). This scope
closes that gap **declaratively**: the manifest carries the widget's option schema as opaque data;
the host relays it generically; the editor renders it; the tile receives the chosen values on the
`ctx.options` / `ctx.fieldConfig` it already gets today.

## Goals

- **Stable widget identity.** `[[widget]]` gains an optional `id` (slug). Default remains
  `slug(label)` so every existing manifest keeps working; the view key stays
  `ext:<ext>/<widget-id>`.
- **Declarative panel options.** `[[widget]]` gains an optional `options = [...]` array using the
  **same option-def shape** built-in views already use in `widget_catalog.json`:
  `{ id, label, scope: "options"|"fieldConfig", path, control, choices?, default? }`. The host
  validates shape, stores it, and relays it verbatim — it never interprets a def.
- **The standard option surface is opt-in by `data`.** A v3 data tile (`data = true`) already
  receives the cell's `fieldConfig` resolved alongside its frames; hosts may therefore offer the
  standard field options (unit, decimals, min/max, thresholds, mappings, displayName, noValue,
  color) for any data tile with **no per-widget declaration needed**. This scope types that
  contract so it's a promise, not a coincidence.
- **Typed SDK contract.** `@nube/ext-ui-sdk` exports `WidgetOptionDef` (the manifest option-def
  shape) and `WidgetFieldConfig` (the standard fieldConfig subset: unit, decimals, min, max,
  thresholds, mappings, displayName, noValue, color), and narrows
  `WidgetCtx.fieldConfig?: WidgetFieldConfig`. Additive — the mount signature is unchanged, so
  this stays contract v4/v5-minor, not a breaking major.
- **Carried end to end:** manifest → `Install`/`ExtUi` → `ext.list` row → `dashboard.catalog`
  `extWidgets[]` — every hop additive, every hop generic.

## Non-goals

- **No option-schema enforcement on save.** `dashboard.save` keeps accepting any `options` blob
  (the same v1 limit built-in views have; the schema walk stays the named follow-up in
  `widget-catalog-scope.md`). The schema here drives the *editor*, not a validator.
- **No extension-provided option editors.** An extension does not ship React code that runs inside
  the host's panel editor — that's a security/contract surface we refuse. Options are declarative
  data rendered by the host's own controls. If the closed control set proves insufficient, revisit
  with its own scope.
- **No new control kinds.** `control` values are the host's existing vocabulary (`text`, `number`,
  `toggle`, `select`, `unit`, `thresholds`, …). A host that meets an unknown `control` string must
  degrade honestly (a labeled raw-value row), never drop the option silently.
- **No formatting stack in the SDK.** `WidgetFieldConfig` is a *type*; unit/date rendering stays
  the host `format.*` boundary. (A small pure helper — e.g. threshold-color resolution — may ship
  with the type if the first consumer proves the need; anything CLDR-shaped may not.)

## Intent / approach

Mirror how built-in views describe themselves — a static `options[]` schema next to the view id —
but sourced from the manifest instead of `widget_catalog.json`. The host is a **relay, not an
interpreter** (rule 10): `manifest.rs` parses + shape-validates the defs; `assets` persists them on
the install record; `ext/row.rs` (`ExtUi`) and `dashboard/catalog.rs` (`ExtWidget`) pass them
through untouched. No host code path branches on an option's meaning, and swapping one extension
for another exercises the identical relay.

Rejected alternative — a `widget.describe` MCP tool the editor calls per widget: it makes the
option surface a runtime round-trip and lets a widget answer differently per call; the manifest is
already the signed, reviewed contract with the host, and static data is cacheable, diffable, and
honest.

Touched (all additive):

| Where | Change |
|---|---|
| `rust/crates/ext-loader/src/manifest.rs` | `Widget` gains `id: Option<String>`, `options: Vec<WidgetOption>`; validate id-slug uniqueness per manifest + option shape |
| `rust/crates/assets/src/install/model.rs` | `ExtUi` gains `id`, `options` (serde-default) |
| `rust/crates/host/src/ext/row.rs` | passes through (derives) |
| `rust/crates/host/src/dashboard/catalog.rs` | `ExtWidget` gains `id` (view key uses it; falls back to `slug(label)`) + `options` passthrough |
| `lb-ext-sdk` | the manifest authoring type gains the same optional fields (as `manifest.rs` already notes for `emits_external`) |
| `lb-ext-ui-sdk` | export `WidgetOptionDef`, `WidgetFieldConfig`; narrow `WidgetCtx.fieldConfig` |

## How it fits

- **Rule 10 / no special-casing:** the schema is opaque per-widget data flowing through the same
  generic seams every extension uses. No named-extension branch anywhere; the deny path is
  untouched.
- **Capabilities:** unchanged — `ext.list` stays gated `mcp:ext.list:call`, `dashboard.catalog`
  stays gated `mcp:dashboard.catalog:call`; option defs grant nothing (a def is UI hinting; the
  tile's reach is still `scope ∩ grant`, re-checked per bridge call).
- **Isolation:** installs are workspace-scoped already; the schema rides the install record, so a
  ws-B editor can never see a ws-A widget's options.
- **API shape:** additive fields on existing verbs' responses; no new verb.
- **SDK/ABI impact — flagged loudly:** `lb-ext-ui-sdk` types are additive (minor tag `ui-v0.7.x`);
  `lb-ext-sdk` authoring type additive (minor `sdk-v*`); the WIT world is untouched (options are
  manifest/UI plumbing, not the tool ABI). Release order per the family workflow: SDKs → lb tag
  `node-v*` → downstream pins bump.

## Example flow

1. An extension's manifest declares:
   ```toml
   [[widget]]
   id     = "zone-comfort"
   entry  = "remoteEntry.js"
   label  = "Zone Comfort"
   scope  = [ "series.latest", "series.watch" ]
   data   = true
   options = [
     { id = "setpointField", label = "Setpoint field", scope = "options", path = "setpointField", control = "field-name" },
     { id = "band",          label = "Comfort band ±",  scope = "options", path = "band", control = "number", default = 1.5 },
   ]
   ```
2. `lb-ext publish` → install persists `ExtUi { id, entry, label, icon, scope, data, options }`.
3. A panel editor calls `dashboard.catalog`; `extWidgets[]` now carries
   `{ ext, widget: "zone-comfort", label, data: true, options: [...] }`; it renders the two custom
   rows with its own controls, plus its standard field options because `data = true`.
4. The author saves; the cell stores `options.setpointField`, `options.band`,
   `fieldConfig.defaults.thresholds`, ….
5. On render the shell resolves `sources[]` via `viz.query` and mounts the tile with
   `ctx.data`, `ctx.fieldConfig` (now typed), and `ctx.options` carrying the chosen values —
   exactly the v4 mount that ships today.

## Testing plan

Real node, real store, no mocks (rule 9):

- **Manifest:** parse tests for `id` + `options` (present, defaulted, duplicate-id rejected,
  malformed def rejected loudly at publish, empty allowed) in `manifest.rs`.
- **Relay:** `widget_catalog_test.rs` extended — publish a manifest with declared options, assert
  `dashboard.catalog.extWidgets[].options` round-trips verbatim and the view key uses `id`.
- **Legacy:** a manifest with no `id`/`options` produces today's exact rows (label-slug key,
  absent options) — the additive guarantee.
- **Capability-deny / workspace-isolation:** existing `ext.list`/`dashboard.catalog` deny + wall
  tests still green; a second-workspace session sees neither the widget nor its schema.
- **SDK:** `lb-ext-ui-sdk` type tests + rebuilt committed `dist/`.

## Risks & hard problems

- **Schema drift between hosts:** two hosts (lb minimal-shell, rubix-ai studio) render the same
  defs; the def shape must stay the `widget_catalog.json` shape verbatim so there is one
  vocabulary. Pin it with a shared fixture test.
- **The unknown-`control` path:** a silent drop would make a widget's option invisible on an older
  host. The degrade-to-raw-row rule must be stated in the SDK docs and tested downstream.
- **Identity migration:** existing dashboards store label-slug view keys. `id` defaulting to
  `slug(label)` means nothing breaks unless an author *adds* an `id` differing from the old slug —
  document that as a breaking rename for that widget's existing cells.

## Open questions

- Should `[[widget]]` also declare accepted data **shapes** (`timeseries`, `tabular`, …) so pickers
  can shape-gate ext tiles like built-ins? Deferred until a real widget needs it; always-enabled is
  the honest default meanwhile.
- Does the first consumer need a pure `resolveThresholdColor(value, fieldConfig)` helper in the UI
  SDK, or is the type alone enough? Decide when the first data tile lands downstream.

## Related

- `../frontend/dashboard/widget-catalog-scope.md` — the palette + its "no config schema" v1 limit
  this scope lifts.
- `../frontend/dashboard/ext-widget-source-binding-scope.md` — the v3 frames-in contract the
  standard options ride on.
- `ext-out-of-tree-scope.md` — the published-SDK split; `ui-federation-scope.md` — the mount seam.
- Downstream consumers: `rubix-ai` → `docs/scope/frontend/dashboard/viz/ext-widget-chart-type-scope.md`
  (the Data Studio picker/options surface); `rubix-ai-extensions` →
  `docs/scope/extensions/hvac-scope.md` (the first extension authored against this).
