# Auth-caps scope — nav gates reach (the curated menu is the allow-list of reachable pages)

Status: scope (the ask). Promotes to `doc-site/content/public/auth-caps/` once shipped.

**This scope deliberately overturns a previously-stated non-negotiable.** `access-model-scope.md` and
the nav SKILL both say *"the nav stays a pure lens — a menu entry never carries a cap and never widens
reach."* That was right for **widening** (a lens must never *grant*). But a live session forced the
opposite direction: a `viewer` given a nav of **exactly one page** could still open every other
read-only page (`/#/t/acme/rules`, `/#/t/acme/flows`, Ingest, Datasources, Data Studio) by URL, and
they *rendered*. The `viewer` role (see `login-hardening-scope.md`, shipped) made those pages
read-only — necessary, but coarse: it is a tier, not "one page." The user's ask is unambiguous:
**the resolved nav must be the allow-list of REACHABLE surfaces, read included. One page in the nav ⇒
every other core surface is denied — not just un-authorable, but not reachable and not rendered.**

So this scope adds the *narrowing* direction the lens never had: **the nav gates reach.** It does so
**without** giving the lens the power to *grant* — reach is still `caller ∩ what-they-hold`, never
widened. The nav can only *subtract* reachable surfaces, never add one the caller couldn't already
reach by cap.

## The snag that dictates the design (read first)

Reach is gated by **capabilities**, and caps are **per-capability-TYPE, not per-PAGE**. The Ingest
surface is gated on `mcp:series.list:call` — but a **dashboard tile** also reads series, so a viewer
*legitimately* holds `series.list`. You therefore **cannot** block the Ingest *page* by dropping
`series.list`: that also breaks every tile that reads series. The Dashboards page (`dashboard.list`)
and Data Studio (`series.list`) have the same property. So "block the page but keep the tile" is
**inexpressible with the existing surface gate-caps** — they conflate *page reach* with *data read*.

The fix is a **new, orthogonal reach dimension**: a page-reach cap distinct from every data-read cap,
checked **only at a surface's entry read**, that the fine-grained data caps (`series.read`,
`viz.query`, `dashboard.get`) never satisfy and are never satisfied by. Blocking the Ingest page
leaves series-reading tiles untouched, because they go through `series.read`, not the page gate.

## The decision: `reach:<surface>:view`, a first-class grammar surface

Add a fifth enforcement **surface** to the cap grammar (`crates/caps/src/request.rs` — the file that
says *"a new surface is a deliberate grammar change, not an ad-hoc string"*): `Reach`, with one action
`View`. A page-reach cap reads `reach:<surface>:view` (e.g. `reach:rules:view`, `reach:ingest:view`).

Why a real cap and not a new claim field or a bespoke per-route check:

- **The enforcement choke point (`lb_caps::check`) reads only `caps`.** A new `Claims` field is
  *invisible* to the matcher (verified: `check.rs` consults `caps` + `constraint`, never `role`/
  `run_id`). Encoding reach as a **cap** makes it ride the one unavoidable authorization primitive
  automatically — no new gate to wire into (and forget on) every route.
- **It keeps the platform invariant literally true.** Reach is still *decided by the token, checked at
  the route* — the token is still a cached projection of `resolve_caps`. The nav becomes the **source**
  of the reach caps, not a live per-request dependency. This is the least-contradictory reconciliation
  of "caps gate reach" with "the nav decides which pages": the nav decides *which reach caps you get*.
  *Alternative rejected — live per-request nav re-resolve:* the guard would run a full `nav.resolve`
  (store reads + `tags.find` + `ext.list`) on every gated read, a real perf + cache-invalidation
  liability on a security path. A cap folded at login is a constant-time set membership at the existing
  choke point, and refreshes on re-login exactly like every other cap already does.
  *Alternative rejected — reuse the existing surface gate-cap:* cannot separate page-reach from
  data-read (the snag above); dropping `series.list` to block Ingest also breaks tiles.

## The reach set is derived from the resolved nav — with a load-bearing fallback

At login (in the `resolve_caps` fold on the `login` route), resolve the subject's nav and emit one
`reach:<surface>:view` cap per `surface` item the resolved nav grants. **Rule 10 holds:** the
derivation is generic over the nav's items — the surface key is **opaque data** carried straight from
`NavItem.surface`; the guard is `reach:<opaque-surface>:view ∈ caps?`, never `match surface { "rules"
=> … }`.

**The fallback is the correctness crux.** `nav.resolve` returns `Fallback` (no items) when *no nav
applies* — which is the state of **every existing member/admin who never authored a custom nav**. If
reach were derived naively, a fallback subject would get an **empty** reach set and be locked out of
everything — a catastrophic regression. So:

- **Fallback (no explicit nav) ⇒ all surfaces reachable.** The reach gate bites **only** when a subject
  has an *explicit, curated* nav (a personal pick, a team-shared nav, or the workspace default). This
  matches intent precisely: the gate restricts *only* someone deliberately handed a menu. A default
  member is unaffected — their reach is unchanged (open), exactly as today.
- **An admin is never locked out of admin surfaces by a nav.** Admin surfaces (`admin`, `system`,
  `extensions`, `data`, `telemetry`, `studio`) stay gated by their existing admin caps; the reach gate
  is *additive* — a surface is reachable iff **(existing surface cap held) AND (reach permits it)**,
  and reach permits everything under fallback. An admin who curates their *own* one-page nav opts into
  the restriction knowingly (and can always un-pick it).

Concretely, the derivation emits a sentinel when the nav is a fallback: `reach:*:view` (the wildcard
`*` matches any surface via the existing grammar), so the guard needs no "is this fallback?" branch —
it just asks `holds reach:<surface>:view?`, and a fallback subject holds `reach:*:view` which grants
all. A curated-nav subject holds only the concrete `reach:<surface>:view` for their menu's surfaces.

## The gate: a dedicated page-entry route `GET /surface/{surface}`

**The list routes are not page-exclusive — this dictated the design.** The pages a curated nav must
block (Ingest, Rules, Flows, Datasources) load via list routes (`GET /series`, `/rules`, `/flows`,
`/datasources`) that are ALSO called by the dashboard source picker, the Data Studio panel builder,
the nav editor, and the channel pin picker (verified: `ui/.../builder/useSourcePicker.ts` fans
`listSeries`/`listRules`/`listFlows`/`listDatasources`/`listExtensions` into every dashboard tile
render). So the server **cannot distinguish** "bob opens the Ingest page" from "bob's dashboard tile
lists series" — they are the SAME HTTP request. Gating those routes would break the pages bob IS
allowed to see. An `X-LB-Surface` request header the server gates on is *not a boundary* either — bob
curls the shared route with any header value.

So reach is enforced on a **dedicated per-page entry route**: `GET /surface/{surface}`, gated on
`reach:<surface>:view` (via the shared `require_reach` guard, which is `authenticate` + the reach cap
check → opaque `403`). A gated page's client loader (`beforeLoad`) awaits this ONCE on mount; a `403`
redirects to the caller's default page. The route carries **no page data** — it is a pure gate. The
shared data routes stay open (tiles/pickers keep working); *opening a page* is the hard server
boundary. Bob curling `GET /series` still gets the series list (that data is what a tile sees anyway),
but he **cannot load** the Ingest page: `GET /surface/ingest` → `403`. That is the reach restriction
asked for, enforced server-side, without breaking a single shared consumer.

Keying on the **surface** (opaque `{surface}` path param — rule 10) rather than the entry verb also
sidesteps the two gate-cap-vs-entry-read mismatches (rules: gate `rules.run` / entry `rules.list`;
data: gate `store.scan` / entry `store.query`) — the reach cap is `reach:<surface>:view` regardless of
what verb the page's data comes from. An `ext` surface maps to no core `reach:` cap — ext reach stays
the opaque `ext.list` install seam (rule 10), unchanged.

*Why a dedicated route, not gating every read verb:* the boundary is the **page**, not the verb, and
the verbs are shared. `dashboard.get`/`series.list`/`rules.list` all serve both their page AND other
surfaces' tiles/pickers; gating any of them breaks the embed. The dedicated entry restricts exactly
"opening this page" while leaving every shared read alone — the orthogonality the snag demands, made
enforceable by giving the page its own route.

## Non-goals

- **Per-page data redaction.** The reach gate governs *opening a surface*, not *which rows a query
  returns* — that stays a datasource-authority concern (as in `access-model-scope.md`).
- **Gating extension surfaces via `reach:`.** An `ext` nav item's reachability stays the `ext.list`
  install seam (rule 10 — ids opaque). This scope covers **core** surfaces only.
- **A new admin UI to author reach.** Reach is *derived* from the nav an admin already authors
  (`nav.save` + share/set_default). There is no separate "reach editor" — the nav IS the reach.
- **Making the client route guard the boundary.** The React `beforeLoad` redirect is **UX + defense-in-
  depth only**; the server-side reach cap is the boundary. Stated loudly in code.
- **Live nav-edit propagation without re-login.** Reach refreshes on re-login (like every cap). A nav
  edit an admin makes reaches a logged-in user on their next login — acceptable, and the same latency
  caps already have. (A future token-refresh could re-fold; out of scope here.)

## How it fits the core

- **Tenancy / isolation (§7):** reach caps are workspace-scoped in the token like every cap; the ws
  wall (`check` Gate 1) runs before reach is ever consulted. A ws-B token carries only ws-B's reach.
- **Capabilities (§3.5, §6.6):** reach is a **capability**, so it composes with the existing
  enforcement order (ws wall → cap match → delegation constraint) at the one choke point. It is a
  strict **narrowing** — a `reach:<surface>:view` cap is required *in addition to* the surface's data
  caps; holding it never grants a data cap and vice-versa. No-widening is preserved: the nav→reach
  derivation only emits reach caps for surfaces whose items *survived* the resolver's existing
  cap-strip, so reach can never name a surface the caller couldn't already reach by cap.
- **Symmetric nodes (rule 1):** pure resolution over store records + the grant projection; no
  `if cloud`. The derivation runs wherever `resolve_caps` runs.
- **MCP surface:** no new MCP verb. Reach caps are *minted*, not *called*. The gate reuses the existing
  `lb_caps::check` primitive via a thin `authorize_reach` host helper.
- **Data (SurrealDB):** read-only over the nav records the resolver already reads. Writes nothing new.
- **Bus / secrets:** N/A.
- **Core knows no extension (rule 10):** the reach surface key is opaque `NavItem.surface` data; the
  guard never branches on a page/ext id. The `surfaces.rs` core-surface table is the only place a
  surface name is named, and it is core (allowed) — an ext item is handled by the `ext.list` seam.

## Example flow

The user's exact scenario, made to work:

1. Admin authors `nav:bob-onboarding` with **one** `surface` item: `dashboards` (a single dashboard
   page). Shares it to `team:onboarding`; `user:bob` is a member of that team and is `role:viewer`.
2. Bob logs in. `resolve_caps` resolves his nav → it has one surviving surface (`dashboards`) → the
   fold emits `reach:dashboards:view` (and NOT `reach:*:view`, because his nav is explicit/curated).
3. Bob's token carries: the viewer floor + `reach:dashboards:view`.
4. Bob opens the Dashboards page: `GET /dashboards` → `authorize_reach(bob, ws, "dashboards")` →
   holds `reach:dashboards:view` → **200**, page renders (tiles read series via `series.read`, fine).
5. Bob deep-links `/#/t/acme/rules` → `GET /rules` → `authorize_reach(bob, ws, "rules")` → he holds
   `reach:dashboards:view` but **not** `reach:rules:view` → **403**. Same for Ingest, Datasources,
   Data Studio — **denied at the server, read included**, even though he still holds `series.read` for
   his dashboard's tiles. The one page he was given is the only surface he reaches.
6. `user:alice` (admin, **no** custom nav → fallback) logs in: the fold emits `reach:*:view` → she
   reaches every surface exactly as before. No regression for anyone not handed a curated nav.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`, all against the **real** gateway +
SurrealDB, seeding real nav/grant/membership records (no mocks — CLAUDE §9). New file
`nav_reach_test.rs` (or extend `viewer_reach_test.rs`):

- **Reach deny (the headline, required):** a subject given a curated nav of **one** surface gets a real
  **403 on the ENTRY READ** (`GET /rules`, `GET /series`, `GET /datasources`) of a surface **not** in
  their nav — proving read-reach is server-side gated, not just hidden. This is the exact live gap.
- **Reach allow:** the same subject **200s** the entry read of the one surface they WERE given, and a
  dashboard tile on that page that reads series (`series.read`) **still 200s** — proving the page-reach
  gate is orthogonal to data-read (the snag's separation is real, not theoretical).
- **Fallback = all-reachable (required regression):** a subject with **no** nav (fallback) 200s the
  entry read of every surface — proving the gate does not lock out the default member/admin. This is
  the catastrophic-regression guard.
- **No-widening (the invariant that must still hold):** a curated-nav subject who lacks a surface's
  underlying data cap is still denied even if the nav named it — reach can only *subtract*, never
  *grant* a surface the caller couldn't reach by cap. (Derivation only emits reach for surviving items,
  so a stripped item yields no reach cap.)
- **Workspace-isolation (required):** a ws-B token carries no ws-A reach; a curated nav in ws-A never
  affects a ws-B subject's reach.
- **Client (UX, explicitly NOT the security test):** a vitest that hitting a non-nav surface URL
  redirects to the default page (guard present) — labeled in the test as UX/defense-in-depth, with a
  comment that the server 403 above is the real boundary.

## Risks & hard problems

- **The fallback branch is load-bearing and easy to get subtly wrong.** If "fallback ⇒ all reachable"
  is implemented as "emit nothing," every existing user is locked out. The `reach:*:view` sentinel makes
  the *guard* branch-free, but the *derivation* must correctly detect `ResolvedSource::Fallback` and
  emit the wildcard. Pin it with the fallback-all-reachable test as the first thing that must stay green.
- **Reach/nav divergence.** The derivation must use the *same* `nav_resolve` the UI renders from, so the
  reachable set and the rendered rail agree — reusing the resolver, not reimplementing surface-strip.
- **Surfaces with no entry route** (reminders, studio load via `/mcp/call`). Their entry verb
  (`reminder.list`, `devkit.templates`) must also consult reach, or they leak. Gate at the tool level
  for those via the same `authorize_reach` keyed on the surface the tool belongs to — but **only for
  the entry verb**, not every reminder/devkit verb (don't re-break the orthogonality).
- **The `admin`/`extensions`/`data` surfaces double-gate.** They already require an admin cap AND now
  reach. Under fallback `reach:*:view` covers them, so admins are fine; a curated nav that *includes*
  an admin surface must emit its reach cap too (it will, generically). Verify an admin given a curated
  nav that omits `system` correctly loses `system` reach (opt-in restriction) without losing the admin
  caps themselves — reach is additive, the admin caps are untouched.

## Open questions

- **Should reach gate the entry verb at the `/mcp/call` bridge generically** (map tool→surface→reach)
  rather than per-route? A single choke at `mcp.rs` is tempting, but tool→surface is many-to-one and
  ambiguous (which surface "owns" `dashboard.get`?). Recommend: gate the **HTTP entry routes**
  explicitly (unambiguous 1:1 page→route) + the two `/mcp/call`-only entries (reminders, studio) by
  their entry tool. Do **not** gate every read verb at the bridge — that re-breaks orthogonality.
- **Wildcard sentinel vs. explicit full-set on fallback.** `reach:*:view` (one cap, grammar wildcard)
  vs. emitting every known surface's reach cap. Recommend the wildcard — smaller token, no "known
  surface list" to keep in sync, and the grammar already supports `*`.
- **Does a pinned surface (nav `pinned`) grant reach?** A pin is a personal shortcut resolved through
  the same pipeline. Recommend yes — a surface the user pinned is one they can reach — but confirm it
  doesn't let a viewer self-widen (a pin only resolves for a surface they already hold the data cap
  for, so it can't widen; it can only re-add a surface to their reach set that the cap-strip kept).

## Related

- **Overturns / amends:** `access-model-scope.md` ("the nav stays a pure lens") and the nav SKILL
  (`../../skills/nav/SKILL.md`) — both must be updated to *"the nav is a pure lens for **widening**
  (never grants) but **gates reach** for **narrowing**: a curated nav is the allow-list of reachable
  core surfaces (`reach:<surface>:view`), a fallback nav reaches all."* Memory
  `[[nav-resolve-owner-shadows-fallback]]` likewise.
- **Builds on:** `login-hardening-scope.md` (the `viewer ⊂ member ⊂ admin` tiers — the prerequisite;
  reach is what turns "a viewer" into "a viewer given one page"), `auth-caps-scope.md` (the grammar we
  extend with the `reach` surface).
- README `§3.5` (enforcement order — reach slots in as an additional cap at the same choke point),
  `§6.6` (RBAC), `§7` (the ws wall runs before reach).
- Source: `crates/caps/src/request.rs` + `grammar.rs` (the new surface), `host/src/nav/surfaces.rs`
  (the core surface→reach map), `host/src/nav/resolve.rs` (the reach-set derivation),
  `role/gateway/src/routes/login.rs` (the fold), the surface entry routes in `role/gateway/src/routes/`.
- Skill: on ship, extend `skills/nav/SKILL.md` to state the reach-gating rule (the nav author now
  controls reach, not just the menu).
