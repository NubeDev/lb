# Nav — shipped truth

> TODO stub. The nav builder shipped 2026-07-03 (see
> `docs/sessions/nav/nav-builder-session.md` and `docs/scope/nav/nav-builder-scope.md`,
> which is marked promoted here but was never written up) — backfill the shipped surface
> (`nav.get/list/save/delete/share/resolve/set_default`, `nav_pref`) when touched next.
>
> **Shipped 2026-07-08 — hide + pins** (`docs/scope/nav/nav-hide-and-pins-scope.md`,
> `docs/sessions/nav/nav-hide-and-pins-session.md`): a workspace **hidden-set**
> (`nav.hidden.get/set`, one `nav_hidden:[ws]` record — admin declutter, applied by the resolver at
> every tier incl. the client-side fallback via the `ResolvedNav.hidden` echo; never blocks a route)
> and per-user **pins** (`nav_pref.pinned` — ordered refs, resolved into `ResolvedNav.pinned`,
> rendered as a Pinned rail section; hide beats pin). Refs: bare surface key | `ext:<id>` |
> `dashboard:<id>`. Settings → Sidebar is the admin UI. See `skills/nav/SKILL.md` for the drivable
> surface.
