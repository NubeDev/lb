---
name: reports
description: >-
  Author and export Lazybones reports programmatically over the node gateway — list, read,
  create/update, delete, and share reports; compose ordered blocks (markdown text, images, and live
  dashboard panels); manage reusable brand profiles; and export a branded PDF. Use when a task says
  "create/seed/edit a report", "add a panel/markdown block to a report", "export a report PDF over the
  API", or "call report.* / brand.* over MCP/REST". Covers the dedicated `/reports` + `/brands` REST
  routes and the universal `POST /mcp/call` bridge, plus the block model (markdown / image / panel).
---

# Authoring reports over MCP / REST

A Lazybones node exposes reports two equivalent ways over its HTTP gateway (default
`http://127.0.0.1:8080`, override with `VITE_GATEWAY_URL`):

1. **Dedicated REST routes** — `GET/POST/DELETE /reports…`, `GET/POST /brands…`,
   `POST /reports/{id}/export.pdf` (ergonomic; the gateway supplies the clock).
2. **The universal MCP bridge** — `POST /mcp/call {tool, args}` for ANY host verb (`report.save`,
   `brand.save`, …). Rule 7: capabilities are MCP tools; the UI, agents, and extensions all call them
   the same way.

Both derive **workspace + principal from the bearer token** — never from the body (the hard wall,
README §6/§7). Every verb is capability-gated server-side; a denial is **opaque** (you cannot tell
"forbidden" from "absent").

## 1. Authenticate

```bash
# dev login: who + which workspace.
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Send `Authorization: Bearer $TOKEN` on every call. Required capabilities (held by the default
**member** role): `mcp:report.list:call`, `mcp:report.get:call`, `mcp:report.save:call`,
`mcp:report.delete:call`, `mcp:report.share:call`, `mcp:report.export:call`, and the `brand.*` verbs
(`mcp:brand.{get,list,save,delete}:call`). Note `report.export` is a **concrete** cap — it is NOT
covered by any `mcp:*.*:call` wildcard, so an admin can grant view-but-not-export. (If a brand-new
verb is denied on an already-seeded dev store, the built-in role row may be stale — the resolver now
unions live built-in caps, so this is fixed on the next token mint; see
`docs/scope/auth-caps/builtin-role-freshness-scope.md`.)

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| List (roster) | `GET /reports` | `{"tool":"report.list","args":{}}` | — |
| Read one | `GET /reports/{id}` | `{"tool":"report.get","args":{"id":"…"}}` | `id` |
| Create/update | `POST /reports` | `{"tool":"report.save","args":{…}}` | `id,title,blocks[],brandId,toolbar,now*` |
| Delete (tombstone) | `DELETE /reports/{id}` | `{"tool":"report.delete","args":{"id":"…","now":…}}` | `id,now*` |
| Share / visibility | `POST /reports/{id}/share` | `{"tool":"report.share","args":{…}}` | `id,visibility,team?,now*` |
| Export PDF | `POST /reports/{id}/export.pdf` | (REST only — binary) | `id,snapshots{}` |

`* now` — a caller-supplied millisecond timestamp (no wall-clock inside a verb). The **dedicated REST
routes fill `now` from the gateway clock**, so their bodies OMIT it; the **`/mcp/call` path requires
`now`** in `args`. `save` is an idempotent **UPSERT on `id`** (read-modify-write: `report.get` → edit
`blocks[]` → `report.save` with the full record). Visibility is set ONLY via `share`, never on `save`.

`brand.*` mirrors the above (`GET/POST /brands`, `GET/DELETE /brands/{id}`) without `share`.

## 3. The model

A report is an ordered `blocks[]` array + brand + sharing metadata:

```jsonc
{
  "id": "q3-energy", "title": "Q3 Site Energy", "owner": "user:ada",
  "visibility": "private",            // private | team | workspace  (set via `share`)
  "brandId": "default",                // a brand profile id (empty = neutral default)
  "toolbar": {},                        // report-level range/vars (host-opaque; the client owns the shape)
  "blocks": [ /* … */ ],
  "schemaVersion": 1, "updated_ts": 0
}
```

A **block** is one of three kinds (tagged on `kind`; every other field is serde-defaulted):

```jsonc
// markdown — a GFM body + optional page break before
{ "kind": "markdown", "body": "# Executive summary\n\n…", "pageBreak": false }

// image — an asset_id into the shipped assets store + caption/width
{ "kind": "image", "assetId": "asset:…", "caption": "Site photo", "width": "full" }

// panel — EXACTLY the shipped Cell duality: an inline spec OR a `panel:{id}` library ref.
// A panel block IS a v3 Cell; it hydrates/validates through the same seams as a dashboard cell.
{ "kind": "panel", "cell": { "i": "p1", "view": "timeseries", "title": "Site kWh",
  "sources": [{ "refId":"A", "tool":"federation.query",
    "args": {"source":"demo-buildings","sql":"SELECT …"}, "datasource": {"type":"federation"} }],
  "options": {}, "binding": {"series":""}, "widget_type":"chart" } }
```

A `panel_ref` block is the same cell with `panelRef: "panel:{id}"` and no spec — the host hydrates it
at `report.get` (edit the library panel once, every report updates).

## 4. Worked example — author a report with markdown + a live panel, then export

```bash
# 1. Save a report with two blocks (markdown summary + a live timeseries panel bound to demo-buildings).
curl -s -X POST http://127.0.0.1:8080/reports \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{
    "id": "q3-energy",
    "title": "Q3 Site Energy",
    "brandId": "default",
    "blocks": [
      { "kind": "markdown", "body": "# Q3 executive summary\n\nEnergy held flat across all sites." },
      { "kind": "panel", "cell": {
          "i": "p1", "view": "timeseries", "title": "Site kWh",
          "widget_type": "chart", "binding": {"series": ""},
          "sources": [{ "refId": "A", "tool": "federation.query",
            "args": {"source":"demo-buildings","sql":"SELECT s.name AS site, AVG(r.value) AS kwh FROM point_reading r JOIN point p ON r.point_id=p.id JOIN meter m ON p.meter_id=m.id JOIN site s ON m.site_id=s.id GROUP BY s.name"},
            "datasource": {"type":"federation","uid":"datasource:acme:demo-buildings"} }],
          "options": {}, "fieldConfig": {"defaults":{},"overrides":[]} } }
    ]
  }' | jq .

# 2. Read it back — the panel block hydrates live (the viewer's caps gate the data at render).
curl -s http://127.0.0.1:8080/reports/q3-energy -H "authorization: Bearer $TOKEN" | jq .

# 3. (optional) share to the workspace.
curl -s -X POST http://127.0.0.1:8080/reports/q3-energy/share \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"visibility":"workspace"}' | jq .

# 4. Export a branded PDF. The browser captures each panel block to PNG under the EXPORTER's caps and
#    sends the snapshots with the request — the server NEVER fetches widget data for export.
curl -s -X POST http://127.0.0.1:8080/reports/q3-energy/export.pdf \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"snapshots": {"p1": "<base64-png-of-the-rendered-panel>"}}' \
  -o q3-energy.pdf
file q3-energy.pdf   # → PDF document
```

The PDF is assembled by the pure `lb-render` crate (Typst, embedded fonts, works offline): cover page,
running header/footer, brand colors/fonts, optional page numbers + TOC.

## 5. Gotchas

- **A report is a LENS, never a grant path.** Sharing a report shares its DEFINITION; every embedded
  panel's data is re-checked against the VIEWER's caps at render. A teammate without the panel's
  datasource cap sees the prose + a denied panel placeholder.
- **Export embeds data as pixels under the EXPORTER's caps** — exporting is the moment access is
  exercised, and the file is thereafter out-of-band. That's why `report.export` is its own cap.
- **`brandId` is never empty for long** — the host seeds one neutral default brand; pickers never show
  an empty list. Brand fonts are a SELECT of the embeddable fonts only (Libertinus Serif, DejaVu Sans
  Mono, New Computer Modern); unknown fonts silently fall back in the PDF.
- **No `report.usage` in v1** — nothing references a report yet, so delete is a plain soft-delete.
- **Max 200 blocks** per report (a save over is rejected loudly, never silently truncated).

## Related

- Scope: `docs/scope/reports/report-builder-scope.md` (the ask + full design).
- Public: `doc-site/content/public/reports/reports.md`.
- Sibling skills: `dashboard-mcp` (the cell/plot-spec model a panel block reuses), `panels`.
