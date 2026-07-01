// Load the source-picker data — the workspace's series (from `series.list`) and installed extensions
// (from `ext.list`), assembled into friendly `SourceEntry`s (widget-builder scope, "The source
// picker"). Both reads are shipped + capability-gated; a workspace with no series / no extensions
// simply yields fewer entries (honest, never a fake catalogue). Re-loads when the workspace changes.

import { useEffect, useState } from "react";

import { listSeries } from "@/lib/ingest/ingest.api";
import { listExtensions, type ExtRow } from "@/lib/ext/ext.api";
import { listFlows, getFlow, listFlowNodes } from "@/lib/flows/flows.api";
import type { Flow, NodeDescriptor } from "@/lib/flows/flows.types";
import { buildSourceEntries, type SourceEntry } from "./sourcePicker";

export interface SourcePickerData {
  entries: SourceEntry[];
  /** The installed extensions (also handed to the cell renderer for `ext:<id>/<widget>` tiles). */
  installed: ExtRow[];
  loading: boolean;
}

/** Load + assemble the source picker. `ws` keys the re-load (the workspace switch). */
export function useSourcePicker(ws: string): SourcePickerData {
  const [data, setData] = useState<SourcePickerData>({
    entries: [],
    installed: [],
    loading: true,
  });

  useEffect(() => {
    let cancelled = false;
    setData((d) => ({ ...d, loading: true }));
    (async () => {
      // Every read tolerates a deny/empty — a workspace may have granted only some of them. The Flows
      // group composes `flows.list` (flows the caller may reach) + `flows.nodes` (descriptors); a flow
      // the caller cannot `flows.get` is silently skipped (it never appears in the picker — the cap-
      // scoped offer, flow-dashboard-binding-ux-scope). No `flows.*` cap → an empty Flows group.
      const [series, installed, flowSummaries, descriptors] = await Promise.all([
        listSeries().catch(() => [] as string[]),
        listExtensions().catch(() => [] as ExtRow[]),
        listFlows().catch(() => [] as { id: string; name: string }[]),
        listFlowNodes().catch(() => [] as NodeDescriptor[]),
      ]);
      const flows = (
        await Promise.all(
          flowSummaries.map((s) => getFlow(s.id).catch(() => null as Flow | null)),
        )
      ).filter((f): f is Flow => f != null);
      if (cancelled) return;
      setData({
        entries: buildSourceEntries(series, installed, flows, descriptors),
        installed,
        loading: false,
      });
    })();
    return () => {
      cancelled = true;
    };
  }, [ws]);

  return data;
}
