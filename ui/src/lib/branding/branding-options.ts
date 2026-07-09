// The branding shape + validation — the single source of truth for what a workspace's brand *is*
// (workspace-branding scope). Mirrors `lib/theme/theme-options.ts`: a typed shape the shell parses
// an opaque prefs blob (`ui_branding`) into, with fail-closed normalization.
//
// Branding is **admin-owned workspace identity** (NOT a per-member preference): every member of a
// workspace resolves the same brand through `prefs.resolve`. The shell falls back to the compiled
// `DEFAULT_BRANDING` when no workspace default is set AND no boot cache has landed yet.
//
// The default is deliberately **neutral** (empty strings): the platform's own name must NEVER show
// in a workspace's chrome. The resolved brand is painted pre-flash from the localStorage boot cache
// (`branding-cache.ts`, mirroring the theme discipline); the neutral default is only the brief
// first-ever-visit state before the first `prefs.resolve` lands. Editor input placeholders use
// `BRANDING_PLACEHOLDERS` (generic examples), NOT the default, so the editor keeps its hint without
// the product name appearing anywhere.
//
// Image marks (logo/favicon/icon) are embedded as data-URIs DIRECTLY in the blob rather than as
// separate `assets.*` records — this keeps the brand atomic on a single prefs read AND sidesteps
// the S4 membership gate (gate 3) that would otherwise block non-admin members from reading an
// admin-owned asset. Brand images are small (capped at MAX_BRAND_IMAGE_BYTES); a future bucket-
// backed pipeline is the deferred slice for larger marks. See `branding-assets.ts` for the
// File→data-URI helper + size/mime validation.

/** The shell's compiled default brand — NEUTRAL (no product name). Shown only for the brief
 *  first-ever-visit window before the boot cache / first `prefs.resolve` lands. A workspace that
 *  has set no brand stays neutral (empty) until an admin sets one in Settings → Branding. */
export const DEFAULT_BRANDING = Object.freeze({
  siteName: "",
  siteAbbr: "",
  tagline: "",
});

/** Generic example placeholders for the admin editor's text inputs — NOT a brand, NOT shown in
 *  chrome. Keeps the editor's hint ("what goes here?") without the product name appearing anywhere
 *  in the shell or its fallbacks. Used by `BrandingTab` for its `<input placeholder=…>`. */
export const BRANDING_PLACEHOLDERS = Object.freeze({
  siteName: "Workspace name",
  siteAbbr: "AB",
  tagline: "a short subtitle",
  loginHeading: "Sign in",
  loginSubheading: "Choose an identity and workspace boundary for this session.",
});

/** The accepted image MIME types for the three brand image slots. SVG is allowed for the logo +
 *  icon (crisp at any rail size); favicons add ICO (the browser-tab canonical) + PNG. */
export const BRAND_IMAGE_MIMES = [
  "image/png",
  "image/jpeg",
  "image/webp",
  "image/svg+xml",
  "image/gif",
  "image/x-icon",
  "image/vnd.microsoft.icon",
] as const;

/** The v1 per-image size ceiling (256 KiB). Stated explicitly per the scope's "state the bound
 *  explicitly" risk. Brand images are small (a logo is typically <50 KiB); a larger payload is
 *  rejected with a clear error so the prefs blob stays bounded. */
export const MAX_BRAND_IMAGE_BYTES = 256 * 1024;

/** A fully-resolved workspace brand. Strings only here — the optional image slots carry a ready-
 *  to-render data-URI (`data:image/<mime>;base64,...`) when an admin uploaded one, `undefined`
 *  when none is set (the shell falls back to the text `siteAbbr` tile). */
export interface Branding {
  /** Full workspace name — the sidebar header + `document.title` (e.g. "Acme"). */
  siteName: string;
  /** Short mark text shown in the tile when no `iconDataUri`/`logoDataUri` is set (e.g. "AC"). */
  siteAbbr: string;
  /** Subtitle under the name (e.g. "workspace ops"). Empty string hides the line. */
  tagline: string;
  /** Optional login-page heading (e.g. "Sign in to Acme"). Falls back to "Sign in" on the login
   *  screen when unset. */
  loginHeading?: string;
  /** Optional login-page sub-heading — the line under the heading (e.g. "Choose an identity and
   *  workspace boundary for this session."). Empty/unset shows the compiled default sub-line. */
  loginSubheading?: string;
  /** Optional login-page logo image data-URI. Rendered in the login card header in place of the
   *  generic sign-in glyph when set. */
  loginLogoDataUri?: string;
  /** Optional full-logo image data-URI (e.g. the "Acme" wordmark). Replaces the tile+name pair in
   *  the sidebar header when present. */
  logoDataUri?: string;
  /** Optional mark image data-URI (e.g. the Google "G" — the small sigil). Replaces the `siteAbbr`
   *  text in the tile when present. */
  iconDataUri?: string;
  /** Optional browser-tab favicon data-URI. The shell writes it to `<link rel="icon">`. */
  faviconDataUri?: string;
}

/** The wire shape — an arbitrary, partially-set brand blob from prefs. */
export type BrandingPref = Partial<Branding>;

/** True when `s` is a non-empty string. */
function isNonEmptyStr(s: unknown): s is string {
  return typeof s === "string" && s.length > 0;
}

/** True when `s` looks like an image data-URI (`data:image/...;base64,...`). Conservative — only
 *  the prefix is checked; the bytes are the admin's responsibility (admin-uploaded, workspace-
 *  scoped, never third-party input). */
function isImageDataUri(s: unknown): s is string {
  if (typeof s !== "string" || !s.startsWith("data:image/")) return false;
  return s.includes(";base64,");
}

/** Validate an unknown prefs blob into a well-formed `Branding`. Each axis falls back to the
 *  compiled default per-axis (fail-closed per field, never whole-blob) so a malformed value in one
 *  field doesn't drop the rest of the brand. Non-object input is `DEFAULT_BRANDING` entirely. */
export function normalizeBranding(value: unknown): Branding {
  const out: Branding = {
    siteName: DEFAULT_BRANDING.siteName,
    siteAbbr: DEFAULT_BRANDING.siteAbbr,
    tagline: DEFAULT_BRANDING.tagline,
  };
  if (!value || typeof value !== "object") return out;
  const c = value as Record<string, unknown>;
  if (isNonEmptyStr(c.siteName)) out.siteName = c.siteName.slice(0, 80);
  if (isNonEmptyStr(c.siteAbbr)) out.siteAbbr = c.siteAbbr.slice(0, 4);
  // tagline + loginHeading/loginSubheading may legitimately be the empty string (hide the line), so accept any string.
  if (typeof c.tagline === "string") out.tagline = c.tagline.slice(0, 120);
  if (typeof c.loginHeading === "string" && c.loginHeading.length > 0) out.loginHeading = c.loginHeading.slice(0, 120);
  if (typeof c.loginSubheading === "string" && c.loginSubheading.length > 0)
    out.loginSubheading = c.loginSubheading.slice(0, 200);
  if (isImageDataUri(c.logoDataUri)) out.logoDataUri = c.logoDataUri;
  if (isImageDataUri(c.iconDataUri)) out.iconDataUri = c.iconDataUri;
  if (isImageDataUri(c.faviconDataUri)) out.faviconDataUri = c.faviconDataUri;
  if (isImageDataUri(c.loginLogoDataUri)) out.loginLogoDataUri = c.loginLogoDataUri;
  return out;
}
