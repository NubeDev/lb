# Data Studio scope — explore any data, build a panel (manual or AI), save it to the library

Status: **v2 SHIPPED** (multi-pane workbench + extracted panel-kit lib). Promoted to
`public/frontend/data-studio.md`; session `sessions/frontend/data-studio-v2-workbench-session.md`.
**v2 supersedes the v1 layout below** — see "v2: multi-pane workbench + extracted panel-kit lib". Builds **entirely on
shipped surfaces** (the `source-picker` package, the `viz.query` render path, the ONE `PanelEditor`
incl. its GenUI "AI widget" tab, `agent.invoke`, and the `panel.*` library asset) — a new **composition**
surface, not a new data/render substrate.

Today a user can only build a chart *inside a dashboard grid*: open a dashboard → Add panel → the editor.
There is no place to **explore data across every source type** (series, Direct SurrealDB, flows, installed
extensions, federation datasources) and then **turn what you found into a reusable panel** — manually or
with AI — without first having a dashboard to hang it on. Library panels (shipped) gave us the standalone
*asset*; this scope gives us the *workbench* that produces them.

## Goals

- **A `data-studio` surface** (`/t/$ws/data-studio`) — its own page in the shell, cap-gated like every
  core route, member-level (any session that can read data can explore).
- **Explore any source, one path.** A left **source picker** (the shipped `@nube/source-picker` +
  `useSourcePicker` loaders — series / Direct SurrealDB / flows / installed-extension tools / federation
  datasources) drives a live **preview** rendered through the **shipped render path** (`WidgetHost` →
  `usePanelData` → `viz.query` → the viz bridge — **no parallel renderer, no parallel query**), toggleable
  between **Table / Chart / JSON** views so the user can inspect the real shape of the data. A range picker
  + `?var-` selections ride the same shipped variable model.
- **Build a panel — the SAME editor.** "Build / edit panel" opens the **shipped `PanelEditor`** on the
  explored draft cell — the identical Grafana-style surface a dashboard uses (Query / Transform / Field /
  Overrides), which **already includes the GenUI "AI widget" tab** (`agent.invoke` authoring). So *manual
  editing*, *AI/genui authoring*, and *the full option surface* are ONE reused component — rule 9, no fork.
- **Save to the library.** The primary output is a **library panel** (`panel.save`, the shipped asset): the
  studio's "Save as library panel" persists the built spec as a `panel:{id}`, immediately reusable on any
  dashboard (a ref cell) and renderable standalone (`/t/$ws/panel/{id}`). No dashboard is required to
  produce a panel — that is the whole point of the workbench.
- **AI to query + build.** The AI paths the user asked for are the **shipped ones, surfaced here**: the
  GenUI "AI widget" tab drives `agent.invoke` under the caller's principal to *design a widget from the
  data* (the genui skill's data-discovery choreography), and the explore preview lets the user *see* what
  the agent (or they) will bind. (A persistent conversational data-Q&A agent like the channel agent is a
  **named follow-up** — the authoring agent + the explorer cover "use AI to query the data AND make
  widgets" in v1 without a second agent runtime in the page.)

## Non-goals

- **No new data path, renderer, or editor.** Everything routes through `viz.query`/`usePanelData`/
  `WidgetHost`/`PanelEditor`/`@nube/source-picker` verbatim. If the studio needs a rendering or query
  behavior the dashboard doesn't have, that is a change to the *shared* component, not a studio fork.
- **No new host verbs, caps, or tables.** The studio composes `series.*`/`store.query`/`federation.query`/
  `flows.*`/`ext.*` (explore, each under its own shipped cap), `viz.query` (render), `agent.invoke`
  (AI authoring), and `panel.save`/`panel.list` (library). Nothing new server-side.
- **Not the Extension Studio.** `features/studio/` is the SDK scaffold/build/publish wizard — unrelated;
  this is a *data/panel* workbench. Distinct surface key (`data-studio`), distinct route.
- **Not a dashboard.** The studio produces panels; placing them on a grid is the dashboard's job (Add
  library panel, shipped). No grid/layout authoring here.
- **No conversational data agent in v1** — the authoring agent (genui tab) + the explorer are v1; a
  channel-style Q&A agent bound to the studio is a named follow-up (it would reuse the same `agent.invoke`
  + run-stream seam the channel agent uses).

## How it fits the core

- **Capabilities (rule 5):** the surface shows for a member who can read data (`series.list`); every
  explore call re-checks the source tool's own cap under the caller (the `viz.query` per-target leash,
  unchanged); "Save as library panel" needs `mcp:panel.save:call`; the AI tab needs
  `mcp:agent.invoke:call` (+ the genui skill). Deny path: a denied source renders the standard
  `usePanelData` denied/empty state (not a studio-specific UX); no `panel.save` → no save-as-library
  affordance (the palette-gate precedent), host re-checks regardless.
- **Tenancy / isolation (rule 6):** nothing new holds state — the studio is a composition of walled verbs;
  the built panel is a workspace-scoped `panel:{id}`. Inherited + tested (the panel asset's ws-isolation).
- **State vs motion / one datastore / symmetric (rules 1–3):** no new state or bus; live previews ride the
  shipped series/flow watch paths through `usePanelData`. No `if cloud`.
- **Core knows no extension (rule 10):** the picker lists extension tools/datasources as **opaque**
  entries via `ext.list`/`datasource.list` (the shipped source-picker loaders) — no branch on an ext id.
- **MCP surface:** **consumes only.** No new verbs.
- **One responsibility per file:** `features/data-studio/` — `DataStudioView.tsx` (layout/compose),
  `useExploreDraft.ts` (the picked-source → draft-cell state), each ≤400 lines; the heavy lifting is the
  reused components.

## Decisions (owner directive: "amazing UX, reuse the same common code, best long-term")

- **The preview IS `WidgetHost` on a draft cell** (not a bespoke results grid) — so Table/Chart/JSON are
  the shipped `views/*` and a "what you see is what the panel renders" guarantee is structural.
- **Build/edit IS the shipped `PanelEditor`** — manual + AI/genui + the full option surface, one modal,
  zero duplicated editor code. The studio owns only the draft state + the save-as-library action.
- **The primary output is a library panel** (`panel.save`), not an inline dashboard cell — the studio is
  the panel factory; dashboards consume its output via the shipped ref-cell flow.
- **AI in v1 = the shipped GenUI authoring tab**, not a second agent runtime; a conversational data agent
  is a named follow-up on the same `agent.invoke`/run-stream seam.
- **Surface, not a dashboard tab** — a dedicated `/t/$ws/data-studio` route (deep-linkable, nav-targetable)
  because "explore + author a reusable panel" is a distinct task from "arrange a dashboard".

## Testing plan

Per `scope/testing/testing-scope.md` — real store/caps/gateway, seeded records (rule 9):

- **Explore (real gateway):** pick a seeded series/store source → the preview renders real rows through
  `viz.query` (Table view), and the Chart/JSON toggles render the same data — proving the ONE render path.
- **Build → save as library (headline):** from an explored source, open the `PanelEditor`, save as a
  library panel → `panel.get` round-trips the built spec; the panel then reuses on a dashboard (ref cell)
  and renders standalone — reusing the shipped panel tests' guarantees.
- **Capability-deny (mandatory):** no `panel.save` → no save-as-library affordance + the verb denied
  server-side; a denied source in the preview renders the standard denied/empty state, not a leak.
- **Workspace-isolation (mandatory):** the built panel is ws-scoped (inherited from the panel asset;
  asserted).
- **Reuse guard:** the studio imports `PanelEditor`/`WidgetHost`/`SourcePicker`/`savePanel` — a test asserts
  no parallel renderer/editor exists (the components are the shipped ones).

## v2: multi-pane workbench + extracted panel-kit lib (owner directive)

v1 (one picker + one preview + a modal editor) is not a workbench. v2 rebuilds the surface as a
**dockable multi-pane data workbench** on **`flexlayout-react`** (ISC license — permissive, accepted):
N explore tabs + N panel-builder tabs open simultaneously, drag to split/tab/dock/float/close, rename
tabs, layout persists per user. The architectural correction over v1: **do NOT reuse the dashboard's
editor chrome verbatim** — extract the panel-building/source-querying *logic* into a shared headless
lib and build Data-Studio-native views on it. The dashboard **loses panel authoring entirely**; it
keeps placing library panels (ref cells) and rendering.

### Decisions (v2)

- **Lib boundary — `ui/src/lib/panel-kit/`, not a `packages/*` package (yet).** The logic layer's
  vocabulary (`Cell`/`View`/`FieldConfig`/`Transformation`, `PanelSpec`, the viz substrate) lives in
  `@/lib`; extracting that whole type graph into a workspace package is a session of its own.
  `panel-kit` is written package-shaped — **no JSX, no `@/components`, no `@/features` imports; only
  `@/lib/*` + `@nube/genui`** — so promotion to `@nube/panel-kit` later is mechanical (the
  `@nube/source-picker` model: pure model + injected loaders). *Rejected:* a package now (type-graph
  extraction too heavy for this session, named follow-up); keeping logic in `features/dashboard/editor`
  (couples the state machine to a surface that no longer authors).
- **Logic layer (headless) — what moved into `panel-kit`:** the panel-spec editing state machine
  (`cellToEditorState`/`editorStateToCell` from the dashboard's `cellEditorState.ts`, verbatim — the
  round-trip contract test moves with it), `defaultCell`, the SQL builder query model + `toSurrealQL`
  compiler (pure), the source→draft-cell mapping (`draftFromSelection`), the headless editor hook
  (`usePanelEditor`: state/patch/switchView/draft/run — the logic `PanelEditor.tsx` used to trap in its
  chrome), save-as-library (`saveAsLibrary` over the shipped `panel.save`/`cellToSpec`), and the GenUI
  authoring hook (`useGenUiAuthor`, moved — it only touches `@/lib/agent` + `@nube/genui`).
  `defaultOptionsForView` is **injected** into the hook (it's a registry over the view modules — view
  substrate, not logic).
- **View layer — `ui/src/features/panel-builder/`:** the prop-driven option-surface components
  (tabs/options/fields/VizPicker/PreviewPane/OptionsSearch/LibraryPanelBar) move out of
  `features/dashboard/editor/` — they are no longer a dashboard concern — plus a new inline
  **`BuilderPane`** (fills its parent, no modal) composed from `usePanelEditor` + those tabs. Data
  Studio mounts `BuilderPane` inside FlexLayout tabs. A third consumer can reuse `panel-kit` logic with
  100% different views; these are just the first views.
- **Dashboard-builder removal:** `AddPanel`, `EditCellButton` (the per-cell ⚙ editor mount), the modal
  `PanelEditor` chrome, and the dead legacy `WidgetBuilder` are **deleted**. The dashboard keeps
  `AddLibraryPanel` (ref cells) + `WidgetHost` rendering + geometry editing. Editing a panel = open it
  in Data Studio (the Library pane) and save back. Existing inline (non-ref) cells still render; they
  are edited by rebuilding in the studio.
- **FlexLayout dock model:** one FlexLayout `Model` per user per surface. Tab kinds: `sources` (the
  source picker, border-docked), `library` (`panel.list` → open/edit), `explore` (picked source →
  `WidgetHost` preview, Table/Chart/JSON), `builder` (a full `BuilderPane`). Each explore/builder tab
  carries its **draft cell JSON in the FlexLayout tab `config`**, so persisting the model persists the
  whole debugging setup (tabs + drafts + renames), not just geometry.
- **Layout persistence — a member-owned SurrealDB record, not localStorage (rule 4 analogue of
  `nav_pref`):** new host verb pair **`layout.get` / `layout.set`** over `ui_layout:[ws, user,
  surface]` — the surface key is opaque data (rule 10), so any future dockable surface reuses it.
  Member-level caps `mcp:layout.get:call` / `mcp:layout.set:call` in the member set; gateway
  `GET/PUT /layout/{surface}`; keyed to the token `sub` (a caller can never read/write another user's
  layout). Saves are debounced on model change. *Rejected:* the generic `assets.put_doc` (doc ids are
  workspace-global, not per-user — two users would clobber one layout); localStorage (rule 4).
- **FlexLayout CSS per the packages/* CSS rules:** the library's stylesheet is already fully scoped
  under `.flexlayout__*` classes with theming via `--color-*` custom properties on
  `.flexlayout__layout` — no preflight, no bare-element bleed. We import the stock `light.css` once and
  alias its custom properties to the shell's shadcn tokens (`--background`, `--muted`, …) in a
  studio-scoped stylesheet, so dark/light parity is automatic.
- **AI wiring:** the GenUI authoring tab rides `agent.invoke` unchanged; any model named in studio-side
  AI plumbing defaults to the latest Claude models (no model ids are hardcoded in this surface — the
  agent core owns model choice).

### v2 testing plan (adds to the plan above)

- **panel-kit unit:** the moved round-trip contract (`editorStateToCell(cellToEditorState(c), c) ≡ c`),
  `draftFromSelection`, `toSurrealQL` — pure, `pnpm test`.
- **Workbench (real gateway):** seed a series → open an explore tab → real rows through `viz.query` →
  open a builder tab from it → save as library panel → `panel.get` round-trips; **layout persists**:
  save a layout via the real verb, re-mount, the tabs + drafts come back; a second user does NOT see
  the first user's layout (member-owned), ws-B does not see ws-A's (isolation); capability-deny on
  `layout.set` and `panel.save`.
- **Rust:** `layout_test.rs` — member-owned keying, ws isolation, cap deny; gateway route test.
- **Removal regression:** the dashboard renders with NO add-panel/edit-cell affordance; `AddLibraryPanel`
  still works; the old editor gateway tests re-target `BuilderPane` (same option surface, new chrome).

### v2 non-goals (explicit deferrals)

- **`@nube/panel-kit` as a `packages/*` package** — the lib is package-shaped; the type-graph
  extraction (`@/lib/dashboard` types + `@/lib/panel`) is the named follow-up.
- **Shared/team layouts, named layout presets** — one layout per (ws, user, surface) for now.
- **A conversational data agent** — unchanged from v1, still a named follow-up.

## Open questions

None for v1 — the conversational data agent + a per-source "run raw query" console (beyond the picker) are
named follow-ups, not open questions. Anything the build surfaces goes here per HOW-TO-CODE.

## Related

- `scope/frontend/dashboard/library-panels-scope.md` — the `panel:{id}` asset the studio produces.
- `scope/genui/genui-scope.md` — the "AI widget" authoring tab the studio reuses (shipped).
- `scope/frontend/dashboard/source-picker-package-scope.md` — the `@nube/source-picker` the explorer reuses.
- `scope/frontend/dashboard/viz/` — the `viz.query`/`usePanelData` render path the preview reuses.
- `scope/agent/agent-scope.md`, `scope/agent-run/` — the `agent.invoke` + run-stream seam the AI tab (and a
  future conversational agent) rides.
- Skill: the shipped `docs/skills/panels/SKILL.md` + `docs/skills/genui-widget/SKILL.md` cover the drivable
  surfaces the studio composes; a `docs/skills/data-studio/SKILL.md` is written on ship.
