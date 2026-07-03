// Load + assemble the source picker from the INJECTED loaders (source-picker-package-scope.md). This
// is the shipped dashboard `useSourcePicker` with its `@/lib/*` imports replaced by a `SourceLoaders`
// param — so it runs from the shell (gateway/Tauri) and an extension (bridge) alike. Every read
// tolerates a deny/empty (a workspace may grant only some): a rejected loader → that group is empty
// (honest, capability-scoped offer), never a fabricated catalogue (CLAUDE §9). Re-loads on `ws` change.

import { useEffect, useRef, useState } from "react";

import type { SourceEntry } from "./sourcePicker";
import { loadSourcePicker } from "./loadSourcePicker";
import type { ExtRow, SourceLoaders } from "./types";

export interface SourcePickerData {
  entries: SourceEntry[];
  /** The installed extensions (also handed to a cell renderer for `ext:<id>/<widget>` tiles). */
  installed: ExtRow[];
  loading: boolean;
}

/** Load + assemble the picker. `loaders` is the host's read seam; `ws` keys the re-load (the workspace
 *  switch). The effect keys on `ws` ONLY and reads `loaders` through a ref kept current every render —
 *  so an UNMEMOIZED `loaders` object (a fresh literal per render, the easy host mistake) does NOT loop.
 *  A host that swaps to a genuinely different transport should also change `ws` (or remount). */
export function useSourcePicker(loaders: SourceLoaders, ws: string): SourcePickerData {
  const [data, setData] = useState<SourcePickerData>({
    entries: [],
    installed: [],
    loading: true,
  });
  const loadersRef = useRef(loaders);
  loadersRef.current = loaders;

  useEffect(() => {
    const loaders = loadersRef.current;
    let cancelled = false;
    setData((d) => ({ ...d, loading: true }));
    (async () => {
      const { entries, installed } = await loadSourcePicker(loaders);
      if (cancelled) return;
      setData({ entries, installed, loading: false });
    })();
    return () => {
      cancelled = true;
    };
    // Keyed on `ws` ONLY — `loaders` is read via a ref (see doc above), so it isn't a dep.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ws]);

  return data;
}
