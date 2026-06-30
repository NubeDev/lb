// The cached tool-catalog hook (channels-command-palette scope). Fetches `tools.catalog` ONCE on
// mount and caches it, so `/` opens from memory in 0ms — NO network in the hot path (the headline
// acceptance criterion). `revalidate()` re-fetches on focus/reconnect or after a grant change (caps
// can move mid-session; worst case a stale tool denies with an opaque inline error, never a crash).
// One hook per file (FILE-LAYOUT) — data only, no markup.

import { useCallback, useEffect, useState } from "react";

import { toolsCatalog } from "@/lib/channel/channel.api";
import type { ToolDescriptor } from "@/lib/channel/palette.types";

export interface CatalogState {
  /** The authorized tools (empty until the first fetch resolves). */
  tools: ToolDescriptor[];
  loading: boolean;
  error: string | null;
  /** Re-fetch the catalog (focus/reconnect/after a grants.* change). */
  revalidate: () => Promise<void>;
}

/** Load + cache the caller's authorized tool catalog. The fetch runs once on mount; the palette
 *  reads `tools` synchronously from this cache, so opening `/` triggers no request. */
export function useCatalog(): CatalogState {
  const [tools, setTools] = useState<ToolDescriptor[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const revalidate = useCallback(async () => {
    try {
      const catalog = await toolsCatalog();
      setTools(catalog.tools);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void revalidate();
  }, [revalidate]);

  return { tools, loading, error, revalidate };
}
