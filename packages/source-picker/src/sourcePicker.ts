// The source-picker MODEL — hide MCP from the author (dashboard widget-builder scope, "The source
// picker"), now a standalone package. The author picks a source by FRIENDLY LABEL grouped by origin,
// and each entry resolves to a `{tool,args}` read source, a write action, or an `ext:<id>/<widget>`
// view. PURE: no I/O, no transport, no React — `buildSourceEntries` maps loader results (the row
// shapes in `types.ts`) to entries. The loader hook + the UI live in sibling files.

import type {
  Action,
  DatasourceRow,
  ExtRow,
  Flow,
  NodeDescriptor,
  QuerySummary,
  RuleParam,
  RuleSummary,
  Source,
} from "./types";

/** A friendly source entry the picker offers. `group` places it; `source`/`action`/`viewKey` is what
 *  selecting it yields (folded into a `SourceSelection` by the caller). */
export interface SourceEntry {
  /** Stable id for the option element + round-trip seeding. */
  id: string;
  /** The grouping origin (the picker's sections). `widget` is a packaged `[[widget]]` tile (a finished
   *  widget the developer shipped — distinct from `extension`, which offers an extension's raw tools). */
  group: "series" | "live" | "extension" | "action" | "sql" | "widget" | "flows" | "rules" | "queries";
  /** What the author sees — never a raw tool name. */
  label: string;
  /** For a `widget` entry: the icon name the tile declared (lucide id). */
  icon?: string;
  /** For a `widget` entry: the `ext:<id>/<widget>` view key the tile resolves to. */
  viewKey?: string;
  /** For a `widget` entry: `true` if the tile is a frames-in DATA view (its manifest set `data = true`).
   *  A data widget KEEPS the cell's `sources[]` (the shell resolves them to `ctx.data`) and shows the
   *  Query + Field tabs; a non-data widget owns its own data and clears targets when picked. */
  data?: boolean;
  /** The resolved read source `{tool,args}` (read/scripted views + a control's optional self-read). */
  source?: Source;
  /** The resolved write action (control views) — `argsTemplate` gets a `{{value}}` slot filled later. */
  action?: Action;
  /** True if the entry's tool writes (drives the Action group + write-capable views). */
  writes: boolean;
  /** For a `rules` entry: the rule's declared params, so a host can render a params form around the
   *  picker and fill the `rules.run` `args.params` (a rule with no params has none/empty). */
  params?: RuleParam[];
}

/** Derive a widget id from a tile — the label slug, lowercased, non-alnum → `-`. The renderer parses
 *  the same slug from the `ext:<id>/<widget>` key, so picker and renderer agree (one slug function).
 *  Exported so a host renderer can reuse it instead of forking a second slugger. */
export function widgetIdOf(w: { label: string }): string {
  return w.label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

/** Heuristic: does a tool name denote a write? Splits an extension's tools into read sources vs write
 *  actions in the picker (labelling only — the host is the real gate: cell.tools ∩ grant). */
function isWriteTool(tool: string): boolean {
  return /\.(publish|write|enqueue|command|set|send|record|create|delete|resolve|derive|simulate)$/.test(
    tool,
  );
}

/** A friendly label for an extension tool (drops the `<ext>.` prefix, keeps the verb). */
function toolLabel(ext: string, tool: string): string {
  const verb = tool.startsWith(`${ext}.`) ? tool.slice(ext.length + 1) : tool;
  return `${ext} · ${verb}`;
}

/** Series entries — each ⇒ `series.read` of that series. */
export function seriesEntries(seriesNames: string[]): SourceEntry[] {
  return seriesNames.map((s) => ({
    id: `series:${s}`,
    group: "series" as const,
    label: s,
    source: { tool: "series.read", args: { series: s } },
    writes: false,
  }));
}

/** Live (Zenoh) entries — each series also offers a live `series.watch` stream. */
export function liveEntries(seriesNames: string[]): SourceEntry[] {
  return seriesNames.map((s) => ({
    id: `live:${s}`,
    group: "live" as const,
    label: `${s} (live)`,
    source: { tool: "series.watch", args: { series: s } },
    writes: false,
  }));
}

/** Installed-extension TOOL entries — split an extension's `ui`/`widgets[]` scope tools into READ
 *  sources and WRITE actions by name heuristic. (A tile's finished-widget entry is `extWidgetEntries`.) */
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

/** Packaged-tile entries — ONE per `row.widgets[]` `[[widget]]`. Selecting it yields a
 *  `view: ext:<id>/<widget>` (the tile owns its data via `scope ∩ grant`). A disabled ext contributes
 *  none. The `viewKey` uses the SAME `widgetIdOf` slug the renderer parses. */
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
        data: tile.data === true,
        writes: false,
      });
    }
  }
  return out;
}

/** Flows entries — one per (flow, node, INPUT/OUTPUT port). An INPUT port → a write Action
 *  (`flows.inject`, a control drives the node's retained input); an OUTPUT port → a read Source
 *  (`flows.node_state`, extract this node's port). A node whose descriptor is missing contributes no
 *  ports (honest empty, never a guess). The author sees `flow › node › port (input|output)`. */
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

/** Rules entries — one per saved rule. Each ⇒ a read `rules.run {rule_id}` source: the rule fetches
 *  from the gated sources, computes over the rows in the cage (the data-stdlib: time/stats/`Frame`),
 *  and RETURNS records the panel draws (rules-as-source-scope). A rule is the most general query — the
 *  picker offers it as one opaque tool source, re-gated at the host per call (`mcp:rules.run:call`);
 *  whether its output is chart-shaped is the rule author's concern, an honest failure if not.
 *
 *  `route:false` on the emitted source makes a panel run READ-ONLY (rules-for-widgets-scope slice 2):
 *  the host skips the `alert()` fan-out so a 30 s auto-refresh doesn't stamp a fresh Inbox item + a
 *  must-deliver Outbox entry on every repaint. The host composes the arg exactly like the params form;
 *  `viz.query` never learns the flag exists (it stays an opaque `{tool, args}` to the viz plane). */
export function rulesEntries(rules: RuleSummary[]): SourceEntry[] {
  return rules.map((r) => ({
    id: `rule:${r.id}`,
    group: "rules" as const,
    label: r.name || r.id,
    source: { tool: "rules.run", args: { rule_id: r.id, route: false } },
    writes: false,
    params: r.params ?? [],
  }));
}

/** Saved-query entries — one per `query.list` row. Each ⇒ a read `query.run {id}` source: the host
 *  compiles the saved PRQL/raw text for the target's dialect and dispatches to `store.query`
 *  (platform) or `federation.query` (datasource), returning the SAME `{columns, rows}` shape every
 *  other tabular source yields. `query.run` COMPOSES the target's cap, it never widens it (rule 5):
 *  the caller needs `mcp:query.run:call` AND the underlying target cap, re-checked per call. Whether
 *  the saved text is currently valid is the author's concern — an honest failure if not. */
export function queryEntries(queries: QuerySummary[]): SourceEntry[] {
  return queries.map((q) => ({
    id: `query:${q.id}`,
    group: "queries" as const,
    label: q.name || q.id,
    source: { tool: "query.run", args: { id: q.id } },
    writes: false,
  }));
}

/** The id of the "SQL query" entry — the visual SQL builder + raw-SQL source over `store.query`. */
export const SQL_SOURCE_ID = "sql:query";

/** The single "SQL query" picker entry. Its `source.tool` is `store.query` so a host's tool set
 *  includes it (the bridge's leash); the concrete `sql` is filled by the host's SQL editor. */
export function sqlSourceEntry(): SourceEntry {
  return {
    id: SQL_SOURCE_ID,
    group: "sql",
    label: "SQL query (direct SurrealDB)",
    source: { tool: "store.query", args: { sql: "" } },
    writes: false,
  };
}

/** Inputs to `buildSourceEntries` — the loader results, each optional (absent → that group is absent). */
export interface SourceInputs {
  series?: string[];
  extensions?: ExtRow[];
  flows?: Flow[];
  descriptors?: NodeDescriptor[];
  datasources?: DatasourceRow[];
  rules?: RuleSummary[];
  queries?: QuerySummary[];
}

/** Assemble the whole picker from loader results. Series/live from `series`; extension + widget from
 *  `extensions`; flows from `flows`+`descriptors`; the SQL entry is always offered (the host's parse
 *  gate + ws wall make it safe regardless of which tables exist). Datasources are the DROPDOWN roster
 *  (`SourceInputs.datasources`), surfaced by the UI separately from these entries. */
export function buildSourceEntries(inputs: SourceInputs): SourceEntry[] {
  return [
    ...seriesEntries(inputs.series ?? []),
    ...liveEntries(inputs.series ?? []),
    ...extensionEntries(inputs.extensions ?? []),
    ...extWidgetEntries(inputs.extensions ?? []),
    ...flowsEntries(inputs.flows ?? [], inputs.descriptors ?? []),
    ...rulesEntries(inputs.rules ?? []),
    ...queryEntries(inputs.queries ?? []),
    sqlSourceEntry(),
  ];
}

/** Fold a chosen entry into a `SourceSelection` (drop the labelling fields; keep what the host stores). */
export function selectionOf(entry: SourceEntry): {
  id: string;
  source?: Source;
  action?: Action;
  viewKey?: string;
} {
  return { id: entry.id, source: entry.source, action: entry.action, viewKey: entry.viewKey };
}
