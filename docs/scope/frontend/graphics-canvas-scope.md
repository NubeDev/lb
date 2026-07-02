# Frontend scope — graphics canvas (plant graphics, floor plans, 3D — one engine, 100% extension)

Status: scope (the ask); **phases 1–2 are now scoped for build** as the `thecrew`
extension — see `rust/extensions/thecrew/docs/thecrew-extension-scope.md` (the
playground proved the look/builder and moved to `rust/extensions/thecrew/`; the
extension keeps that id rather than `graphics-canvas`). Promotes to
`public/frontend/` once shipped. Per the `control-engine` precedent, the
implementation docs **co-locate with the extension** — the core stays canvas-ignorant.

We want a **free-form graphics surface** — the Tridium-Niagara "PX page" class of UI:
AHU/plant graphics, floor plans, mimic diagrams, and 3D buildings — that is *not* the
react-grid dashboard. A user can draw it by hand; an **AI agent can draw it for them**
(the 2026 default); new equipment symbols arrive as **data (symbol packs), not code**.
Shapes bind to live data through the same contract dashboard widgets already use.
Three hard decisions frame this scope: **one engine** (three.js via
`@react-three/fiber` — flat scenes *and* 3D from the same document, never built twice),
**one deliverable posture** (a **100% UI extension**, zero core code), and **one source
of truth** (a declarative scene document the human editor, the AI, and the renderer all
share).

## Goals

- A **scene document**: a declarative, versioned JSON scene graph (workspace-scoped
  record) that fully describes a graphics page — shapes, transforms, styling, data
  bindings, actions. Renderer-agnostic and dimension-agnostic: a "2D" AHU page and a 3D
  building are the same format (flat scenes just keep `z = 0` and an orthographic
  camera).
- **One engine: three.js via `@react-three/fiber` + `@react-three/drei`.** Flat plant
  graphics render under an orthographic top-down camera (it looks and edits like a 2D
  drawing tool); 3D buildings are the same scene with a perspective camera. One
  library, one renderer, one editor — 3D is a camera setting, not a second build.
- A **manual editor** built on the r3f ecosystem's chrome, not from zero: drei
  `TransformControls` (move/rotate/scale gizmos), `MapControls`/`OrbitControls`
  (pan/zoom/orbit), per-mesh pointer events (raycast hit-testing is the library's job),
  drei `Html`/`Text` for labels and overlays, `Grid` + snapping. We write the palette,
  the property rail, and box-select.
- **100% UI extension.** A federated `[ui]` page (the full-bleed graphics page) +
  `[[widget]]` entries (the dashboard-cell viewer) from one `extension.toml`. **No new
  core verbs, tables, capabilities, or WIT surface.** Persistence and data all ride
  shipped core tools through the host-mediated bridge.
- **AI drawing as a first-class producer**: the agent edits the *same scene document*
  through the *same shipped tools* the editor uses; every open canvas re-renders live.
  No AI-specific rendering pipeline.
- **Symbol packs — new equipment as data, not code**: a pack = GLTF models / SVG
  symbols + a prop/binding schema, uploaded as workspace assets. Installing a "chiller
  plant pack" requires no code deploy, works offline, and is something an AI can
  generate.
- **Live data on shapes**: any shape prop binds to a read source (`Target`/`source`
  vocabulary) and any shape can carry an `action` (`{tool, argsTemplate}`), reusing
  `fieldConfig`/`format.ts` for units/thresholds/prefs — under the **viewer's** grant.

## Non-goals

- **Not two renderers.** No SVG/Konva/React-Flow 2D path alongside three.js. One
  engine or none.
- **Not core code.** If a slice seems to need a core change, that's a finding to
  surface (and probably a generic primitive with its own scope), not something to
  smuggle in.
- **Not a whiteboard.** No freehand ink, sticky notes, or multiplayer cursors — this
  is data-bound plant graphics, not Miro.
- **Not CAD/BIM.** Floor plans are drawn shapes, a background image, or an imported
  GLTF — not IFC round-tripping.
- **Not a new streaming UI protocol.** No A2UI wire protocol, no SSE "draw channel" —
  agent drawing is document edits + the existing refresh/watch path.

## Intent / approach

**The key idea: one scene document, one engine, and everything — human editor, AI
agent, symbol pack — is just a writer of that document or an asset it references.**

```
scene document (SurrealDB via shipped verbs, workspace-walled)
   ▲ written by                                  ▼ read by
   ├── manual editor (palette + gizmos + rail)   └── <SceneCanvas> (r3f renderer)
   ├── AI agent (same tools, skill-guided)            ├─ built-in primitives
   └── template/import                                └─ symbol packs (GLTF/SVG assets)
```

A scene is a flat map of shapes (ids referencing ids — easy for an LLM to patch
incrementally, the model A2UI and the flows graph both validate):

```jsonc
{
  "v": 1,
  "camera": { "mode": "ortho-top" },            // or "persp" — the 2D/3D switch
  "bg": { "asset": "floorplan-l2.svg" },        // optional underlay (image/SVG/GLTF)
  "shapes": {
    "sf1": {
      "type": "hvac.fan",                        // primitive or symbol-pack type
      "t": { "x": 420, "y": 310, "z": 0, "r": 0, "s": 1 },
      "props": { "label": "SF-1" },
      "bind": {                                  // prop ← data, same Target vocab as v3 cells
        "running": { "source": { "tool": "series.latest", "args": { "series": "ahu1.sf1.status" } } },
        "speed":   { "source": { "tool": "series.latest", "args": { "series": "ahu1.sf1.speed" } },
                     "fieldConfig": { "unit": "percent" } }
      },
      "action": { "tool": "flows.inject", "argsTemplate": { "flow": "ahu1", "port": "sf1_cmd" } }
    },
    "duct1": { "type": "shape.path", "props": { "d": "M420 358 H 760", "style": "duct" } },
    "lbl1":  { "type": "shape.text", "t": { "x": 420, "y": 420 }, "props": { "text": "Supply Fan" } }
  }
}
```

**Why three.js (r3f) and not a 2D library.** The ask includes 3D buildings, and the
constraint is *one* library. Konva/fabric (canvas-2D) and any SVG approach dead-end at
3D — adopting one means building the renderer twice, the exact thing we refuse to do.
three.js through `@react-three/fiber` keeps every shape an ordinary React component
(`(props, boundValues) → JSX meshes`), which keeps symbol rendering, the property rail,
and future pluggability idiomatic. Flat mode is not a hack: an orthographic top-down
camera over `z=0` geometry *is* a 2D drawing surface — same gizmos, same hit-testing.
Costs accepted: a ~1 MB engine bundle (paid only by this extension — see below), and
WebGL text/lines needing drei's `Text`/`Line` rather than DOM. Rejected alternatives:

- **React Flow** — considered (already in-stack for flows) and rejected: its idiom is
  node-and-edge diagrams, and fighting it to *not* look like a flow chart — plus no 3D,
  ever — makes it the wrong base for this surface.
- **Konva / fabric.js / PixiJS** — good 2D editors/renderers, but 2D-only: choosing one
  forces the second build when 3D lands. Rejected on the one-engine constraint.
- **Babylon.js** — the other credible 3D engine, batteries included, but its React
  story is thin; r3f's component model is what makes shapes/symbols composable React.
- **GoJS / JointJS+** — commercial licenses, 2D-only. Out.
- **tldraw / Excalidraw** — whiteboards (watermark/paid license; hand-drawn genre; own
  store fighting rule 2). Wrong genre, and 2D-only.
- **Awaken's `awaken-ext-generative-ui` crate / A2UI v0.8 wire protocol** — evaluated
  against the clone at `/tmp/awaken`. Solid (MIT/Apache-2.0, tested) but coupled to the
  Awaken runtime, fixed catalog, and aimed at streaming chat-form UI (territory
  `channels-rich-responses` already covers). **Patterns adopted** — flat id-referenced
  maps, incremental patch updates, normalize-before-validate for LLM sloppiness —
  dependency rejected. (Same verdict shape as `agent-run/`.)
- **A live SSE "draw channel"** — rejected: motion carrying state (rule 3), evaporates
  on refresh, needs persistence anyway. Durable-document-plus-refresh gives replay,
  undo, audit, and multi-viewer sync free.

**Why 100% extension.** The core gains nothing canvas-shaped: no new verbs, no new
tables, no WIT change. The extension ships a federated `[ui]` page (full-bleed graphics
pages, deep-linkable per `routing-scope.md`) and `[[widget]]` canvas cells for
dashboards, both mounting the same `<SceneCanvas>` over the same bridge. A pleasant
consequence of the extension posture: **only this remote bundles three.js**, so there
is no shared-singleton/import-map problem in federation — the engine is an
implementation detail of one bundle. Precedent: `control-engine` (100% extension, core
CE-ignorant, docs co-located).

**Persistence with zero core additions.** Scene documents and symbol packs are
workspace **assets/documents** written and read through the shipped generic verbs the
manifest `scope` lists (the asset/document surface of `files/`/`document-store/`; exact
verb set is Open question 1). The bridge enforces capability ∩ workspace as it does for
every widget today. Concurrency uses the document layer's revision check (stale write →
reload, retry) — the flows `write_locked` discipline at the client.

**AI drawing = editing the document with the tools it already has.** No new protocol:
the agent (guided by `skills/graphics-canvas/SKILL.md`) reads the scene, applies
shape-map edits, and saves through the same asset verbs — each save re-renders every
open canvas. The skill carries the shape catalog + worked recipes; the extension's
loader **normalizes and validates** every document before render (unknown type →
labeled placeholder box, never a crash; the Awaken `normalize_args` lesson), so an LLM
mid-draft can't take down a page. A channel rich-response embeds a graphic as
`render:{view:"ext:graphics-canvas/scene", options:{sceneId}}` — the shipped ext-widget
path, nothing new.

**Symbol packs instead of code plugins.** New equipment types are **data**: a pack
manifest (JSON: type names, prop schemas, binding slots, anchor points) + GLTF models
(3D) and/or SVG symbols (flat), stored as workspace assets. The palette merges
installed packs; scenes reference `pack.type` names; a missing pack renders labeled
placeholders. This is deliberately chosen over code-level shape plugins
(extension-to-extension code composition is unsolved surface) — and it means an AI can
*generate a symbol pack* as readily as a scene. Code-level plugins stay an explicit
non-slice until a symbol pack provably can't express something.

## How it fits the core

- **Tenancy / isolation:** scenes and packs are workspace assets; every read/write goes
  through the bridge, workspace resolved from the signed token. Nothing canvas-specific
  to enforce — the wall is the existing one. Isolation still tested (below).
- **Capabilities:** the manifest `scope` lists the tools the page/widget may call
  (asset read/write + the read tools bindings use, e.g. `series.latest`). Bindings and
  actions execute under the **viewer's** grant — an admin-authored scene never widens a
  viewer. Deny path: a bound shape renders its no-access state; a denied save surfaces
  the deny honestly.
- **Placement:** either — assets + bridge work identically on edge and cloud; a canvas
  over a local node's series works offline. No `if cloud`.
- **MCP surface:** **none added.** The extension consumes shipped verbs; the API-shape
  checklist (§6.1) is satisfied by the document layer it rides. If document-granular
  patch ops or a watch feed prove necessary (Open questions 1–2), that's a *generic*
  document-layer ask, scoped there — not a canvas verb.
- **Data (SurrealDB):** no new tables. State = scene/pack assets in the store.
- **Bus (Zenoh):** nothing canvas-specific. Live values ride the existing widget
  refresh/watch path; live co-editing (if ever) rides a generic document watch.
- **Sync / authority / secrets:** inherited from the asset layer; no secret material.
- **Stateless extension:** the page/widget holds no durable state — pure render of
  (scene doc, bound values). Hot-reload safe by construction.
- **No mocks:** tests seed real scenes/packs into the real store and exercise the real
  gateway + federation loader (`pnpm test:gateway`); the ext widget renders through the
  real `WidgetHost`.
- **SDK/WIT impact:** **none.** Frozen `RemoteMount` contract, existing `[ui]`/
  `[[widget]]` manifest blocks, no WASM ABI change. (This is the payoff of the
  100%-extension + symbol-packs-as-data decisions.)
- **One responsibility per file:** the extension's `ui/` follows FILE-LAYOUT — renderer
  (`scene/` shape components, one per primitive), editor (`edit/` one gesture per
  file), binding (`data/useSceneData.ts`), pack loading (`packs/`).
- **Skill doc:** yes — `skills/graphics-canvas/SKILL.md` (agent-drivable surface: the
  scene schema, shape catalog, read-modify-save recipes, a worked "draw an AHU" run).
  Written by the implementing session from a live run.

## Example flow — "AI, draw me the AHU-1 graphic"

1. User (in a channel or the canvas's "draw with AI" rail): *"Draw AHU-1: outside-air
   damper, filter, supply fan SF-1 bound to `ahu1.sf1.*`, cooling coil, duct run."*
2. The agent (skill-guided) creates a scene asset, then saves it in a few
   read-modify-write steps: ducts and underlay first, then equipment shapes with
   `bind` blocks, then labels.
3. Every call crosses the host chokepoint: capability + workspace checked; a malformed
   shape is caught by the extension's validator at render and shown as a labeled
   placeholder — the skill teaches the agent to re-read and fix.
4. The user's open canvas re-renders on each save — the page draws itself in steps.
5. The agent answers in-channel with the ext-widget render payload — the graphic is
   live in the thread.
6. User drags SF-1 with the gizmo, retitles a label in the property rail, saves —
   revision-checked write; the reverse edit lands in the undo journal.
7. Clicking the fan fires its `action` through the bridge under the viewer's grant;
   deny renders as the shape's no-access state.
8. Later: the same scene, `camera: "persp"`, extruded walls from the floor plan, fan
   status colored on the 3D model — same document, same engine, no second build.

## Testing plan

Per `scope/testing/testing-scope.md` — mandatory categories:

- **Capability deny:** widget whose viewer lacks a binding's source tool → no-access
  state; scene save without the write grant → surfaced deny (real gateway).
- **Workspace isolation:** scene/pack assets created in ws A invisible from ws B
  through the same verbs; a ws-B viewer of a shared dashboard cannot reach ws-A series.
- **Unit:** scene validate/normalize (unknown type, missing transform, cyclic refs,
  bad bind path); pack-manifest validation; camera-mode mapping.
- **Integration (real store/gateway, `pnpm test:gateway`):** create → save → reload
  round-trip byte-stable (the flow-read empty-source shadowing trap is the cautionary
  tale); revision-conflict retry; agent-path test drives the full read-modify-save loop
  through the real MCP surface and asserts the resulting document.
- **Federation:** the widget loads through the real `remoteEntry.js` path and renders a
  seeded scene in `WidgetHost`; missing symbol pack → placeholder, not crash.
- **Hot-reload:** re-publish the extension; open canvas remounts cleanly (stateless).
- **Render smoke:** headless WebGL is flaky in CI — assert on the *scene graph built
  from the document* (r3f test renderer), not pixels; pixel/screenshot checks stay a
  local `verify` step, stated here so CI doesn't silently skip rendering.

## Risks & hard problems

- **Editor-from-engine gap.** drei gives gizmos, controls, and hit-testing, but
  box-select, snapping, alignment, grouping, and undo wiring are ours, in WebGL rather
  than DOM. This is the honest cost of the one-engine decision — budget it. Phase
  ruthlessly: place/move/scale/delete + property rail first.
- **Binding fan-out.** A plant page binds 200+ props. Per-shape polling won't fly: one
  `useSceneData` multiplexer — collect, dedupe, batch per tool, fan out. The hardest
  engineering in phase 1.
- **WebGL contexts per cell.** Browsers cap live WebGL contexts (~8–16); a dashboard
  of many canvas cells will hit it. Mitigate: render-on-demand + context release for
  offscreen cells; a shared-canvas/portal approach if real dashboards prove it.
- **Text and 2D crispness.** WebGL text (drei `Text`/SDF) and thin lines need care to
  match DOM crispness on a flat page; symbol SVGs rasterize at zoom. Prototype the flat
  look early — it's the first thing a Niagara user judges.
- **LLM-emitted garbage.** Validation must be total and errors must *teach* (return
  the catalog + the failing shape) or AI drawing feels broken. Whole-doc
  read-modify-write also risks clobbering concurrent edits — revision checks are not
  optional.
- **Document-layer dependency.** The zero-core-additions posture leans on the shipped
  asset/document surface; if `document-store/` phases land late, phase 1 needs an
  honest interim (Open question 1) — not a private table.
- **Symbol-pack schema longevity.** Packs are the public contract other people author
  (and AIs generate); version the manifest from day one, additive-only.

## Phases

1. **Viewer** — scene schema + validate/normalize, `<SceneCanvas>` (r3f, ortho flat
   mode), built-in primitives, `useSceneData` binding multiplexer, actions,
   `[[widget]]` cell + `[ui]` page, seeded demo scene.
2. **Editor** — palette, gizmo transforms, property rail (schema-driven), box-select,
   snap grid, save with revision check, undo wiring.
3. **AI drawing** — `skills/graphics-canvas/SKILL.md`, teaching-error validation,
   draw-with-AI rail, channel rich-response embed.
4. **Symbol packs** — pack manifest + loader, palette merge, an `hvac` starter pack
   (dogfooded as the first pack), placeholder-on-missing.
5. **3D** — perspective camera, GLTF building/underlay import, extrusion helpers,
   status-bound materials; 3D *viewing* before 3D editing.

## Open questions

1. **Which shipped verbs persist scenes?** — **ANSWERED (2026-07-02):** the
   `assets.*` surface is shipped: scenes = `assets.put_doc`/`get_doc`/`list_docs`,
   packs = `assets.put_asset`/`get_asset`. No `kv.*` fallback needed. **Finding:**
   `put_doc` has no revision check today (last-writer-wins) — the revision-checked
   save this scope assumed is a generic `document-store/` ask; the interim mitigation
   is in `rust/extensions/thecrew/docs/thecrew-extension-scope.md` §Risks.
2. **Live repaint transport:** widget `refreshKey` polling is enough for phases 1–2;
   does phase 3's watch-the-AI-draw UX justify asking for a *generic* document watch
   feed (its own scope), or is 2s polling honestly fine?
3. **Where does the scene-edit undo land** — the core undo journal (via the document
   layer's reverse ops) or an editor-local stack persisted in the scene asset's
   history? Leaning core journal, via whatever `document-store/` ships.
4. **Flat-mode interaction defaults:** snap grid size, rotation steps, and whether
   flat mode locks orbit entirely (leaning yes — flat pages should never accidentally
   tilt).
5. **Pack authoring UX:** hand-written JSON + uploaded GLTF/SVG first, or a minimal
   "new pack" wizard in phase 4? (Leaning hand-written + a documented example pack.)

## Related

- **`rust/extensions/thecrew/`** — the extension this ships as. Began as the
  standalone UI/UX test bed (scope set at `rust/extensions/thecrew/docs/`), which
  proved the look + builder feel on the same engine and scene schema; the lift into
  a real extension (phases 1–2 here) is scoped at
  `rust/extensions/thecrew/docs/thecrew-extension-scope.md`.
- `extensions/ui-federation-scope.md` — the `[ui]`/`[[widget]]` mount + bridge this
  rides; `rust/extensions/control-engine/docs/control-engine-scope.md` — the
  100%-extension precedent (core ignorant, docs co-located).
- `frontend/dashboard-widgets-scope.md`, `frontend/widget-kit-scope.md` — the ext-widget
  cell contract, `fieldConfig`, mount versioning.
- `channels/channels-rich-responses-scope.md` — the in-channel embed path.
- `document-store/document-store-scope.md`, `files/` — the persistence surface (Open
  question 1); `undo/` — Open question 3; `frontend/routing-scope.md` — deep links.
- README `§3` (rules 1–7), `§6.12` (file store).
- Evaluated: Awaken generative-UI (`/tmp/awaken`, MIT/Apache-2.0) — patterns adopted,
  dependency rejected. React Flow, Konva/fabric/Pixi, Babylon, tldraw/Excalidraw,
  GoJS/JointJS+ — rejected (see Intent).
- `skills/graphics-canvas/SKILL.md` — owned by the implementing session (phase 3).
