// Load + assemble the source picker from the INJECTED loaders — the PURE async fn (no React). Projections
// off the ONE `loadCatalog` orchestration (system-catalog scope, "Two state contracts, one hook"):
// `loadCatalog` runs every loader the host wired with per-section deny-tolerance; this fn folds the
// READY sections into picker `SourceEntry` inputs and collapses DENIED → empty (the picker's
// existing contract). The explorer skin surfaces the same per-section state VISIBLY; the picker
// hides it behind the empty-group collapse. One loader path, two projections — never a second run.

import { buildSourceEntries, type SourceEntry } from "./sourcePicker";
import { loadCatalog } from "./loadCatalog";
import type {
  DatasourceRow,
  ExtRow,
  Flow,
  FlowSummary,
  NodeDescriptor,
  RuleSummary,
  SourceLoaders,
} from "./types";

/** The assembled picker data (sans loading flag — the caller owns that). */
export interface SourcePickerResult {
  entries: SourceEntry[];
  installed: ExtRow[];
}

/** Run every loader (deny-tolerant; absent loader ⇒ absent input) and fold the results into picker
 *  entries. The Flows group composes `flows.list` + `flows.nodes` + a per-flow `flows.get` — the
 *  catalog exposes the first two as `flowSummaries`/`flowDescriptors`; `getFlow` is per-flow so it
 *  stays picker-side (the catalog is a per-loader record, not a per-item join). */
export async function loadSourcePicker(loaders: SourceLoaders): Promise<SourcePickerResult> {
  const cat = await loadCatalog(loaders);
  const flowSummaries = readyData(cat.flowSummaries, [] as FlowSummary[]);
  const descriptors = readyData(cat.flowDescriptors, [] as NodeDescriptor[]);
  const getFlow = loaders.getFlow;
  const flows: Flow[] = getFlow
    ? (
        await Promise.all(flowSummaries.map((s) => getFlow(s.id).catch(() => null as Flow | null)))
      ).filter((f): f is Flow => f != null)
    : [];

  // Project the catalog's READY sections into picker inputs (DENIED → empty, the picker's contract).
  const series = readyData(cat.series, [] as string[]);
  const installed = readyData(cat.extensions, [] as ExtRow[]);
  const datasources = readyData(cat.datasources, [] as DatasourceRow[]);
  const rules = readyData(cat.rules, [] as RuleSummary[]);

  return {
    entries: buildSourceEntries({
      series,
      extensions: installed,
      flows,
      descriptors,
      datasources,
      rules,
    }),
    installed,
  };
}

/** Project a catalog section's READY data, collapsing denied/loading/absent into the empty fallback
 *  (the picker's existing contract: deny ⇒ empty group). */
function readyData<T>(section: { status: "loading" } | { status: "ready"; data: T } | { status: "denied"; error: string } | undefined, empty: T): T {
  return section?.status === "ready" ? section.data : empty;
}

