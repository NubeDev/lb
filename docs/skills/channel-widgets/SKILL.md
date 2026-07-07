---
name: channel-widgets
description: >-
  Show a LIVE widget in this conversation by writing a `rich_result` render envelope as a fenced
  ```lb-widget code block INSIDE your final answer text. The host splits it off and renders it
  through the SAME widget renderer dashboards use — no `channel.post`, no `dashboard.save`. The user
  can then pin it to a dashboard (dashboard.pin) as a durable widget.
---

# channel-widgets — answer with a live widget, not a wall of text

When the user asks for data that is better *seen* than read (a table of rows, a timeseries, a single
stat, a composed layout), write the widget **as a fenced ```lb-widget block inside your final answer
text**. The host worker (which owns the conversation's channel id) splits the block off, strips it
from your persisted prose answer, and posts it as a `rich_result` item that the dock renders live —
re-fetching the bound `source` on every view, so the widget stays true to the store, not a snapshot.

**You do NOT call `channel.post`. You do NOT need to know the channel id. Just write the block.**

## The choreography (prove, then embed)

1. **Prove the data first.** Run the exact `{tool, args}` you intend to bind — e.g.
   `federation.query { source, sql }` or `store.query { sql }` or `viz.query` — and confirm it
   returns non-empty rows with the columns you expect. An unproven binding is a dead widget.
2. **Write your final answer with the widget embedded.** Put prose for the user, then a fenced
   ```lb-widget block carrying the `rich_result` envelope JSON, then any closing prose. The block is
   removed from what the user reads in your text answer; the widget renders as its own card right
   below. A minimal example:

```
Here are the latest readings by site:

```lb-widget
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

Let me know if you want this pinned to a dashboard.
```

3. **One widget per answer.** The first valid ```lb-widget block wins; later ones are left in your
   text. If the user wants a second widget, they will ask again.

## Envelope fields

- `kind` — always `"rich_result"`.
- `v` — always `2` (the envelope version).
- `view` — `"table"` for rows, `"timeseries"` for time-keyed numeric data, `"stat"` for one number,
  or `"genui"` for a composed layout (see below). Use the ids the dashboard catalog declares
  (`dashboard.catalog` when unsure).
- `source` — the `{tool, args}` the viewer re-runs to load data. This is what makes the widget LIVE
  and honest: bind the exact proven query. Never inline `data` instead of a source — an inline-only
  envelope has no read path and degrades.
- `tools` — list every tool the `source` (and any `action`) names. The host intersects it with the
  viewer's own grant on every load, so you can never widen what a viewer may read.
- `options` / `fieldConfig` — optional presentation (same shapes as dashboard cells).

## GenUI / OpenUI layouts — a composed card in one block

A composed layout (stat + chart + table in one card) uses `view: "genui"`. **A preview never calls
`dashboard.save`** — write the envelope in your answer and the host renders the live GenUI surface
right in the conversation:

````
```lb-widget
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
````

- The IR is the typed GenUI spec the `core.genui-widget` skill documents (flat id-referenced
  `components` map, catalog names like `stack`/`card`/`stat`/`gauge`/`table`/`timeseries`/
  `barchart`/`piechart`/`text`/`markdown`); emit the IR directly — no prose around the IR inside the
  fence, just the JSON.
- Bind data by JSON Pointer against `/data/{refId}` — one entry per `sources[]` target
  (`/data/A/rows` for rows, `/data/A/value` for a scalar). Multiple refIds are fine.
- Only when the user asks to KEEP it: `dashboard.pin { dashboard, now, envelope }` with this same
  envelope minus `kind`/`v` — the pinned cell renders identically on the dashboard.

### Common IR mistakes (the host drops the widget silently — write it correctly)

The IR dialect is EXACTLY the one above; other UI-tree dialects you may know do not render here. If
your block is malformed, **no widget appears and the raw block stays in your text answer** — the user
sees what you tried, so write the canonical shape:

- `options.genui.ir` is a **JSON object, not a JSON-encoded string** — pass the IR inline,
  unquoted. (A string that parses to a valid IR is accepted and normalized, but don't rely on it.)
- Each component names its kind with **`component`, never `type`**
  (`{"id":"tbl","component":"table",...}`, not `{"type":"table"}`).
- Each component **repeats its map key as `id`** — `"tbl": { "id": "tbl", ... }`. A missing or
  mismatched `id` is rejected.
- The IR **must carry `"v": 1`** and a **`surface: { "surfaceId": "...", "root": "<id>" }`**
  whose `root` names a defined component. No surface → nothing renders → rejected.
- Component names must come from the catalog (`dashboard.catalog` lists them); an unknown name is
  rejected.
- A `slider`/`button`/`switch` without an `action` is decorative — wire the action (or leave the
  control out) so the widget does what it looks like it does.

## Capabilities

You need NO special capability to write the widget block — the host worker posts the envelope under
its OWN authority (the conversation channel is the run's own). Viewers load the `source` under their
OWN grant: a viewer without the read cap sees the standard denied state, never your data. If you
prove the data first (step 1), the capability flow is already exercised.

## Saving it as a durable widget / panel

The rendered item carries a **pin** affordance in the shell; headless, the same envelope (minus
`kind`/`v`) is the `dashboard.pin` argument:
`dashboard.pin { dashboard, title?, now, envelope: { view, source, options?, tools } }` — this mints
a persisted dashboard cell that renders identically. Offer this when the user says "keep", "save", or
"add to a dashboard".
