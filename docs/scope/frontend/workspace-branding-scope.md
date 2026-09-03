# Frontend scope — workspace branding

Status: scope (the ask), **partly shipped**. Sibling of `theme-customizer-scope.md`. Promotes to
`public/frontend/frontend.md` and `public/workspace/workspace.md` once shipped.

> **Shipped 2026-09-03 — the pre-auth read seam.** `GET /public/branding?ws=<ws>` is live in the
> gateway (`role/gateway/src/routes/public_branding.rs`), serving the workspace-default `ui_branding`
> + `ui_theme` blobs with no token. It closes the last gap this scope named: a browser that has never
> signed in painted the compiled product default, because the login-branding slice could only repaint
> a brand some earlier authenticated visit had cached in `localStorage`
> ([`sessions/frontend/login-branding-session.md`](../../sessions/frontend/login-branding-session.md)),
> and nothing seeds that cache on a new device, a new browser, or a cleared profile. Downstream report:
> [NubeIO/rubix-ai#306](https://github.com/NubeIO/rubix-ai/issues/306). Session:
> [`sessions/frontend/public-branding-route-session.md`](../../sessions/frontend/public-branding-route-session.md).
> Route shape as built, differing from the sketch below in three ways settled during the build:
> **(1)** `?ws=` query, not `/{ws}` path — a path segment makes a missing workspace a router `404`,
> which is a different answer from an unknown one, i.e. an oracle. **(2)** The whitelist is the two
> **opaque prefs blobs** (`ui_branding`, `ui_theme`), not four flattened string fields — branding
> landed as one admin-owned JSON blob on the prefs record, and the theme rides beside it because a
> deployment's login layout (`split`, the ESR case) is chosen by the theme, not the brand. **(3)** `ws`
> is **required**; there is no "this node's own workspace" fallback, because finding one means
> enumerating workspaces pre-auth — the exact thing the route must not do.

Let a workspace admin brand the shell as their own: a **logo**, a **favicon**, and the **site name /
login heading** ("My Company") that show in the app chrome and — the hard part — on the **login page,
before anyone has authenticated**. Branding is **workspace identity owned by admins**, not a per-member
preference: every member of a workspace sees the same logo and name, and a member cannot change it. This
is the natural companion to the per-member theme in `theme-customizer-scope.md` — same workspace-default
prefs seam, opposite ownership — but it carries one genuinely new problem the theme does not: rendering a
specific workspace's brand on a **pre-auth** page, which is a deliberate, opt-in, read-only break in the
workspace wall.

## Goals

- Let a workspace **admin** set, per workspace: a **logo** image, a **favicon** image, a **site name**,
  and a **login-page heading** (may default to the site name).
- Render that branding in the **authenticated shell** (nav header logo + name, browser tab title +
  favicon) for every member of the workspace, with no member override.
- Render it on the **unauthenticated login page** for the workspace the visitor is signing into —
  resolved from the URL/host or from the workspace the visitor selects — served read-only through a
  narrow public seam that exposes **only** branding fields and nothing else behind the wall.
- Reuse the shipped seams: **string** fields via workspace-default prefs (`prefs.set_default`, admin);
  **image** assets via the shipped `assets.*` surface; no new datastore, no new capability grammar.
- Fall back cleanly to a neutral **instance default** brand (the Lazybones mark/name) when a workspace
  has set none, or before a workspace is known.

## Non-goals

- **Not a per-member setting.** Members do not override branding (contrast: theme, which they do). If a
  member wants a personal look, that's the theme customizer, not this.
- **No public/anonymous serving of anything but branding.** The pre-auth seam returns a fixed whitelist
  of branding fields for one workspace — never records, members, prefs, or any other workspace data.
- **No full white-label domain program** (custom domains, per-workspace TLS, email templates, OG/meta
  images) in this slice — logo, favicon, site/login name only. Custom-domain routing is a later scope.
- **No rich theme editing here** — that's `theme-customizer-scope.md`. A workspace's *default theme* is
  set there via the same `prefs.set_default`; this scope owns the brand *identity* (marks + names).
- **No streaming/large-image pipeline.** Logos/favicons are small; large binary/streaming assets remain
  the S7 files concern (`files-scope.md`).

## Intent / approach

Split branding by data kind onto the seams that already exist, and keep **admin-workspace** as the only
owner:

- **Strings** (`site_name`, `login_heading`) are **workspace-default prefs** under reserved keys (e.g.
  `ui.branding.site_name`, `ui.branding.login_heading`). An admin writes them with the admin-gated
  `prefs.set_default` (which targets `workspace_prefs:[ws]`); members read them through the normal
  `prefs.resolve` fold, where — because there is no per-member branding axis — the workspace default is
  what everyone resolves. No member-set path is exposed for these keys.
- **Images** (`logo`, `favicon`) are **assets** stored via the shipped `assets.put_doc` under reserved
  ids (e.g. `branding:logo`, `branding:favicon`), `content_type` = image, content = a data-URI/base64
  string (record-backed today, bucket-backed later — the `files-scope.md` seam). Admin write is gated by
  `mcp:assets.put_doc:call`; authed members read via `assets.get_doc`. Small images only, so a record
  body is fine now.
- **Application** is a small shell module mirroring `theme-dom.ts`: it sets `document.title` from the
  site name, swaps the `<link rel="icon">` href to the favicon, and feeds the logo/name into the nav
  header — one responsibility, no component branching.

**The pre-auth login problem is the crux.** The login page runs before any token exists, and today the
visitor **types** the workspace on the form (`LoginView` defaults to `nube`). So the shell cannot know
*whose* brand to show until the workspace is identified, and it has no authenticated way to read that
workspace's prefs/assets. The resolution:

1. **Identify the workspace pre-auth** from the URL when possible (subdomain/path, e.g.
   `nube.lazybones.app` → `nube`), else from the workspace the visitor enters/selects on the login form
   (fetch-on-blur), else fall back to instance-default branding.
2. **Serve branding through a narrow public read seam** — a dedicated unauthenticated gateway route
   `GET /public/branding?ws=<workspace>` (**shipped**) that returns **only** `{ ui_branding, ui_theme }`
   for that workspace and nothing else. This is a conscious, opt-in, read-only break in the workspace
   wall, following the exact precedent of the document-store's public published-doc serving
   (README §6.12) — whitelisted fields, read-only, no capability because there is no principal yet, and
   the handler physically cannot reach any non-branding data. As built, the whitelist is enforced **by
   construction, not by filtering**: the handler destructures the loaded `Prefs` into two named fields
   and builds the body from those, so a future prefs axis cannot leak by merely existing — it would have
   to be hand-added to that file. Every miss (unknown workspace, unbranded workspace, malformed slug,
   absent `ws`, store error) returns the identical `200 {"ui_branding":null,"ui_theme":null}`, so the
   route is not a workspace-existence oracle, and it carries its own per-IP ceiling (60/min) beside the
   invite routes' limiter.

**Rejected alternative — put branding in a per-member pref like the theme.** Branding is workspace
identity, not personal taste; a member-writable brand would let anyone repaint the company logo, and
resolve would return different brands to different members of the same workspace. Admin-owned
workspace-default prefs + assets is the correct ownership. Rejected.

**Rejected alternative — bake branding into the authed `prefs.resolve`/`assets.get_doc` path only (no
public seam).** Then the login page can never be branded (no token, no workspace), which defeats half the
ask ("login page heading/name"). The public seam is unavoidable — the design work is making it *narrow*.
Rejected.

**Rejected alternative — a new `branding.*` MCP verb family.** Tempting for validation/versioning, but
strings already have `prefs.set_default` and images already have `assets.put_doc`; a new verb family
would duplicate caps and storage for no new capability. Reuse now; promote to a dedicated verb only if
versioning/migration pain appears. The **only** genuinely new surface is the pre-auth public *read*
route, which by definition can't be an authed MCP verb.

## How it fits the core

- **Tenancy / isolation:** branding is a **workspace-scoped** identity — strings on `workspace_prefs:[ws]`,
  images as workspace-walled assets. The authed paths inherit the prefs/assets workspace wall. The public
  read route is the one deliberate wall break, and it is bounded to a **branding-only field whitelist for a
  single named workspace** — isolation is proven by a test that the route leaks nothing but branding and
  serves workspace A's brand only for A.
- **Capabilities:** setting brand strings requires the admin-gated `mcp:prefs.set_default:call`; setting
  brand images requires `mcp:assets.put_doc:call` (admin, for `branding:*` ids); authed reads use
  `prefs.resolve` / `mcp:assets.get_doc:call`. The **public** read route has no principal, so it is gated
  not by a capability but by being **structurally incapable** of returning anything but the branding
  whitelist — the security review target of this scope.
- **Symmetric nodes:** no cloud/edge branch. The public branding route and the shell application run the
  same on browser and Tauri; a single-workspace edge node simply serves its own brand. Workspace
  identification is config/role (subdomain map or entered workspace), never `if cloud`.
- **One datastore:** brand strings live in SurrealDB `workspace_prefs`, brand images as SurrealDB-backed
  assets. No new table, no separate blob service — the bucket API seam over records (`files-scope.md`).
- **No mocks / no fake backend:** admin-set + member-read + pre-auth read are all tested against a **real**
  spawned gateway with a seeded real workspace and real admin/member tokens. No `*.fake.ts`.
- **State vs motion:** branding is **state** (workspace identity in SurrealDB), not motion — no Zenoh
  subject. A brand change is picked up on next resolve/read; a live push is out of scope.
- **Stateless extensions:** unchanged. Extensions inherit the shell's chrome; they don't own branding and
  no extension-id branch is added. (An extension *contributing* branding is not a thing — branding is the
  workspace's, mediated by admin verbs.)
- **MCP is the contract:** admin writes and authed reads are the existing `prefs.*` / `assets.*` MCP tools
  under reserved keys/ids. The pre-auth read is a gateway HTTP route (§6.13), the correct shape for a
  no-principal, browser-facing read.
- **API shape (§6.1):**
  - *CRUD (write):* `prefs.set_default` for `ui.branding.{site_name,login_heading}` (admin);
    `assets.put_doc` for `branding:{logo,favicon}` (admin). Delete/reset = write empty → falls back to
    instance default, client- or handler-resolved. No per-item batch — a handful of fields, always fast.
  - *Get / list:* authed members read via `prefs.resolve` + `assets.get_doc`. **New (shipped):** the
    unauthenticated `GET /public/branding?ws=<workspace>` read (whitelist only) for the login page.
  - *Live feed:* **N/A** — branding rarely changes and login/boot re-reads suffice.
  - *Batch:* **N/A** — bounded, always-fast single writes.
- **Durability:** N/A — brand writes have no cross-node must-deliver effect; plain workspace-scoped
  `prefs.set_default` / `assets.put_doc`, not the outbox.
- **One responsibility per file:** `lib/branding/` (options/keys, brand-dom application: title + favicon
  link + logo feed, storage/resolve, provider/hook) separate from `features/settings/` admin editors
  (logo upload, favicon upload, name fields) and from the gateway `routes/public_branding.rs`. Reserved
  keys/ids live in one data file, not scattered literals.
- **SDK/WIT impact:** none. Plugin boundary and host-callback ABI untouched.

## Example flow

1. An **admin** opens Workspace Settings → Branding, uploads a logo and favicon, and sets the site name
   to "My Company" and the login heading to "Sign in to My Company".
2. The images `assets.put_doc` to `branding:logo` / `branding:favicon`; the strings `prefs.set_default`
   to `ui.branding.site_name` / `ui.branding.login_heading` on the workspace record. Non-admins never see
   these controls; a forged call is denied (opaque).
3. A **member** loads the app. The shell reads branding (member reads `prefs.resolve` + `assets.get_doc`),
   sets `document.title` to "My Company", swaps the favicon, and renders the logo + name in the nav header.
4. A **new visitor** hits `nube.lazybones.app/login` (or types `nube` into the login form). The login page
   calls `GET /public/branding?ws=nube`, gets `{ ui_branding, ui_theme }`, and renders "Sign in to My
   Company" with the logo, the branded favicon and the workspace's login layout — **before** any
   authentication. A shell that already has a cached brand paints it first and lets this response
   correct it, so the fetch never costs a blank frame.
5. The visitor authenticates; the authed shell re-reads branding through the normal walled path and the
   brand carries seamlessly from login into the app.
6. A workspace that has set no branding renders the **instance-default** Lazybones mark/name at every step.

## Testing plan

Pure-frontend application tests **and** the mandatory platform categories (this touches the real store and
a new public route).

- **Brand application (unit, `pnpm test`):** given resolved branding, the shell sets `document.title`,
  swaps the `<link rel=icon>` href, and renders logo + name; empty branding falls back to the instance
  default without a flash.
- **Admin write + authed read (real gateway, `pnpm test:gateway`):** a seeded **admin** sets strings via
  `prefs.set_default` and images via `assets.put_doc`; a seeded **member** reads them back via
  `prefs.resolve` + `assets.get_doc`. No fake backend.
- **Capability deny (mandatory):** a **non-admin** member is denied on `prefs.set_default` and on
  `assets.put_doc` for `branding:*` (opaque deny) — assert the honest deny, controls hidden.
- **Workspace isolation (mandatory):** workspace A's branding is not readable/writable as workspace B via
  the authed path; **and** `GET /public/branding?ws=A` returns A's branding only — never B's, and never any
  non-branding field. Seed two real workspaces and assert both. **(Shipped:**
  `role/gateway/tests/public_branding_route_test.rs::serves_only_the_named_workspaces_brand`.**)**
- **Pre-auth public read (real gateway):** `GET /public/branding?ws=<ws>` returns the brand whitelist with
  **no token**, returns the neutral empty brand for an unbranded/unknown workspace, and a field-audit test
  confirms the response body contains *only* the whitelisted keys (the wall-break guard). **(Shipped:**
  the same test file — `serves_the_workspace_brand_with_no_token`,
  `body_carries_the_brand_axes_and_nothing_else` (every other prefs axis is set on the record and each is
  asserted absent from the raw body), `every_miss_answers_identically` (the oracle guard), and
  `is_rate_limited_per_client`.**)**
- **Build/lint:** `pnpm build`, `pnpm lint`, `cd rust && cargo test --workspace` green.

## Risks & hard problems

- **The public seam is a wall break — keep it hairline.** The one route that serves data without a
  principal must be structurally unable to return anything but the branding whitelist for one workspace.
  Treat it as a security-review item (`/security-review`), test the field whitelist explicitly, and never
  let it grow a "just also return X" field.
- **Pre-auth workspace identification.** Today the login form has the visitor *type* the workspace
  (`LoginView` default `nube`); there is no subdomain→workspace resolution yet. Decide the source of
  truth (subdomain/path vs. entered value vs. instance default) — this may need a small workspace-lookup
  step and touches `nav`/routing.
- **Favicon/title races.** Setting `document.title` and the favicon link must not fight the theme layer
  or a later route change; one owner module, applied at boot and on brand change, mirroring `theme-dom`.
- **Image size / format.** Record-backed data-URI images must stay small; validate type/size on upload
  and cap dimensions, or a huge logo bloats every read. Large assets are deferred to the bucket backend.
- **Instance default vs. empty workspace.** "Unbranded" must render a clean neutral default, not a broken
  empty header — define the instance-default mark/name once.
- **Caching & staleness.** ~~The public branding response should be cacheable~~ **Settled:** the shipped
  route sends `Cache-Control: public, max-age=60` — short enough that a rebrand shows up on the next
  sign-in rather than being pinned for a session, long enough that repainting a login page is free. No
  etag: the body is two small blobs, so a conditional round-trip would cost more than it saves.

## Open questions

- ~~**Pre-auth workspace source:**~~ **Settled for the backend half:** the route takes an explicit `ws`
  and never guesses — no subdomain map, no "sole workspace on this node" fallback, because either would
  have the gateway enumerate or infer workspaces for an anonymous caller. Picking the hint stays the
  shell's job (deep-link `#/t/<ws>`, the typed email's last workspace, the sole cached brand — the order
  `resolveBootBrand` already uses downstream). A subdomain→workspace map, if it ever lands, resolves to a
  `ws` in front of this route rather than inside it.
- **Reserved key/id names:** `ui.branding.site_name` / `ui.branding.login_heading` and
  `branding:logo` / `branding:favicon` — confirm against the prefs value-shape rules and the assets id
  conventions.
- **Where the admin editor lives:** Workspace Settings (recommended, alongside members/roles) vs. a
  Branding tab in the Customizer next to theme. Recommendation: Workspace Settings — branding is admin
  identity, the Customizer is member preference; keeping them apart matches the ownership split.
- **Instance-default brand asset:** ship a neutral Lazybones logo/name as the compiled fallback — where
  does it live (bundled asset vs. seeded workspace default)?
- **Favicon per workspace on a shared origin:** a single browser tab shows one favicon; confirm we swap it
  live on workspace switch (multi-workspace shell) rather than only at boot.
- **Custom domains / white-label** (per-workspace domain, TLS, email/OG images): explicitly deferred —
  is there near-term demand, or is subdomain + in-app brand enough for now?

## Related

- `theme-customizer-scope.md` — the member-preference sibling; shares the workspace-default prefs seam
  (`prefs.set_default`) but is member-owned/overridable, where branding is admin-owned and not.
- `../prefs/user-prefs-scope.md` — the `prefs.set_default` (admin, workspace default) write and the
  resolve fold branding strings ride.
- `../files/files-scope.md` — the `assets.*` bucket-API-over-records seam the logo/favicon use, and the
  small-vs-streaming-image boundary.
- `../workspace/workspace-scope.md`, `../tenancy/tenancy-scope.md` — the workspace-as-tenant wall this is
  scoped by, and the admin CRUD surface the branding editor extends.
- `../document-store/document-store-scope.md` — the precedent for a deliberate, opt-in, read-only public
  serving break in the workspace wall (README §6.12) that the `/public/branding/{ws}` route follows.
- `nav-rail-scope.md`, `routing-scope.md` — the shell header the logo/name render into and the pre-auth
  workspace resolution touches.
- `../../README.md` §6.6 (identity/caps — the admin gate), §6.12 (files + public serving), §6.13
  (frontend shell).
- **Skill doc:** N/A. Admin branding writes reuse the existing `prefs.set_default` / `assets.put_doc`
  verbs (already cataloged); the only new surface is a browser-facing pre-auth **read** route, not an
  agent-/API-drivable task. Flag the public route for `/security-review`, not a skill.
</content>
</invoke>
