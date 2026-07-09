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

import { BUILTIN_PICK, getNavPref, resolveNav, setNavPins, setNavPref, type ResolvedItem } from "@/lib/nav";

export interface ResolvedNavState {
  /** The resolved menu items, or `null` (loading / no nav / denied → fall back to SURFACES). */
  items: ResolvedItem[] | null;
  /** The workspace hidden-set echo — refs the fallback rail must subtract (declutter only). */
  hidden: string[];
  /** The caller's pinned favorites, resolved server-side (cap-, ext-, hidden-stripped), in order. */
  pinned: ResolvedItem[];
  /** Flip one pin ref (bare surface key | `ext:<id>` | `dashboard:<id>`) and re-resolve. */
  togglePin: (ref: string) => void;
  // ── no-lockout scope: the escape hatch (anyone handed a too-narrow nav can bail to all pages) ──
  /** Is the caller currently forcing the built-in sidebar (their pick is the `__builtin__` sentinel)? */
  usingBuiltin: boolean;
  /** Force the built-in sidebar — write the `__builtin__` pick and re-resolve. Member-owned. */
  showAllPages: () => void;
  /** Undo the force — clear the pick so normal (team/default) resolution resumes. Member-owned. */
  useMyMenu: () => void;
}

/** The caller's resolved menu + hidden echo + pins (nav / hide-and-pins scopes). */
export function useResolvedNav(ws: string): ResolvedNavState {
  const [items, setItems] = useState<ResolvedItem[] | null>(null);
  const [hidden, setHidden] = useState<string[]>([]);
  const [pinned, setPinned] = useState<ResolvedItem[]>([]);
  // no-lockout: whether the caller is currently forcing the built-in sidebar (read from their pref).
  const [usingBuiltin, setUsingBuiltin] = useState(false);

  const reload = useCallback(() => {
    let cancelled = false;
    // Resolve the menu AND read the raw pick in parallel — the pick tells us whether the caller is
    // FORCING the built-in sidebar (`__builtin__` sentinel), which `resolveNav` reports only as a
    // generic `fallback` (indistinguishable from "no nav exists"). We need the distinction to show
    // the right escape-hatch label ("Use my menu" only when they've explicitly forced built-in).
    Promise.all([resolveNav(), getNavPref().catch(() => null)])
      .then(([r, pref]) => {
        if (cancelled) return;
        // `fallback` (or an empty menu) → null, so NavRail renders the built-in SURFACES —
        // still minus `hidden`, still with `pinned` above (both apply to every tier).
        setItems(r.source === "fallback" || r.items.length === 0 ? null : r.items);
        setHidden(r.hidden ?? []);
        setPinned(r.pinned ?? []);
        setUsingBuiltin(pref?.active === BUILTIN_PICK);
      })
      .catch(() => {
        // A deny / transport error → fall back to SURFACES (never a blank rail, never an error rail).
        if (!cancelled) {
          setItems(null);
          setHidden([]);
          setPinned([]);
          setUsingBuiltin(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // no-lockout: force / un-force the built-in sidebar via the member-owned pick, then re-resolve.
  const showAllPages = useCallback(() => {
    void setNavPref(BUILTIN_PICK)
      .then(() => reload())
      .catch(() => {
        /* a deny/transport error leaves the rail as-is */
      });
  }, [reload]);
  const useMyMenu = useCallback(() => {
    void setNavPref("")
      .then(() => reload())
      .catch(() => {
        /* a deny/transport error leaves the rail as-is */
      });
  }, [reload]);

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

  return { items, hidden, pinned, togglePin, usingBuiltin, showAllPages, useMyMenu };
}
