// The source picker model — hide MCP from the author (widget-builder scope, "The source picker").
// "I don't know from MCP" is a requirement: the author picks a source by FRIENDLY LABEL grouped by
// origin, and each entry resolves to a `{tool, args}` (read source) or an action tool (write control).
// The picker reads ONLY shipped surfaces — `series.find`/`series.list` and `ext.list` — so a new
// source kind (a future tool, a new extension's verb) needs zero builder changes; it's just a tool.

import type { ExtRow } from "@/lib/ext/ext.api";
import type { Source, Action } from "@/lib/dashboard";
import type { Flow, NodeDescriptor } from "@/lib/flows/flows.types";
import { widgetIdOf } from "./ExtWidget";

/** A friendly source entry the picker offers. `kind` groups it; `resolve()` gives the `{tool, args}`. */
export interface SourceEntry {
  /** Stable id for the option element. */
  id: string;
  /** The grouping origin (the picker's left-rail sections). `widget` is a packaged extension tile (a
   *  finished `[[widget]]` the developer shipped — distinct from `extension`, which offers raw tools). */
  group: "series" | "live" | "extension" | "action" | "sql" | "widget" | "flows";
  /** What the author sees — never a raw tool name. */
  label: string;
  /** For a `widget` entry: the icon name the tile declared (lucide id), carried onto the option. */
  icon?: string;
  /** For a `widget` entry: the `ext:<id>/<widget>` view key the selected tile resolves to (a packaged
   *  tile IS its own view — no view chooser; the cell's `view`/`widget_type` is set to this). */
  viewKey?: string;
  /** The resolved read source `{tool, args}` (for read/scripted views + a control's optional self-read). */
  source?: Source;
  /** The resolved write action (for control views) — `argsTemplate` gets a `{{value}}` slot filled later. */
  action?: Action;
  /** True if the entry's tool writes (drives the Action group + write-capable scripted/control views). */
  writes: boolean;
}

/** Heuristic: does a tool name denote a write? Used to split an extension's tools into read sources vs
 *  write actions in the picker. The host is the real gate (cell.tools ∩ grant); this is labelling only. */
function isWriteTool(tool: string): boolean {
  return /\.(publish|write|enqueue|command|set|send|record|create|delete|resolve|derive|simulate)$/.test(
    tool,
  );
}

/** A friendly label for an extension tool (drops the `<ext>.` prefix, title-cases the verb). */
function toolLabel(ext: string, tool: string): string {
  const verb = tool.startsWith(`${ext}.`) ? tool.slice(ext.length + 1) : tool;
  return `${ext} · ${verb}`;
}

/** Build the Series entries from concrete series names (each ⇒ `series.read` of that series). */
export function seriesEntries(seriesNames: string[]): SourceEntry[] {
  return seriesNames.map((s) => ({
    id: `series:${s}`,
    group: "series" as const,
    label: s,
    source: { tool: "series.read", args: { series: s } },
    writes: false,
  }));
}

/** Build the Live (Zenoh) entries — each series also offers a live `series.watch` stream. */
export function liveEntries(seriesNames: string[]): SourceEntry[] {
  return seriesNames.map((s) => ({
    id: `live:${s}`,
    group: "live" as const,
    label: `${s} (live)`,
    source: { tool: "series.watch", args: { series: s } },
    writes: false,
  }));
}

/** Build the installed-extension entries from `ext.list`. An extension's `ui.scope` + `widgets[].scope`
 *  name the tools it may call; we split them into READ sources and WRITE actions by name heuristic. */
export function extensionEntries(rows: ExtRow[]): SourceEntry[] {
  const out: SourceEntry[] = [];
  for (const row of rows) {
    if (!row.enabled) continue;
    const tools = new Set<string>();
    row.ui?.scope?.forEach((t) => tools.add(t));
    row.widgets?.forEach((w) => w.scope?.forEach((t) => tools.add(t)));
    for (const tool of tools) {
      const writes = isWriteTool(tool);
      out.push({
        id: `ext:${row.ext}:${tool}`,
        group: writes ? "action" : "extension",
        label: toolLabel(row.ext, tool),
        source: writes ? undefined : { tool, args: {} },
        action: writes ? { tool, argsTemplate: {} } : undefined,
        writes,
      });
    }
  }
  return out;
}

/** Build the packaged-tile entries from `ext.list` — ONE entry per `row.widgets[]` `[[widget]]` tile.
 *  Unlike `extensionEntries` (which harvests a tile's *tools* as build-your-own sources), this emits the
 *  *finished tile* itself: selecting it produces a `view: "ext:<id>/<widget>"` cell (no view chooser, no
 *  tool wiring — the tile owns its data via `scope ∩ grant`, re-checked at the host). The widget id is
 *  derived with the SAME `widgetIdOf` slug the renderer parses, so the key the picker builds == the key
 *  `ExtWidget` mounts. A disabled extension contributes no tiles. */
export function extWidgetEntries(rows: ExtRow[]): SourceEntry[] {
  const out: SourceEntry[] = [];
  for (const row of rows) {
    if (!row.enabled) continue;
    for (const tile of row.widgets ?? []) {
      const widgetId = widgetIdOf(tile);
      out.push({
        id: `widget:${row.ext}/${widgetId}`,
        group: "widget",
        label: `${row.ext} · ${tile.label}`,
        icon: tile.icon,
        viewKey: `ext:${row.ext}/${widgetId}`,
        writes: false,
      });
    }
  }
  return out;
}

/** Build the Flows group — one entry per (flow, node, INPUT/OUTPUT port). The picker reads only
 *  shipped verbs (`flows.list`→flow, `flows.get`→nodes, `flows.nodes`→descriptors), so a flow node
 *  becomes a binding with no hand-typed tool name (flow-dashboard-binding-ux-scope, the headline UX):
 *
 *  - an INPUT port → a write Action `{tool:"flows.inject", argsTemplate:{id,node,port,value:"{{value}}"}}`
 *    — a switch/slider/JSON control drives that port's retained `flow_input`;
 *  - an OUTPUT port → a read Source reading the node's `payload` via `flows.node_state` (instant +
 *    canvas-cadence refresh; NOT polling `runs.get`, NOT a series.watch on an arbitrary node — D).
 *
 *  The author sees `cooler-ctl › setpoint-in › payload (input)`, never `flows.inject`. A node whose
 *  descriptor is missing (an unknown ext type) contributes no ports — an honest empty, never a guess. */
export function flowsEntries(flows: Flow[], descriptors: NodeDescriptor[]): SourceEntry[] {
  const byType = new Map(descriptors.map((d) => [d.type, d]));
  const out: SourceEntry[] = [];
  for (const flow of flows) {
    for (const node of flow.nodes ?? []) {
      const desc = byType.get(node.type);
      if (!desc) continue;
      for (const port of desc.inputs ?? []) {
        out.push({
          id: `flows:in:${flow.id}:${node.id}:${port}`,
          group: "flows",
          label: `${flow.name || flow.id} › ${node.id} › ${port} (input)`,
          // The control fills `{{value}}` on interaction; the host re-checks the cap + ws + grant.
          action: {
            tool: "flows.inject",
            argsTemplate: { id: flow.id, node: node.id, port, value: "{{value}}" },
          },
          writes: true,
        });
      }
      for (const port of desc.outputs ?? []) {
        out.push({
          id: `flows:out:${flow.id}:${node.id}:${port}`,
          group: "flows",
          label: `${flow.name || flow.id} › ${node.id} › ${port} (output)`,
          // The read reads the whole flow's node_state; the view/control extracts THIS node's port.
          source: {
            tool: "flows.node_state",
            args: { id: flow.id, __flowNode: node.id, __flowPort: port },
          },
          writes: false,
        });
      }
    }
  }
  return out;
}

/** The id the picker uses for the "SQL query" entry — the visual SQL builder + raw-SQL Code source
 *  (widget-builder Slice C). Selecting it opens the Builder⇄Code editor, which resolves to a
 *  `{ tool: "store.query", args: { sql, vars? } }` source (Slice A). */
export const SQL_SOURCE_ID = "sql:query";

/** The single "SQL query" picker entry. Its `source.tool` is `store.query` so the cell's tool set
 *  includes it (the bridge's leash); the concrete `sql` is filled in by the Builder⇄Code editor and
 *  written back onto `source.args` before the cell is added. The schema dropdowns the visual builder
 *  needs come from `store.schema` (also leashed, read at authoring time in the trusted shell). */
export function sqlSourceEntry(): SourceEntry {
  return {
    id: SQL_SOURCE_ID,
    group: "sql",
    label: "SQL query (direct SurrealDB)",
    source: { tool: "store.query", args: { sql: "" } },
    writes: false,
  };
}

/** Assemble the whole picker from the shipped surfaces. `seriesNames` from `series.list`/`series.find`;
 *  `rows` from `ext.list`. The author sees labels grouped by origin; the cell gets the resolved tools.
 *  The "SQL query" entry is always offered (the parse gate + workspace wall + row cap at the host make
 *  it safe regardless of which tables exist). */
export function buildSourceEntries(
  seriesNames: string[],
  rows: ExtRow[],
  flows: Flow[] = [],
  descriptors: NodeDescriptor[] = [],
): SourceEntry[] {
  return [
    ...seriesEntries(seriesNames),
    ...liveEntries(seriesNames),
    ...extensionEntries(rows),
    ...extWidgetEntries(rows),
    ...flowsEntries(flows, descriptors),
    sqlSourceEntry(),
  ];
}
