# Nav scope — admin hide + user pins (sidebar curation)

Status: **BUILT (2026-07-08)** — see
[`sessions/nav/nav-hide-and-pins-session.md`](../../sessions/nav/nav-hide-and-pins-session.md)
(includes the open-question resolutions and one deliberate deviation: the verbs ride the existing
`nav.save`/`nav.resolve` grants, like `nav.set_default`, instead of minting new caps). Promoted to
`public/nav/nav.md`.

Two small, symmetric additions on top of the shipped nav builder
(`nav-builder-scope.md`). **(1) Admin hide:** a workspace admin, from the admin/settings
area, hides entries from the sidebar for everyone in the workspace — e.g. "hide the
Dashboard surface" — without having to author a full replacement nav. **(2) User pins:**
any member pins pages they care about (a dashboard, an extension page, a system surface)
to a "Pinned" section at the top of their rail — personal favorites, ordered by the user.
Both are **lenses over existing access** exactly like the nav itself: hiding never blocks
a route, pinning never grants one.

---

## Goals

- **A workspace hidden-set** — one admin-managed record `nav_hidden:[ws]` holding an
  ordered-irrelevant set of item refs to suppress: surface keys (`"dashboard"`,
  `"channels"`, …), `ext:<id>` page refs (opaque, rule 10), and `dashboard:<id>` refs.
  Applied inside `nav.resolve` at **every** tier — personal pick, team nav, workspace
  default, **and the built-in `SURFACES` fallback** — so "hide dashboard" works even in a
  workspace that never authored a nav.
- **Admin verbs** — `nav.hidden_get{}` / `nav.hidden_set{hidden: []}` (full-set LWW
  write, like `nav.set_default`), admin-capped, wired end to end
  (store → cap → MCP → gateway → `http.ts` → UI).
- **A settings surface** — a "Sidebar" tab in the existing admin area (alongside the nav
  builder tab in the access-console, per the shipped decision) listing every visible
  source (built-in `SURFACES`, `ext.list` pages) with a hide toggle per row.
- **User pins in `nav_pref`** — extend the existing member-owned `nav_pref:[ws, user]`
  record (already scoped as the home for "optional pinned/reordered favorites") with
  `pinned: NavItemRef[]` — an **ordered** list of the same entry refs. No new table, no
  new verb family: pins ride the `nav_pref` read/write path the active-pick already uses.
- **Rail rendering** — NavRail renders a **Pinned** section above the resolved menu.
  Pin/unpin affordances: a star toggle on rail items (context/hover) and on page headers
  where cheap. Pinned items are cap-stripped and hidden-stripped like everything else.
- **Deep links untouched** — `CoreGate` and server-side re-checks are unchanged. A hidden
  page a caller is permitted to reach still loads by URL; hide is declutter, not authz.

## Non-goals

- **No per-user hide** — a member who wants a smaller rail picks/authors a personal nav
  (shipped). Hide is a workspace-admin curation tool only.
- **No per-team hide** — team-shaped menus are the nav builder's job (share a nav to the
  team). One hidden-set per workspace; deferred if a real ask appears.
- **No pin sharing / team pins** — pins are strictly personal. A shared "starter set" is a
  workspace-default nav, which exists.
- **No new authorization semantics** — hide and pin grant/deny **nothing** (nav-builder
  non-goals hold verbatim).
- **No pinning of arbitrary URLs** — pins reference the same entry kinds `nav.items[]`
  already supports; no free-text links in v1.

## Intent / approach

**Both features are deltas on the shipped nav plane, not a new system.** The obvious
alternative for "admin hides the dashboard" — *author a workspace-default nav that omits
it* — already works but is heavyweight (the admin must enumerate everything they *do*
want, and the hide silently stops applying the moment a user picks a personal or team
nav). A subtractive hidden-set composes with all of it: authored navs stay additive,
`nav.resolve` applies the subtraction last, and the built-in fallback is covered too.
*Rejected:* a `hidden` flag per `SURFACES` entry in UI state/localStorage — state belongs
in SurrealDB (rule 2) and must apply to every client of the workspace.

**Pins live where the scope already said they would.** `nav-builder-scope.md` reserved
`nav_pref:[ws, user]` for "which nav am I using (and optional pinned/reordered
favorites)". We fill that reservation: `pinned` is a second field on the same record,
member-owned, same caps. *Rejected:* a separate `nav_pin` table or a `lb-prefs` axis —
both add surface for a list that is inseparable from the user's nav state (and the prefs
axis set is deliberately closed to formatting/localization).

**Precedence: hide beats pin.** An admin-hidden ref is stripped even from a user's pinned
section. Rationale: hide is the workspace's one curation lever; if pins could resurrect a
hidden entry the admin has no way to actually declutter the rail. The user loses nothing
real — the page remains reachable by deep link. *Rejected:* pin-wins — it makes
`nav.hidden_set` advisory and its effect untestable.

## How it fits the core

- **Tenancy / isolation (rule 6):** `nav_hidden:[ws]` is keyed by workspace; `nav_pref`
  already is (`[ws, user]`). ws-B can never read/write ws-A's hidden-set or pins. Tested.
- **Capabilities (rule 5):**
  - `mcp:nav.hidden_get:call` — member-level read (`nav.resolve` needs it internally;
    the settings tab reads it).
  - `mcp:nav.hidden_set:call` — **admin write**, granted to the `workspace-admin` built-in
    role by default, revocable like any grant (no bypass, rule 10).
  - Pins reuse `store:nav_pref:read|write` (member-owned; a member writes only their own
    record — cannot set another user's pins).
  - **Deny path:** a member calling `nav.hidden_set` without the cap → denied at gate 2,
    nothing persists. A user pinning a page they can't read → the pin persists as data but
    `nav.resolve` **strips it** (lens), and the route re-check still denies. Both tested.
- **Placement:** either (rule 1) — store state + cap checks, no cloud branch.
- **MCP surface (API shape §6.1):**
  - **Get / set** — `nav.hidden_get{}` / `nav.hidden_set{hidden}` (full-set LWW, one
    record; no per-item CRUD needed for a small set — say so, per §6.1). Pins: no new
    verbs; the existing `nav_pref` read/write carries `pinned[]`.
  - **CRUD / live feed / batch — N/A.** The hidden-set is one bounded record (cap: 200
    refs, `BadInput` over); pins are bounded (cap: 50 per user). Changes are rare; the UI
    re-resolves on focus/visit like the rest of the nav plane. No SSE, no jobs.
- **Data (SurrealDB, rule 2):** one new single-record table `nav_hidden` (SCHEMAFULL:
  `hidden: array<string>` of typed refs, `updated_ts`); one additive field
  `pinned: array` on `nav_pref`. Additive migration only — existing `nav_pref` records
  without `pinned` resolve as empty.
- **Bus (Zenoh, rule 3):** N/A — pure state; no nav events (consistent with the shipped
  nav plane).
- **Sync / authority:** same posture as `nav`/`nav_pref` — node-local store, LWW single
  records merge cleanly offline.
- **Secrets:** N/A.
- **Rule 10 (core knows no extension):** hidden/pinned `ext:<id>` refs are **opaque
  strings** matched by equality against `ext.list`-discovered pages — no branch on any
  named extension, no special-case icon/route.
- **SDK/WIT impact:** none — no plugin-boundary change.

## Example flow

1. An admin opens **Settings → Sidebar**. The tab lists the built-in surfaces and the
   `ext.list` pages. They toggle **Dashboard** off → `nav.hidden_set{hidden:
   ["dashboard"]}` writes `nav_hidden:[ws]`.
2. **Ada** (no personal nav; workspace has no default) reloads. `nav.resolve` falls back
   to `SURFACES`, subtracts the hidden-set → her rail has no Dashboard entry. A bookmark
   to `/dashboard/cooler-health` still loads (she holds read; `CoreGate` allows).
3. Ada stars the **Rules** surface and the `ext:mqtt` status page → her
   `nav_pref.pinned = ["rules", "ext:mqtt"]`. Her rail now shows **Pinned ▸ Rules ·
   MQTT** above the menu, in her order.
4. **Ben** (same workspace) sees no Pinned section — pins are per-user — and also no
   Dashboard entry — hide is per-workspace.
5. The admin later hides `ext:mqtt` too. On Ada's next resolve her pinned MQTT entry is
   stripped (hide beats pin); her `nav_pref` record is untouched, so un-hiding restores it.
6. The `mqtt` extension is uninstalled → the pinned ref no longer matches `ext.list` and
   strips silently at resolve, exactly like the shipped uninstalled-ext rule.

## Testing plan

Per `scope/testing/testing-scope.md`, real store/caps/gateway, seeded real records
(no mocks, rule 9):

- **Capability deny (mandatory)** — `nav.hidden_set` denied without the admin cap,
  nothing persists; a member cannot write another user's `nav_pref` pins.
- **Workspace isolation (mandatory)** — ws-A's hidden-set has no effect on ws-B's
  resolve; ws-B cannot read/write ws-A's `nav_hidden` or pins.
- **Hide never blocks (headline)** — hide the `rules` surface for a caller who holds
  `rules.*`: `nav.resolve` omits it from menu *and* pins, yet the direct route/verb still
  succeeds. Symmetric with the shipped "nav never widens" test.
- **Hide applies at every tier** — with a personal pick, a team nav, a workspace default,
  and the bare `SURFACES` fallback: the hidden ref is absent in all four.
- **Pin lens** — a pin to a dashboard the user can't read is stripped; un-hide/regrant
  restores it without rewriting `nav_pref`.
- **Bounds** — `nav.hidden_set` over 200 refs and a 51st pin → `BadInput`, no silent
  truncation.
- **Idempotent LWW** — `nav.hidden_set` twice merges cleanly; additive `nav_pref`
  migration (old record without `pinned` resolves as empty).
- **UI (real spawned gateway, `pnpm test:gateway`)** — the Sidebar settings tab
  round-trips a hide toggle → NavRail drops the entry; star/unstar round-trips a pin →
  Pinned section renders, ordered, cap- and hidden-stripped.

## Risks & hard problems

- **The strip pipeline gains a third filter** — `nav.resolve` now composes cap-strip,
  uninstalled-ext-strip, and hidden-strip (then pins on top). Keep it one ordered pure
  function with the precedence (hide > pin) stated in one place, or the tiers will drift.
- **Settings tab must enumerate honestly** — the hide list must be driven from the same
  `SURFACES` source and live `ext.list`, never a copy (the shipped fallback-lockstep risk,
  again). A stale copy hides the wrong things or misses new surfaces.
- **Locked-out-of-settings footgun** — hiding the admin/settings surface itself would
  strand the UI-only admin. Either exclude the settings surface from the hideable list or
  rely on deep links; **decide in build** (open question below).
- **Pin refs rot** — pinned dashboards get deleted, exts uninstalled. Strip-silently is
  the shipped answer; make sure stripping never mutates the stored record (so restores are
  free) and never errors the rail.

## Open questions

- Can the admin hide the settings/admin surface itself? Recommend: yes, but the Sidebar
  tab shows a warning and deep links remain the escape hatch — pick during build.
- Does the pin star appear on page headers in v1, or rail-context-menu only? Recommend:
  rail-only first (one seam), header stars as a follow-up.
- Should `nav.resolve`'s response mark *why* an item was stripped (debug field) to make
  the settings tab's preview honest? Recommend: yes, behind a `debug: true` arg.

## Related

- `nav-builder-scope.md` — the shipped nav plane this extends (`nav.resolve`,
  `nav_pref`, the lens principle, the access-console builder tab).
- `public/nav/nav.md` — the shipped truth to update on promote.
- `scope/auth-caps/access-console-scope.md` — home of the admin settings tab.
- `scope/prefs/user-prefs-scope.md` — why pins are **not** a prefs axis.
- `scope/frontend/routing-scope.md` — `SURFACES`, `allowedSurfaces`, `CoreGate` (hide
  never touches these gates).
- README `§3` rules 2 (one datastore), 5 (capability-first), 6 (workspace wall),
  10 (core knows no extension).
- Skill: `skills/nav/SKILL.md` — **required update on ship** (new `nav.hidden_get/set`
  verbs and the `nav_pref.pinned` field are agent-drivable surface).
