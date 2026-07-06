// The branding ⇄ prefs bridge — persist and read the workspace's `Branding` through the shipped
// `prefs` verbs, riding the `ui_branding` axis on the prefs record (workspace-branding scope). Two
// operations, mapping 1:1 to prefs verbs:
//   - readResolvedBranding() → `prefs.resolve` (member → workspace-default → built-in fold; the
//     workspace-default link carries the brand in practice, since branding is admin-owned).
//   - persistWorkspaceDefaultBranding(brand) → `prefs.set_default` (admin-gated).
//
// Sibling of `lib/theme/theme-prefs.ts`; same shape, one new axis. No per-member write — branding
// is workspace identity, so there is no `persistBranding` (member) sibling of `persistTheme`.

import { getPrefs } from "@/lib/prefs/get";
import { resolvePrefs } from "@/lib/prefs/resolve";
import { setDefaultPrefs } from "@/lib/prefs/set";
import { normalizeBranding, type Branding } from "./branding-options";

/** Resolve the viewer's workspace brand from the prefs chain, normalized. Returns the compiled
 *  `DEFAULT_BRANDING` when no workspace default is set (never null — the shell always has a brand). */
export async function readResolvedBranding(): Promise<Branding> {
  const resolved = await resolvePrefs();
  return normalizeBranding(resolved.ui_branding);
}

/** Read ONLY the workspace-default brand (not the folded chain) — the right shape for the admin
 *  editor to know exactly what the workspace record carries. Null when the workspace has no brand
 *  default; the editor seeds from `DEFAULT_BRANDING` in that case. */
export async function readWorkspaceDefaultBranding(): Promise<Branding | null> {
  // `prefs.set_default` has no `get_default` verb on the gateway; the admin editor reads the
  // folded `prefs.resolve` (which includes the ws-default link) and edits that. For the rare case
  // of a member-local preview, the own getPrefs() would carry the brand too — but branding is
  // admin-only, so resolve() is the source of truth.
  const resolved = await resolvePrefs();
  if (resolved.ui_branding == null) return null;
  return normalizeBranding(resolved.ui_branding);
}

/** For the admin editor's "explicitly set" indicator: read the viewer's OWN record (not the chain)
 *  so the editor can show "no brand set" vs the resolved fallback. Unused today but mirrors
 *  `theme-prefs.readOwnTheme`'s role for the theme editor. */
export async function readOwnBranding(): Promise<Branding | null> {
  const own = await getPrefs();
  if (!own || own.ui_branding == null) return null;
  return normalizeBranding(own.ui_branding);
}

/** Persist `brand` as the WORKSPACE-default brand. Admin-gated `mcp:prefs.set_default:call`; a
 *  non-admin is denied opaquely at the host (the editor hides for non-admins via `hasCap`). */
export function persistWorkspaceDefaultBranding(brand: Branding): Promise<void> {
  return setDefaultPrefs({ ui_branding: brand });
}
