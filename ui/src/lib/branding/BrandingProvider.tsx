// The branding provider — mounts once in the shell (beside `ThemeProvider`), resolves the
// workspace brand once and on workspace changes, applies it to the DOM (title + favicon), and
// exposes it through `BrandingContext` so `NavRail` and other chrome can read the resolved name +
// logo. One responsibility: resolve + provide the workspace brand.
//
// Mirror of `ThemeProvider`: same provider pattern, same "real prefs.resolve over the real
// gateway" discipline, no localStorage cache (branding is workspace identity, not a per-member
// roaming preference — the resolve IS the cache).

import { useEffect, useMemo, useState } from "react";

import { DEFAULT_BRANDING, type Branding } from "./branding-options";
import { applyBranding } from "./branding-dom";
import { readResolvedBranding } from "./branding-prefs";
import {
  BrandingContext,
  type BrandingContextValue,
  DEFAULT_BRANDING_CONTEXT,
} from "./branding-context";

interface Props {
  children: React.ReactNode;
  /** The current workspace id. Re-resolves on change (the brand is workspace-scoped, so switching
   *  workspaces must re-read the brand of the new workspace). */
  workspace: string;
}

export function BrandingProvider({ children, workspace }: Props) {
  const [brand, setBrand] = useState<Branding>({ ...DEFAULT_BRANDING });
  const [loading, setLoading] = useState(true);

  // Re-resolve when the workspace changes (workspace switch = a new brand). On a successful read,
  // swap the brand + apply it to the DOM. On failure, keep the compiled default (the shell still
  // renders coherently — branded with the Lazybones fallback).
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    void readResolvedBranding()
      .then((resolved) => {
        if (cancelled) return;
        setBrand(resolved);
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

  // Pre-resolve window: keep the default-brand context (no flash). Once the first resolve lands,
  // `loading=false` and the real brand propagates.
  const ctx = loading ? DEFAULT_BRANDING_CONTEXT : value;
  return <BrandingContext.Provider value={ctx}>{children}</BrandingContext.Provider>;
}
