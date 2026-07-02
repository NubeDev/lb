# Session — reusable ce-wiresheet-style panel (`@nube/panel`), first used on the dashboard Edit widget

Scope: `docs/scope/frontend/nav-rail-scope.md` (the handover). Status: **shipped**.

## The ask (verbatim)

> Make a **common, reusable panel** that **looks like the ce-wiresheet panel**, built with
> **shadcn/ui**. The **first place we use it is the lb dashboard "Edit panel" widget**
> (`ui/src/features/dashboard/editor/PanelEditor.tsx`).

The prior pass mis-delivered a nav rail (`@nube/nav-rail`) and swapped its `NavMenu` into the
editor's tab strip — the shell stayed a cramped fixed `sm:max-w-3xl` Sheet. The CE panel is
**rich, dense, and resizable** ("so many options on resize"); ours was thin and fixed. This
session built the actual panel and rebuilt the editor on it.

## What shipped

### 1. `@nube/panel` — a reusable, resizable, dense side panel

A new pnpm workspace package under `packages/panel`, mirroring `@nube/nav-rail`'s build/theming
discipline. Source panel copied faithfully: `ce-wiresheet/src/ui/InspectPanel.tsx` — its
identity header, `Section` grouping, dense monospace property tables, and KV rows — ported to
**shadcn/ui primitives + scoped `hsl(var(--lbp-*))` tokens**, data-driven via props (ce's engine
types / `useStore` / REST dropped).

- `Panel.tsx` — the docked, **resizable** shell (Radix-dialog overlay + focus trap + escape,
  vendored like nav-rail's sheet). Width is controlled by `useResizable`, not a Tailwind
  max-width, so the drag actually widens it. Header (title/description/`headerAside`), scrollable
  body (host composes children), optional `footer` action row.
- `useResizable.ts` + `ResizeHandle.tsx` — a dependency-free pointer-drag left-edge handle
  (drag left → wider → more option columns), clamped to `[min,max]`, keyboard-operable (a
  focusable `separator` with arrow-key resize). This is the "so many options on resize" behavior.
  Chose a hand-rolled resizer over adding `react-resizable-panels` (the handover *suggested* it) —
  no registry access this session, and CE itself hand-rolls its drawer chrome, so this is more
  faithful and dependency-free.
- `Section.tsx`, `PropTable.tsx` (columns + rows, ellipsizable cells, per-row `tone:"warn"`),
  `KV.tsx` — the ce InspectPanel structural pieces, one responsibility per file (FILE-LAYOUT).
- `panel.css` + `panel-theme.css` — theme + utilities only, **NO preflight** (`grep -c '@layer
  base' dist/panel.css` → `0`); tokens scoped to `.lb-panel`, host-overridable. Dev React/types/
  lucide pinned to match `ui` (`react@^18.3.1` / `@types/react@^18.3.12` / `lucide-react@^0.460.0`).
- `@nube/panel` **re-exports `NavMenu`** from `@nube/nav-rail` (kept as an internal dependency) —
  so the nav rail is now a legit sub-component of the panel, not "the panel." Resolves the
  handover's "delete or keep" question: **kept as a dependency**.

### 2. First use — the dashboard Edit panel rebuilt on it

`ui/src/features/dashboard/editor/PanelEditor.tsx`: the fixed `Sheet` (`side="right"
sm:max-w-3xl`) replaced by `<Panel initialWidth={960} minWidth={560} maxWidth={1400}>`. All
wiring preserved — `cellToEditorState`/`editorStateToCell`, `PreviewPane`, `VizPicker`, the
`NavMenu` options rail, `OptionsSearch`, the six tab bodies, save/cancel (now in `footer`). Only
the shell + look changed; the round-trip is untouched. `ui` gains `@nube/panel` (`workspace:*`)
and `import "@nube/panel/style.css"` in `main.tsx`.

## Tests (green)

- **`@nube/panel`**: `pnpm test` → 7/7 (real Radix dialog, real DOM, real keyboard + pointer
  drag — no fakes, CLAUDE §9). `pnpm typecheck` clean. `pnpm build` clean; `dist/panel.css` has
  **0** `@layer base`.
- **`@nube/nav-rail`**: unchanged, 12/12 still green.
- **`ui` unit**: `pnpm test` → **322/322**.
- **`ui` gateway**: `pnpm test:gateway` → 236/240. The 4 failures
  (`DashboardView`, `SystemView` subsystem sheet, `sqlSource` visual-editor, `CommandPalette`
  agent) are the **pre-existing baseline set** — verified byte-identical against a stashed clean
  tree (same 4 files, same test names). **Zero new failures.** The two suites the handover
  required stay green: `panelEditor.gateway.test.tsx` ✅ and `flowsPanelEditor.gateway.test.tsx` ✅.
- **`ui` build**: `vite build` succeeds. (`pnpm build` runs `tsc --noEmit` over the whole tree
  and stops on 2 **pre-existing** errors in `FlowsCanvas.gateway.test.ts` — confirmed on the clean
  baseline, unrelated to this work; my touched files typecheck clean.)

## Decisions / rejected

- **Hand-rolled resizer over `react-resizable-panels`** — see above (dependency-free, CE-faithful,
  no registry this session).
- **Kept `@nube/nav-rail`** as an internal dependency (re-exported `NavMenu`) rather than deleting
  it — the section rail is a real use of it; deleting would have meant re-vendoring the same nav.
- **Queried the panel by its title** in tests, not `aria-label` — Radix wires the `SheetTitle` as
  `aria-labelledby`, which wins over `aria-label` for the accessible name.

## Fix — panel must follow the HOST theme (not ship its own palette)

First cut shipped `@nube/panel` with its own fixed `--lbp-*` dark palette (copying nav-rail's
self-themed pattern). Wrong for this ask: the panel ignored the app theme — in the app's **light**
mode the panel stayed **dark**, and even in dark mode its surface didn't match the app's `--card`.

Fix: `panel-theme.css` now **aliases the host's shadcn tokens** —
`--lbp-panel: var(--card, <fallback>)`, `--lbp-fg: var(--foreground, …)`,
`--lbp-border: var(--border, …)`, `--lbp-muted: var(--muted-foreground, …)`,
`--lbp-accent: var(--ring, …)`, `--lbp-amber: var(--destructive, …)`, etc. The hard-coded HSL is
now only a **fallback** for a standalone/external mount with no shadcn tokens. So inside lb `ui`
the panel inherits the app's light/dark/`data-theme-accent` theme automatically (the host flips
`--card`/`--foreground`/… on `.dark`), and the portal (Radix mounts on `<body>`) still resolves
them since the vars cascade from `<html>`. Removed the `:root` defaults (no more global dark-token
leak) and the `.theme-light` block; dropped the `--font-sans` remap so the panel inherits the app
font. Verified the built `dist/panel.css` and the `ui` bundle both carry `--lbp-panel:var(--card…)`
and still have **0** `@layer base`. Re-ran: panel 7/7, ui unit 322/322, both editor gateway suites
green.

## Fix 2 — library stylesheets leaked GLOBAL utilities + didn't follow the theme (broke the app)

Symptom: after the theme-alias fix, the **app's left sidebar vanished** and the editor's selected
nav item ("Query") rendered as a **fixed near-black block** in the host's light theme.

Two root causes, both in the shared packages' stylesheets:

1. **Unscoped utility leak.** Both `@nube/panel` and `@nube/nav-rail` shipped a *global*
   `@import 'tailwindcss/utilities.css'` — emitting ~200 unscoped `.flex`/`.grid`/`.border`/
   `.w-full`/… rules. Dropped into a host that ALSO ships Tailwind (the app is v4.1; the panel
   built v4.3), these collide and override the app's own utilities — killing the sidebar layout.
   (nav-rail had leaked since the prior pass; adding a second, newer-version copy via panel tipped
   it over.) **Fix:** scope every generated utility under the package root class using the v4
   nesting form —
   ```css
   @layer utilities { .lb-panel { @tailwind utilities } }   /* panel */
   @layer utilities { .nav-rail { @tailwind utilities } }   /* nav-rail */
   ```
   so the classes only apply *inside* the component and can't touch app elements. Verified the
   built `dist/*.css` (and the app bundle) now emit `.lb-panel .flex` / `.nav-rail .flex`, with
   **zero** unscoped structural utilities and still **0** `@layer base`.

2. **nav-rail didn't follow the host theme.** Its `NavMenu` selected item uses `bg-nr-bg`, and
   `--nr-bg` was a fixed near-black — so in the app's light theme the active tab was a black block.
   **Fix:** same aliasing as the panel — `nav-rail-theme.css` now maps `--nr-*` onto the host
   shadcn vars (`--nr-bg: var(--muted-bg, …)`, `--nr-panel: var(--card, …)`,
   `--nr-fg: var(--foreground, …)`, `--nr-accent: var(--ring, …)`, …) with dark fallbacks, scoped
   to `.nav-rail`. The rail now follows light/dark with the app.

**Rule (for any future `packages/*` UI library):** a Tailwind library stylesheet must ship
**scoped utilities (under its root class) + theme tokens that alias the host's shadcn vars +
NO preflight**. Never a global `@import 'tailwindcss'`/`utilities.css` and never a fixed palette.
This generalises the earlier preflight lesson (`nav-rail.css`) to utilities and theme.

Re-verified after fix 2: nav-rail 12/12, panel 7/7, ui unit 322/322, both editor gateway suites
green, `vite build` clean.

## Follow-ups (not blocking)

- The 4 baseline gateway failures + 2 baseline tsc errors pre-date this work; not chased.
- `@nube/panel` currently only exposes the docked-overlay shell; an in-flow (non-overlay) variant
  and a `PropTable` that auto-adds columns as width grows are natural next steps if a second
  consumer wants them.
