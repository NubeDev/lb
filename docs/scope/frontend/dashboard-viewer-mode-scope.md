# Frontend scope — read-only "viewer" mode for the Dashboards surface (admin-only editing)

Status: shipped. Promotes to `public/frontend/dashboard.md` (viewer-mode section). Target stage:
**S9+ collaboration UI** — a small hardening slice on the shipped `dashboard-scope.md` surface
(`ui/src/features/dashboard/`). Session log: `sessions/frontend/dashboard-viewer-mode-session.md`.

We want the Dashboards surface to have **two postures**, decided purely by the caller's role:

- **Admin** (a workspace-admin) sees the surface as it is today — the roster (left switcher +
  create/rename/delete), drag/resize layout, per-cell edit/delete, add-library-panel, variable
  editor, delete-dashboard.
- **Viewer** (any member *without* an admin cap) opens dashboards and reads the **live widgets**, but
  the **entire authoring surface is gone**: no roster (they land directly on their nav-selected /
  default dashboard), no create/rename, no drag/resize, no per-cell edit/delete, no add-panel, no
  delete-dashboard.

The rule in one line: **editing the dashboard surface is admin-only; a viewer reads it.**

---

## Goals

- A single `canEdit` (== `isAdmin`) boolean, **resolved once** in `DashboardView.tsx` and threaded
  down (FILE-LAYOUT §8 — no scattered `hasCap` checks). It already flows to `DashboardRoster.canEdit`
  and `Grid.editable`; extend that same seam.
- Gate every authoring affordance off it:
  - **No roster** for a viewer — `<DashboardRoster>` does not render at all (no create input, no
    `+` button, no inline rename, no switcher).
  - **Grid not editable** — `editable={canEdit}`, so `isDraggable`/`isResizable` are `false` and the
    per-cell move/remove buttons (`{editable && …}`) are hidden.
  - **No add-library-panel** — the `AddLibraryPanel` bar (`{canEdit && …}`) is hidden.
  - **No delete-dashboard**, **no variable-editor** button — hidden.

## The correctness fix (why this scope exists)

`DashboardView.tsx` previously computed `const canEdit = hasCap(caps, CAP.dashboardSave)`. But
`mcp:dashboard.save:call` is **member-level** — the dev-login/member role holds it (see
`role/gateway/src/session/credentials.rs` `member_caps()` and `lib/session/admin-caps.ts`;
cross-ref memory `dev-login-missing-set-default-cap`). So **every member counted as an editor** and
"viewer mode" was unreachable.

**Decision — reuse the admin-role signal, do NOT mint a new server cap.** `canEdit` is now
`isAdmin(caps)` (`lib/session/admin-caps.ts`), which is true iff the session holds any
`ADMIN_SECTION_CAPS` cap (`teams.manage`, `members.manage`, …) — exactly the workspace-admin role.

- *Rejected: a new `CAP.dashboardAdmin` (`mcp:dashboard.admin:call`).* It would need a server-side
  cap string, a grant on the admin role in `credentials.rs`, and a new gate — churn for no behavioral
  gain over the existing role signal, which already distinguishes admin from member. If a per-surface
  admin cap is ever wanted (finer than "is a workspace admin"), that's a future scope; the UI seam
  (`canEdit`) doesn't change.

## Non-goals / boundaries

- **This UI gate is convenience, not the security boundary (CLAUDE §5).** The gateway still re-checks
  `dashboard.save` / `dashboard.delete` server-side per verb. A viewer who forges a save request is
  refused **server-side** regardless of the UI. That deny is what the mandatory negative-path test
  proves — see below.
- **No server change.** `dashboard.*` stays member-level exactly as shipped; the roster/list a viewer
  *would* see is still membership-filtered by gate 3. Viewer mode is purely a shell posture.
- **No core knowledge of any extension (CLAUDE §10).** Nothing here branches on an extension id.

---

## How it fits the core

| Checklist | This slice |
|---|---|
| Capabilities (§5) | UI gate on `isAdmin`; **server re-checks `dashboard.save`/`.delete`** — the real wall. Mandatory deny-test: a viewer token → **403** on save/delete. |
| Tenancy (§6) | Unchanged — every `dashboard.*` verb is workspace-first; a viewer only ever sees their own workspace's dashboards. Existing isolation test still holds. |
| Symmetric nodes (§1) | No role branch in any core crate — the posture is a UI-side cap read only. |

## Testing plan

Real spawned gateway (`DashboardView.gateway.test.tsx`, CLAUDE §9 — no fake). Two new cases:

1. **viewer** — `signInWithCaps("user:*", ws, [dashboardList, dashboardGet, dashboardSave])` (member
   caps, **no admin cap**), seed a dashboard through the real write path, render, and assert the
   authoring surface is **absent**: no "new dashboard title" input, no roster (`dashboard rail`), the
   grid is present but **not draggable** (no move/remove cell buttons), no "add library panel", no
   "delete dashboard", no "edit variables". The live widget still renders.
2. **admin** — `signInReal` (dev login == admin), same seed, assert every one of those affordances is
   **present**.
3. **server-side deny (mandatory)** — a viewer token calls `saveDashboard` / `deleteDashboard`
   directly and gets **403** (the real wall holds even if the UI were bypassed).

## Open questions

- *Resolved:* reuse `isAdmin` vs new cap → **reuse** (above).
- *Resolved:* where does a viewer land with no roster? → their nav-selected / default dashboard via
  the existing `?d=<id>` URL param (nav resolve already picks it); the roster was only a switcher.
