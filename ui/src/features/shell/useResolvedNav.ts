// Fetch the caller's resolved nav menu (nav scope) — the one `nav.resolve` payload NavRail renders.
// Re-resolves on workspace change and on window focus/visit (the scope's "the UI reloads
// `nav.resolve` on focus/visit, like the dashboard cache does" — a menu changes rarely, and a
// tag-group's membership moves through the tags plane, picked up on the next resolve). `items` is
// `null` while loading or on any error/deny, so NavRail cleanly falls back to the built-in `SURFACES`
// (never a blank rail). The nav is a LENS — it grants nothing; the resolve is already cap-stripped
// server-side.
//
// hide-and-pins scope: the payload also carries the workspace `hidden` echo (subtracted from the
// client-side fallback — the one tier the server can't strip) and the caller's resolved `pinned`
// favorites, plus a `togglePin(ref)` that flips one pin in the member-owned `nav_pref` (a partial
// write — the active pick is never clobbered) and re-resolves.

import { useCallback, useEffect, useState } from "react";

import { getNavPref, resolveNav, setNavPins, type ResolvedItem } from "@/lib/nav";

export interface ResolvedNavState {
  /** The resolved menu items, or `null` (loading / no nav / denied → fall back to SURFACES). */
  items: ResolvedItem[] | null;
  /** The workspace hidden-set echo — refs the fallback rail must subtract (declutter only). */
  hidden: string[];
  /** The caller's pinned favorites, resolved server-side (cap-, ext-, hidden-stripped), in order. */
  pinned: ResolvedItem[];
  /** Flip one pin ref (bare surface key | `ext:<id>` | `dashboard:<id>`) and re-resolve. */
  togglePin: (ref: string) => void;
}

/** The caller's resolved menu + hidden echo + pins (nav / hide-and-pins scopes). */
export function useResolvedNav(ws: string): ResolvedNavState {
  const [items, setItems] = useState<ResolvedItem[] | null>(null);
  const [hidden, setHidden] = useState<string[]>([]);
  const [pinned, setPinned] = useState<ResolvedItem[]>([]);

  const reload = useCallback(() => {
    let cancelled = false;
    resolveNav()
      .then((r) => {
        if (cancelled) return;
        // `fallback` (or an empty menu) → null, so NavRail renders the built-in SURFACES —
        // still minus `hidden`, still with `pinned` above (both apply to every tier).
        setItems(r.source === "fallback" || r.items.length === 0 ? null : r.items);
        setHidden(r.hidden ?? []);
        setPinned(r.pinned ?? []);
      })
      .catch(() => {
        // A deny / transport error → fall back to SURFACES (never a blank rail, never an error rail).
        if (!cancelled) {
          setItems(null);
          setHidden([]);
          setPinned([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Flip `ref` in the member's OWN stored pins (read the raw record — the resolved `pinned` is
  // stripped, so it can't be the write source), then re-resolve so the rail reflects it.
  const togglePin = useCallback(
    (ref: string) => {
      void getNavPref()
        .then((pref) => {
          const current = pref.pinned ?? [];
          const next = current.includes(ref)
            ? current.filter((p) => p !== ref)
            : [...current, ref];
          return setNavPins(next);
        })
        .then(() => reload())
        .catch(() => {
          // A deny/transport error leaves the rail as-is — the pin simply doesn't flip.
        });
    },
    [reload],
  );

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

  return { items, hidden, pinned, togglePin };
}
