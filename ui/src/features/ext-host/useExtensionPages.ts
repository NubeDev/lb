// Discover the extension PAGES installed in the current workspace (ui-federation scope). Reads
// `ext.list` and keeps the rows that declare a `[ui]` block — each becomes a cap-gated sidebar nav
// slot. Widget contributions (`row.widget`) are surfaced separately by the dashboard's palette.

import { useEffect, useState } from "react";

import { listExtensions, type ExtRow, type ExtUi } from "@/lib/ext/ext.api";

/** One extension page available in the sidebar. */
export interface ExtPage {
  ext: string;
  ui: ExtUi;
}

export interface ExtensionPagesResult {
  pages: ExtPage[];
  loading: boolean;
}

/** Fetch the workspace's extension pages. Re-runs when the workspace changes. */
export function useExtensionPages(ws: string): ExtensionPagesResult {
  const [pages, setPages] = useState<ExtPage[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let live = true;
    if (!ws) {
      setPages([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    // Reads the real node's installed extensions; no demo seed (the fake is gone).
    listExtensions()
      .then((rows: ExtRow[]) => {
        if (!live) return;
        setPages(
          rows
            .filter((r) => r.ui && r.ui.entry)
            .map((r) => ({ ext: r.ext, ui: r.ui as ExtUi })),
        );
      })
      .catch(() => live && setPages([]))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [ws]);

  return { pages, loading };
}
