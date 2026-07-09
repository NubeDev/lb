# Nav scope — no-lockout (an admin is never trapped in a curated menu; everyone can escape)

Status: **SHIPPED (2026-07-09)** — fixes a recurring live issue. Built end to end: host resolve
(admin-skip + `__builtin__` sentinel) + `nav.save` reserved-id guard, and the shell escape-hatch
(`useResolvedNav` + `NavRail` footer). Builds on the shipped nav builder
([`nav-builder-scope.md`](nav-builder-scope.md)) and its 4-tier resolve. See
[`sessions/nav/nav-no-lockout-session.md`](../../sessions/nav/nav-no-lockout-session.md).

---

## The problem (observed live, more than once)

The sidebar is `nav.resolve`'s output. When **any** nav applies, `NavRail`'s `resolvedMenu` **replaces**
the entire surface list with that nav's (cap-stripped) items. The 4-tier pick is:

```
personal pick  →  first team-shared nav  →  workspace-default  →  built-in SURFACES fallback
```

Two of those tiers apply a nav the user **never chose**:

- **Tier 2 (team share):** the *first* team-shared nav readable by the caller auto-applies. An admin who
  is a member of a team that someone shared a 1-page nav to is **silently narrowed to that page** — the
  admin's entire console (People/Teams/Roles/Nav/…) vanishes from the rail.
- **Tier 3 (workspace default):** setting a workspace default narrows *everyone*, admin included.

And there is **no escape hatch**: once narrowed, the admin has no in-app control to say "show me all my
pages again." They must know to go delete/unshare the nav (which requires reaching the Nav tab — itself
possibly hidden) or clear a pref they can't see. This has bitten the maintainer repeatedly, most recently
via the onboarding wizard sharing a test nav.

**Why this is a real bug, not intended behavior:** a curated nav is meant to *shape a member's* menu, not
to *replace an administrator's console*. Rule 5 says the nav is a **lens that grants nothing** — but a
lens that can *subtract the admin console* from an admin is a footgun, and one that can trap the only
person able to undo it is a lockout.

## The ask

1. **An admin is never auto-narrowed.** Tiers 2 and 3 must NOT auto-apply to a workspace admin. An admin
   sees the built-in console unless **they themselves** set a personal pick (tier 1 — an explicit opt-in).
   Building, sharing, or defaulting a nav can then never strand an admin.
2. **Everyone can escape.** A always-reachable "Show all pages (built-in sidebar)" control that forces the
   fallback rail for the caller, so even a *member* handed a too-narrow nav can bail to the full set of
   pages they can reach. The route gates are untouched (every page was always reachable by deep link);
   this just restores the *menu*.

## Intent / approach

### Part A — admin opt-in only (server; the root cause)

In `nav/resolve.rs::pick_nav`, gate **tiers 2 and 3** on "the caller is not a workspace admin". Concretely:
before returning a `Team` or `WorkspaceDefault` nav, check the principal's caps for the admin marker (the
same `is_admin` the shell uses — a workspace-admin holds the admin cap bundle). If admin, **skip tiers 2
and 3** and fall through to tier 4 (built-in). **Tier 1 (personal pick) still applies to an admin** — an
admin who deliberately picks a nav gets it; the opt-in is explicit and self-inflicted, never silent.

- *Rejected:* "admin sees the union of the nav + all admin pages." That reintroduces the drift the nav
  scope fought (two menu sources); and a nav is a *replace*, not an *augment*, by design. Cleaner to say:
  an admin's rail is only ever narrowed by the admin's own pick.
- *Rejected:* a per-nav `applies_to_admins` flag. More surface, more state, and the failure mode
  (forgetting to set it) is exactly today's. Default-safe (admins opt in) beats opt-out.
- **Symmetry (rule 1):** this is a cap check inside the existing resolve fold — no cloud/edge branch, no
  new verb. It reads the principal's caps, which resolve identically everywhere.

### Part B — the escape hatch (frontend + a reserved pick value)

Reuse `nav.pref.set` with a **reserved sentinel** `active = "__builtin__"` meaning *"I explicitly want the
built-in sidebar; ignore team/default."* `pick_nav` tier 1 treats the sentinel as "force fallback": if
`pref.active == "__builtin__"`, return `None` immediately (skip tiers 2–3). A real nav id is unchanged.

- The shell renders a persistent **"Show all pages"** action (in the rail footer or the member menu) that
  calls `setNavPref("__builtin__")`; and, when a nav is applied, a paired **"Use my menu"** that clears it
  (`setNavPref("")` → normal resolution). Both are `nav.pref.set` (member-level, member-owned — no admin
  cap), so anyone can un-trap themselves.
- The sentinel is **reserved**: `nav.save` must reject `id == "__builtin__"` (and any `__…__` reserved
  shape) so no real nav can collide with it. State the reserved id in `bounds.rs`.

**Why a pref sentinel, not a new verb?** The pick record already exists, is member-owned, and is exactly
"which menu am I using." "Built-in" is one more value of that same axis — no new store, cap, or route.

## How it fits the core

- **Capabilities (rule 5):** unchanged grant surface. Part A reads the caller's existing admin caps inside
  resolve (no new cap). Part B rides `mcp:nav.resolve:call` (already the pref cap). No nav ever grants.
- **Workspace wall (rule 6):** the pref + the admin check are workspace-scoped by the token; nothing
  crosses.
- **Data:** no schema change. `nav_pref.active` gains one reserved string value; `nav.save` gains a
  reserved-id rejection. No migration (existing prefs are real ids or empty).
- **Placement / FILE-LAYOUT:** the admin-skip lives in `nav/resolve.rs::pick_nav` (one function, a few
  lines); the reserved-id guard in `nav/save.rs` + the constant in `nav/bounds.rs`; the UI control in the
  shell rail (its own small component). One responsibility per file.

## Testing plan

Real store/caps/gateway, no mocks (rule 9):

- **Admin not auto-narrowed (headline)** — share a 1-page team nav that the admin can read AND set a
  workspace default; the admin's `nav.resolve` returns `fallback` (built-in), NOT the team/default nav. A
  **non-admin** member of the same team still resolves the team nav (unchanged for members).
- **Admin opt-in still works** — the admin sets a personal pick to that nav → `nav.resolve` returns it
  (`pick`). The opt-in is honored; only the *silent* tiers are gated.
- **Escape hatch** — a member handed a narrow nav sets `nav_pref = "__builtin__"` → resolve returns
  `fallback`; clearing it (`""`) restores the team nav. Round-trips through the real `nav.pref.set`.
- **Reserved id rejected** — `nav.save(id="__builtin__")` is refused server-side (nothing persists).
- **Workspace isolation + deny** — the mandatory pair (ws-B can't read ws-A's pref; `nav.pref.set` still
  needs its cap).

## Risks

- **Defining "admin" for the skip.** Must use the SAME admin signal the rest of the platform uses
  (the workspace-admin cap bundle / `is_admin`) so "who is an admin" has one definition. Don't invent a
  nav-local notion of admin. Pin it to the shared check.
- **A workspace that *wants* to narrow admins.** Out of scope / rejected: if that need ever appears, it's
  an explicit per-nav opt-in, not the default. Default-safe wins.
- **Sentinel collision.** Guard `nav.save` against the reserved shape so a real nav can never BE the
  sentinel. Tested.
