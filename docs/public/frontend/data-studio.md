# Data Studio

Data Studio (`/t/$ws/data-studio`) is the **multi-pane data workbench**: a dockable, tabbed,
splittable surface where a user opens many data sources and many panel builders at once — side by side —
explores real data, and turns what they find into reusable library panels (manually or with AI). It is
the **panel factory**; dashboards consume its output. It is a composition of shipped substrate, not a new
data or render path.

## What exists

- **A dockable workbench (FlexLayout).** Built on `flexlayout-react` (ISC): N explore tabs + N
  panel-builder tabs open simultaneously; drag to split, tab, dock, float (pop out to a window), close;
  double-click to rename. Two border-docked panes — **Sources** and **Library** — plus the center
  working area.
- **Explore any source, one render path.** The **Sources** pane is the shipped `@nube/source-picker`
  across every source type (series / Direct SurrealDB / flows / installed-extension tools / federation
  datasources). Picking one opens an **explore tab** that renders the live data through the shipped path
  (`WidgetHost` → `usePanelData` → `viz.query`), toggleable Table / Chart / JSON — no parallel renderer,
  no parallel query.
- **Build panels — the full option surface, inline.** An explore tab's **Build panel** (or the toolbar's
  **New panel**, or opening a **Library** panel) opens a **builder tab**: the complete Grafana-style
  option surface (Query / Transform / Field / Overrides) plus the GenUI **"AI widget"** authoring tab —
  the same option model a dashboard cell has, rendered inline (no modal). Many builder tabs open at once
  for compare/debug.
- **Save to the library.** Any builder tab's **Save as library panel** persists the built spec as a
  `panel:{id}` (`panel.save`) — immediately reusable on any dashboard (Add library panel → a ref cell)
  and renderable standalone at `/t/$ws/panel/{id}`. No dashboard is required to produce a panel.
- **Layout persists per user.** The whole arrangement — every tab, its draft cell, renames, splits —
  persists to a **member-owned SurrealDB record** (`ui_layout:[ws, user, "data-studio"]`) via the
  `layout.get` / `layout.set` host verbs, debounced on change. A reload restores the exact debugging
  setup. It is never localStorage (rule 4), and it is keyed to the token `sub` — a member can only ever
  read/write their own layout.

## The shared substrate

Data Studio's builder views are built on a **headless logic lib**, `ui/src/lib/panel-kit/`, extracted so
any surface can author panels with its own views (logic and views strictly separated):

- **Logic (headless, no JSX):** `cellToEditorState`/`editorStateToCell` (the ONE panel-spec
  (de)serializer), `usePanelEditor` (the editing state machine), `defaultCell`, the SQL builder model +
  `toSurrealQL`, `draftFromSelection`, `saveDraftAsPanel`, and `useGenUiAuthor` (the AI authoring hook).
- **Views:** `features/panel-builder/` (the option-surface tabs + the inline `BuilderPane`) and
  `features/data-studio/` (the FlexLayout panes). A third consumer can reuse the panel-kit logic with
  100%-different views.

The genuinely-shared primitives are reused, not forked: `viz.query`/`usePanelData` (the one query path),
`WidgetHost`/`views/*` (rendering), `@nube/source-picker`, the `panel.*` library asset, and the GenUI
authoring seam.

## Panel authoring moved off the dashboard

The dashboard no longer authors panels. It **places** library panels (Add library panel → a ref cell)
and **renders** them; the Add-panel builder and the per-cell edit affordance were removed. To edit a
panel, open it in Data Studio's Library pane and save it back — one place authors panels now.

## How it fits the core

- **Capabilities (rule 5):** the surface shows for a member who can read data (`series.list`); every
  explore call re-checks its source tool's own cap (the `viz.query` per-target leash); Save-as-library
  needs `mcp:panel.save:call`; layout persistence needs `mcp:layout.get:call` / `mcp:layout.set:call`
  (member-level). Deny paths degrade honestly — a denied source renders the standard `usePanelData`
  denied state; no `panel.save` → no save affordance (the host re-checks regardless).
- **Tenancy / isolation (rule 6):** the built panel is a workspace-scoped `panel:{id}`; the layout record
  is walled by workspace AND keyed to the user. Nothing crosses.
- **Core knows no extension (rule 10):** the picker lists extension tools/datasources as opaque entries;
  the `layout.*` `surface` key is opaque data, so any future dockable surface reuses the same verbs.
- **MCP surface:** consumes the existing explore/render/library verbs; adds exactly the member-owned
  `layout.get` / `layout.set` pair (the `nav_pref` pattern generalized).

## Not yet

- `@nube/panel-kit` as a standalone `packages/*` package (the logic is package-shaped; the type-graph
  extraction is a follow-up).
- Shared/team layouts and named layout presets (one layout per user per surface today).
- A conversational data-Q&A agent (the GenUI authoring tab + the explorer are the AI paths today).
