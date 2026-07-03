// Load + assemble the source picker from the INJECTED loaders — the PURE async fn (no React). Extracted
// from `useSourcePicker` so a host can drive it through its OWN cache layer (the lb dashboard wraps this
// in react-query for de-dup, dashboard-query-cache-scope) while the package hook keeps the simple
// self-contained behaviour. Every read tolerates a deny/empty (a workspace may grant only some): a
// rejected loader → that group is empty (honest, capability-scoped offer), never a fabricated catalogue
// (CLAUDE §9). One responsibility: turn a `SourceLoaders` bag into `{entries, installed}`.

import { buildSourceEntries, type SourceEntry } from "./sourcePicker";
import type { ExtRow, Flow, NodeDescriptor, SourceLoaders } from "./types";

/** The assembled picker data (sans loading flag — the caller owns that). */
export interface SourcePickerResult {
  entries: SourceEntry[];
  installed: ExtRow[];
}

/** Run every loader (deny-tolerant) and fold the results into picker entries. The Flows group composes
 *  `flows.list` (flows the caller may reach) + `flows.nodes` (descriptors) + a `flows.get` per flow; a
 *  flow the caller cannot `flows.get` is silently skipped. */
export async function loadSourcePicker(loaders: SourceLoaders): Promise<SourcePickerResult> {
  const [series, installed, flowSummaries, descriptors, datasources] = await Promise.all([
    loaders.listSeries?.().catch(() => [] as string[]) ?? Promise.resolve([] as string[]),
    loaders.listExtensions?.().catch(() => [] as ExtRow[]) ?? Promise.resolve([] as ExtRow[]),
    loaders.listFlows?.().catch(() => []) ?? Promise.resolve([]),
    loaders.listFlowNodes?.().catch(() => [] as NodeDescriptor[]) ??
      Promise.resolve([] as NodeDescriptor[]),
    loaders.listDatasources?.().catch(() => []) ?? Promise.resolve([]),
  ]);
  const getFlow = loaders.getFlow;
  const flows = getFlow
    ? (
        await Promise.all(flowSummaries.map((s) => getFlow(s.id).catch(() => null as Flow | null)))
      ).filter((f): f is Flow => f != null)
    : [];
  return {
    entries: buildSourceEntries({ series, extensions: installed, flows, descriptors, datasources }),
    installed,
  };
}
