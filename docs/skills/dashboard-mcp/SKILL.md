---
name: dashboard-mcp
description: >-
  Manage Lazybones dashboards programmatically over the node gateway — list, read, create/update,
  delete, and share dashboards, add chart/stat/table panels bound to real data sources, and configure
  the X/Y plot builder. Use when a task says "create/seed/edit a dashboard", "add a widget/panel over
  the API", "automate dashboards", or "call dashboard.* over MCP/REST". Covers both the dedicated
  `/dashboards` REST routes and the universal `POST /mcp/call` bridge, plus the cell + plot-spec model.
---

# Managing dashboards over MCP / REST

A Lazybones node exposes dashboards two equivalent ways over its HTTP gateway (default
`http://127.0.0.1:8080`, override with `VITE_GATEWAY_URL`):

1. **Dedicated REST routes** — `GET/POST/DELETE /dashboards…` (ergonomic; the gateway supplies the clock).
2. **The universal MCP bridge** — `POST /mcp/call {tool, args}` for ANY host verb, dotted name
   (`dashboard.save`, `viz.query`, `store.query`, …). This is rule 7: capabilities are MCP tools, and
   the UI, agents, and extensions all call them the same way.

Both derive the **workspace + principal from the bearer token** — never from the request body (the hard
wall, README §6/§7). Every verb is capability-gated server-side; a denial is **opaque** (you cannot tell
"forbidden" from "absent").

## 1. Authenticate

```bash
# dev login: who + which workspace. An empty workspace bootstraps the caller as workspace-admin.
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Send it on every subsequent call as `Authorization: Bearer $TOKEN`.

Required capabilities (held by the default **member** cap set): `mcp:dashboard.list:call`,
`mcp:dashboard.get:call`, `mcp:dashboard.save:call`, `mcp:dashboard.delete:call`,
`mcp:dashboard.share:call`. Data-source reads need the source's own cap too (e.g.
`mcp:store.query:call`, `mcp:federation.query:call`).

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| List (roster) | `GET /dashboards` | `{"tool":"dashboard.list","args":{}}` | — |
| Read one | `GET /dashboards/{id}` | `{"tool":"dashboard.get","args":{"id":"…"}}` | `id` |
| Create/update | `POST /dashboards` | `{"tool":"dashboard.save","args":{…}}` | `id,title,cells[],variables[],now*` |
| Delete (tombstone) | `DELETE /dashboards/{id}` | `{"tool":"dashboard.delete","args":{"id":"…","now":…}}` | `id,now*` |
| Share / visibility | `POST /dashboards/{id}/share` | `{"tool":"dashboard.share","args":{…}}` | `id,visibility,team?,now*` |

`* now` — a caller-supplied millisecond timestamp (determinism, README §3: no wall-clock inside a verb).
The **dedicated REST routes fill `now` from the gateway clock**, so their bodies OMIT it; the **`/mcp/call`
path requires you to pass `now`** in `args`. `save` is an idempotent **UPSERT on `id`** — see the
read-modify-write rule below. Visibility is set ONLY via `share`, never on `save`.

## 3. The model

A dashboard is a persisted grid of cells + sharing metadata + variables:

```jsonc
{
  "id": "test", "title": "Test", "owner": "user:ada",
  "visibility": "private",          // private | team | workspace  (set via `share`)
  "variables": [],                   // dashboard variables ($var), optional
  "cells": [ /* … */ ],
  "updated_ts": 0
}
```

A **cell** (v3 shape — all v3 fields are additive; a v1/v2 cell still loads):

```jsonc
{
  "i": "demo_line",                  // stable grid key (unique per cell)
  "x": 0, "y": 10, "w": 6, "h": 5,   // react-grid-layout geometry (12-col grid, row units)
  "v": 3,
  "widget_type": "chart",            // legacy tag, keep "chart"|"stat"|"gauge"
  "title": "CPU & memory over time",
  "view": "timeseries",              // the renderer — see views below
  "binding": { "series": "" },       // required; empty for a query-bound cell
  "sources": [                        // v3 targets (the query); sources[0] is the primary
    { "refId": "A", "datasource": { "type": "surreal" },
      "tool": "store.query", "args": { "sql": "SELECT …" } }
  ],
  "options": { /* per-view options + the plot spec, see §5 */ }
}
```

Common `view` ids: `timeseries`, `barchart`, `piechart`, `stat`, `gauge`, `bargauge`, `table`,
`histogram`. (`chart` is an alias for `timeseries`.) The **X/Y plot builder** (§5) applies to the
cartesian charts: `timeseries` / `barchart` / `piechart`.

## 4. Data sources (`sources[].tool`)

A cell reads through any granted MCP tool. The `datasource.type` is a hint; the `tool` is what runs:

- **Direct SurrealDB** — `tool:"store.query"`, `datasource.type:"surreal"`, `args.sql` a single
  parse-allowlisted `SELECT` (workspace-walled at the host). Great for tabular multi-column rows.
- **Federation / external** — `tool:"federation.query"`, `datasource:{type:"federation","uid":"datasource:<ws>:<name>"}`.
- **Series** — `tool:"series.read"|"series.latest"|"series.find"` for the native time-series store.
- **Flows** — `tool:"flows.node_state"` to read a flow node's output.

`store.query` can return synthetic-but-real rows from an inline array (no table needed) — useful for
seeding demos:

```sql
SELECT * FROM [{t:'2026-06-01T00:00:00Z', cpu:50, mem:45}, {t:'2026-06-01T01:00:00Z', cpu:63, mem:39}]
```

## 5. The X/Y plot builder (`options.plot`)

For `timeseries`/`barchart`/`piechart`, set `options.plot` to a **PlotSpec** and the panel renders
through the shared chart (real titled X/Y axes, gridlines, legend). Omit it to keep the legacy chart.

```jsonc
"options": {
  "plot": {
    "type": "line",                  // line | area | bar | scatter | pie | histogram
    "xField": "t",                   // column name for the x/category axis
    "yFields": ["cpu", "mem"],       // one or more numeric columns → series
    "seriesField": "host",           // OPTIONAL: split one y column into one series per value (long→wide)
    "smooth": true,                  // line/area curve
    "stacked": false,                // bar/area
    "horizontal": false,             // bar orientation
    "bins": 12                       // histogram only
  }
  // legend/tooltip per-viz options may sit alongside, e.g.
  // "legend": {"showLegend": true, "displayMode": "list", "placement": "bottom", "calcs": []},
  // "tooltip": {"mode": "single", "sort": "none"}
}
```

Field kinds are inferred from the rows (time / number / category), so `xField` is typically the
temporal or categorical column and `yFields` the numeric ones. `pie` aggregates `yFields[0]` per
`xField` category. `histogram` bins `yFields[0]`.

> The in-channel query result has the twin per-user surface: `channel.chart_pref.get` / `.set`
> (a `PlotSpec` keyed by `channel__item__user`) merged over the host's auto-pick. Same model, different
> home (a viewer's choice vs. an owned panel).

## 6. Preview a panel's data before saving

`viz.query` runs a panel's `sources` under your authority and returns the rows/frames it would draw —
use it to confirm a source + SQL are right before you `save`:

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"viz.query",
  "args":{"panel":{"sources":[{"refId":"A","datasource":{"type":"surreal"},
    "tool":"store.query","args":{"sql":"SELECT * FROM [{host:\"a\",cpu:12},{host:\"b\",cpu:20}]"}}]}}}'
# → {"rows":[{"cpu":12,"host":"a"},…],"frames":[…]}
```

## 7. Read-modify-write (the one rule that bites)

`dashboard.save` **replaces the whole `cells` array**. To add or edit ONE panel you must `get` the
dashboard, mutate the `cells` list, and `save` the full list back — otherwise you drop every other
panel. Preserve `variables` too.

```bash
# add a multi-series line panel to dashboard "test", keeping its existing cells
python3 - <<'PY'
import json, os, time, urllib.request
TOK=os.environ["TOKEN"]; BASE="http://127.0.0.1:8080/mcp/call"
def call(tool,args):
    r=urllib.request.Request(BASE,json.dumps({"tool":tool,"args":args}).encode(),
        {"authorization":f"Bearer {TOK}","content-type":"application/json"})
    return json.load(urllib.request.urlopen(r))

d=call("dashboard.get",{"id":"test"})
cells=[c for c in d["cells"] if c["i"]!="demo_line"]        # read-modify-write: keep the rest
sql=("SELECT * FROM ["+", ".join(
    f"{{t:'2026-06-01T{h:02d}:00:00Z', cpu:{50+h}, mem:{45-h}}}" for h in range(12))+"]")
cells.append({
  "i":"demo_line","x":0,"y":10,"w":6,"h":5,"v":3,"widget_type":"chart",
  "title":"CPU & memory over time","view":"timeseries","binding":{"series":""},
  "sources":[{"refId":"A","datasource":{"type":"surreal"},"tool":"store.query","args":{"sql":sql}}],
  "options":{"plot":{"type":"line","xField":"t","yFields":["cpu","mem"],"smooth":True}},
})
call("dashboard.save",{"id":"test","title":d["title"],"cells":cells,
     "variables":d.get("variables",[]),"now":int(time.time()*1000)})   # `now` REQUIRED on /mcp/call
print("cells now:", len(cells))
PY
```

Via the dedicated REST route the same save is `POST /dashboards` with body
`{"id","title","cells","variables"}` (no `now`).

## Gotchas

- **Workspace/owner come from the token**, never args. To act in another workspace, `login` into it.
- **`save` is a full UPSERT of `cells`** — read-modify-write or you erase panels.
- **`now` is required on `/mcp/call`** save/delete/share; the REST routes fill it themselves.
- **Visibility only via `share`** (`private|team|workspace` + optional `team`); `save` never sets it.
- **Denials are opaque** — a missing cap and a missing dashboard both surface as forbidden/absent; check
  your token's caps if a call "vanishes".
- **`store.query` is SELECT-only**, parse-allowlisted and workspace-walled at the host.
- The plot spec (`options.plot`) only affects the cartesian views; other views ignore it.

## Related

- Dashboard model + verbs (source of truth): `docs/public/frontend/dashboard.md`.
- The X/Y plot builder: `docs/public/frontend/dashboard.md` → "X/Y plot builder",
  `docs/scope/frontend/dashboard/viz/xy-plot-builder-scope.md`.
- Capability / workspace rules: `README.md` §3, §6, §7; `docs/scope/auth-caps/`.
