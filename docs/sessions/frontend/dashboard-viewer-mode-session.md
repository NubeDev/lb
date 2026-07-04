# Dashboard read-only "viewer" mode (admin-only editing) — session

**Date:** 2026-07-04
**Area:** frontend / dashboard
**Status:** shipped (green)
**Scope:** [scope/frontend/dashboard-viewer-mode-scope.md](../../scope/frontend/dashboard-viewer-mode-scope.md)
**Ask:** Add a viewer mode to the Dashboards surface — a non-admin sees dashboards but cannot change
them; only an admin can edit. Editing the dashboard surface is **admin-only**.

## The bug this fixes (why the ask exists)

`DashboardView.tsx` computed `const canEdit = hasCap(caps, CAP.dashboardSave)`. But
`mcp:dashboard.save:call` is **member-level** — the dev-login/member role holds it (see
[credentials.rs](../../../rust/role/gateway/src/session/credentials.rs) `member_caps()` lines 193-201,
and [admin-caps.ts](../../../ui/src/lib/session/admin-caps.ts) line 66-68; cross-ref memory
`dev-login-missing-set-default-cap`). So **every member counted as an editor** and viewer mode was
unreachable — the authoring surface showed for everyone.

## Decision: reuse `isAdmin`, do NOT mint a new server cap

`canEdit` now gates on `isAdmin(caps)` ([admin-caps.ts](../../../ui/src/lib/session/admin-caps.ts)
line 152) — true iff the session holds any `ADMIN_SECTION_CAPS` cap (`teams.manage`,
`members.manage`, …), i.e. the workspace-admin role. The dev login holds those; a plain member does not.

- **Rejected:** a new `CAP.dashboardAdmin` (`mcp:dashboard.admin:call`). It needs a server cap string,
  a grant on the admin role in `credentials.rs`, and a new gate — churn with no behavioral gain over the
  role signal that already separates admin from member. If a per-surface admin cap is ever wanted, that's
  a future scope; the UI seam (`canEdit`) is unchanged either way.
- The server-side gates are **untouched** — `dashboard.save`/`.delete` stay member-level and the gateway
  re-checks them per verb (§5). This UI gate is **defense-in-depth**; the real deny holds server-side.

## What shipped (UI — one boolean, threaded)

All in [DashboardView.tsx](../../../ui/src/features/dashboard/DashboardView.tsx) — resolved once, passed down (FILE-LAYOUT §8):

1. `canEdit = isAdmin(caps)` (was `hasCap(caps, CAP.dashboardSave)`).
2. **Roster hidden for viewers** — `{canEdit && <DashboardRoster … />}`. A viewer has no left switcher,
   no "New dashboard…" input, no `+`, no inline rename; they land on their nav-selected / default
   dashboard via the existing `?d=<id>` URL param.
3. **Grid non-editable** — `editable={canEdit}` (was a bare literal `editable`). `Grid.tsx` already keys
   `isDraggable`/`isResizable` and the `{editable && …}` per-cell move/remove buttons off it, so a viewer
   gets a static grid with no drag/resize/edit/delete.
4. **AddLibraryPanel hidden** — the `{canEdit && …}` add-panel bar (already gated; now correctly off for
   members).
5. **Delete-dashboard + variable-editor buttons** — wrapped in `{canEdit && …}`.

No change needed in `Grid.tsx` / `DashboardRoster.tsx` / `AddLibraryPanel.tsx` — they already consumed
`editable` / `canEdit`; the fix was **what feeds them** and hiding the roster wholesale.

## Tests (real spawned gateway — CLAUDE §9, no fake)

Added to [DashboardView.gateway.test.tsx](../../../ui/src/features/dashboard/DashboardView.gateway.test.tsx):

1. **VIEWER** — admin seeds a dashboard + shares it `workspace`; a different principal signs in via
   `signInWithCaps` with member caps and **no admin cap**. Asserts the cell mounts (reads OK) and the
   whole authoring surface is **absent**: no create input/`+`, no `dashboard rail`, no `move cell`/
   `remove cell`, no add-library-panel, no delete-dashboard, no edit-variables.
2. **ADMIN** — dev login (== admin); asserts every one of those affordances is **present** (the mirror).
3. **VIEWER DENY (mandatory, server-side)** — a token narrowed to `[dashboard.list, dashboard.get]`
   (no `dashboard.save`) calls `saveDashboard`/`deleteDashboard` directly and both **reject** — the
   gateway is the wall even if the UI were bypassed.

### Green output (stable across 3 consecutive runs)

```
 ✓ src/features/dashboard/DashboardView.gateway.test.tsx (11 tests) 2356ms
   ✓ … VIEWER: a non-admin member gets NO authoring surface — no roster/create/drag/edit/delete/add
   ✓ … ADMIN: a workspace admin gets the full authoring surface — roster/create/drag/edit/delete/add
   ✓ … VIEWER DENY (server-side, mandatory): a viewer without admin still can't save/delete …
 Test Files  1 passed (1)
      Tests  11 passed (11)
```

Unit: `DashboardRoster.test.tsx` **5/5** (the `hasCap→isAdmin` swap didn't touch the roster's own
`canEdit` contract). Pre-existing, out-of-scope tsc reds untouched (accordion `Collapse`, flows
`FlowsCanvas.gateway`, `transformDebug` unused import) — none in files this session touched.

## Notes / gotchas

- The viewer test first hit the **workspace wall working correctly**: a private dashboard owned by the
  admin is invisible to a second principal (gate 3). Fixed by sharing it `workspace` — the point is
  role (member vs admin), not ownership.
- `react-grid-layout` always renders `react-resizable-handle` spans structurally even when
  `isResizable=false`; the honest edit markers to assert on are the gated `move cell`/`remove cell`
  buttons, not the handle DOM.
- **Harness fix (not viewer-mode, but required for green):** mid-session, concurrent in-flight
  theme/motion work landed in the tree (`components/app/page.tsx` now renders a `Reveal` motion
  primitive → `useMotionPref` → `useTheme`). That made a bare `DashboardView` render throw
  `useTheme must be used within ThemeProvider` and broke **every** test in the gateway file (not just
  the new ones). Fixed by wrapping both render helpers in the **real** `ThemeProvider` (exactly as the
  shell's `App.tsx` does) — no fake theme layer. This is a test-harness alignment with the shipped
  shell, orthogonal to the viewer-mode change.

## Cross-links

- Scope: [scope/frontend/dashboard-viewer-mode-scope.md](../../scope/frontend/dashboard-viewer-mode-scope.md)
- Public: [public/frontend/dashboard.md](../../public/frontend/dashboard.md) (viewer-mode section)
- No debug entry — nothing broke beyond the self-corrected test setup above.
