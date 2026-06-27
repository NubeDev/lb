// Discover the extension PAGES installed in the current workspace (ui-federation scope). Reads
// `ext.list` and keeps the rows that declare a `[ui]` block — each becomes a cap-gated sidebar nav
// slot. Widget contributions (`row.widget`) are surfaced separately by the dashboard's palette.

import { useEffect, useState } from "react";

import { listExtensions, type ExtRow, type ExtUi } from "@/lib/ext/ext.api";
import { gatewayUrl } from "@/lib/ipc/http";
import { seedDevExtensions } from "@/lib/ipc/ext.fake";

/** One extension page available in the sidebar. */
export interface ExtPage {
  ext: string;
  ui: ExtUi;
}

/** Fetch the workspace's extension pages. Re-runs when the workspace changes. */
export function useExtensionPages(ws: string): ExtPage[] {
  const [pages, setPages] = useState<ExtPage[]>([]);

  useEffect(() => {
    let live = true;
    // No-backend dev build: ensure the reference extensions (incl. the UI one) exist so the slot shows
    // out of the box, exactly as the Extensions console does. The gateway path ignores this.
    if (!ws) return;
    if (gatewayUrl() === "" && import.meta.env.MODE !== "test") seedDevExtensions();
    listExtensions()
      .then((rows: ExtRow[]) => {
        if (!live) return;
        setPages(
          rows
            .filter((r) => r.ui && r.ui.entry)
            .map((r) => ({ ext: r.ext, ui: r.ui as ExtUi })),
        );
      })
      .catch(() => live && setPages([]));
    return () => {
      live = false;
    };
  }, [ws]);

  return pages;
}
