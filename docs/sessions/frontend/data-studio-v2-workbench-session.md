# Data Studio v2 — the multi-pane workbench + the extracted panel-kit lib (session)

Branch: `master`. Scope: `scope/frontend/data-studio-scope.md` (v2 section). Public:
`public/frontend/data-studio.md`. Related: `library-panels-scope.md`, `viz/`, `genui-scope.md`.

## The ask

Rebuild Data Studio (`/t/$ws/data-studio`) from v1's single-picker/single-preview/modal-editor into a
**dockable multi-pane data workbench** on `flexlayout-react`: N explore tabs + N panel-builder tabs
open at once, drag to split/tab/dock/float/close/rename, layout persists per user so a debugging setup
survives reload. The architectural correction: **do NOT reuse the dashboard's editor chrome verbatim** —
extract the panel-building/source-querying *logic* into a shared headless lib and build Data-Studio-native
views on it. The dashboard **loses panel authoring entirely** (keeps placing library panels + rendering).

## What shipped

### 1. `ui/src/lib/panel-kit/` — the headless logic layer

The panel-editing logic, lifted out of `features/dashboard/editor/` so any surface can author panels with
its own views. **No JSX, no `@/components`, no `@/features` imports — only `@/lib/*` + `@nube/genui`**
(package-shaped; promotion to `@nube/panel-kit` is a named follow-up, the blocker being the
`@/lib/dashboard` type graph). Files:

- `cellEditorState.ts` (moved verbatim, incl. its round-trip contract test) — `cellToEditorState` /
  `editorStateToCell`, the ONE (de)serializer ADD and EDIT share; the pinned contract
  `editorStateToCell(cellToEditorState(c), c) ≡ c` moved with it.
- `defaultCell.ts` (moved) — now takes the per-view default `options` **injected** (the view substrate's
  `defaultOptionsForView` registry owns "what a fresh <view> looks like"; panel-kit stays headless).
- `sql/query.ts` + `sql/toSurrealQL.ts` (moved) — the SQL builder query model + compiler (pure);
  `emptySqlSource` moved here from the view file (re-exported for existing importers).
- `usePanelEditor.ts` (**new**) — the headless state machine the modal `PanelEditor` chrome used to trap:
  state/patch/viewC/switchView/draft/toCell/refreshKey/run/flowKind/canPlot.
- `draftFromSelection.ts` (**new**, from v1's `useExploreDraft`) — a picked `@nube/source-picker`
  selection → a fresh draft `Cell` (single v3 target `A`).
- `saveAsLibrary.ts` (**new**) — `saveDraftAsPanel` over the shipped `panel.save`/`cellToSpec`.
- `useGenUiAuthor.ts` (moved) — the GenUI "AI widget" authoring hook (only touches `@/lib/agent` +
  `@nube/genui`; models are the agent core's choice — nothing hardcoded here, so "latest Claude" holds).

### 2. `ui/src/features/panel-builder/` — the option-surface views

`features/dashboard/editor/` → `features/panel-builder/` (the editor is no longer a dashboard concern).
The prop-driven tabs/options/fields/VizPicker/PreviewPane/OptionsSearch/LibraryPanelBar moved wholesale
plus a **new inline `BuilderPane.tsx`** (fills its parent, no modal) composed from `usePanelEditor` + those
tabs. `AddLibraryPanel.tsx` moved BACK to `features/dashboard/` (it's placement, a dashboard concern).

### 3. Dashboard-builder removal

Deleted: `AddPanel.tsx`, `EditCellButton.tsx`, the modal `PanelEditor.tsx`, and the dead legacy
`WidgetBuilder.tsx` (+ its gateway test). `DashboardView` drops the "Add panel" bar (keeps
`AddLibraryPanel`); `Grid` drops the per-cell ⚙ edit mount + `canEdit`/`onEditCell` props. `seedEntryId`
(the one live export `WidgetBuilder` still owned) moved to `builder/sourcePicker.ts`.

### 4. Data Studio v2 workbench — `features/data-studio/`

- `workbenchModel.ts` — the FlexLayout model vocabulary: tab kinds (`sources`/`library` border-docked,
  `explore`/`builder` center), per-tab `config` (each explore/builder tab carries its draft cell JSON in
  the FlexLayout tab config → persisting the model persists the whole debugging setup), the default model,
  tab-json factories.
- `useWorkbenchLayout.ts` — load/persist seam: `layout.get` on visit, debounced `layout.set` on change.
- `panes/{SourcesPane,LibraryPane,ExplorePane,BuilderTabPane}.tsx` — the four pane views.
- `DataStudioView.tsx` — the workbench: FlexLayout `Model`, the tab factory, open-tab actions.
- `datastudio-dock.css` — the FlexLayout→shell-token bridge (stock `light.css` imported once; its
  `--color-*` custom properties re-declared under `.data-studio-dock` aliased to the shell shadcn tokens
  → automatic dark/light parity). `public/popout.html` for the float-to-window feature.
- `flexlayout-react` **ISC license** — permissive, accepted; CSS already fully scoped under
  `.flexlayout__*` (no preflight bleed).

### 5. Layout persistence — a member-owned SurrealDB record (rule 4)

New host verb pair `layout.get` / `layout.set` over `ui_layout:[ws, user, surface]` (the `nav_pref`
pattern generalized; the `surface` key is opaque data, rule 10). Rust: `crates/host/src/layout/`
(model/store/get/set/tool/error, one responsibility per file), registered in `lib.rs` + `tool_call.rs`;
member caps `mcp:layout.get:call` / `mcp:layout.set:call` in `credentials.rs`; gateway
`GET/PUT /layout/{surface}` in `role/gateway/src/routes/layout.rs`. UI client `lib/layout/`; `http.ts`
mapping; `CAP.layoutGet`/`layoutSet`. The record is always keyed to the token `sub` (a caller can never
read/write another user's layout); the model is opaque JSON bounded at 256 KB (reject, don't truncate).
*Rejected:* `assets.put_doc` (doc ids are ws-global — two users clobber one layout); localStorage (rule 4).

## Testing (real infra, rule 9 — no mocks/fakes)

- **panel-kit unit** — the moved round-trip contract (`cellEditorState.test.ts`, adjusted for injected
  options) + `toSurrealQL.test.ts`. **UI unit suite 437/437 green.**
- **Data Studio (real gateway)** — `DataStudio.gateway.test.tsx` **4/4**: explore → build → save-as-library
  round-trips (`panel.get` carries the built spec); LAYOUT PERSISTS (a reload restores the tabs + drafts
  via the real `layout.*` verbs); MEMBER-OWNED (a second member sees their own default workbench);
  workspace isolation; capability-deny on `panel.save` (no affordance + verb refused server-side).
  (jsdom quirk: FlexLayout culls content in a 0×0 rect — the test stubs `getBoundingClientRect` + fires a
  `resize`; documented in the file.)
- **Removal regression** — `DashboardView.gateway.test.tsx` **8/8**: the dashboard has NO "Add panel" / no
  per-cell edit; a library panel still places (ref cell) + hydrates + renders; the old editor gateway tests
  (`panelEditor`/`flowsPanelEditor`/`valueMappingUsability`) re-target `BuilderPane` (same option surface,
  new chrome) — all green.
- **Rust** — `crates/host/tests/layout_test.rs` **6/6** (round-trip/LWW, member-owned, per-surface,
  ws-isolation, cap-deny, bounds); `role/gateway/tests/layout_routes_test.rs` **3/3** (round-trip +
  member-owned keying, cap-deny per verb, ws-isolation). Existing `nav`/`panel`/`gateway`/session tests
  still green. `cargo build -p lb-host -p lb-role-gateway` + gateway tests clean.

Pre-existing / out-of-scope failures (NOT this work): the missing-WASM-fixture suites (github-bridge,
proof-panel) and `sqlSource.gateway` (a case-sensitivity assertion, `/seq|payload/` vs rendered
`Seq`/`Payload`) fail on clean master. A concurrent AI session's in-flight `CodeEditor.tsx` edit
(`theme is not defined`) cross-pollutes RulesView/SystemView in the full gateway run — both pass in
isolation; left untouched per the owner ("just leave the git", another session edits those files).

## Deferrals (explicit non-goals, stated in scope)

- `@nube/panel-kit` as a `packages/*` package (lib is package-shaped; the `@/lib/dashboard` type-graph
  extraction is the follow-up).
- Shared/team layouts, named layout presets (one layout per (ws, user, surface) for now).
- A conversational data agent (unchanged v1 follow-up).
