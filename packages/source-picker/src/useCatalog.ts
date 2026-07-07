// The catalog hook — wraps the per-section loaders in React state, re-keyed on `ws`. LAZY per
// section: on mount (and on ws change) every section the host WIRED starts as `idle` (collapsed, no
// loader fired); the explorer fires `loadSection(kind)` the first time a user expands a section.
// Re-expand keeps the cached data (no refire). This is the user's explicit contract: "do the API
// call once I open/close the tree" — page load does NOT fan out every `*.list` verb.
//
// PER-SECTION HONEST TRI-STATE (system-catalog scope): each section resolves independently as its
// loader resolves — a fast `listSeries` shows its rows while a slow `readSchema` is still loading
// its skeleton, and a denied `datasource.list` shows "Not permitted." without waiting on the rest.
// This is the contract the explorer surfaces visibly; the picker collapses it into empty groups.
//
// The hook keys on `ws` ONLY and reads `loaders` through a ref kept current every render — so an
// UNMEMOIZED `loaders` object (a fresh literal per render, the easy host mistake) does NOT loop
// (same discipline as `useSourcePicker`).

import { useCallback, useEffect, useRef, useState } from "react";

import { runSectionLoader, type CatalogSections } from "./loadCatalog";
import type { CatalogSectionKind } from "./catalog";
import type { SourceLoaders } from "./types";

/** The set of sections the host actually wired (loader present). Computed once per mount to seed the
 *  `idle` record — sections with no loader stay `undefined` (absent), as in the eager path. */
function wiredKinds(loaders: SourceLoaders): CatalogSectionKind[] {
  const out: CatalogSectionKind[] = [];
  if (loaders.listDatasources) out.push("datasources");
  if (loaders.readSchema) out.push("schema");
  if (loaders.listSeries) out.push("series");
  if (loaders.listChannels) out.push("channels");
  if (loaders.listInsights) out.push("insights");
  if (loaders.listInbox) out.push("inbox");
  if (loaders.listQueries) out.push("queries");
  if (loaders.listExtensions) out.push("extensions");
  if (loaders.listRules) out.push("rules");
  if (loaders.listFlows) out.push("flowSummaries");
  if (loaders.listFlowNodes) out.push("flowDescriptors");
  return out;
}

/** Build the initial idle record for `loaders` — every wired section starts as `{status:"idle"}`;
 *  absent loaders yield absent (undefined) fields. */
function idleRecord(loaders: SourceLoaders): CatalogSections {
  const initial: CatalogSections = {};
  for (const kind of wiredKinds(loaders)) {
    (initial as Record<string, never>)[kind] = { status: "idle" } as never;
  }
  return initial;
}

/** The lazy catalog — the per-section state record plus the `loadSection(kind)` action the explorer
 *  fires on first expand. The host reads `sections` for rendering; passes `loadSection` to the
 *  `<CatalogExplorer>` so its section headers can trigger their own loads. */
export interface UseCatalogResult {
  sections: CatalogSections;
  /** Fire one section's loader (deny-tolerant; absent loader ⇒ the section stays `undefined`).
   *  Idempotent — calling it again on an already-loaded section is a no-op (the cached state persists). */
  loadSection: (kind: CatalogSectionKind) => void;
}

/** Lazy catalog. `loaders` is the host's read seam; `ws` keys the re-init (the workspace switch). The
 *  initial idle record is computed once per `loaders` reference via `useState`'s lazy initializer —
 *  every wired section starts `idle` on FIRST render (no useEffect timing gap). The `ws` effect resets
 *  the record on workspace switch (the user re-opens each section to re-fetch under the new ws). */
export function useCatalog(loaders: SourceLoaders, ws: string): UseCatalogResult {
  const [sections, setSections] = useState<CatalogSections>(() => idleRecord(loaders));
  const loadersRef = useRef(loaders);
  loadersRef.current = loaders;

  // On ws switch, reset every wired section to `idle` — the prior data is dropped (different workspace
  // = different data). Re-reading `loadersRef.current` (not the dep) so a ws switch to a host that
  // also swapped loaders picks up the new section set.
  useEffect(() => {
    setSections(idleRecord(loadersRef.current));
  }, [ws]);

  const loadSection = useCallback((kind: CatalogSectionKind) => {
    setSections((current) => {
      const existing = current[kind];
      // Idempotent: no-op if already loaded (loading/ready/denied) — the cached state persists.
      if (existing && existing.status !== "idle") return current;
      // Mark loading and fire the loader. The publish merges the resolved state.
      const next = { ...current, [kind]: { status: "loading" } } as CatalogSections;
      void runSectionLoader(loadersRef.current, kind).then((state) => {
        if (!state) return; // absent loader ⇒ absent section — leave as loading (shouldn't happen).
        setSections((cur) => ({ ...cur, [kind]: state }) as CatalogSections);
      });
      return next;
    });
  }, []);

  return { sections, loadSection };
}
