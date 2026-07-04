# Frontend dashboard scope — the widget catalog: let AI discover the palette and reject widgets that don't exist

Status: scope (the ask). Promotes to `public/frontend/dashboard.md` once shipped. Topic: `frontend`.
Target stage: **S9+**, on top of the **shipped** widget-builder v2 + genui host-validation. Sibling of
[`widget-palette-scope.md`](widget-palette-scope.md) (that surfaced ext tiles in the *human* picker; this
surfaces the *whole palette + config schema* to an **AI** author, and enforces it on save).

> **Slice A of the system-wide widget program.** This is one slice of
> [`../../widgets/widget-platform-scope.md`](../../widgets/widget-platform-scope.md) — the umbrella that
> maps widgets as a whole-system capability (four widget sources; one renderer across dashboards, channels,
> and the app). This slice builds **source #1** (the built-in view palette + config) and **closes G4** (the
> AI hallucinating views `dashboard.save` then accepts). It is the discovery + gate foundation the later
> slices (pin-to-dashboard, result-render coverage, channel-origin authoring) build on.

When the AI authors a dashboard it calls `dashboard.save` with a `cells[]` array, each cell carrying a
`view` (`timeseries`/`stat`/`gauge`/`table`/… , an `ext:<id>/<widget>` federation key, or `genui`) and a
per-view `options` blob. **The AI invents views and config fields that don't exist** — because it has no
way to *discover* the valid palette, and **nothing rejects a bad write** — *except* `genui` cells, which the
host already validates on save against an embedded catalog
([`genui.rs`](../../../../rust/crates/host/src/dashboard/genui.rs) + `genui_catalog.json`). We give the AI a
machine-readable **widget catalog** over MCP (the palette — every built-in view with a **per-widget
version** + its config-field schema, plus installed extension tiles + genui components) and **extend the
genui-style save-validation to every cell**, so a hallucinated view or a bad option is rejected loudly at
write time — for the shell, a routed-Zenoh writer, and a headless external agent alike.

**Backend-driven, client-agnostic (the design decision).** The catalog is a **hand-authored JSON data file
owned by the host** (`rust/crates/host/src/dashboard/widget_catalog.json`), `include_str!`'d into the
binary and served over the `dashboard.catalog` **MCP verb** — *not* generated from the TypeScript editor.
That makes the host the single source of truth for "what widgets exist", reachable identically by every
client: the web UI, an AI agent over `POST /mcp/call`, and the **React-Native app** at
[`app/`](../../../../app) — which can render a backend-driven dashboard page (views + config) from the same
verb + `dashboard.get`, with no web-specific palette. (An earlier draft generated the catalog *from* the
TS registry; rejected — see Intent.)

---

## The gap, precisely

- **Discovery — MISSING.** There is no backend answer to "which views exist and what each configures". The
  view set + config schema lived only in the TS editor (`VizPicker.VIEWS`, the `View` union, the
  `OPTION_REGISTRY`), which the AI can't read. Extension tiles are discoverable (`ext.list.widgets[]`) and
  genui components are discoverable (`genui_catalog.json`), but the **built-in views and their config schema
  are not exposed over any MCP verb**. The AI guesses.
- **Enforcement — PARTIAL.** [`genui.rs::check_genui_cells`](../../../../rust/crates/host/src/dashboard/genui.rs)
  runs on `dashboard.save` and rejects a `genui` cell whose component name isn't in the embedded catalog —
  the exact "the host is the boundary; reject a malformed cell loudly, not degrade at view time" pattern we
  want. But it fires **only** for `cell.view == "genui"`. A cell with `view: "sparklne"` (typo), `view:
  "heatmap"` (doesn't exist yet), or a valid `view` with an invented `options.foo` sails straight through
  and persists — the renderer then shows a broken/empty tile, which is exactly the symptom reported.

There is no widget-catalog MCP verb, and no host-side view/option validation outside genui.

---

## Goals

- **A `dashboard.catalog` MCP verb.** One read verb returning the palette: every built-in `view` (id,
  label, `kind` = viz/control/scripted/read/genui, a **per-widget `version`**, whether it consumes `data`
  and/or writes an `action`) **with its per-view config-field schema**; the installed extension
  `[[widget]]` tiles (folded generically from `ext.list` — id stays **opaque**); and the genui component
  set. Modeled on [`tools.catalog`](../../../../rust/crates/host/src/tools/catalog.rs) — "the menu *is* the
  palette." Self-describing via a `ToolDescriptor` so it also appears in `tools.catalog`.
- **Backend-owned catalog data file — the single source of truth.** `widget_catalog.json` is a
  **hand-authored** JSON file in the host crate (`rust/crates/host/src/dashboard/`), `include_str!`'d by
  both the `dashboard.catalog` verb and the save-validator. Not generated from TS — the host owns the
  answer to "what widgets exist", so any client (web, AI, the RN app) reads one authority. Editing a
  widget = edit this file (same discipline as `genui_catalog.json`).
- **Per-widget version, declared now.** Each built-in view carries a `version` in the catalog so widgets
  can evolve their schema independently long-term. The AI never *chooses* a version — it copies the
  current one from the catalog. **No cell-stamping and no migration in this slice** (dev mode; there is no
  v2 widget to migrate to yet) — the field is the forward-looking contract, the machinery is deferred.
- **Client-agnostic rendering surface.** Because the catalog is a plain MCP verb, the RN app at `app/` can
  render a dashboard page (views + config + which cells need data) from `dashboard.catalog` + `dashboard.get`
  exactly as the web UI does — backend-driven dashboards, no web-only palette. (Building the app UI itself
  is out of scope — this *enables* it.)
- **Host-side save-validation for *all* cells.** Generalize the genui check: on `dashboard.save`, reject a
  cell whose `view` is neither a known built-in (in the catalog), a well-formed `ext:<id>/<widget>` key, nor
  `genui`. Loud `BadInput`, same shape genui uses, on every write path.
- **The AI authors correctly, not just legally.** With the config schema in hand the AI sets `unit`,
  `decimals`, `thresholds`, `legend.showLegend`, etc. by their real ids — not guessed keys — so a
  round-tripped cell renders as intended, not as a default-everything fallback.

## Non-goals

- **No new cell/view contract.** The v3 `Cell` shape, the `View` vocabulary, the `ext:<id>/<widget>`
  federation key, and the `options`/`fieldConfig` roots are frozen as shipped. This scope *exposes and
  validates* them; it does not add a view or change a schema.
- **No change to how a widget renders or gets data.** The federation bridge, `ctx.data` frames, trust
  tiers, and `cell.tools ∩ grant` re-check are untouched. Discovery + validation only.
- **No AI generation logic.** How the central agent *uses* the catalog to compose a dashboard (prompting,
  planning) is the agent's business — this scope hands it the menu and enforces the result. No prompt
  engineering lives here.
- **No per-extension special-casing.** The catalog folds `ext.list.widgets[]` generically; no core code
  branches on an extension id (rule 10). An ext tile appears in the catalog as opaque `{ext, widget, label,
  icon, scope}` data.
- **No skill instead of MCP.** A prose skill was considered and rejected (see Intent) — the catalog is
  structured, gated data, not documentation.

## Intent / approach

**Hand-author a backend `widget_catalog.json` owned by the host, serve it read-only over a new
`dashboard.catalog` MCP verb, and make `dashboard.save` validate every cell against it — reusing the genui
embedded-catalog + host-validation machinery verbatim.**

Three pieces, each a thin copy of the shipped genui precedent:

1. **The catalog data file.** `rust/crates/host/src/dashboard/widget_catalog.json` — hand-authored,
   `{ v, views:[{id,label,kind,version,buildable,data,action,options:[{id,label,scope,path,control,
   choices?}]}] }`. The **host owns it** (same as `genui_catalog.json`): editing a widget edits this file.
   No TS codegen — see "Why backend-owned" below.
2. **`dashboard.catalog` verb.** A new host-native read verb under `host/src/dashboard/` (`catalog.rs`).
   It `include_str!`s `widget_catalog.json` for the built-ins, calls the generic `ext.list` projection for
   installed tiles (opaque `{ext, widget, label, icon, data, scope}`), and reads the genui component names
   from the existing `genui_catalog.json` — returning one merged `{ v, views, extWidgets, genuiComponents }`
   document. Dispatched with the full `&Arc<Node>` (like `nav.*`, which also needs ext discovery) via its
   own branch **before** the generic store-only `dashboard.` branch. A `ToolDescriptor` registers it in
   `tools.catalog`. Authorized `mcp:dashboard.catalog:call` (member-level); workspace-first so a ws-B caller
   sees only ws-B's tiles.
3. **Save-validation for all views.** A new `host/src/dashboard/views.rs` validator (sibling of `genui.rs`),
   called from `dashboard.save` right where `check_genui_cells` is. It loads the embedded view-name set
   (from the same JSON) and for each cell: accepts a known built-in view; accepts a **well-formed**
   `ext:<nonempty>/<nonempty>` key **structurally** (see below); defers `genui` to the existing check; else
   `BadInput("cell {i}: unknown view '{view}' — call dashboard.catalog")`. This validator is **store-only**
   (no `Node`), so `dashboard.save`'s signature is unchanged.

**Why `ext:` keys are validated structurally, not resolved against installs.** Hard-rejecting an
`ext:<id>/<widget>` whose extension isn't currently installed would couple `dashboard.save` to
extension-install lifecycle: uninstalling/disabling an ext would make every dashboard that mentions its tile
**unsavable**, and would force `dashboard.save` to take the `&Node` (ext discovery) it currently doesn't
need. So the validator only checks the key is well-formed; an un-installed tile still renders the existing
"unknown widget" placeholder (unchanged behavior), and the **catalog** verb is where the AI learns which ext
tiles actually exist. Resolving-on-save can be a later additive slice if hallucinated ext keys prove common.

**Why MCP, not a skill.** A skill is prose that (a) goes stale the moment a view or option is added, (b)
isn't capability-gated or workspace-scoped, and (c) enforces nothing — the AI can still hallucinate and the
save still succeeds. An MCP verb is the universal contract (rule 7): the same structured menu the web UI, the
RN app, the central agent, and any third-party extension read the same way, gated by the same cap, and
**paired with a host-side rejection** so discovery and enforcement are two ends of one boundary. Discovery
*alone* wouldn't fix the reported bug; the save-validator is the load-bearing half.

**Why backend-owned, not generated from TS.** An earlier draft *generated* the catalog from the TS editor
registry (a `tsx` codegen + CI freshness gate, mirroring genui's `gen:skill`). Rejected: it makes the **TS
editor** the source of truth for a **backend/AI/app** contract, so the answer to "what widgets exist" would
live in the web frontend — the opposite of backend-driven. A hand-authored host-owned JSON (the
`genui_catalog.json` pattern) is the single authority every client reads; the AI catalog and the save-gate
key off the same file with no cross-build chain. The one cost — the file and the `WidgetView` render switch
can drift — is accepted for now (dev mode); a lightweight consistency check is an Open question.

**Rejected alternatives:**

- *A markdown skill listing the widgets.* Rejected — stale, ungated, unenforced (above).
- *Generate the catalog from the TS option registry (codegen + freshness gate).* Rejected — puts the
  source of truth for a backend contract in the web frontend; see "Why backend-owned".
- *Validate only in the shell (client-side).* Rejected — a headless `POST /mcp/call` or routed-Zenoh writer
  (an external agent) bypasses the shell entirely; the host is the only boundary every writer crosses
  (the same argument `genui.rs`'s header makes).
- *Fold built-in views into `tools.catalog`.* Rejected — `tools.catalog` enumerates *callable MCP tools*;
  a widget view is a render kind, not a tool. Different vocabulary, different consumer. Keep them distinct;
  `dashboard.catalog` can *reference* tool ids for a view's data binding, but it is its own verb.

## How it fits the core

- **Tenancy / isolation (rule 6):** `dashboard.catalog`'s built-in view set is workspace-independent
  (the static embedded palette), but the **ext-tile** portion is workspace-scoped — folded from the
  caller's `ext.list` (workspace-partitioned) so a ws-B caller sees only ws-B's installed tiles. Tested with
  the mandatory two-session isolation case (ws-A's catalog lists ws-A's tile, not ws-B's). The
  save-validator is view-shape-only (no cross-workspace read), so it introduces no isolation surface.
- **Capabilities (rule 5/7):** new member-level read cap `mcp:dashboard.catalog:call` (every member may read
  the palette — it grants nothing but knowledge; the *write* stays gated on `dashboard.save`). Deny path:
  a principal without it gets an opaque `ToolError::Denied`, tested. The save-validator adds **no** new cap
  — it's a correctness gate on the already-gated `dashboard.save`. Note `dashboard.catalog` follows the
  `granted = requested ∩ admin_approved` path like every verb (no pre-approval; rule 10).
- **Placement:** either — pure node-local read + validation, no cloud authority, no `if cloud`. Symmetric.
- **One datastore:** no new persistence. The catalog is a compiled-in **hand-authored** JSON
  (`include_str!`), not a table; ext tiles come from the existing `ext` records via `ext.list`. Nothing new
  in SurrealDB. Per-widget `version` lives in that file; it is **not** stamped onto saved cells (deferred).
- **MCP surface (API shape):** **get/list only.** `dashboard.catalog` is a single read (the palette is one
  document; no id-addressed sub-fetch is needed — a caller filters client-side). **No CRUD** — the catalog
  is host-owned data, never written over MCP. **No live feed** — the palette changes only when code ships or
  an extension is installed/removed; a caller re-reads on demand (an ext install already invalidates the
  UI's `ext.list`). **No batch.** The *enforcement* rides the existing `dashboard.save` write verb — no new
  write.
- **Bus (Zenoh):** N/A — no motion; this is state/read + a synchronous validation on an existing write.
- **Sync / authority:** node-local; the catalog is deterministic from the build + the workspace's installs.
  No offline divergence (the JSON is embedded; ext tiles follow the same offline story as `ext.list`).
- **SDK/WIT impact:** none — no plugin boundary change. An extension already declares `[[widget]]` tiles;
  this only *surfaces* them in a new read. Flag if a future slice lets an ext declare its tile's config
  schema (not this slice — see Open questions).
- **Skill doc:** **Yes** — this adds an agent-drivable surface. The implementing session writes
  `skills/dashboard-widgets/SKILL.md`: how an agent discovers the palette (`dashboard.catalog`), picks a
  view for a data shape, sets options by their real ids, and what a save-rejection means — grounded in a
  live run against a real gateway. (The genui slice's skill is the template.)

## Example flow

1. The central agent (or the RN app) is asked to "add a temperature gauge to the Ops dashboard." It calls
   `dashboard.catalog` → gets `{ v:1, views:[ …, {id:"gauge", label:"Gauge", kind:"viz", version:1,
   data:true, action:false, options:[{id:"min",…},{id:"max",…},{id:"thresholds",control:"thresholds",…},
   {id:"unit",…}]}, … ], extWidgets:[…], genuiComponents:[…] }`.
2. The agent picks `view:"gauge"` (a `data:true` viz, so it binds a `series.latest` source), sets
   `options.min/max` and `fieldConfig.defaults.unit:"celsius"` and `thresholds` **by their catalog ids** —
   no guessing.
3. It calls `dashboard.save` with the new cell. The host runs `check_view_cells` (new) + `check_genui_cells`
   (existing): `gauge` is a known built-in → the save persists.
4. Contrast — the agent instead hallucinates `view:"heatmap"` (not shipped). `dashboard.save` returns
   `BadInput("cell 4: unknown view 'heatmap' — call dashboard.catalog for the palette")`. The agent reads
   the error, re-reads the catalog, and corrects. The broken tile **never persists**.
5. A headless external agent (`POST /mcp/call dashboard.save`) with the same bad cell gets the **same**
   rejection — the shell isn't in the loop; the host is the boundary.

## Testing plan

Real gateway + real store, no fakes (rule 9). Mandatory categories:

- **Capability deny (required):** a principal without `mcp:dashboard.catalog:call` is denied
  (`ToolError::Denied`, opaque) — real gateway, real token.
- **Workspace isolation (required):** two sessions, ws-A and ws-B, ws-A with a real installed `[[widget]]`
  extension (`proof-panel`). `dashboard.catalog` for ws-A lists ws-A's ext tile; for ws-B it does **not**.
  The built-in view set is identical for both (workspace-independent).
- **Save-validation (the core):** `dashboard.save` with (a) a valid built-in view → persists; (b) an
  unknown `view` (`"heatmap"`, typo) → `BadInput`, nothing persisted; (c) a well-formed `ext:<id>/<widget>`
  key → persists (structural, not install-resolved); (d) a malformed `ext:` key (`"ext:"`, `"ext:x/"`) →
  `BadInput`; (e) a `genui` cell → still routed through the existing check (regression). Assert the
  rejection is identical over the shell path and a headless `POST /mcp/call`.
- **Catalog completeness (unit, Rust):** `widget_catalog.json` parses; view ids are unique; every viz view
  has a non-empty `options` list; every id in the validator's valid-set comes from the same file (the verb
  and the validator agree).
- **Round-trip authoring (integration):** a cell authored purely from catalog ids (view + options) saves,
  reloads, and renders through the real `WidgetView` without falling back to defaults.

## Risks & hard problems

- **Catalog ↔ renderer drift.** The hand-authored JSON and the `WidgetView` render `switch` are two lists;
  adding a renderable view without cataloging it (or vice-versa) silently mis-classifies a widget. Accepted
  for now (dev mode) — mitigate with the comment cross-link on both files and, optionally, a lightweight
  consistency check (Open question). This is the deliberate trade for backend-owned-not-generated.
- **Option-schema fidelity.** The catalog carries a curated subset of each viz view's knobs (`unit`,
  `decimals`, `thresholds`, `legend`, `orientation`, …), not the entire editor option registry. Enough for
  the AI to author correctly; widen per-view if a needed knob is missing. It is the AI's contract, not a
  mirror of the human editor.
- **Ext-tile config schema.** An `ext:<id>/<widget>` tile has no declared option schema today (it owns its
  own config). The catalog lists the tile but can't tell the AI its knobs. Acceptable for v1 (the AI places
  it, doesn't configure it — same stance as `widget-palette-scope.md`); a future slice could let a `[[widget]]`
  declare a config schema.

## Open questions

- **A consistency guard for catalog ↔ `WidgetView` drift?** A tiny test (TS or Rust) asserting the catalog's
  view ids match the renderer's switch cases would catch the one dangerous divergence without making TS the
  source of truth. Recommend adding it if drift bites; skipped in the first cut (dev mode).
- **Verb name: `dashboard.catalog` vs `widget.catalog`?** Chose `dashboard.catalog` — it lives in the
  `dashboard.` host-native family (existing prefix, no new dispatcher family) and the palette is a
  dashboard-authoring concept.
- **Per-widget version consumption.** The `version` field ships now but nothing reads it yet (no
  cell-stamping, no migration — dev mode). Resolve *when* the first widget gets a breaking v2: stamp
  `cell.plugin_version` at save and fold stale cells at render (the `schema_version` pattern).
- **App rendering.** This slice makes the catalog app-consumable but does not build the RN dashboard page.
  Track the app-side render as its own `app/` task keyed off `dashboard.catalog` + `dashboard.get`.
- **Where does the freshness gate run** — `packages/genui`'s vitest, or a new `packages/…`/`ui` test? The
  registry lives in `ui/src/features/panel-builder`; the gate should sit with the codegen script. Decide the
  script's home (likely a small `ui`-side `bin/` + `ui` test) at build.

## Related

- Precedents reused: [`genui.rs`](../../../../rust/crates/host/src/dashboard/genui.rs) +
  `genui_catalog.json` (embedded hand-authored catalog + save-validation),
  [`tools/catalog.rs`](../../../../rust/crates/host/src/tools/catalog.rs) (the "menu is the palette" verb),
  `nav/resolve.rs` (a `dashboard.`/`nav.`-family verb taking `&Arc<Node>` for ext discovery).
- Source of truth: `rust/crates/host/src/dashboard/widget_catalog.json` (host-owned). The `WidgetView`
  render switch ([`WidgetView.tsx`](../../../../ui/src/features/dashboard/views/WidgetView.tsx)) is the
  renderer it must stay in step with.
- App consumer (enabled, not built here): [`app/`](../../../../app).
- Siblings: [`widget-palette-scope.md`](widget-palette-scope.md) (human picker for ext tiles),
  [`widget-builder-scope.md`](widget-builder-scope.md), [`widgets-scope.md`](widgets-scope.md),
  [`../dashboard-scope.md`](../dashboard-scope.md).
- Core rules: README §3 (rules 5/6/7/10), `docs/scope/extensions/extensions-scope.md` (opaque ext ids).
- Skill (build writes it): `skills/dashboard-widgets/SKILL.md`.
