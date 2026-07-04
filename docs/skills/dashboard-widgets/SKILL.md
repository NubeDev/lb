---
name: dashboard-widgets
description: >-
  Author a dashboard from the real widget palette: call dashboard.catalog to discover which views
  exist and how each is configured, pick a view for your data shape, set options by their real
  catalog ids (not guessed keys), and save via dashboard.save — which REJECTS an unknown view at
  write time. Grounded in a live run against a real gateway.
---

# dashboard-widgets — author dashboard cells the host will accept

You are composing a dashboard page: a `cells[]` array where each cell carries a `view` (a render
kind), an `options`/`fieldConfig` blob, and a data `source` binding. **You do not know the palette —
so discover it, don't guess.** The host owns the answer to "which views exist and what each
configures" and hands it to you over one MCP read; then it **enforces** your choice on save. Discovery
and enforcement are two ends of one boundary (widget-catalog scope, Slice A).

The rule this skill exists for: **a view you invent is rejected at `dashboard.save`, not degraded to a
broken tile at render time.** So author from the catalog and your save is accepted; guess and it is
refused loudly with a message that names the fix.

## Step 1 — discover the palette (`dashboard.catalog`)

Call the verb first, every time (it is cheap, read-only, member-level). It returns one merged document
`{ v, views, extWidgets, genuiComponents }`:

- **`views`** — every built-in view with its per-view config-field schema. This is the vocabulary.
- **`extWidgets`** — the workspace's installed extension `[[widget]]` tiles as opaque
  `{ ext, widget, label, icon, data, scope }`. Compose one as `view: "ext:<ext>/<widget>"`.
- **`genuiComponents`** — the genui component names (author a `view:"genui"` cell via the
  [genui-widget](../genui-widget/SKILL.md) skill).

A real response (booted node, plain member token — abridged):

```json
{
  "v": 1,
  "views": [
    { "id": "timeseries", "label": "Time series", "kind": "viz", "version": 1,
      "buildable": true, "data": true, "action": false,
      "options": [
        { "id": "unit", "label": "Unit", "scope": "fieldConfig", "path": "unit", "control": "unit" },
        { "id": "custom.showPoints", "scope": "fieldConfig", "path": "custom.showPoints",
          "control": "select", "choices": ["auto", "never", "always"] },
        { "id": "legend.showLegend", "scope": "options", "path": "legend.showLegend", "control": "toggle" }
      ] },
    { "id": "gauge", "kind": "viz", "buildable": true, "data": true,
      "options": [ { "id": "min", "path": "min", "control": "number", "scope": "fieldConfig" },
                   { "id": "max", "path": "max", "control": "number", "scope": "fieldConfig" },
                   { "id": "thresholds", "path": "thresholds", "control": "thresholds", "scope": "fieldConfig" } ] }
  ],
  "extWidgets": [
    { "ext": "proof-panel", "widget": "Proof Tile", "label": "Proof Tile", "icon": "gauge",
      "data": true, "scope": ["series.latest"] }
  ],
  "genuiComponents": ["barchart", "gauge", "stat", "table", "timeseries", "..."]
}
```

## Step 2 — pick a view for your data shape

Read each view's fields, not just its id:

- **`kind`** — `viz` (renders data), `control` (writes an action), `read`, `scripted`, `genui`.
- **`data: true`** — the view consumes a data `source`; bind one (a `series.*` read, a `flows.*` read,
  `store.query`, an ext tool). `data: false` (a control) instead writes an `action`.
- **`action: true`** — the view calls a write tool on interaction (`switch`, `slider`, `button`, …).
- **`buildable`** — `true` = author new cells of this kind freely. `false` = an alias / escape hatch
  (`chart` aliases `timeseries`; `plot`/`d3`/`template`/`button` are compatibility/scripted). The save
  validator ACCEPTS both, but as an author **choose only `buildable: true` views** for new cells.

Match by shape: a single number → `stat` or `gauge`; a value over time → `timeseries`; rows →
`table`; parts of a whole → `piechart`. A live value plus a threshold → `gauge` with `thresholds`.

## Step 3 — set options by their real catalog ids

Each option carries the four fields you need to place it correctly — **do not invent keys**:

- **`id`** — the option's stable id (`unit`, `min`, `thresholds`, `legend.showLegend`).
- **`scope`** — WHERE it lives on the cell: `"fieldConfig"` → under `fieldConfig.defaults.<path>`;
  `"options"` → under `options.<path>`. This is the single most common authoring mistake — a unit set
  under `options` instead of `fieldConfig.defaults` renders as a default. Honor `scope`.
- **`path`** — the dotted path within that root (`custom.lineWidth`, `reduceOptions.calcs`).
- **`control`** + **`choices`** — the input kind; for a `select`, the value MUST be one of `choices`.

Example — a temperature gauge authored from the ids above:

```json
{
  "i": "temp", "x": 0, "y": 0, "w": 6, "h": 4, "view": "gauge",
  "options": { "min": 0, "max": 120, "showThresholdMarkers": true },
  "fieldConfig": { "defaults": { "unit": "celsius",
    "thresholds": { "steps": [ { "value": null, "color": "green" }, { "value": 80, "color": "red" } ] } } },
  "sources": [ { "refId": "A", "tool": "series.latest", "args": { "series": "office/temp" } } ]
}
```

`min`/`max`/`showThresholdMarkers` are `scope:"options"` → they sit on `options`; `unit`/`thresholds`
are `scope:"fieldConfig"` → under `fieldConfig.defaults`. Setting them by their catalog ids is what
makes the round-tripped cell render as intended instead of a default-everything fallback.

## Step 4 — save, and read a rejection

Call `dashboard.save { id, title, cells, now }`. The host validates **every cell's view name** (the
same authority for the shell, a `POST /mcp/call`, a routed-Zenoh writer, and a headless agent):

- a **known built-in** view → accepted;
- a well-formed **`ext:<id>/<widget>`** key → accepted *structurally* (it is NOT resolved against
  installs, so an uninstalled tile still saves; the catalog's `extWidgets` is where you learn which
  actually exist — an un-installed one renders the "unknown widget" placeholder);
- **`genui`** → its IR is validated separately (genui-widget skill);
- anything else → **`BadInput`**, and the **whole save is refused** (one bad cell blocks the page).

The message names the offending cell and view so the fix is one edit:

```
cell temp: unknown view 'heatmap' — call dashboard.catalog for the palette
```

```
cell temp: malformed extension view 'ext:x/' — expected 'ext:<id>/<widget>' (both non-empty)
```

When you see it: re-read `dashboard.catalog`, correct the `view`, save again. Never work around it by
degrading the cell — the rejection is the platform telling you the view does not exist.

## What this slice does NOT check (so author carefully anyway)

- **Option KEYS are not validated** (a named follow-up). A *valid* view carrying an invented
  `options.foo` still persists — it just won't render. The catalog gives you the real ids precisely so
  you don't rely on the host to catch a bad option; use them.
- **`version` is informational** — the catalog declares a per-view `version`, but no cell stamps it
  this slice. Don't copy it into a cell.
- **Ext-tile config** — an `extWidgets` entry has no option schema (the extension owns its config). You
  can PLACE an ext tile; you cannot generically configure its knobs yet.

## Capabilities

Reading the palette needs `mcp:dashboard.catalog:call` (member-level — it grants knowledge, not
access). Saving needs `mcp:dashboard.save:call`. Binding a source needs that source's own read cap
(`mcp:series.latest:call`, `mcp:flows.node_state:call`, `mcp:store.query:call`, …) — checked again at
render under the viewer, so a cell can never widen what a viewer may read. Listing `extWidgets` uses
`mcp:ext.list:call`; without it the built-in palette still returns in full, just with no ext tiles.

## Pin a tool result to a dashboard (`dashboard.pin`)

(widget-platform scope, Slice B) A tool that declares a `result` render envelope (source #2 —
`reminder.list` is the shipped example) is already a widget; `dashboard.pin` mints a persisted
`dashboard:{id}` cell from that envelope, host-side. The pin path is GENERIC over the tool id (rule 10):
the host treats `envelope.source.tool` as opaque data, never branches on it. So `reminder.list` is
dashboard-addable with ZERO reminder-specific code in the pin path — and so is any future tool that
declares a `result` (Slice C widens the coverage).

**The envelope.** The `x-lb-render` shape — the SAME object a tool's `ToolDescriptor.result` carries,
or a channel `rich_result` body minus its `kind`/`v` wire tags:

```json
{
  "view": "table",
  "source": { "tool": "reminder.list", "args": {} },
  "options": { "rowControls": [ … ] },
  "fieldConfig": { "defaults": {}, "overrides": [ … ] },
  "tools": ["reminder.list", "reminder.update", "reminder.fire", "reminder.delete"]
}
```

**The call.** `dashboard.pin { dashboard, envelope, title?, now }` over `POST /mcp/call`, or
`POST /dashboards/{id}/pin` from the browser. `title` is used only when creating a fresh dashboard
(idempotent UPSERT on `dashboard` — fresh id creates owner=caller, existing id updates owner-only); an
existing dashboard keeps its title.

```
POST /mcp/call   { "tool": "dashboard.pin",
                   "args": { "dashboard": "ops", "title": "Ops", "now": 10,
                             "envelope": { "view": "table", "source": { "tool": "reminder.list" }, … } } }
```

The host mints a v3 cell from the envelope — `i = "pin-{slug(source.tool||view)}"` (idempotent: re-pinning
the SAME envelope REPLACES the cell in place, preserving its layout; pinning a DIFFERENT envelope
appends) — runs it through the Slice A validation chain (`check_view_cells`/`check_genui_cells`/
`check_cells_bounds`), and persists. It returns the updated dashboard (hydrated). The cell renders through
the shipped `WidgetView` exactly as the envelope renders in a channel — a pinned reminder widget shows
its rows AND its row controls (enable switch + run-now + delete) on the dashboard, interactive.

**Idempotency by tool id (v1 limit).** `i = pin-{slug(source.tool)}` means ONE cell per tool per
dashboard: re-pinning `reminder.list` refreshes the cell, not duplicates. Pinning two DIFFERENT
envelopes from the same tool (e.g. `reminder.list` with different filters) currently collide on the same
cell — a known limit, not a silent bug. A future envelope-hash `i` widens this if a second filter matters.

**Capability.** `mcp:dashboard.pin:call` (member-level, its own cap — distinct from `dashboard.save`).
A plain member who can pin but not free-edit cells still works. A non-owner with the pin cap is denied on
an existing dashboard they don't own (owner-only-update). A hallucinated `view` in the envelope
(`"heatmap"`) is rejected through the pin path — the Slice A view-validator still fires.

**Headless agent.** Given `reminder.list`'s `result` from `tools.catalog`, an AI agent pins it the SAME
way — `POST /mcp/call dashboard.pin { dashboard, envelope: <descriptor.result>, now }`. No shell in the
loop; the host is the boundary. The resulting cell is byte-identical to a channel-origin pin (same mint
function). That is the "widgets are system-wide" payoff: every tool that declares a `result` is
dashboard-addable, by any client, through one generic MCP verb.
