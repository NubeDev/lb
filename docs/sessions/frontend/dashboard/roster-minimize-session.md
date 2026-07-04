# Dashboard roster minimize — fold the rail to a thin strip (session)

- Date: 2026-07-04
- Scope: n/a (small UI affordance; sits inside the shipped dashboard surface)
- Stage: post-S8 (frontend polish)
- Status: done

## Goal
Give the dashboard roster (the left sub-sidebar listing the workspace's dashboards, with its per-row
rename/delete icons) a single **minimize** affordance so an admin can fold it down to a thin strip and
give the live grid the full body width — and a symmetric control to bring it back. Matches the
sub-sidebar-collapse pattern users already know from Grafana/Linear/Notion.

## What changed

**`DashboardRoster.tsx`** — new optional `onCollapse?: () => void` prop. When wired, the rail header
gains a `PanelLeftClose` icon button (aria-label `minimize dashboard rail`) next to the existing
**New dashboard…** input + **+** create button. No-op when not wired (so the unit harness and any
other caller are unchanged). The doc comment names the new affordance and points at the symmetric
expand control in `DashboardView`.

**`DashboardView.tsx`** — owns the `rosterOpen` state (defaults **true** — existing tests and the
default admin visit are unchanged). When open it renders `DashboardRoster` as before plus
`onCollapse={() => setRosterOpen(false)}`. When closed it renders a thin `w-10` rail (`aside`,
aria-label `dashboard rail collapsed`) holding a single `PanelLeftOpen` icon button
(aria-label `expand dashboard rail`) that flips the state back. The grid is `min-w-0 flex-1` already,
so it just absorbs the freed width — no layout math. The whole branch is still gated on `canEdit`
(roster remains an admin-only authoring surface; a viewer still lands directly on their nav-selected
dashboard).

## Decisions & alternatives
- **State in the view, not the roster.** The roster owns markup + local edit state; the open/closed
  *rail* is a layout decision the host (`DashboardView`) owns — same split as the rename/delete wiring
  (caller supplies the verb, roster supplies the chrome). Rejected hosting the state in the roster: it
  would force the roster to render its own collapsed strip and double its responsibilities.
- **Symmetric controls, two icons.** `PanelLeftClose` when open (the rail is visible, the icon shows
  what folding does), `PanelLeftOpen` when collapsed (the strip points back). Rejected a single
  `Minus`/`ChevronLeft`-style toggle — the `PanelLeft*` pair is the conventional idiom and reads
  without a tooltip.
- **No persistence.** First cut defaults open each visit. If users want it remembered, that's a
  follow-up (a `localStorage` key or a `?rail=0` URL param so it round-trips with the shareable
  dashboard link) — not in this slice.
- **AppRail untouched.** `AppRail` is the shared chrome for Flows/Dashboard/Rules; folding is wired
  per-surface, not pushed into the shared primitive (Flows/Rules didn't ask for it).

## Tests
- **UI unit** (`pnpm test`, `DashboardRoster.test.tsx`): +2 —
  `renders no minimize button when onCollapse is not wired` (the opt-in contract), and
  `fires onCollapse when the minimize button is clicked`. The existing 5 roster tests unchanged.
  Suite: 563 passed (was 561).
- No backend, no capability, no workspace-isolation surface touched (roster is markup + a state flip;
  the host re-checks `dashboard.save`/`.delete` server-side, unaffected).

```
# ui unit (dashboard roster)
 ✓ src/features/dashboard/DashboardRoster.test.tsx (7 tests)
```

Mandatory categories: **capability-deny** n/a (no new cap surface — the rail is already admin-gated
by `canEdit = isAdmin(caps)` in `DashboardView`), **workspace-isolation** n/a (read-path UX only).

## Debugging
None opened.

## Public / scope updates
Minor UX affordance — no scope was opened for it and the `public/frontend/dashboard.md` "Data Studio
editing loop" framing is unchanged. Recorded here so the next session can see the rail is foldable.
