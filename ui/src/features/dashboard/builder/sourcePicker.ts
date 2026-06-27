// The source picker model — hide MCP from the author (widget-builder scope, "The source picker").
// "I don't know from MCP" is a requirement: the author picks a source by FRIENDLY LABEL grouped by
// origin, and each entry resolves to a `{tool, args}` (read source) or an action tool (write control).
// The picker reads ONLY shipped surfaces — `series.find`/`series.list` and `ext.list` — so a new
// source kind (a future tool, a new extension's verb) needs zero builder changes; it's just a tool.

import type { ExtRow } from "@/lib/ext/ext.api";
import type { Source, Action } from "@/lib/dashboard";

/** A friendly source entry the picker offers. `kind` groups it; `resolve()` gives the `{tool, args}`. */
export interface SourceEntry {
  /** Stable id for the option element. */
  id: string;
  /** The grouping origin (the picker's left-rail sections). */
  group: "series" | "live" | "extension" | "action";
  /** What the author sees — never a raw tool name. */
  label: string;
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

/** Assemble the whole picker from the shipped surfaces. `seriesNames` from `series.list`/`series.find`;
 *  `rows` from `ext.list`. The author sees labels grouped by origin; the cell gets the resolved tools. */
export function buildSourceEntries(seriesNames: string[], rows: ExtRow[]): SourceEntry[] {
  return [
    ...seriesEntries(seriesNames),
    ...liveEntries(seriesNames),
    ...extensionEntries(rows),
  ];
}
