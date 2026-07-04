# Data Studio v3 — one stacked query/preview view (session)

Branch: `master`. Scope: `scope/frontend/data-studio-scope.md` (v3 section). Public:
`public/frontend/data-studio.md`. Builds on v2 (`data-studio-v2-workbench-session.md`).

## The ask

v2 shipped explore and build as **two separate tabs**: picking a source opened a read-only `explore`
tab (Table/Chart/JSON over `WidgetHost` + a "Build panel" button), and building opened a *different*
`builder` tab. Opening an existing chart landed the user on the explore Table view — a wall of raw JSON
payload rows — with the panel controls one tab away. Wrong model.

v3 collapses them into **ONE stacked view** — rendered preview on **top**, query/panel builder on the
**bottom** — so a user sees the data and shapes the chart together. Opening an existing chart puts the
chart in focus with its source/query beneath it. The SQL/query editor appears in the bottom section only
when the datasource needs it (SurrealDB / federation), which the shipped conditional `QueryTab` already
does — v3 just surfaces it in the new layout.

## What shipped

### 1. `BuilderPane` gains a `layout` prop — `"split"` | `"stacked"`

`features/panel-builder/BuilderPane.tsx` already held BOTH halves (the live `PreviewPane` + viz picker
AND the full Query/Transform/Field/Overrides option surface); v2 laid them **left/right**. v3 extracts
the two halves into `previewHalf`/`optionsHalf` locals and arranges them by a new `layout` prop:

- `"split"` (default — backward compatible; the dashboard-parity gateway tests + any other consumer keep
  the left/right arrangement untouched);
- `"stacked"` — the preview grows to fill the **top** (full-width, `flex-[3]`), the options rail (Query
  first → the SQL editor lives there) sits **below** (`flex-[2]`, `border-t`). The only prop-driven
  difference in `previewHalf` is the preview box class: `min-h-0 flex-1` (grows) vs `h-56 shrink-0`.

*Rejected:* a nested FlexLayout `row`/`column` inside the tab (a second dock model per tab to persist +
the jsdom rect-cull quirk ×2 — needless for a fixed top/bottom split). The arrangement is a `BuilderPane`
concern, so the prop lives there (rule 9's one editor — no fork).

### 2. The `explore` tab-kind merged into `builder` — one working-tab kind

- `DataStudioView.openExplore` now seeds a **chart** draft from the picked source
  (`draftFromSelection(sel, "timeseries", …)`) and calls `openBuilder` directly — no read-only explore
  hop. `LibraryPane.onOpen` and `New panel` already opened builders; now everything does.
- `BuilderTabPane` passes `layout="stacked"` to `BuilderPane`.
- **Deleted:** `panes/ExplorePane.tsx`. **Removed from `workbenchModel.ts`:** the `explore` `PaneKind`,
  `ExploreConfig`, `exploreTabJson`, `EXPLORE_VIEWS`. **Removed from `DataStudioView.tsx`:** the `explore`
  factory case, the now-unused `picker.installed` wiring (the builder's `WidgetView` resolves installed
  via the cache provider). `range` still drives the header date pickers + `useVarScope`.
- A persisted v2 layout carrying `explore` tabs still loads: an unknown component hits the factory's
  fallback pane (no crash), and `modelFrom`'s try/catch already guards shape drift. The user reopens from
  Sources.

### 3. SQL editor — surfaced, not rebuilt

No change to `QueryTab.tsx`: it already renders the Builder⇄Code `SqlQueryEditor` (`isSql`), the raw
federation `Textarea` (`isFederation`), and the friendly picker (series/flows) off the target's
datasource. v3's job was to prove that conditional editor shows in the new **stacked bottom** section —
covered by a new gateway test.

## Testing (real infra, rule 9 — no mocks/fakes)

`DataStudio.gateway.test.tsx` **6/6** (re-targeted + 2 new), against the real spawned gateway:

- **headline** — pick a seeded series → the stacked builder mounts directly (no explore hop); the live
  `panel preview` renders through `viz.query`; name + Save-as-library → `panel.get` round-trips the built
  spec (`series.read` source); LAYOUT PERSISTS (reload restores the builder tab + draft via the real
  `layout.*` verbs).
- **SQL-editor-when-needed (new)** — a series source shows NO `sql query editor`; switching the Query
  section's source to `sql:query` ("SQL query (direct SurrealDB)") surfaces the `SqlQueryEditor`.
- **open-existing (new)** — seed a `panel:` record, open it from the Library dock pane → ONE stacked
  builder (`panel builder` + `panel preview`), title round-trips into the editor. (The Library pane is
  border-docked; the test clicks the Library border-tab button — matched via
  `.flexlayout__border_button` — to mount it, per the jsdom rect quirk.)
- **member-owned** — a second member in the same ws gets their own default workbench (no builder tab);
  the model JSON asserts on `"builder"` now (was `"explore"`).
- **workspace-isolation** — the layout + saved panel never cross to ws-B.
- **capability-deny (mandatory)** — no `panel.save`: the Save-as-library affordance is absent AND the
  verb is refused server-side.

Unit suite `pnpm test` **443/443** green (panel-kit round-trip untouched — `BuilderPane` split-vs-stacked
is a layout prop, no logic change). Panel-builder gateway parity (`flowsPanelEditor`,
`valueMappingUsability`) + the dashboard removal regression (`DashboardView.gateway`) still green
(13/13) — they use `BuilderPane`'s default `split` layout, unchanged.

Pre-existing / out-of-scope failures (NOT this work, unchanged from clean master): the missing-WASM
fixture suites (github-bridge, proof-panel), `sqlSource.gateway` (a case-sensitivity assertion), and a
concurrent session's in-flight `CodeEditor.tsx` edit cross-polluting `SystemView` (`theme is not
defined`). All documented in the v2 session + the `preexisting-failing-tests` memory.

## Files touched

- `ui/src/features/panel-builder/BuilderPane.tsx` — the `layout` prop + the stacked arrangement.
- `ui/src/features/data-studio/panes/BuilderTabPane.tsx` — `layout="stacked"` + header comment.
- `ui/src/features/data-studio/workbenchModel.ts` — dropped the `explore` kind + its factories/config.
- `ui/src/features/data-studio/DataStudioView.tsx` — pick-source opens a builder directly; explore case
  + unused wiring removed.
- `ui/src/features/data-studio/panes/ExplorePane.tsx` — **deleted**.
- `ui/src/features/data-studio/DataStudio.gateway.test.tsx` — re-targeted + 2 new tests.
- Docs: scope v3 section + this session + `public/frontend/data-studio.md` + `STATUS.md`.

## Deferrals (unchanged)

`@nube/panel-kit` as a `packages/*` package; shared/team layouts + named presets; a conversational data
agent. All named follow-ups.
