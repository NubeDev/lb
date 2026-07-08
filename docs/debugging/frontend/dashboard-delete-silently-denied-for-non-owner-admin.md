# Admin clicks Delete on a dashboard they don't own — confirm dialog runs, nothing happens

**Area:** dashboard ownership + admin surface (host + gateway + UI)
**Date:** 2026-07-08
**Symptom:** An admin opens a dashboard's delete affordance in the roster, the destructive-confirm
dialog appears, they click Delete — and the dashboard stays in the roster. No visible crash; the
confirm dialog just closes and nothing changes. Reported as "I can't seem to be able to delete a
dashboard from the UI."

## Root cause

`DashboardRoster` shows the rename/delete hover controls whenever `canEdit` is true, and
`DashboardView` sets `canEdit = isAdmin(caps)` — i.e. holding **any** admin cap (`ADMIN_SECTION_CAPS`)
is enough for the delete button to render, for every dashboard in the roster, regardless of who owns
it.

But `dashboard_delete` (`crates/host/src/dashboard/delete.rs`) enforced **owner-only**, with no admin
override — the same convention as `dashboard.save`/`.share` and the panel/nav/assets delete verbs
(`d.owner != principal.owner_sub() → Denied`). So an admin who isn't the owner (e.g. deleting a
dashboard an agent created on a member's behalf, or one another admin made) gets an opaque `Denied`.
The error DOES surface via `AppPage`'s `role="alert"` strip — but it's easy to miss, and from the
user's point of view the delete affordance simply "does nothing," because the UI never told them the
button only works for dashboards they own.

## Fix

Added a scoped admin override, `mcp:dashboard.delete_any:call` — checked as a **second** gate inside
`dashboard_delete`, only when the caller isn't the owner (so a non-admin never pays its cost, and
`save`/`share` stay strictly owner-only, matching the rest of the asset model, §3.5):

```rust
if d.owner != principal.owner_sub()
    && authorize_dashboard(principal, ws, "dashboard.delete_any").is_err()
{
    return Err(DashboardError::Denied);
}
```

- Registered `dashboard.delete_any` in the host tool catalog (`system/catalog.rs`).
- Granted `mcp:dashboard.delete_any:call` in the dev-login cap set (`gateway/session/credentials.rs`)
  — the dev login "doubles as admin," same convention as `prefs.set_default`. A real non-admin member
  role must not carry it; a real admin role picks it up via `roles_define`/`grants_assign` like any
  other admin cap.
- Added the matching UI cap constant `CAP.dashboardDeleteAny` (`lib/session/admin-caps.ts`) for future
  use (e.g. gating the delete affordance per-item once `DashboardSummary` exposes ownership).

This is a capability-model change, not a role flag — the host has no `principal.is_admin()`;
"admin" is only ever a bundle of granted caps (rule 5). `dashboard.delete_any` is one more capability
string, checked the same way every other MCP verb is.

## Regression tests

- `dashboard_test::delete_any_cap_lets_a_non_owner_admin_delete` — a non-owner with only the base
  `dashboard.delete` cap stays denied; the same principal granted `dashboard.delete_any` too succeeds.
- `credentials::tests::dev_login_carries_dashboard_delete_any` — locks the dev-grant in (same shape as
  the `prefs.set_default` regression test).
- Full `dashboard_test` suite (11 tests) and `dashboard_routes_test` (6 tests, real gateway) green.

**Not fixed here (follow-up, out of scope):** the roster still shows the delete button for every
dashboard an admin can see, even ones `dashboard.delete_any` won't help with if that cap isn't granted
to their role — the UI doesn't yet distinguish "mine" from "not mine" because `DashboardSummary` has
no owner/mine field. Worth a follow-up if per-item gating (vs. a clear denied-error message) is wanted.
