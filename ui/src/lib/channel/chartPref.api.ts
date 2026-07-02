// The per-viewer chart-preference client (channel query charts, "best long term" persistence). A
// `query_result` item is authored by the query worker and is IMMUTABLE — the canonical result never
// changes. A viewer's choice of how to plot it is separate state: a small per-user, per-item record the
// host stores and returns, merged over the host's default at render. So two people can plot the same
// result differently, and the author-ownership + state/motion invariants stay intact.
//
// Reached over the universal MCP bridge (rule 7) via `mcp_call`, exactly like `tools.catalog` — no
// bespoke gateway route. The workspace + user come from the session token (the hard wall, §6/§7), never
// an argument. One call per export (FILE-LAYOUT frontend rules).

import type { PlotSpec } from "@/lib/charts";
import { readPlotSpec } from "@/lib/charts";
import { invoke } from "@/lib/ipc/invoke";

/** Read the caller's saved plot override for one query-result item, or null when they never saved one
 *  (the surface then shows the host's default). Mirrors `channel.chart_pref.get`. */
export async function getChartPref(channel: string, item: string): Promise<PlotSpec | null> {
  const r = await invoke<{ spec: unknown | null }>("mcp_call", {
    tool: "channel.chart_pref.get",
    args: { channel, item },
  });
  return readPlotSpec(r?.spec ?? null);
}

/** Save (upsert) the caller's plot override for one query-result item. Mirrors `channel.chart_pref.set`. */
export async function setChartPref(channel: string, item: string, spec: PlotSpec): Promise<void> {
  await invoke<{ ok: true }>("mcp_call", {
    tool: "channel.chart_pref.set",
    args: { channel, item, spec },
  });
}
