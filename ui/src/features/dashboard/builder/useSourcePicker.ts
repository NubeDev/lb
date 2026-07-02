// The SHELL ADAPTER for the reusable source picker (source-picker-package-scope.md). It is the ONE
// place `@/lib/*` (the shell's gateway/Tauri API clients) meets `@nube/source-picker`: it builds the
// package's injected `SourceLoaders` from the shipped clients and delegates to the package hook. Every
// dashboard consumer keeps importing `useSourcePicker` from here unchanged; the model + orchestration
// now live in the package (reusable from an extension, which supplies bridge-backed loaders instead).
//
// The loaders bag is module-level (stable) — the package hook keys its reload on `ws`.

import {
  useSourcePicker as usePkgSourcePicker,
  type SourceEntry,
  type SourceLoaders,
} from "@nube/source-picker";

import { listSeries } from "@/lib/ingest/ingest.api";
import { listExtensions, type ExtRow } from "@/lib/ext/ext.api";
import { listFlows, getFlow, listFlowNodes } from "@/lib/flows/flows.api";
import { listDatasources } from "@/lib/datasources";

/** The shell's picker data — same shape the package returns, but `installed` typed as the shell's
 *  fuller `ExtRow` (what `WidgetView`/`ExtWidget` consume). The runtime value IS the shell row —
 *  `listExtensions()` returns it — the package just widens it to its structural subset internally. */
export interface SourcePickerData {
  entries: SourceEntry[];
  installed: ExtRow[];
  loading: boolean;
}

// The shell's read seam — the shipped, capability-gated verbs the picker offers. Each is deny-tolerant
// inside the package hook (a denied read → that group is empty). Stable module-level object.
const shellLoaders: SourceLoaders = {
  listSeries: () => listSeries(),
  listExtensions: () => listExtensions(),
  listFlows: () => listFlows(),
  getFlow: (id) => getFlow(id),
  listFlowNodes: () => listFlowNodes(),
  listDatasources: () => listDatasources(),
};

/** Load + assemble the source picker for the shell. `ws` keys the re-load (the workspace switch). */
export function useSourcePicker(ws: string): SourcePickerData {
  const data = usePkgSourcePicker(shellLoaders, ws);
  // `installed` is the real shell `ExtRow` at runtime (from `listExtensions`); the package types it as
  // its structural subset. Re-assert the shell type for consumers that need the fuller row.
  return { entries: data.entries, installed: data.installed as ExtRow[], loading: data.loading };
}
