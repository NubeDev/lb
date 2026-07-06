---
name: channel-widgets
description: >-
  Show a LIVE widget in the current conversation: post a `rich_result` render envelope with
  channel.post — a table/chart/stat bound to the real query you already proved — instead of (or in
  addition to) a text answer. The user can then pin it to a dashboard (dashboard.pin) as a durable
  widget. Grounded in the shipped channel rich-responses render path.
---

# channel-widgets — answer with a live widget, not a wall of text

When the user asks for data that is better *seen* than read (a table of rows, a timeseries, a
single stat), you can render it as a real widget inside this conversation. The shell renders a
`rich_result` item through the SAME widget renderer dashboards use — the data is re-fetched live
from the `source` you bind, so the widget stays true to the store, not to a snapshot.

## The choreography (prove, then post)

1. **Prove the data first.** Run the exact `{tool, args}` you intend to bind — e.g.
   `federation.query { source, sql }` or `store.query { sql }` or `viz.query` — and confirm it
   returns non-empty rows with the columns you expect. An unproven binding is a dead widget.
2. **Post the envelope.** Call `channel.post` with the conversation's channel id (given to you in
   the goal as `[conversation channel: <cid>]`) and a `body` that is the JSON **string** of a
   `rich_result` payload:

```json
{
  "kind": "rich_result",
  "v": 2,
  "view": "table",
  "source": { "tool": "federation.query",
              "args": { "source": "demo-buildings",
                        "sql": "SELECT s.name, AVG(r.value) AS avg_kw FROM point_reading r JOIN point p ON p.id=r.point_id JOIN meter m ON m.id=p.meter_id JOIN site s ON s.id=m.site_id GROUP BY s.name" } },
  "tools": ["federation.query"]
}
```

   The call:
   `channel.post { "cid": "<cid>", "id": "<a fresh unique id, e.g. widget-<topic>-1>", "ts": <now>, "body": "<that JSON, serialized as one string>" }`
   — `id` is required (idempotency key: re-posting the same id replaces, not duplicates).
3. **Still answer in text.** The widget complements your answer; a one-line summary plus the
   widget beats either alone. Post the widget BEFORE you finish so it lands in the transcript.

## Envelope fields

- `view` — `"table"` for rows, `"timeseries"` for time-keyed numeric data, `"stat"` for one
  number. Use the ids the dashboard catalog declares (`dashboard.catalog` when unsure).
- `source` — the `{tool, args}` the viewer re-runs to load data. This is what makes the widget
  LIVE and honest: bind the exact proven query. Never inline `data` instead of a source — an
  inline-only envelope has no read path and degrades.
- `tools` — list every tool the `source` (and any `action`) names. The host intersects it with
  the viewer's own grant on every load, so you can never widen what a viewer may read.
- `options` / `fieldConfig` — optional presentation (same shapes as dashboard cells).

## GenUI / OpenUI layouts — preview in the conversation, not dashboard.save

A composed layout (stat + chart + table in one card) uses `view: "genui"`. **A preview is a
channel post, never a `dashboard.save`** — post the envelope and the shell renders the live
GenUI surface right in the conversation:

```json
{
  "kind": "rich_result", "v": 2,
  "view": "genui",
  "options": { "genui": { "v": 1, "ir": {
    "v": 1,
    "surface": { "surfaceId": "s1", "root": "root" },
    "components": {
      "root":  { "id": "root",  "component": "stack", "props": {}, "children": ["title", "tbl"] },
      "title": { "id": "title", "component": "text",  "props": { "value": "Latest point readings" } },
      "tbl":   { "id": "tbl",   "component": "table", "props": { "rows": { "$bind": "/data/A/rows" } } }
    }
  } } },
  "sources": [ { "refId": "A", "tool": "federation.query",
                 "args": { "source": "demo-buildings", "sql": "<your proven SQL>" } } ],
  "tools": ["federation.query"]
}
```

- The IR is the typed GenUI spec the `core.genui-widget` skill documents (flat id-referenced
  `components` map, catalog names like `stack`/`card`/`stat`/`gauge`/`table`/`timeseries`/
  `barchart`/`piechart`/`text`/`markdown`); emit the IR directly — no Lang text on this path.
- Bind data by JSON Pointer against `/data/{refId}` — one entry per `sources[]` target
  (`/data/A/rows` for rows, `/data/A/value` for a scalar). Multiple refIds are fine.
- Only when the user asks to KEEP it: `dashboard.pin { dashboard, now, envelope }` with this same
  envelope minus `kind`/`v` — the pinned cell renders identically on the dashboard.

## Saving it as a durable widget / panel

The rendered item carries a **pin** affordance in the shell; headless, the same envelope (minus
`kind`/`v`) is the `dashboard.pin` argument:
`dashboard.pin { dashboard, title?, now, envelope: { view, source, options?, tools } }` — this
mints a persisted dashboard cell that renders identically. Offer this when the user says "keep",
"save", or "add to a dashboard".

## Capabilities

Posting needs `mcp:channel.post:call` **and** the channel's `bus:chan/<cid>:pub` — both are the
caller's; a deny is honest, state it rather than retrying. Viewers load the `source` under their
OWN grant: a viewer without the read cap sees the standard denied state, never your data.
