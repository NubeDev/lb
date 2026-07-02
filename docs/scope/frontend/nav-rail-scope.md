# HANDOVER — reusable ce-wiresheet-style panel (shadcn), first used on the dashboard Edit widget

Status: **SHIPPED** — built as `packages/panel` (`@nube/panel`) and the dashboard Edit panel
(`ui/src/features/dashboard/editor/PanelEditor.tsx`) is rebuilt on it (wide + resizable, no longer
a cramped fixed Sheet). `@nube/nav-rail` kept as an internal dependency (its `NavMenu` is the
section rail, re-exported). Package CSS is preflight-free; `ui` unit 322/322 green, gateway adds
zero new failures (the two editor suites stay green). See
`docs/sessions/frontend/reusable-panel-session.md` for the full record. The original handover is
kept below for reference.

---

---

## The goal (verbatim intent)

> Make a **common, reusable panel** that **looks like the ce-wiresheet panel**, built with
> **shadcn/ui**. The **first place we use it is the lb dashboard "Edit panel" widget**
> (`ui/src/features/dashboard/editor/PanelEditor.tsx`).

That is the entire brief. Copy the ce-wiresheet panel's **look and structure**, make it a
**reusable shadcn component**, then rebuild the dashboard Edit panel on top of it.

## What went wrong before (so you don't repeat it)

A previous pass built a `@nube/nav-rail` **sidebar/nav-menu** and swapped it into the Edit
panel's tab strip. **That was the wrong artifact** — a nav rail is not the panel. The Edit
panel still "looks like it did before" (a cramped fixed-width Sheet, `sm:max-w-3xl`, sparse
options). The user's words: *"the one in CE has so many options on resize"* — the ce panel is
**rich, dense, and resizable**; ours is thin and fixed. Fix **that** gap.

Decide with the user (or from the files) whether to **delete `@nube/nav-rail`** or keep it as an
internal dependency of the new panel. Do not leave a nav rail masquerading as "the panel."

## Step 0 — confirm the source panel (one decision, then go)

"The ce-wiresheet panel" = one of these files in `/home/user/code/c/ce/ce-wiresheet/src/`.
Pick the one the user means (the screenshot they shared is the lb Edit panel, i.e. the
**target**, not the source). Best candidates, largest/most-option-dense first:

| File | Lines | What it is |
|---|---|---|
| `components/DiagPanel.tsx` | 589 | the biggest panel — many sections + controls |
| `ui/UiTabHost.tsx` | 374 | the tabbed panel host (resizable drawer that holds panels) |
| `ui/InspectPanel.tsx` | 271 | dense detail panel: identity header + grouped `Section`s + property/edge/metadata tables ("so many options" look) |
| `ui/TabShell.tsx` | 103 | generic IDE tab-shell: pinned index tab + closeable tabs (render-prop) |

Read the chosen file top-to-bottom. That is the thing you are copying.

## Step 1 — copy it faithfully into a reusable shadcn package

- Create/extend a package under `packages/` (repo-root pnpm workspace already includes
  `packages/*`; `ui/` depends on it via `workspace:*`). Suggested name: `@nube/panel`.
- **Port the ce panel's structure and look**: its header, its **`Section`** grouping, its dense
  tables/rows, its resizability, its spacing/typography. Keep the *look*; drop ce-specific data
  wiring (engine types, `useStore`, REST) — make it **data-driven via props**.
- **Build on shadcn/ui primitives** (the user asked for shadcn explicitly). The lb app already
  vendors shadcn under `ui/src/components/ui/*` — mirror those primitives into the package
  (`sheet`, `resizable`, `separator`, `scroll-area`, `input`, `button`, `tabs`/section headers,
  `cn`), or depend on a shared copy. **Resizability**: use shadcn's `resizable`
  (react-resizable-panels) so widening the panel reveals more option columns — this is the
  "so many options on resize" behavior the user wants.
- **Self-themed** like ce-wiresheet: all color via `hsl(var(--token))` scoped to a root class,
  host-overridable. lb `ui/` is now on **Tailwind v4** (migrated in the prior pass — keep that),
  so a v4 `@theme` + tokens is fine; **the package stylesheet must ship theme+utilities only,
  NO preflight** (a library must not reset its host — this exact bug already bit us:
  `docs/debugging/frontend/react-types-19-collision.md` neighbours; see also the `@layer base`
  drop-in error).
- **Deps discipline** (a prior bug): pin the package's dev React/types/lucide to match `ui`
  (`react@^18.3.1`, `@types/react@^18.3.12`, `lucide-react@^0.460.0`) or you split the
  `@types/react` world and break `ui`'s lucide typecheck. See
  `docs/debugging/frontend/react-types-19-collision.md`.
- **FILE-LAYOUT**: one responsibility per file, ≤400 lines. `Panel.tsx` (shell), `Section.tsx`,
  the row/table pieces, `items.ts`/types, each shadcn primitive its own file. No `utils.ts`.

## Step 2 — first use: the dashboard Edit panel

Rebuild `ui/src/features/dashboard/editor/PanelEditor.tsx` on the new panel:

- It is currently a fixed-width `Sheet` (`side="right" sm:max-w-3xl`) — **replace that** with the
  reusable resizable panel so it is **wide and resizable**, matching the ce look.
- Keep the existing wiring: `cellToEditorState`/`editorStateToCell`, `PreviewPane`, `VizPicker`,
  the section bodies (`QueryTab`/`TransformTab`/`PanelOptionsTab`/`FieldTab`/`OverridesTab`),
  `OptionsSearch`, save/cancel. Only the **shell + look** changes.
- The section navigation (Query / Transform / Panel options / Field / Overrides) should be
  presented in the **ce panel's idiom** (its `Section`/tab treatment), not a plain text list.
- Preserve behavior: the gateway tests must stay green
  (`ui/src/features/dashboard/editor/panelEditor.gateway.test.tsx`,
  `flowsPanelEditor.gateway.test.tsx`) and the DashboardView render test.

## Guardrails / tests (must stay green)

- `pnpm -C ui build` (Tailwind v4, `tsc --noEmit && vite build`).
- `pnpm -C ui test` (322 unit tests today).
- `pnpm -C ui test:gateway` — the panel-editor + flows-panel-editor + DashboardView suites.
  (Baseline has **4 pre-existing** gateway failures unrelated to this work: `DashboardView`,
  `SystemView` subsystem sheet, `sqlSource` visual-editor, agent-command palette — they fail on
  the untouched tree too; don't chase them, just don't add new ones.)
- New package: its own `pnpm test` (real component, no fakes — CLAUDE §9), `typecheck`, `build`.
- Package CSS ships **0** `@layer base` blocks (grep the built `dist/*.css`).

## Files & pointers

- **Source (copy this):** `/home/user/code/c/ce/ce-wiresheet/src/` — the panel file chosen in
  Step 0. Its theming pattern: `src/wiresheet.css` + `src/wiresheet-theme.css` (`@theme` over
  scoped `hsl(var())` tokens). Its lib build to mirror: `vite.lib.config.ts`.
- **Target (first use):** `ui/src/features/dashboard/editor/PanelEditor.tsx` and its
  `tabs/*`, `PreviewPane.tsx`, `VizPicker.tsx`, `OptionsSearch.tsx`.
- **shadcn primitives to mirror:** `ui/src/components/ui/{sheet,resizable,separator,scroll-area,
  input,button,tabs}.tsx`, `ui/src/lib/utils.ts` (`cn`). (If `resizable` isn't vendored yet, add
  it — `react-resizable-panels`.)
- **Prior-pass debris to reconcile:** `packages/nav-rail/` (the wrong artifact — reuse or remove),
  the `NavMenu` swap inside `PanelEditor.tsx` (revert to make room for the real panel).
- **Prior-pass wins to KEep:** the Tailwind-v4 migration of `ui/`
  (`docs/sessions/frontend/tailwind-v4-migration-session.md`) — do not revert it.

## Definition of done

1. A reusable shadcn panel package that **visually reads like the ce-wiresheet panel**
   (dense, sectioned, **resizable — more options as it widens**).
2. The dashboard Edit panel (`PanelEditor.tsx`) rebuilt on it — no longer a cramped fixed Sheet.
3. All the guardrail builds/tests green; no new gateway failures; package CSS preflight-free.
4. The stray nav-rail artifact resolved (kept-as-dependency or deleted), not left pretending to
   be the panel.
