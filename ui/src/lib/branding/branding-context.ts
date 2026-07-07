import { createContext, useContext } from "react";

import { DEFAULT_BRANDING, type Branding } from "./branding-options";

/** The brand context value. `brand` is always set (the provider seeds with `DEFAULT_BRANDING`
 *  before the first prefs resolve lands); `loading` covers the brief pre-resolve window so a
 *  consumer can choose to defer a flash. */
export interface BrandingContextValue {
  brand: Branding;
  loading: boolean;
}

/** The compiled-default brand shown for the brief pre-resolve window (no flash of "no name"). */
export const DEFAULT_BRANDING_CONTEXT: BrandingContextValue = {
  brand: { ...DEFAULT_BRANDING },
  loading: true,
};

export const BrandingContext = createContext<BrandingContextValue | null>(null);

/** Read the brand context (throws if used outside `BrandingProvider`). The shell's chrome always
 *  mounts under the provider, so this is the strict variant — the same discipline as `useTheme`. */
export function useBranding(): BrandingContextValue {
  const ctx = useContext(BrandingContext);
  if (!ctx) throw new Error("useBranding must be used within BrandingProvider.");
  return ctx;
}

/** The non-throwing variant: returns the brand context, or null outside a `BrandingProvider`.
 *  Mirrors `useThemeOptional` — for a consumer that legitimately renders outside the shell chrome
 *  (e.g. a bare-mounted test) and would rather fall back to defaults than crash. */
export function useBrandingOptional(): BrandingContextValue | null {
  return useContext(BrandingContext);
}
