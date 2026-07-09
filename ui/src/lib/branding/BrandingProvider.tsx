// The branding provider — mounts once in the shell (beside `ThemeProvider`), resolves the
// workspace brand once and on workspace changes, applies it to the DOM (title + favicon), and
// exposes it through `BrandingContext` so `NavRail` and other chrome can read the resolved name +
// logo. One responsibility: resolve + provide the workspace brand.
//
// Mirror of `ThemeProvider`: same provider pattern, same "real prefs.resolve over the real
// gateway" discipline. The localStorage boot cache (`branding-cache.ts`, workspace-keyed) is the
// first-paint brand so the chrome never flashes the neutral default on a refresh — the resolved
// brand is authoritative; the cache only seeds the initial paint. Reverses the prior "no
// localStorage cache" stance (workspace-branding scope) which left a one-round-trip flash; the
// cache mirrors the shipped theme discipline and stays best-effort.

import { useEffect, useMemo, useState } from "react";

import { DEFAULT_BRANDING, type Branding } from "./branding-options";
import { loadCachedBrand, saveCachedBrand } from "./branding-cache";
import { applyBranding } from "./branding-dom";
import { readResolvedBranding } from "./branding-prefs";
import { BrandingContext, type BrandingContextValue } from "./branding-context";

interface Props {
  children: React.ReactNode;
  /** The current workspace id. Re-resolves on change (the brand is workspace-scoped, so switching
   *  workspaces must re-read the brand of the new workspace). */
  workspace: string;
}

export function BrandingProvider({ children, workspace }: Props) {
  // First paint: the workspace's cached brand (no flash on refresh/revisit); the neutral default
  // only when no cache exists (first-ever visit). `loading` stays true until the live resolve
  // confirms or corrects the cached value.
  const [brand, setBrand] = useState<Branding>(
    () => loadCachedBrand(workspace) ?? { ...DEFAULT_BRANDING },
  );
  const [loading, setLoading] = useState(true);

  // Re-resolve when the workspace changes (workspace switch = a new brand). On a successful read,
  // swap the brand, cache it for the next boot, and apply it to the DOM. On failure, keep the
  // cached/neutral value (the shell still renders coherently).
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    void readResolvedBranding()
      .then((resolved) => {
        if (cancelled) return;
        setBrand(resolved);
        saveCachedBrand(workspace, resolved);
        setLoading(false);
      })
      .catch(() => {
        if (cancelled) return;
        setBrand({ ...DEFAULT_BRANDING });
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [workspace]);

  // Apply the brand to the DOM whenever it changes (initial mount + every workspace switch). The
  // sidebar logo/name render is component-side in NavRail, NOT here — only document-level chrome.
  useEffect(() => {
    applyBranding(document, brand);
  }, [brand]);

  const value = useMemo<BrandingContextValue>(() => ({ brand, loading }), [brand, loading]);

  // `brand` is always the best-known value: the cached brand during the resolve window (no flash),
  // the resolved brand once it lands. `loading` lets a consumer that needs the *confirmed* value
  // defer (e.g. a one-shot effect) — but the chrome paints the cached brand throughout.
  return <BrandingContext.Provider value={value}>{children}</BrandingContext.Provider>;
}
