# Data Studio — resizable + scrollable panel editor (stacked builder)

## Ask
In Data Studio the stacked panel builder (preview on TOP, options rail BELOW) split its
height by fixed flex ratios (`flex-[3]` / `flex-[2]`) with `overflow-hidden`. Tall option
content and the viz picker got clipped with no scrollbar, and the user couldn't change how
much height the preview vs. the options rail got.

Requested: a scrollbar for the panel editor and the preview/selector, and an adjustable
height split between them.

## Change
- New `ui/src/features/panel-builder/useVerticalSplit.ts` — a headless hook owning the
  top/bottom split fraction and a pointer-drag on the divider, clamped to `MIN_FRACTION`
  (0.15) so neither pane collapses. View-agnostic; returns `containerRef`, `topBasis`,
  `onHandleDown`, `dragging`.
- `BuilderPane.tsx` (stacked branch only — `layout="stacked"`, Data Studio's one builder):
  - Replaced the fixed `flex-[3]/flex-[2]` split with the draggable split: top pane gets
    `flexBasis: split.topBasis`, a `role="separator"` divider handle (row-resize cursor,
    hover-highlight, padded hit area) sits between them, bottom pane takes the remainder.
  - `previewHalf` now scrolls internally in stacked mode (`h-full overflow-y-auto`), and the
    preview area got a real `min-h-[16rem]` so it doesn't crush to zero inside the scroller —
    so the viz picker below it is never clipped.
  - Options half already scrolls its tab content (`overflow-y-auto`); unchanged.
  - `split` layout (dashboard-parity, default) is untouched.

## Tests
- `cd ui && pnpm test` → 72 files / 459 tests green (includes `panelEditor.gateway.test.tsx`,
  `flowsPanelEditor.gateway.test.tsx`, `DataStudio.gateway.test.tsx`).
- `tsc --noEmit` clean for the touched files (one pre-existing unrelated unused-var error in
  `transformDebug.gateway.test.tsx`).

Nothing broke, so no debugging entry.

## Follow-up (round 2) — options rail was still cut off with no scrollbar

Root cause: FlexLayout's tab content defaults to `overflow: auto`, so the WHOLE builder tab
scrolled as one block. That let the preview push the options rail off the bottom (cut off, no
inner scroll) and defeated each half's own scrollbar.

Fixes:
- `datastudio-dock.css` — `.data-studio-dock .flexlayout__tab { overflow: hidden }` so the tab
  content is bounded and each half's `min-h-0 flex-1 overflow-y-auto` chain owns scrolling.
- `BuilderPane.tsx` — the resize divider is now a clear full-width band (bordered, `bg-panel`,
  centered grip, hover-highlight) between the two halves, not a thin bar. Lowered the preview
  min-height (`16rem`→`8rem`) so dragging the divider genuinely shrinks the preview instead of
  it clipping.

Re-tested: `pnpm exec vitest run src/features/panel-builder src/features/data-studio` → 8 files
/ 41 tests green. (The 2 failing `theme/*` tests are pre-existing uncommitted theme-customizer
work, not touched here.)

## Round 3 — same up/down resize on the Rules page

The Rules workbench (`RulesView`) had a fixed `max-h-[45%] min-h-[9rem]` result region under the
CodeMirror editor — no way to resize. Same ask as Data Studio.

Refactor for reuse (both pages want the identical band):
- Promoted the hook to `src/lib/split/useVerticalSplit.ts` (generalized the comment) and added a
  shared `SplitHandle.tsx` view (the clear bordered band + centered grip). `src/lib/split/index.ts`
  exports both. Deleted `features/panel-builder/useVerticalSplit.ts`.
- `BuilderPane.tsx` now imports from `@/lib/split` and renders `<SplitHandle/>` instead of its
  inline divider markup.
- `RulesView.tsx` — wrapped the editor in a top pane (`flexBasis: split.topBasis`), the result
  region became the `flex-1` bottom pane, `<SplitHandle/>` between them, split container ref on the
  column. Default 0.7 (editor is the larger half). Dropped the old `max-h-[45%]`/`border-t`.

Re-tested: `pnpm exec vitest run src/features/rules src/features/panel-builder src/features/data-studio`
→ 9 files / 46 tests green (incl. `RulesView.gateway.test.tsx`, `RulesMessaging.gateway.test.tsx`).
`tsc` clean for all touched files.
