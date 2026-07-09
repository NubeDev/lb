// The branding boot cache — the localStorage-backed first-paint brand, keyed by workspace. This is
// the flash-elimination layer that mirrors `lib/theme/theme-storage.ts`: the resolved brand is
// authoritative (read through `prefs.resolve` in `branding-prefs.ts`); this cache ONLY paints the
// chrome (document.title + favicon, via the inline boot script in `index.html`) and seeds the
// `BrandingProvider` initial state so the sidebar header paints the real brand before the first
// resolve round-trip lands.
//
// Workspace-keyed (`lb.brand.<ws>`) because branding is workspace identity, not a per-member
// preference — a user switching workspaces switches brand. The `<ws>` is parsed from the URL hash
// (`/t/<ws>/…`) by the boot script; the provider re-resolves + corrects on mount regardless, so a
// stale or mismatched cache key degrades to the neutral default (never the product name).
//
// A best-effort store: localStorage can be unavailable (private mode, locked-down webviews) or over
// quota (brand images can be sizable data-URIs). Both reads and writes swallow those errors — a
// cache miss simply falls through to the neutral default + a live resolve.

import { normalizeBranding, type Branding } from "./branding-options";

/** The localStorage key prefix; the workspace id is appended (`lb.brand.acme`). Mirrors the `lb.*`
 *  namespace the theme (`lb.theme`) established. */
const BRAND_STORAGE_PREFIX = "lb.brand.";

interface BrandStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

/** Read the cached brand for `workspace`, normalized (garbage never partially applies). `null` when
 *  no cache exists (first-ever visit, or a different workspace) — the caller falls through to the
 *  neutral `DEFAULT_BRANDING` and lets the live resolve land. */
export function loadCachedBrand(
  workspace: string,
  storage: BrandStorage | undefined = globalThis.localStorage,
): Branding | null {
  if (!storage || !workspace) return null;
  try {
    const raw = storage.getItem(BRAND_STORAGE_PREFIX + workspace);
    if (!raw) return null;
    return normalizeBranding(JSON.parse(raw));
  } catch {
    return null;
  }
}

/** Persist `brand` as the cached brand for `workspace` (the resolved brand on a successful resolve).
 *  Best-effort: a quota error (large image data-URIs) or unavailable storage is silently dropped —
 *  the next boot simply re-resolves and the chrome paints the neutral default for one round-trip. */
export function saveCachedBrand(
  workspace: string,
  brand: Branding,
  storage: BrandStorage | undefined = globalThis.localStorage,
): void {
  if (!storage || !workspace) return;
  try {
    storage.setItem(BRAND_STORAGE_PREFIX + workspace, JSON.stringify(brand));
  } catch {
    // Quota exceeded (large image data-URIs) or storage disabled — leave the previous cache as-is.
  }
}

/** Drop the cached brand for `workspace` (the admin cleared the brand, or a stale entry must go). */
export function clearCachedBrand(
  workspace: string,
  storage: BrandStorage | undefined = globalThis.localStorage,
): void {
  if (!storage || !workspace) return;
  try {
    storage.removeItem(BRAND_STORAGE_PREFIX + workspace);
  } catch {
    // Storage disabled — nothing to clear.
  }
}
