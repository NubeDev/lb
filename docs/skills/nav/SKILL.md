---
name: nav
description: >-
  Manage Lazybones NAVIGATION MENUS (the nav builder) programmatically over the node gateway — a nav is
  a user-/team-authored `nav:{id}` asset: an ordered menu of entries linking to core surfaces, specific
  dashboards, extension pages, or dynamic tag-groups, assigned to teams and resolved per-member to their
  effective menu. List, read, create/update, delete, share, set the workspace default, set your own
  active pick, and RESOLVE the effective (tag-expanded, cap-stripped) menu. Use when a task says "build a
  menu/sidebar for a team", "assign these pages to a team", "a dynamic menu of tagged dashboards", or
  "call nav.* over MCP/REST". The nav is a LENS over existing access — it grants nothing.
---

# Managing navigation menus over MCP / REST

A **nav** is a workspace asset — the `dashboard` shape cloned: a `nav:{id}` record holding an ordered
`items[]` menu, an owner, and `private|team|workspace` visibility, shared to teams via the S4 `share`
edge. NavRail renders the resolved nav, **falling back** to the built-in `SURFACES` set when no nav
exists (never a blank rail).

**The nav is a LENS, never a grant.** An item carries no caps and cannot widen reach. `nav.resolve`
strips every item the caller can't reach, and the gateway re-checks every page verb on click regardless.
"Give the ops team these pages" = define a **role** (the cap bundle) + share a **nav** to the team — the
role grants, the nav shapes the menu.

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
| Read/set my pick | `GET|POST /nav/pref` | `{"tool":"nav.pref.get|set","args":{…}}` | `id?,now*` |

## 3. The four entry kinds (+ one `group` level)

`items[]` is a flat, ordered, typed array; each item is exactly one kind:

- `surface` — a core page by its **opaque** key (`"channels"`, `"rules"`, …).
- `dashboard` — a specific dashboard (`dashboard:{id}`).
- `ext` — an extension page by its **opaque** ext id (rule 10 — never branched on; resolved via `ext.list`).
- `tag-group` — a **dynamic** entry `{ label, facets: [{key, value?}] }` expanded (via `tags.find`) to
  the dashboards carrying those facets at resolve time (tag a new dashboard → it appears, no nav edit).
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

## 5. Rules that bite

- **Nav never widens (the headline):** an item the caller lacks the cap for is stripped by `resolve`
  AND still 403s on direct access — the lens grants nothing. Never author a cap into an item.
- **Caps: 100 items per nav** (authored, incl. `group` children); **50 dashboards per expanded
  tag-group** at resolve (over-cap authored → `BadInput`; over-cap expansion is logged, not silently
  dropped).
- **The wall (rule 6):** navs + `nav_pref` are workspace-scoped; ws-B never resolves ws-A's navs.
- **Opaque ext ids (rule 10):** an `ext` item stores the id as data; rendering goes through the generic
  `ext.list`/`ExtHost` seam — never a branch on a named extension.
- **Precedence + stale pick:** a `nav_pref` pointing at a deleted/unshared nav falls through to the next
  tier, never errors.
