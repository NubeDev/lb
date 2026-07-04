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
  "heatmap"` (in the TS `View` union but with **no renderer case** — see Intent), or a valid `view` with an
  invented `options.foo` sails straight through and persists — the renderer then shows a broken/empty tile,
  which is exactly the symptom reported. **This slice enforces the view-name half only**: an unknown `view`
  is rejected at save; an invented option key on a *valid* view is mitigated by discovery (the catalog gives
  the AI the real ids) but not yet rejected — option-schema enforcement is a named follow-up (see Non-goals).

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
  can evolve their schema independently long-term. In this slice the field is **catalog-only and
  informational**: no writer (AI or UI) copies it into a cell, no cell-stamping, no migration (dev mode;
  there is no v2 widget to migrate to yet). Honest cost note: adding `version` later would have been
  additive anyway — it ships now only because it is free and documents the intended evolution path; the
  machinery is deferred until the first breaking widget change.
- **Client-agnostic rendering surface.** Because the catalog is a plain MCP verb, the RN app at `app/` can
  render a dashboard page (views + config + which cells need data) from `dashboard.catalog` + `dashboard.get`
  exactly as the web UI does — backend-driven dashboards, no web-only palette. (Building the app UI itself
  is out of scope — this *enables* it.)
- **Host-side save-validation for *all* cells — view name only.** Generalize the genui check: on
  `dashboard.save`, reject a cell whose `view` is neither a known built-in (in the catalog), a well-formed
  `ext:<id>/<widget>` key, nor `genui`. Loud `BadInput`, same shape genui uses, on every write path.
  Option *keys* are not validated in this slice (see Non-goals).
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
- **No option-schema enforcement on save.** The validator rejects unknown `view` names only. A *valid* view
  with an invented `options.foo` still persists in this slice — the mitigation is discovery (the catalog
  hands the AI the real option ids) plus the round-trip authoring test. Rejecting unknown option keys is a
  **named follow-up** (it needs a per-view option schema walk, incl. `fieldConfig` overrides, and a decision
  on unknown-key strictness for hand-edited dashboards) — not silently promised here.
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
   No TS codegen — see "Why backend-owned" below. `buildable:false` means "valid to *save/render* but do
   not *author new* cells of this kind" — aliases and escape hatches (`chart`, `plot`, `d3`, `template`,
   `button`) that exist for compatibility or scripted use; an AI author picks only `buildable:true` views,
   the validator accepts both.
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

**Reconcile the TS `View` union (in scope).** The catalog and `WidgetView`'s render switch agree exactly
(17 ids), but the TS `View` union (`ui/src/lib/dashboard/dashboard.types.ts`) also declares `histogram`,
`state-timeline`, `status-history`, `heatmap`, and `text` — ids with **no renderer case and no catalog
entry**. A typed client can author them "legally" today and the new validator would reject them. This slice
**trims those dead ids from the union** (they render nothing anyway) so the type contract, the renderer,
and the catalog are one list. Note the blast radius of validation: `dashboard.save` validates the whole
`cells[]`, so **one** unknown-view cell makes the **entire dashboard unsavable** (even a title edit). That
is acceptable in dev mode and it is the genui precedent's behavior — but it is a deliberate choice, and the
error message must name the offending cell index and view so the fix is one edit away.

**Why `ext:` keys are validated structurally, not resolved against installs.** Hard-rejecting an
`ext:<id>/<widget>` whose extension isn't currently installed would couple `dashboard.save` to
extension-install lifecycle: uninstalling/disabling an ext would make every dashboard that mentions its tile
**unsavable**, and would force `dashboard.save` to take the `&Node` (ext discovery) it currently doesn't
need. So the validator only checks the key is well-formed; an un-installed tile still renders the existing
"unknown widget" placeholder (unchanged behavior), and the **catalog** verb is where the AI learns which ext
tiles actually exist. Honest hole: a hallucinated-but-well-formed ext key persists **silently** — the
placeholder render is the only signal, and nothing counts occurrences, so "if it proves common" is not
measurable as stated. The named middle path (a follow-up, not this slice): `dashboard.save` returns a
**non-fatal `warnings[]`** entry when an `ext:` key doesn't resolve in the caller's `ext.list` — the save
succeeds (no install-lifecycle coupling), the AI can self-correct, a human sees it. Hard resolve-on-save
stays rejected.

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
  **Wiring (required, or the verb is dead-on-arrival):** the member credential map
  (`rust/role/gateway/src/session/credentials.rs::member_caps()`) enumerates the dashboard caps explicitly
  and its wildcards (`mcp:*.{get,list,write,create,update,delete,post}:call`) do **not** match `.catalog` —
  `tools.catalog` works only because it is individually listed. This slice adds
  `mcp:dashboard.catalog:call` to `member_caps()` (mirroring `tools.catalog`); the happy-path test must run
  with a **plain member token** so it proves the grant, not an admin bypass. (Same trap as
  `prefs.set_default`.)
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

## How the app renders a page (the intended app-render contract)

This slice **enables** a backend-driven app render surface without building it; this section pins the
contract so the eventual `app/` task builds *to* it instead of re-deciding it. **Nothing here ships in
Slice A** — it is the forward contract the catalog exists to serve.

**Two documents, two questions — the app reads both.** "The app knows what to render for a page" is *two*
backend answers composed, not one:

1. **The catalog** (`dashboard.catalog`, this slice) answers *"what widget kinds exist and how is each
   configured"* — the vocabulary. Static per build + the workspace's installed ext tiles.
2. **The page document** (`dashboard.get`, **already shipped**) answers *"what does THIS page show"* — an
   array of cells, each with a layout (`i,x,y,w,h`), a `view` (a catalog id), an `options`/`fieldConfig`
   blob, and a data `source` binding.

The app renders a page by walking the page document's cells; for each cell it looks up the `view` in the
catalog to know **how** to draw it and which cells need data, then binds data through the same
host-mediated bridge (`cell.tools ∩ grant`, re-checked at the host) the web renderer uses. **No app-side
palette, no hardcoded view list, no per-extension branch** — the app holds the same zero tool/ext knowledge
the web shell does (rule 7 + rule 10). A view added in the backend, or an ext tile installed in the
workspace, appears in the app with no app release.

**What this contract deliberately does NOT yet cover** (so the app task scopes honestly):

- **The renderer itself.** The app needs a component that maps a catalog `view` id → a native/RN render.
  Slice A gives it the *schema* to render against, not the renderer. This is the bulk of the app task.
- **Ext-tile config.** The catalog lists an ext `[[widget]]` tile as opaque `{ext,widget,label,icon,scope}`
  but carries **no config schema** for it (the extension owns its config). So a generic app can *place* an
  ext widget on a page but cannot generically *configure* its knobs — same v1 limit as the web palette.
  A future slice letting a `[[widget]]` declare a config schema lifts this for all surfaces at once.
- **Option-key validation.** Slice A rejects unknown *view kinds* at save; it does not yet reject a valid
  view carrying garbage *option keys*. The catalog tells the app the real option ids to read, but a page
  document may still carry an invented key the app must tolerate (ignore-unknown, not crash).
- **Non-dashboard app pages.** This contract is for a *dashboard-shaped* page (a cell grid). A channel
  response in the app (a single `rich_result` envelope, no grid) rides the **same** `view`+catalog
  vocabulary through one `WidgetView`-equivalent — but wiring the app's channel surface is umbrella
  Slice C/D territory, not a dashboard page.

The payoff is the same envelope everywhere: a `{view, source|data, options, action, tools}` cell authored
once renders identically in a web grid, a channel response, and an app page — because all three resolve
`view` against this one catalog and bind data through the one gated bridge.

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
  (`ToolError::Denied`, opaque) — real gateway, real token. The paired **happy path runs as a plain
  member** (dev-login `member_caps()`), not an admin — it proves the new cap is actually in the member
  grant, not that an admin bypasses it.
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
- **Catalog ↔ renderer consistency guard (unit, TS — required, this slice):** a `ui`-side test imports
  `rust/crates/host/src/dashboard/widget_catalog.json` and asserts its view ids exactly match `WidgetView`'s
  render-switch cases (and the trimmed `View` union). This does **not** make TS the source of truth — it
  makes the renderer *accountable to* the backend truth. Without it, catalog↔renderer drift reproduces the
  exact G4 symptom this slice exists to kill, now with the host vouching for the broken view.
- **Round-trip authoring (integration):** a cell authored purely from catalog ids (view + options) saves,
  reloads, and renders through the real `WidgetView` without falling back to defaults.

## Risks & hard problems

- **Catalog ↔ renderer drift.** The hand-authored JSON and the `WidgetView` render `switch` are two lists;
  adding a renderable view without cataloging it (or vice-versa) silently mis-classifies a widget — the
  same bug class this slice fixes, relocated. **Not** accepted: the consistency-guard test (Testing plan)
  ships in this slice, plus the comment cross-link on both files. This is the price of
  backend-owned-not-generated, paid up front instead of on drift.
- **Option-schema fidelity.** The catalog carries a curated subset of each viz view's knobs (`unit`,
  `decimals`, `thresholds`, `legend`, `orientation`, …), not the entire editor option registry. Enough for
  the AI to author correctly; widen per-view if a needed knob is missing. It is the AI's contract, not a
  mirror of the human editor.
- **Ext-tile config schema.** An `ext:<id>/<widget>` tile has no declared option schema today (it owns its
  own config). The catalog lists the tile but can't tell the AI its knobs. Acceptable for v1 (the AI places
  it, doesn't configure it — same stance as `widget-palette-scope.md`); a future slice could let a `[[widget]]`
  declare a config schema.
- **`genuiComponents` is names-only.** The verb returns `genui_component_names()`, not the full per-component
  prop schemas that `genui_catalog.json` carries. Enough for Slice A (an author knows genui exists and which
  components are legal); the channel-tenant genui author (umbrella Slice D) will want the schemas — widen the
  projection then, don't discover it late.

## Open questions

- ~~A consistency guard for catalog ↔ `WidgetView` drift?~~ **Resolved: yes, in this slice** — see Testing
  plan. Drift reproduces the exact bug this slice fixes; "add it if drift bites" was the wrong default.
- **Option-key enforcement on save** (the other half of the reported symptom) — explicitly out of this
  slice (see Non-goals); pick it up as its own follow-up once the catalog's option schemas have been
  exercised by a real AI author.
- **Verb name: `dashboard.catalog` vs `widget.catalog`?** Chose `dashboard.catalog` — it lives in the
  `dashboard.` host-native family (existing prefix, no new dispatcher family). Honest caveat: channel and
  app authors read this verb too (widgets are system-wide, per the umbrella), so the name is a dispatch
  convenience, not a scoping claim. If the umbrella's Slice D wants a `widget.catalog` façade composing
  this + `tools.catalog`, it layers on top — this verb doesn't move.
- **Per-widget version consumption.** The `version` field ships now but nothing reads it yet (no
  cell-stamping, no migration — dev mode). Resolve *when* the first widget gets a breaking v2: stamp
  `cell.plugin_version` at save and fold stale cells at render (the `schema_version` pattern).
- **App rendering.** This slice makes the catalog app-consumable but does not build the RN dashboard page.
  The intended contract is pinned above ("How the app renders a page") — catalog (`view` vocabulary) +
  `dashboard.get` (page document) + the gated data bridge, no app-side palette. Track the app-side renderer
  as its own `app/` task keyed off that contract; its first open questions are the RN `view`→component map
  and how the app tolerates an unknown option key (ignore, per the contract).

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
