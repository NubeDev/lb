// The catalog hook — wraps `loadCatalog` in React state, re-keyed on `ws`. Mirrors `useSourcePicker`
// (the picker's loader hook): same ref-not-dep pattern so an UNMEMOIZED `loaders` object (a fresh
// literal per render, the easy host mistake) does NOT loop. Returns the per-section `SectionState`
// record; the explorer skin renders it; the picker projects it into a flat entries array.
//
// PER-SECTION HONEST TRI-STATE (system-catalog scope): each section resolves independently as its
// loader resolves — a fast `listSeries` shows its rows while a slow `readSchema` is still loading
// its skeleton, and a denied `datasource.list` shows "Not permitted." without waiting on the rest.
// This is the contract the explorer surfaces visibly; the picker collapses it into empty groups.

import { useEffect, useRef, useState } from "react";

import { loadCatalog, type CatalogSections } from "./loadCatalog";
import type { SourceLoaders } from "./types";

/** Initial state: every section starts `loading`. The host's loaders decide which sections exist —
 *  an absent loader resolves to an absent (undefined) field on first load. */
const EMPTY: CatalogSections = {};

/** Load + surface the catalog. `loaders` is the host's read seam; `ws` keys the re-load (the
 *  workspace switch). The effect keys on `ws` ONLY and reads `loaders` through a ref kept current
 *  every render — so a fresh loaders literal per render does NOT loop (same discipline as
 *  `useSourcePicker`). A host that swaps to a genuinely different transport should also change `ws`. */
export function useCatalog(loaders: SourceLoaders, ws: string): CatalogSections {
  const [sections, setSections] = useState<CatalogSections>(EMPTY);
  const loadersRef = useRef(loaders);
  loadersRef.current = loaders;

  useEffect(() => {
    const l = loadersRef.current;
    let cancelled = false;
    setSections(EMPTY);
    // Each loader resolves independently; we surface each as it lands so a fast section's rows show
    // while a slow section's skeleton is still loading (the per-section honest tri-state). The
    // `publish` callback no-ops once `cancelled` is true, so a ws switch drops late-arriving
    // sections from the previous ws.
    void loadCatalog(l, (merge) => {
      if (cancelled) return;
      setSections((current) => merge(current));
    }).catch(() => {
      // An unexpected orchestration fault (not a per-section deny — that's caught inside). Swallow
      // so a host's late-arriving reject after unmount doesn't crash render. The section record
      // stays at whatever partial state landed.
    });
    return () => {
      cancelled = true;
    };
    // Keyed on `ws` ONLY — `loaders` is read via a ref (see doc above), so it isn't a dep.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ws]);

  return sections;
}

