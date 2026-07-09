# Session — sidebar "Position: Right" layout fix

Scope: workspace **Settings → Theme → Layout → Position** (Left / Right). Bug report: choosing
**Right** broke the whole app shell — the routed page, Settings panel, and action chrome lost their
structure and slid under the right-pinned sidebar. Left worked. Ask: make Right mirror Left.

## Root cause

The vendored shadcn sidebar (`ui/src/components/ui/sidebar.tsx`) was ported without upstream's
side-aware spacing. The real sidebar body (`sidebar-container`) is `position: fixed`; the only thing
reserving space for the main content is an in-flow `sidebar-gap` spacer that renders **before**
`SidebarInset` in DOM order — so it always reserved width on the **left**. When `side="right"` the
fixed container correctly pinned to `right-0`, but nothing pushed the content off the right edge, so
`SidebarInset` rendered full-width under the sidebar. `side` was threaded correctly end to end
(`theme.layout.side` → `NavRail` → `<Sidebar side>` → `data-side` on the wrapper); the gap was
purely a CSS/layout omission.

## What shipped

`ui/src/components/ui/sidebar.tsx` — two changes, no logic, no new props/state:

- **`Sidebar` wrapper** now carries `order-first ... data-[side=right]:order-last`. On the right, this
  moves the entire sidebar wrapper — its space-reserving `sidebar-gap` included — *after*
  `SidebarInset` in the flex row (`SidebarProvider`'s wrapper is `flex w-full`). The gap therefore
  reserves its width on the right, mirroring the left layout exactly. The gap stays the single source
  of spacing (no duplicated icon/expanded/offcanvas width logic on a margin), and it already collapses
  to `w-0`/icon width per collapsible mode, so all three modes reclaim/reserve the correct side
  automatically.
- **`sidebar-container` border** is now side-aware: `border-r` on the left, `border-l` on the right,
  so the divider sits on the inner edge in both positions. (`floating`/`inset` variants keep their
  symmetric `p-2` and need no mirroring.)

Deliberately **not** changed: `RoutedShell.tsx` does not branch on `side` or pass it to
`SidebarProvider` — the fix lives entirely in the vendored sidebar CSS keyed off the `data-side` the
wrapper already emits, matching upstream shadcn and keeping the shell generic.

## Testing

- `cd ui && pnpm exec vitest run src/components/ui src/features/theme src/lib/theme src/features/routing`
  → **23 files / 113 tests pass**, including
  `LayoutTab > switches sidebar variant, collapsible mode, and position through accessible cards`.
- Full `pnpm test` had 3 pre-existing failures in `src/features/flows/debug/DebugValueView.test.tsx`
  (work-in-progress debug feature — `DebugRow.tsx` was untracked in the tree at session start) plus a
  vitest worker OOM on the ~25-min full run. Neither is related to this change; the isolated layout
  suite is fully green.
- Visual end-to-end drive in the real app was not run: this environment has no browser binary and the
  UI requires a real spawned gateway node (no mocks, rule 9) — a heavy cold start to verify a
  self-contained flexbox `order` swap already covered by the passing `LayoutTab` interaction test.
  Recommend a manual pass: for each variant (Default/Floating/Inset) × collapsible mode
  (Off Canvas/Icon/None), toggle Left/Right and confirm Right mirrors Left with no overlap or dead
  strip, and Cmd/Ctrl+B reserves the correct side.
