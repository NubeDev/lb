---
name: nav
description: >-
  Manage Lazybones NAVIGATION MENUS (the nav builder) programmatically over the node gateway — a nav is
  a user-/team-authored `nav:{id}` asset: an ordered menu of entries linking to core surfaces, specific
  dashboards, extension pages, or dynamic tag-groups, assigned to teams and resolved per-member to their
  effective menu. List, read, create/update, delete, share, set the workspace default, set your own
  active pick, and RESOLVE the effective (tag-expanded, cap-stripped) menu. Also publishes a **template
  dashboard as a per-site menu** — a `template-group` entry fans one dashboard out into one bound page
  per tag/option value (reusable pages). Use when a task says "build a menu/sidebar for a team", "assign
  these pages to a team", "a dynamic menu of tagged dashboards", "one dashboard per site/plant/tenant",
  or "call nav.* over MCP/REST". The nav is a LENS over existing access — it grants nothing.
---

# Managing navigation menus over MCP / REST

A **nav** is a workspace asset — the `dashboard` shape cloned: a `nav:{id}` record holding an ordered
`items[]` menu, an owner, and `private|team|workspace` visibility, shared to teams via the S4 `share`
edge. NavRail renders the resolved nav, **falling back** to the built-in `SURFACES` set when no nav
exists (never a blank rail).

**The nav is a LENS for WIDENING (never a grant), but GATES REACH for NARROWING.** An item carries no
caps and cannot *widen* reach: `nav.resolve` strips every item the caller can't reach, and the gateway
re-checks every page verb on click regardless. "Give the ops team these pages" = define a **role** (the
cap bundle) + share a **nav** to the team — the role grants, the nav shapes the menu.

**Update (`nav-reach-scope.md`, shipped): a curated nav now GATES REACH.** The resolved nav is turned
into `reach:<surface>:view` caps at login; the dedicated `GET /surface/{s}` route enforces them
server-side. So a subject given a **one-page** nav can reach ONLY that page (read included — other core
pages 403 at `/surface/{s}` and are dropped from the rail). A **fallback** nav (no curated menu) yields
`reach:*:view` and reaches all, so a default member/admin is never locked out. This still never widens:
reach is only emitted for a surface the resolver already kept, so the nav can *subtract* reachable pages
but never *add* one. To restrict a viewer to specific pages: give them the `viewer` role AND a curated
nav — the role makes them read-only, the nav makes only those pages reachable.

The gateway derives the **workspace + owner from the bearer token**, never the body (README §6/§7).

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Caps: reads (`mcp:nav.get|list|resolve:call`) are **member-level** (every member resolves their own
menu + curates their own pick — `nav.pref.*` gate on `nav.resolve`); writes
(`mcp:nav.save|delete|share:call`, and `nav.set_default` under `nav.save`) are **admin-ish** (the
`workspace-admin` role holds them). Store caps `store:nav:read|write`, `store:nav_pref:read|write`.

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| List (roster) | `GET /navs` | `{"tool":"nav.list","args":{}}` | — |
| Read one (full items) | `GET /navs/{id}` | `{"tool":"nav.get","args":{"id":"…"}}` | `id` |
| Create/update | `POST /navs` | `{"tool":"nav.save","args":{…}}` | `id,title,items[],now*` |
| Delete (tombstone) | `DELETE /navs/{id}` | `{"tool":"nav.delete","args":{"id":"…","now":…}}` | `id,now*` |
| Share / visibility | `POST /navs/{id}/share` | `{"tool":"nav.share","args":{…}}` | `id,visibility,team?,now*` |
| Set workspace default | `POST /nav/default` | `{"tool":"nav.set_default","args":{"id":"…","now":…}}` | `id,now*` |
| Resolve effective menu | `GET /nav/resolve` | `{"tool":"nav.resolve","args":{}}` | — |
| Read/set my pick + pins | `GET|POST /nav/pref` | `{"tool":"nav.pref.get|set","args":{…}}` | `id?,pinned?,now*` |
| Read/set workspace hidden-set | `GET|POST /nav/hidden` | `{"tool":"nav.hidden.get|set","args":{…}}` | `hidden[],now*` |

## 3. The entry kinds (+ one `group` level)

`items[]` is a flat, ordered, typed array; each item is exactly one kind:

- `surface` — a core page by its **opaque** key (`"channels"`, `"rules"`, …).
- `dashboard` — a specific dashboard (`dashboard:{id}`); may carry a pinned binding
  `vars: {name: value}` (reusable-pages) → the link becomes `?var-<name>=<value>` (a named instance).
- `ext` — an extension page by its **opaque** ext id (rule 10 — never branched on; resolved via `ext.list`).
- `tag-group` — **Dashboards by tag**: a dynamic entry `{ label, facets: [{key, value?}] }` expanded
  (via `tags.find`) to the **dashboards** carrying those facets (tag a new dashboard → it appears).
- `template-group` — **One dashboard per ⟨value⟩** (reusable-pages): a dynamic entry
  `{ label, dashboard, var, facets|{tool,args} }` expanded to one link **per option value** of the
  parameter — the SAME dashboard bound `?var-<var>=<value>` (see §5).
- `group` — `{ label, items: [...] }`, one nesting level, for sections.

```bash
curl -s -X POST http://127.0.0.1:8080/navs -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
    "id": "ops", "title": "Operations",
    "items": [
      { "kind": "surface", "surface": "channels", "label": "Channels" },
      { "kind": "dashboard", "dashboard": "dashboard:cooler-health", "label": "Cooler" },
      { "kind": "tag-group", "label": "Sites", "facets": [{ "key": "site" }] },
      { "kind": "group", "label": "Admin", "items": [
        { "kind": "surface", "surface": "rules" }, { "kind": "surface", "surface": "flows" }
      ] }
    ]
  }'
```

Assign it to a team, then let members resolve it:

```bash
curl -s -X POST http://127.0.0.1:8080/navs/ops/share -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{ "visibility": "team", "team": "team:ops" }'
```

## 4. Resolve — the one composite read

`nav.resolve` returns the caller's **effective** menu: the picked nav (personal pick → team-shared →
workspace-default → built-in `SURFACES` fallback), with **tag-groups expanded** and every item the
caller can't reach **already stripped**. NavRail renders this payload and re-implements no filtering.

```bash
curl -s http://127.0.0.1:8080/nav/resolve -H "Authorization: Bearer $TOKEN" \
  | jq '{ source, nav_id, items: [.items[] | {kind, label, surface, dashboard}] }'
```

`source` is `pick | team | workspace-default | fallback`. A `fallback` carries no items — the UI renders
its built-in surfaces (never blank).

The payload also carries (hide-and-pins scope, on EVERY source incl. `fallback`):

- `hidden[]` — the workspace **hidden-set** echo. The rail subtracts these refs from its built-in
  fallback menu (the one tier the server can't strip); resolved `items`/`pinned` arrive already
  stripped. Hiding is DECLUTTER, never authz — a permitted deep link still loads.
- `pinned[]` — the caller's **pinned favorites**, resolved (cap-, uninstalled-ext-, and
  hidden-stripped — hide beats pin), in the member's order. Rendered as a "Pinned" section above
  whichever menu applies.

Refs share one grammar everywhere: a bare surface key (`"rules"`), `ext:<id>`, or `dashboard:<id>`.

```bash
# Admin: hide the Dashboards surface + one extension page for the whole workspace.
curl -s -X POST http://127.0.0.1:8080/nav/hidden -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{ "hidden": ["dashboards", "ext:mqtt"] }'

# Member: pin favorites (ordered). A PARTIAL write — omitting `id` leaves the active pick alone.
curl -s -X POST http://127.0.0.1:8080/nav/pref -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{ "pinned": ["rules", "dashboard:cooler-health"] }'
```

Caps: `nav.hidden.get` rides `mcp:nav.resolve:call` (member); `nav.hidden.set` rides
`mcp:nav.save:call` (admin — the same authority as `nav.set_default`). Pins ride `nav.pref.*`
(member-owned, keyed to the token `sub`). Bounds: 200 hidden refs, 50 pins (`BadInput` over).

## 5. Publish a template dashboard as a per-site menu (reusable pages)

A **template dashboard** is an ordinary dashboard whose `variables[]` are its parameters — mark one
`required` (via `dashboard.save`) and an unbound page renders the "select a `<site>`" gate instead of
firing `$site`-literal queries. To turn it into **one navigable page per site**, add a
**`template-group`** entry — `nav.resolve` fans it out at render time.

From a real run: a `dashboard:site-overview` with a required `site` variable, and entities tagged
`site:plant-1|plant-2|plant-3` (any tagged entity supplies the value; the fan-out enumerates the
**distinct values** of the facet key). Author the menu:

```bash
curl -s -X POST http://127.0.0.1:8080/navs -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
    "id": "ops", "title": "Operations",
    "items": [
      { "kind": "template-group", "label": "Sites",
        "dashboard": "dashboard:site-overview", "var": "site",
        "facets": [{ "key": "site" }] }
    ]
  }'
```

Resolve it — the one entry expands to one bound instance per value (each on the SAME dashboard record):

```bash
curl -s http://127.0.0.1:8080/nav/resolve -H "Authorization: Bearer $TOKEN" \
  | jq '.items[] | select(.kind=="group") | .items[] | {label, dashboard, vars}'
# → {"label":"plant-1","dashboard":"dashboard:site-overview","vars":{"site":"plant-1"}}
#   {"label":"plant-2", … "vars":{"site":"plant-2"}}  …  (tag site:plant-4 → plant-4 appears, no edit)
```

The UI folds each `vars` into `?var-site=<value>` on click — that URL *is* the instance. Variants:

- **General option source** — instead of `facets`, give `{ "tool": "store.query", "args": {...} }`
  (the `Variable.query` shape); the rows' values become the fan-out (re-checked under the caller's caps).
- **A curated single instance** — a plain `dashboard` entry with `"vars": { "site": "plant-2" }` pins
  one named page ("Plant-2 Overview"). A one-off that needs its *own* cells is a **fork** (a real second
  dashboard), never a per-instance override.

**Lens (bites here too):** a `template-group` whose option source (the `tags.find` cap, or the query
tool's cap) the caller lacks is **stripped whole** — no option value leaks; a caller who cannot *read*
the template dashboard sees no entry. Instances grant nothing — the dashboard + every cell source
re-check server-side on visit.

## 6. Rules that bite

- **Nav never widens (the headline):** an item the caller lacks the cap for is stripped by `resolve`
  AND still 403s on direct access — the lens grants nothing. Never author a cap into an item.
- **Nav GATES reach (the narrowing half, `nav-reach-scope.md`):** a curated nav is the allow-list of
  reachable core pages. Login folds the resolved nav into `reach:<surface>:view` caps; `GET /surface/{s}`
  enforces them. One page in the nav ⇒ only that page reaches (read included). A fallback nav reaches
  all (`reach:*:view`) — the gate bites only for an explicitly curated menu, so no default lock-out.
- **Caps: 100 items per nav** (authored, incl. `group` children); **50 dashboards per expanded
  tag-group** at resolve (over-cap authored → `BadInput`; over-cap expansion is logged, not silently
  dropped).
- **The wall (rule 6):** navs + `nav_pref` are workspace-scoped; ws-B never resolves ws-A's navs.
- **Opaque ext ids (rule 10):** an `ext` item stores the id as data; rendering goes through the generic
  `ext.list`/`ExtHost` seam — never a branch on a named extension.
- **Precedence + stale pick:** a `nav_pref` pointing at a deleted/unshared nav falls through to the next
  tier, never errors.
