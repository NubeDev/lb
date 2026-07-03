# Dashboard rename + delete (roster) — session

**Date:** 2026-07-03
**Area:** frontend / dashboard
**Ask:** "For a dashboard I have no way to delete a page or rename it." Wire up delete and rename in the
UI, updating backend and UI as needed.

## Finding: the backend already supports both

- `dashboard.delete(id, now)` — owner-only tombstone-upsert, idempotent
  ([rust/crates/host/src/dashboard/delete.rs](../../../rust/crates/host/src/dashboard/delete.rs)).
- `dashboard.save(id, title, cells, variables, now)` — one idempotent UPSERT; a title-only save on an
  existing id **is** a rename (id + owner + visibility preserved)
  ([rust/crates/host/src/dashboard/save.rs](../../../rust/crates/host/src/dashboard/save.rs)).
- Gateway routes + the `dashboard.api.ts` client (`deleteDashboard`, `saveDashboard`) already exist.

**So this was a UI-only gap — no backend change.** Delete existed as one header button (unconfirmed);
rename had no affordance at all.

## What shipped (UI)

1. **`useDashboard.rename(id, title)`** — title-only save that first loads the target's cells/variables
   (from `current` when it's the open dashboard, else `getDashboard(id)`) so a rename never blanks the
   layout ([ui/src/features/dashboard/useDashboard.ts](../../../ui/src/features/dashboard/useDashboard.ts)).
2. **`DashboardRoster`** — per-item **rename** (pencil → inline edit → confirm/cancel) and **delete**
   (trash → shared `ConfirmDestructive` gate), both gated on `canEdit` (`mcp:dashboard.save:call`);
   controls reveal on row hover
   ([ui/src/features/dashboard/DashboardRoster.tsx](../../../ui/src/features/dashboard/DashboardRoster.tsx)).
3. **`DashboardView`** — wires `onRename`/`onRemove`/`canEdit` into the roster and routes the existing
   header Delete button through `ConfirmDestructive` too
   ([ui/src/features/dashboard/DashboardView.tsx](../../../ui/src/features/dashboard/DashboardView.tsx)).

## Tests

- **`DashboardRoster.test.tsx`** (new, unit) — create-with-slug, inline rename, cancel-rename,
  delete-behind-confirm, and the `canEdit` gate hides the controls. **5/5 green** in the full unit suite.
- **`DashboardView.gateway.test.tsx`** — added two real-gateway cases (rename preserves layout + persists;
  delete through the confirm gate removes the row). NOTE: the whole `test:gateway` suite could not run
  green in this sandbox — the spawned `test_gateway` node / jest-dom global setup isn't available here, so
  **every** test in that file fails identically (including the pre-existing ones), not just the new ones.
  The new cases are written in the same real-backend style (no mocks, per CLAUDE §9) and should be
  re-run where the gateway bin is available.

## Capability / isolation

The rename/delete controls are gated client-side on the session `dashboard.save` grant, but the boundary
is the host: `dashboard.save`/`dashboard.delete` re-check workspace-first then
`mcp:dashboard.<verb>:call` and enforce owner-only mutation. The per-verb deny + gate-3 membership deny
are proven in the Rust dashboard tests; workspace isolation is covered by the existing
"fresh workspace shows no dashboards" gateway case.
