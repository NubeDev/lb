// Fetch the caller's resolved nav menu (nav scope) — the one `nav.resolve` payload NavRail renders.
// Re-resolves on workspace change and on window focus/visit (the scope's "the UI reloads
// `nav.resolve` on focus/visit, like the dashboard cache does" — a menu changes rarely, and a
// tag-group's membership moves through the tags plane, picked up on the next resolve). Returns
// `null` while loading or on any error/deny, so NavRail cleanly falls back to the built-in `SURFACES`
// (never a blank rail). The nav is a LENS — it grants nothing; the resolve is already cap-stripped
// server-side.

import { useCallback, useEffect, useState } from "react";

import { resolveNav, type ResolvedItem } from "@/lib/nav";

/** The caller's resolved menu items, or `null` (loading / no nav / denied → fall back to SURFACES). */
export function useResolvedNav(ws: string): ResolvedItem[] | null {
  const [items, setItems] = useState<ResolvedItem[] | null>(null);

  const reload = useCallback(() => {
    let cancelled = false;
    resolveNav()
      .then((r) => {
        if (cancelled) return;
        // `fallback` (or an empty menu) → null, so NavRail renders the built-in SURFACES.
        setItems(r.source === "fallback" || r.items.length === 0 ? null : r.items);
      })
      .catch(() => {
        // A deny / transport error → fall back to SURFACES (never a blank rail, never an error rail).
        if (!cancelled) setItems(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const cleanup = reload();
    // Re-resolve on focus (a tag-group's membership may have moved; a share may have changed).
    const onFocus = () => reload();
    if (typeof window !== "undefined") window.addEventListener("focus", onFocus);
    return () => {
      cleanup?.();
      if (typeof window !== "undefined") window.removeEventListener("focus", onFocus);
    };
    // Re-resolve when the workspace changes (the wall — a different ws is a different menu).
  }, [ws, reload]);

  return items;
}
