# Frontend scope — graphics canvas (drawing pages, plant graphics, AI-drawn UI)

Status: scope (the ask). Promotes to `public/frontend/` once shipped.

We want a **free-form graphics surface** — the Tridium-Niagara "PX page" class of UI:
AHU/plant graphics, floor plans, mimic diagrams, eventually 3D buildings — that is *not*
the react-grid dashboard. A user can draw it by hand; an **AI agent can draw it for
them** (the 2026 default); an **extension can add new shape types** (a fan, a valve, a
chiller). Shapes bind to live data through the same contract dashboard widgets already
use. One design serves both asks: the "drawing page" and the "AI canvas widget" are the
same document with two producers.

## Goals

- A **scene document**: a declarative, versioned JSON scene graph (`scene:{id}` record,
  workspace-scoped) that fully describes a graphics page — shapes, transforms, styling,
  data bindings, actions.
- A **canvas view** (`view: "canvas"`) rendered through the shipped v2 widget contract:
  works as a dashboard cell, as a full-bleed "graphics page" (a dashboard with one cell),
  and as a channel rich-response.
- A **manual editor**: palette of shape types, drag-to-place, select/move/resize/rotate,
  a property rail, zoom/pan, undo — the flows-canvas interaction patterns on an SVG
  scene instead of a DAG.
- **AI drawing as a first-class producer**: the agent draws by calling `scene.*` MCP
  verbs (create + patch ops) against the *same durable record* the human edits; every
  open canvas repaints live via watch. No second "AI rendering" pipeline.
- **Extension shape types**: an extension registers custom shapes (render + prop schema)
  through the existing UI-federation mount, declared in `extension.toml` — the shape
  analog of `[[widget]]`.
- **Live data on shapes**: any shape prop can bind to a read source (`Target`/`source`)
  and any shape can carry an `action` (click a valve → `{tool, argsTemplate}`), reusing
  `fieldConfig`/`format.ts` for units/thresholds/prefs.
- A **3D path that doesn't fork the model**: the scene schema is renderer-agnostic so a
  later `react-three-fiber` renderer draws `3d.*` shape types from the same document.

## Non-goals

- **Not a whiteboard.** No freehand ink, sticky notes, or multiplayer cursors — this is
  data-bound plant graphics, not Miro. (If a whiteboard is ever wanted, it's a separate
  scope.)
- **Not phase-1 3D.** The schema reserves room (z, `3d.*` namespace); no 3D renderer
  ships in the first slices.
- **Not a CAD/BIM tool.** Floor plans are drawn shapes or a background image/SVG asset,
  not IFC imports.
- **Not a new streaming UI protocol.** No A2UI wire protocol, no SSE "render channel" —
  agent drawing is record edits + the existing watch feed (see rejected alternatives).
- **Not a replacement for the grid dashboard.** The grid stays the default for
  tile-of-widgets pages; canvas is for spatial/diagrammatic pages.

## Intent / approach

**The key idea: the scene document is the single source of truth, and everything —
human editor, AI agent, extension — is just a writer or renderer of that document.**

```
scene:{id} (SurrealDB, workspace-walled)
   ▲ scene.create/update/apply (MCP verbs)          ▼ scene.get / scene.watch
   │                                                │
   ├── manual editor (palette + property rail)      └── <CanvasView>  (SVG renderer)
   ├── AI agent (patch ops via scene.apply)              ├─ built-in shapes
   └── import/seed (a template AHU page)                 └─ ext shapes (federation registry)
```

A scene is a flat map of shapes (ids referencing ids — the same flat-list model A2UI
and the flows graph use, which is easy for an LLM to patch incrementally):

```jsonc
{
  "v": 1,
  "canvas": { "w": 1920, "h": 1080, "bg": { "asset": "file:floorplan-l2.svg" } },
  "shapes": {
    "sf1": {
      "type": "hvac.fan",                    // built-in or "ext:{extId}/{shape}"
      "t": { "x": 420, "y": 310, "w": 96, "h": 96, "r": 0 },   // z reserved for 3D
      "props": { "label": "SF-1" },
      "bind": {                               // prop ← data, same Target vocab as v3 cells
        "running": { "source": { "tool": "series.latest", "args": { "series": "ahu1.sf1.status" } } },
        "speed":   { "source": { "tool": "series.latest", "args": { "series": "ahu1.sf1.speed" } },
                     "fieldConfig": { "unit": "percent" } }
      },
      "action": { "tool": "flows.inject", "argsTemplate": { "flow": "ahu1", "port": "sf1_cmd" } }
    },
    "duct1": { "type": "shape.path", "t": { "x": 0, "y": 0 }, "props": { "d": "M420 358 H 760", "stroke": "duct" } },
    "lbl1":  { "type": "shape.text", "t": { "x": 420, "y": 420 }, "props": { "text": "Supply Fan" } }
  },
  "order": ["duct1", "sf1", "lbl1"]           // paint order (z-index)
}
```

**Renderer: our own SVG scene renderer, not tldraw.** Built-in shape primitives
(`shape.rect|ellipse|path|line|text|image`) plus a domain starter set (`hvac.*` pipes,
ducts, fans, valves, gauges) render as React SVG components; extension shapes are React
components resolved from the federation registry. SVG gives us DOM events, CSS theming
(token-bound, per `ui-standards-scope.md`), crisp zoom, and lets an extension shape be
an arbitrary React component. Editor interaction (palette drag-on, selection, transform
handles, config rail, undo buffer) ports the proven patterns from
`ui/src/features/flows/FlowCanvas.tsx` / `Palette.tsx` / `NodeConfigPanel.tsx`.

**AI drawing = editing the record.** The agent gets `scene.create` and `scene.apply`
(a bounded batch of RFC-6902-style ops over `/shapes/*` and `/order`) as MCP tools plus
a `skills/graphics-canvas/SKILL.md` teaching the shape catalog. Because every open
canvas holds a `scene.watch`, the user *sees the agent draw* shape-by-shape — streaming
UX without a streaming protocol. A `normalize + validate` layer (unknown type → reject
with the catalog; missing transform → default) absorbs LLM sloppiness before anything
touches the store — the pattern proven by Awaken's `normalize_args`.

**Page vs widget resolves to "both, one mechanism".** `canvas` is a v2 `view`; a
dashboard cell hosts it like any widget. A "graphics page" is a dashboard whose single
cell is a full-bleed canvas — deep-linkable via `routing-scope.md`, shareable via the
existing dashboard authz. Extensions that mount their own dashboards get canvas for
free. A channel rich-response with `view:"canvas"` renders a scene inline (source =
`scene.get`, or small inline `data`).

**Rejected alternatives:**

- **tldraw** — closest off-the-shelf editor, but rejected: (a) its license requires the
  watermark or a paid business license; (b) it brings its own reactive store/persistence
  model, which fights rule 2 (one datastore) and our undo journal; (c) custom shapes are
  registered at editor mount and its `ShapeUtil` API churns — a bad foundation for a
  frozen extension contract; (d) it's a freeform whiteboard, and the data-binding layer
  (the whole point) would be ours anyway. We'd be maintaining a fork-shaped integration
  for the ~20% we use.
- **Excalidraw** — MIT, but hand-drawn aesthetic, same no-binding gap, weaker custom
  shape story. Wrong genre.
- **Adopting the Awaken `awaken-ext-generative-ui` crate / A2UI v0.8 wire protocol** —
  evaluated against the clone at `/tmp/awaken`. The crate is solid (MIT/Apache-2.0,
  well-tested) but tightly coupled to the Awaken runtime's plugin/tool/sink traits, and
  its component catalog is *fixed*, not extension-pluggable — the exact gap we need
  closed. Its A2UI is the a2ui.org v0.8 spec (not Google's), aimed at streaming
  *form/card chat UI*, which our channels rich-responses scope already covers with the
  v2 widget contract. **We adopt its patterns** — flat id-referenced component maps,
  JSONL/JSON-patch incremental updates, arg normalization before validation — and skip
  the dependency. (Same verdict shape as `agent-run/`: ideas reviewed, framework
  rejected.)
- **A live SSE "draw channel" instead of record edits** — rejected: it would be motion
  carrying state (rule 3), the drawing would evaporate on refresh, and we'd need a
  second persistence step anyway. Durable-record-plus-watch gives replay, undo, audit,
  and multi-viewer sync for free.
- **`<canvas>`/WebGL 2D renderer** — rejected for phase 1: loses DOM events, CSS
  tokens, and React-component extension shapes. Revisit only if a scene with thousands
  of shapes actually janks (measure first).

## How it fits the core

- **Tenancy / isolation:** `scene:{id}` records are workspace-scoped like `dashboard:{id}`;
  every `scene.*` verb resolves the workspace from the signed token. Isolation tested.
- **Capabilities:** one cap per verb (`mcp:scene.get:call`, `…create`, `…update`,
  `…apply`, `…delete`, `…watch`). Shape *bindings* execute under the **viewer's** grant
  through the host-mediated bridge (same leash as dashboard cells — a scene authored by
  an admin does not widen a viewer). Deny path: a bound shape whose source tool the
  viewer lacks renders its no-access state, not an error page.
- **Placement:** either — scenes are records; a canvas over an edge node's series works
  offline like any dashboard. No `if cloud`.
- **MCP surface (§6.1):** `scene.list|get|create|update|delete|watch` per
  `core/resource-verbs-scope.md`, plus `scene.apply` — a **small, bounded, synchronous
  batch** (cap ~200 ops per call, per-op results, rev-checked) — a drawing gesture or
  one agent step, not an import. A bulk import/template-instantiation that could run
  long would be an `lb-jobs` job, deferred until a caller exists.
- **Data (SurrealDB):** one new `scene` table (doc + `rev` + timestamps). Cell references
  it as `options.sceneId`; small inline scenes allowed in rich-responses only. State only.
- **Bus (Zenoh):** `scene.watch` rides the existing store-watch/SSE path — change
  notifications (motion), document truth in the store (state). Fire-and-forget class.
- **Sync / authority:** same authority story as dashboards. Concurrent writes are
  rev-checked (`scene.apply` carries `expect_rev`; stale → retry with fresh doc), the
  same discipline as `write_locked` in flows.
- **Stateless extensions:** an extension shape is a pure `(props, boundValues) → SVG/JSX`
  render + a prop JSON-Schema; no instance state. Hot-reload safe.
- **No mocks:** tests seed real scenes into the real store, render through the real
  `WidgetHost`, and exercise `scene.*` over the real gateway (`pnpm test:gateway`).
- **SDK/WIT impact — flagged loudly:** this extends the *frontend* federation contract
  (an `[[shape]]` manifest block + a `shapes` export on the remote, versioned per
  `widget-kit-scope.md`'s mount-context versioning). The WASM WIT ABI is untouched.
- **Skill doc:** yes — `skills/graphics-canvas/SKILL.md` (the agent-drivable surface:
  the shape catalog, `scene.create`/`scene.apply` recipes, a worked "draw an AHU" run).
  The implementing session writes it from a live run.

## Extension shape contract (the `[[widget]]` sibling)

```toml
[[shape]]
entry  = "remoteEntry.js"
prefix = "hvac-pro"                   # types register as ext:{extId}/{prefix}.{name}
scope  = ["series.latest"]            # tools the shapes' bindings may call
```

The remote exports `shapes: Record<string, ShapeDef>` where
`ShapeDef = { schema, defaults, render(props, bound, ctx), anchors? }`. Trusted
(allow-listed key) publishers render in-process; untrusted shapes follow the
`ui-federation-scope.md` sandbox tier (phase 1 supports **trusted only** — an
iframe-per-shape is unworkable, so untrusted shape sandboxing is an open question).
A scene referencing an uninstalled shape type renders a labeled placeholder box — a
scene must never hard-fail on a missing extension.

## Example flow — "AI, draw me the AHU-1 graphic"

1. User (in a channel or the canvas's "draw with AI" rail): *"Draw AHU-1: outside-air
   damper, filter, supply fan SF-1 bound to `ahu1.sf1.*`, cooling coil, duct run."*
2. The agent (skill-guided) calls `scene.create {name:"AHU-1"}` → gets `scene:{id}`,
   then 4–6 `scene.apply` batches: ducts first, then equipment shapes with `bind`
   blocks, then labels and `order`.
3. The host checks capability + workspace per call; invalid shape types bounce back
   with the catalog (normalize/validate), and the agent self-corrects.
4. The user's canvas holds `scene.watch` — the page draws itself shape-by-shape as the
   agent works.
5. The agent answers in-channel with `render:{view:"canvas", source:{tool:"scene.get",
   args:{id}}}` — the graphic is live in the thread.
6. User drags SF-1 two grid units left, retitles a label (property rail), hits save →
   `scene.apply` with `expect_rev`; the undo journal records the reverse ops.
7. Clicking the fan fires its `action` through the bridge under the viewer's grant —
   deny renders as the shape's no-access state.

## Testing plan

Per `scope/testing/testing-scope.md` — mandatory categories:

- **Capability deny:** `scene.apply` without `mcp:scene.apply:call` → deny; a bound
  shape whose viewer lacks the source tool renders no-access (WidgetHost test).
- **Workspace isolation:** scene created in ws A invisible to `scene.get/list` in ws B;
  watch feeds don't leak across the wall.
- **Unit:** scene schema validate/normalize (unknown type, missing transform, bad op
  path); `scene.apply` rev-conflict; painter's-order ops.
- **Integration (real store/gateway, `pnpm test:gateway`):** create → apply → get
  round-trip byte-stable (no v2/v3-style shadowing — see
  `debugging/` flow-read binding trap); watch delivers applied ops; agent-path test
  drives `scene.*` through the real MCP surface and asserts the rendered scene.
- **Extension:** a test extension registers one `[[shape]]`; scene renders it through
  the real federation loader; uninstall → placeholder, not crash.
- **Hot-reload:** re-publish the shape extension; open canvas re-resolves without state loss.

## Risks & hard problems

- **Binding fan-out.** A plant page can bind 200+ props. Per-shape polling won't fly:
  the canvas must **multiplex** — collect all sources, dedupe, one batched
  read/watch per tool, fan values out to shapes. This is the hardest engineering in
  phase 1 and should be built as a reusable hook (`useSceneData`, the multi-target
  sibling of `usePanelData`).
- **Schema longevity.** The scene doc will outlive renderer rewrites (that's the point —
  it's what makes 3D a new renderer, not a new format). `v` field + additive-only
  evolution + a migration note per bump. Getting `t` (transform) right now — including
  reserved `z`/`sz` — is cheap; retrofitting is not.
- **LLM-emitted garbage.** Validation must be total (every op checked before any
  applied) and error messages must be *teaching* (return the catalog + the failing op),
  or agent drawing will feel broken. Budget real time on the normalize layer.
- **Editor scope creep.** Transform handles, snapping, alignment guides, grouping,
  multi-select… each is a week. Phase the editor ruthlessly: place/move/resize/delete +
  property rail first; rotation/grouping/guides later.
- **Untrusted extension shapes** have no good sandbox story yet (in-process SVG is the
  model). Trusted-tier only until solved.
- **3D optimism.** r3f over the same doc is credible but the *authoring* UX for 3D is a
  different discipline. Treat phase-3D as scene-schema-compatible *viewing* (a 3D
  building with bound status colors) before any 3D *editing*.

## Phases

1. **Scene plane** — `scene` table, `scene.*` verbs + caps, schema validate/normalize,
   read-only `<CanvasView>` (built-in shapes, bindings via `useSceneData`, actions),
   `view:"canvas"` wired into WidgetHost + rich-responses.
2. **Manual editor** — palette, place/move/resize/delete, property rail (schema-driven,
   like `NodeConfigPanel`), paint order, undo, save with rev.
3. **AI drawing** — `scene.apply` agent recipes, `skills/graphics-canvas/SKILL.md`,
   teaching-error validation, live watch repaint, "draw with AI" rail.
4. **Extension shapes** — `[[shape]]` manifest, remote `shapes` export, catalog merge,
   placeholder-on-missing, the hvac starter set possibly dogfooded *as* an extension.
5. **3D viewer** — r3f renderer, `3d.*` types, gltf assets via `files/`; editing deferred.

## Open questions

- Does the **hvac starter shape set** ship built-in or as a first-party extension
  (dogfooding `[[shape]]` from day one)? Leaning extension — it proves the contract.
- `scene.watch` transport: reuse the dashboard `refreshKey` polling first, or land
  store-watch SSE in phase 1? (Phase 1 can poll; watch is needed by phase 3 for the
  draw-live UX.)
- Do scenes participate in the **dashboard share/authz model** directly, or inherit
  from the referencing dashboard? (Leaning: own record, own share bits — a scene can
  back several dashboards.)
- Snap grid + anchor points (`anchors?` on ShapeDef) for connecting ducts/pipes: phase 2
  or phase 4? Connectors-that-stay-attached is the feature that makes it feel like
  Niagara, and it may deserve its own slice.
- Background floor-plan assets: image/SVG upload via `files/`/`document-store/` — which
  lands first, and does canvas block on it? (Phase 1 can ship with `bg` optional.)

## Related

- `frontend/dashboard-scope.md`, `frontend/dashboard-widgets-scope.md`,
  `frontend/widget-kit-scope.md` — the v2/v3 cell contract, `fieldConfig`, federation
  mount versioning this builds on.
- `extensions/ui-federation-scope.md` — trust tiers + bridge the shape registry extends.
- `channels/channels-rich-responses-scope.md` — `view:"canvas"` as one more rich view.
- `core/resource-verbs-scope.md` — the `scene.*` verb grammar.
- `undo/` — reverse-ops journal for scene edits; `frontend/routing-scope.md` — deep links.
- README `§3` (rules 2/3/5/6), `§6.1` (API shape), `§6.5` (dispatch chokepoint).
- Evaluated: Awaken generative-UI (`/tmp/awaken`, MIT/Apache-2.0) — patterns adopted,
  dependency rejected (see Intent). tldraw/Excalidraw rejected (see Intent).
- `skills/graphics-canvas/SKILL.md` — owned by the implementing session (phase 3).
