# Session — nav no-lockout (an admin is never trapped in a curated menu; everyone can escape)

Date: 2026-07-09 · Scope: [`scope/nav/nav-no-lockout-scope.md`](../../scope/nav/nav-no-lockout-scope.md)

## Trigger (live)

While building the onboarding wizard, the maintainer shared a test nav and their **entire admin
sidebar was replaced by that nav's single item** — the admin console (People/Teams/Roles/Nav/…) vanished
from the rail, with no in-app way back. They noted it had happened before. Root cause: `nav.resolve`'s
tier-2 (any readable team-shared nav) and tier-3 (workspace default) **auto-apply to anyone, admin
included**, and `NavRail` **replaces** the surface list with the nav's items. A curated nav meant to
shape a *member's* menu was silently subtracting the *admin's* console — a lockout.

## What shipped

**Part A — admin never auto-narrowed (server, the root fix).** In `nav/resolve.rs::pick_nav`, tiers 2
and 3 are now **skipped for a workspace admin**. An admin falls straight through to the built-in
fallback rail unless **they themselves** set a tier-1 personal pick (explicit opt-in, never silent). The
admin signal is cap-based — a new `nav/admin_lens.rs::is_workspace_admin` mirrors the shell's
`ADMIN_SECTION_CAPS` (any admin-marker cap present), so "who is an admin" has one definition. Members are
unaffected: tiers 2/3 still apply to them.

**Part B — the escape hatch (everyone).** A reserved `nav_pref.active = "__builtin__"` sentinel
(`nav::BUILTIN_PICK`) means "force the built-in sidebar; skip team/default." `pick_nav` returns `None`
for it immediately. `nav.save` rejects the reserved `__…__` id shape (`bounds.rs::check_id`) so no real
nav can collide. The shell (`useResolvedNav` + `NavRail` footer) renders **"Show all pages"** when a nav
is applied and **"Use my menu"** when forcing built-in — both write the member-owned pick, so anyone can
un-trap themselves, no admin needed.

## Files

- Rust: `nav/admin_lens.rs` (new — `is_workspace_admin`), `nav/resolve.rs` (`pick_nav` skip + sentinel),
  `nav/bounds.rs` (`BUILTIN_PICK` + `check_id`), `nav/save.rs` (`check_id` call), `nav/mod.rs` +
  `lib.rs` (exports `NAV_BUILTIN_PICK`).
- UI: `lib/nav/nav.api.ts` (`BUILTIN_PICK`), `features/shell/useResolvedNav.ts` (`usingBuiltin` +
  `showAllPages`/`useMyMenu`), `features/shell/NavRail.tsx` (footer control),
  `features/routing/RoutedShell.tsx` (wire-through).

## Key decisions (rejected alternatives)

- **Admins opt-in, not opt-out.** *Rejected* a per-nav `applies_to_admins` flag — its failure mode
  (forgetting to set it) is exactly today's bug. Default-safe wins.
- **Nav replaces, not augments.** *Rejected* "admin sees nav ∪ all admin pages" — reintroduces the
  two-menu-source drift the nav scope fought. An admin's rail is only ever narrowed by their own pick.
- **A pref sentinel, not a new verb.** The pick record already exists and is member-owned; "built-in" is
  one more value of the same "which menu am I using" axis — no new store/cap/route.
- **Cap-based admin signal, one definition.** *Rejected* a nav-local notion of admin; mirror the shell's
  `ADMIN_SECTION_CAPS` so it can't drift.

## Tests

`nav_test.rs` (real store/node, no mocks):

- `admin_never_auto_narrowed_member_still_is` — an admin with a team-shared nav AND a workspace default
  resolves to `Fallback`; a non-admin member of the same team still resolves `Team`; the admin's own
  explicit pick still applies (`Pick`).
- `builtin_pick_sentinel_forces_fallback` — a member forces built-in via `__builtin__` → `Fallback`;
  clearing (`""`) → `Team` again.
- `reserved_nav_id_rejected` — `nav.save` refuses `__builtin__` and any `__…__` id.

Existing nav tests unaffected (their principals hold only `nav.*` caps, never an admin marker, so the
admin-skip never triggers for them). UI: `tsc` clean; new files lint clean.

## Follow-up

- The escape-hatch labels live in the rail footer; a future pass could also surface "you're seeing a
  curated menu" more prominently (a one-line banner) the first time a narrow nav applies.
