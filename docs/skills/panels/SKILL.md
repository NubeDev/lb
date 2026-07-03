---
name: panels
description: >-
  Manage Lazybones LIBRARY PANELS programmatically over the node gateway — a panel is the reusable,
  non-layout half of a dashboard cell (a `panel:{id}` asset) that many dashboards reference AND that
  renders standalone on `/t/$ws/panel/{id}`. List, read, create/update, delete (with delete-safety),
  share, and query usage; reference a panel from a dashboard cell (a "ref cell") and let the host
  hydrate it; open the standalone page. Use when a task says "make this chart reusable", "save as a
  library panel", "reuse a panel on another dashboard", "a chart not on a dashboard", or "call
  panel.* over MCP/REST".
---

# Managing library panels over MCP / REST

A **library panel** is the non-layout half of a v3 dashboard `Cell` (the spec: `view`/`title`/
`sources[]`/`transformations`/`fieldConfig`/`options`/…), lifted into its own workspace asset
`panel:{id}`. It is the `dashboard` asset cloned one level down — same slug id, owner, `private|team|
workspace` visibility, S4 `share` edge, tombstone delete, cap-gated verbs. It does two things a plain
inline cell cannot:

- **Reuse** — many dashboards **reference** it (a *ref cell*: layout + a `panelRef` + bounded overrides,
  no spec). Edit the panel once → every referencing dashboard shows the change on next load.
- **Standalone** — it renders on its own page `/t/$ws/panel/{id}`, no dashboard grid at all.

A panel is a **lens over data access, never a grant**: sharing a panel shares its *definition*; the
`sources[]` it reads are re-checked under the **viewer's** caps at render (`viz.query`'s per-target
leash) — a shared panel whose query you can't run renders "no data", never a leak.

The gateway derives the **workspace + owner from the bearer token**, never the body (the hard wall,
README §6/§7). Every verb is capability-gated server-side; a denial is **opaque**.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Required caps (held by the default **member** set): `mcp:panel.get:call`, `mcp:panel.list:call`,
`mcp:panel.save:call`, `mcp:panel.delete:call`, `mcp:panel.share:call`, `mcp:panel.usage:call`.
Data-source reads still need the source's own cap (`mcp:series.read:call`, `mcp:store.query:call`, …).

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| List (roster) | `GET /panels` | `{"tool":"panel.list","args":{}}` | — |
| Read one (full spec) | `GET /panels/{id}` | `{"tool":"panel.get","args":{"id":"…"}}` | `id` |
| Create/update | `POST /panels` | `{"tool":"panel.save","args":{…}}` | `id,title,spec,now*` |
| Delete (tombstone) | `DELETE /panels/{id}?force=` | `{"tool":"panel.delete","args":{"id":"…","force":false,"now":…}}` | `id,force?,now*` |
| Share / visibility | `POST /panels/{id}/share` | `{"tool":"panel.share","args":{…}}` | `id,visibility,team?,now*` |
| Usage (which dashboards) | `GET /panels/{id}/usage` | `{"tool":"panel.usage","args":{"id":"…"}}` | `id` |

`*` the MCP bridge takes an explicit `now` (a `u64` logical clock); the REST routes supply the clock.

## 3. Create a library panel

The `spec` is exactly the non-layout half of a v3 cell.

```bash
curl -s -X POST http://127.0.0.1:8080/panels -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
    "id": "cooler-temp-24h",
    "title": "Cooler temp (24h)",
    "spec": {
      "v": 3, "view": "timeseries", "widget_type": "chart",
      "title": "Cooler temp (24h)",
      "sources": [{ "refId": "A", "tool": "series.read", "args": { "series": "cooler.temp" } }]
    }
  }'
```

Make it reusable across the workspace:

```bash
curl -s -X POST http://127.0.0.1:8080/panels/cooler-temp-24h/share \
  -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{ "visibility": "workspace" }'
```

## 4. Reference it from a dashboard (a "ref cell")

A ref cell carries **layout + the ref + bounded overrides only** — NO spec. `dashboard.save` validates
the ref resolves in-workspace (loud `BadInput` on a typo/cross-ws ref); `dashboard.get` **hydrates** the
spec from the panel record at read time, host-side, keeping `panelRef` as a marker.

```bash
curl -s -X POST http://127.0.0.1:8080/dashboards -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
    "id": "exec-summary", "title": "Exec summary",
    "cells": [ { "i": "c1", "x": 0, "y": 0, "w": 8, "h": 4, "panelRef": "panel:cooler-temp-24h" } ]
  }'

# The read comes back HYDRATED — the cell has the panel's view/sources, plus panelRef as a marker:
curl -s http://127.0.0.1:8080/dashboards/exec-summary -H "Authorization: Bearer $TOKEN" \
  | jq '.cells[0] | { i, panelRef, view }'
# → { "i": "c1", "panelRef": "panel:cooler-temp-24h", "view": "timeseries" }
```

**Edit once, reuse everywhere:** `panel.save` the panel again and every referencing dashboard shows the
change on next `dashboard.get`. The ref is **authoritative** — any inline spec a client echoes back on a
ref cell is stripped on save (you cannot accidentally de-link by re-sending a stale copy).

## 5. Delete-safety + dangling refs

`panel.delete` **refuses** while dashboards reference the panel, returning the usage list — pass
`force=true` to tombstone anyway. A referencing cell then hydrates to an honest **placeholder**
(`panelMissing: true`, no spec leaked) until it is relinked or removed. Re-saving the panel un-hides it
(the ref re-hydrates).

```bash
curl -s http://127.0.0.1:8080/panels/cooler-temp-24h/usage -H "Authorization: Bearer $TOKEN" | jq
# → { "usage": [ { "dashboard": "exec-summary", "title": "Exec summary", "cells": 1 } ] }
curl -s -X DELETE "http://127.0.0.1:8080/panels/cooler-temp-24h" -H "Authorization: Bearer $TOKEN" -i \
  | head -1            # → HTTP/1.1 400 Bad Request (in use)
curl -s -X DELETE "http://127.0.0.1:8080/panels/cooler-temp-24h?force=true" -H "Authorization: Bearer $TOKEN" -i \
  | head -1            # → HTTP/1.1 204 No Content
```

## 6. The standalone page

Open `/t/<ws>/panel/<id>` (hash-routed UI): it renders the ONE panel full-bleed through the same shipped
render path (`WidgetHost` → `WidgetView`/`usePanelData` → the viz bridge — no parallel renderer), with
its own range picker + `?var-<name>=` URL selections. Cap-gated on `panel.get`. This is the "chart not on
a dashboard" surface, and a natural nav-entry target.

## 7. Rules that bite

- **The wall (rule 6):** a panel is workspace-scoped; ws-B cannot read ws-A's panels, and a ws-B
  dashboard referencing a ws-A `panel:{id}` is **rejected at save**.
- **Lens, not grant (rule 5):** sharing a panel never widens data access — the `sources[]` re-check under
  the viewer's caps at render. Denied → an empty frame, not a leak.
- **Slug is forever, title is free:** the id is the stable ref; rename changes `title` only.
- **No versioning in v1** — LWW by slug, like dashboards.
