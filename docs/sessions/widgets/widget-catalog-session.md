# Session — widget catalog + save-validation (widget-platform Slice A)

Status: **done** (2026-07-04)

Slice A of the [widget-platform umbrella](../../scope/widgets/widget-platform-scope.md): the
backend-owned widget palette served over a new `dashboard.catalog` MCP verb, plus host-side
`dashboard.save` rejection of unknown `view` kinds — closing G4 (the AI hallucinating views the save
then accepted, which rendered as a broken/empty tile).

- **Scope (the ask):** [widget-catalog-scope.md](../../scope/frontend/dashboard/widget-catalog-scope.md).
- **Public (promoted truth):** [public/frontend/dashboard.md](../../public/frontend/dashboard.md).
- **Skill:** [skills/dashboard-widgets/SKILL.md](../../skills/dashboard-widgets/SKILL.md).
- **Precedents copied:** `genui.rs` + `genui_catalog.json` (embedded host-owned catalog +
  save-validation), `tools/catalog.rs` ("the menu is the palette"), `nav/tool.rs` (a
  `dashboard.`/`nav.`-family verb taking `&Arc<Node>` for `ext.list` discovery).

---

## What shipped

Three thin copies of the genui precedent, plus the wiring the scope named "load-bearing".

1. **The catalog data file** — `rust/crates/host/src/dashboard/widget_catalog.json` (hand-authored,
   pre-existing). Treated as the **source of truth**, not regenerated from TS. 17 built-in views, each
   `{ id, label, kind, version, buildable, data, action, options:[{id,label,scope,path,control,choices?}] }`.

2. **`dashboard.catalog` verb** — new `host/src/dashboard/catalog.rs`. `include_str!`s the JSON for the
   built-ins, folds installed ext `[[widget]]` tiles **generically** from `ext.list` (id opaque — no
   branch on any ext id, rule 10), appends `genui_component_names()`. Returns one merged
   `{ v, views, extWidgets, genuiComponents }`. Dispatched with `&Arc<Node>` (like `nav.resolve`) via
   its **own branch before** the generic store-only `dashboard.` branch in `tool_call.rs`. A
   `ToolDescriptor` (`catalog_descriptor()`) registers it in `host_descriptors()` so it self-describes
   in `tools.catalog`. Workspace-first (a ws-B caller sees only ws-B's tiles).

3. **The save-validator** — new `host/src/dashboard/views.rs` (sibling of `genui.rs`),
   `check_view_cells(&[Cell])`, called from `dashboard.save` right where `check_genui_cells` is.
   **Store-only, no `Node`** → `dashboard.save`'s signature is unchanged. Per cell: accept a known
   built-in view (from the SAME JSON); accept a well-formed `ext:<nonempty>/<nonempty>` key
   **structurally** (NOT resolved against installs — resolving would couple save to install-lifecycle
   and force `&Node`); defer `genui` to the existing IR check; else
   `BadInput("cell {i}: unknown view '{view}' — call dashboard.catalog for the palette")`. Empty view
   (a v1 cell) is skipped (not a hallucination). View-NAME only — option keys are a named follow-up.

4. **Capability wiring** — `mcp:dashboard.catalog:call` added to `member_caps()` in
   `role/gateway/src/session/credentials.rs`. Load-bearing: the `mcp:*.{get,list,write,…}:call`
   wildcards do NOT match `.catalog`, so without this line every member call is denied (the same trap
   `tools.catalog` sits behind — asserted together in a new `member_caps` unit test).

5. **View-union reconcile (TS)** — trimmed the dead ids `histogram`, `state-timeline`,
   `status-history`, `heatmap`, `text` from the `View` union in `ui/src/lib/dashboard/dashboard.types.ts`.
   They had no `WidgetView` render case and no catalog entry — leaving them let a typed client author a
   view the new validator rejects. Now the union, the render switch (17 cases), and the catalog are one
   list.

## Tests (real gateway + real store, no fakes — rule 9)

- **`rust/crates/host/tests/widget_catalog_test.rs` (8 tests, green):**
  - **capability deny + plain-member happy path** — a no-cap principal is denied opaquely
    (`ToolError::Denied`); a **plain member** holding only `mcp:dashboard.catalog:call` reads the full
    17-view palette (proves the grant, not an admin bypass).
  - **workspace isolation** — ws-A with a **real seeded `[[widget]]` install** (`record_install`) lists
    its tile; ws-B lists none; the built-in `views` are identical across both.
  - **save-validation (core)** — valid built-in persists · unknown `"heatmap"` → `BadInput`, nothing
    persisted · well-formed `ext:<id>/<widget>` persists structurally · malformed `ext:`/`ext:x/`/
    `ext:/w`/`ext:onlyid` → `BadInput` · genui cell still routed through the IR check. The rejection is
    asserted **identical over the shell path (`dashboard_save`) AND the headless `POST /mcp/call`
    (`call_tool`)**.
  - **round-trip authoring** — a cell authored purely from catalog ids (view `gauge`, options
    `min`/`max`, `fieldConfig.defaults.unit`) saves and reloads intact.
- **`views.rs` unit (Rust):** catalog parses · view ids unique · every viz view has non-empty options ·
  the validator's accept-set is derived from the same file (verb + validator agree by construction).
- **`credentials.rs` unit:** `member_caps()` carries `mcp:dashboard.catalog:call` (+ `tools.catalog`).
- **`ui/src/features/dashboard/views/widgetCatalog.consistency.test.ts` (TS, 3 tests):** imports the
  host-owned `widget_catalog.json` and asserts its view ids EXACTLY match `WidgetView`'s render-switch
  cases (parsed from source) and that no trimmed dead id reappears — the catalog ↔ renderer drift guard
  the scope requires this slice.

Green: `cargo test -p lb-host` (widget_catalog 8/8, dashboard 10, genui 8, nav 11, tools_catalog 4,
catalog_mcp 8) · `cargo fmt` · `pnpm test` (536 passed). Pre-existing failures unchanged and confirmed
red on a stashed tree: `fleet_monitor_test` (needs `cargo build -p fleet-monitor` native binary),
plus the scope's named `SystemView.gateway` / `sqlSource.gateway` / `agent_routed_test` /
`DashboardView.gateway` "tab Field". `tsc` errors are pre-existing (flows/transform gateway tests),
none from the View-union trim (verified via git stash).

## Decisions / notes

- **`extWidgets.widget`** is set from the tile's `label` — `ExtUi` carries no separate slug, and the
  federation key an author composes is `ext:<ext>/<label>`. Surfaced `label` separately for display.
- **`ext.list` absence is soft** in `dashboard.catalog`: a caller without `mcp:ext.list:call` still
  gets the full built-in palette (ext tiles just empty), rather than a hard deny of the whole catalog —
  the built-in palette is workspace-independent and grants nothing.
- **Out of scope (named follow-ups):** option-key validation on save · per-widget version stamping/
  migration · resolving `ext:` keys against installs (the `warnings[]` middle path) · the RN app
  renderer (Slice A only enables it) · umbrella Slices B/C/D.
