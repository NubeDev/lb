// The brand-profile wire shapes — mirror the gateway's `brand.*` routes + the host `Brand` record
// (reports scope). Branding is a STANDALONE, cross-cutting workspace resource (the report builder is
// merely its first consumer): a named profile with a logo, a four-colour palette, heading/body fonts,
// and header/footer text. Many profiles per workspace; a report stores a `brandId`. The seeded default
// is never empty (the host seeds one, deriving its initial logo/name from `ui_branding` when present),
// so the BrandPicker always has at least one option.

/** The report brand colour roles the Typst template consumes. */
export interface BrandColors {
  primary: string;
  accent: string;
  text: string;
  background: string;
}

/** The heading/body font selection. Both MUST be one of {@link EMBEDDABLE_FONTS} (lesson 4): only
 *  `typst-assets` fonts render in the PDF; unknown names silently fall back, so the brand editor is a
 *  SELECT of these, never a free-text field. */
export interface BrandFonts {
  heading: string;
  body: string;
}

/** A full brand-profile record. */
export interface Brand {
  id: string;
  name: string;
  /** The logo, as an `asset_id` into the shipped `assets.*` store OR an inline data-URI (small logos
   *  may inline, the branding-blob pattern, 256 KiB cap). Empty ⇒ no logo. */
  logoAssetId: string;
  colors: BrandColors;
  fonts: BrandFonts;
  /** Running header/footer text with `{page}`/`{title}`/`{date}` tokens (data, not a template
   *  language — the Typst layer substitutes tokens). */
  headerText: string;
  footerText: string;
  updated_ts?: number;
  deleted?: boolean;
}

/** The only fonts that render embedded in the PDF (`typst-assets`) — the brand editor's font control
 *  is a SELECT of exactly these (lesson 4). */
export const EMBEDDABLE_FONTS = [
  "Libertinus Serif",
  "DejaVu Sans Mono",
  "New Computer Modern",
] as const;

export type EmbeddableFont = (typeof EMBEDDABLE_FONTS)[number];
