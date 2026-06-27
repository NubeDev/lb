// Load the source-picker data — the workspace's series (from `series.list`) and installed extensions
// (from `ext.list`), assembled into friendly `SourceEntry`s (widget-builder scope, "The source
// picker"). Both reads are shipped + capability-gated; a workspace with no series / no extensions
// simply yields fewer entries (honest, never a fake catalogue). Re-loads when the workspace changes.

import { useEffect, useState } from "react";

import { listSeries } from "@/lib/ingest/ingest.api";
import { listExtensions, type ExtRow } from "@/lib/ext/ext.api";
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
      // Both reads tolerate a deny/empty — a workspace may have granted only one of them.
      const [series, installed] = await Promise.all([
        listSeries().catch(() => [] as string[]),
        listExtensions().catch(() => [] as ExtRow[]),
      ]);
      if (cancelled) return;
      setData({
        entries: buildSourceEntries(series, installed),
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
