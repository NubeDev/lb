// Apply a resolved `Branding` to the document — the ONE place the brand touches the DOM. Mirrors
// `lib/theme/theme-dom.ts`'s discipline. Sets:
//   - `document.title` (the workspace name, e.g. "Acme"),
//   - the `<link rel="icon">` href (the favicon data-URI), creating the link element if absent.
//
// The sidebar logo/name render is component-side in `NavRail`, not here — only the document-level
// chrome (title + favicon) lives in this DOM-apply file. One responsibility: branding → DOM writes.

import { DEFAULT_BRANDING, type Branding } from "./branding-options";

/** Apply `brand` to the document root: title from the site name, favicon from the data-URI. A
 *  brand without a favicon data-URI leaves any existing favicon link in place (the browser falls
 *  back to /favicon.ico); a brand without a site name is impossible (normalizeBranding fills it). */
export function applyBranding(doc: Document, brand: Branding): void {
  // Title — the workspace name. Append nothing else (the rail/header carry the brand separately).
  if (brand.siteName) doc.title = brand.siteName;
  else doc.title = DEFAULT_BRANDING.siteName;

  // Favicon — write/replace the `<link rel="icon">`. Skip when no data-URI is set (leave whatever
  // the page already has — typically /favicon.ico served by the gateway).
  if (brand.faviconDataUri) {
    let link = doc.querySelector<HTMLLinkElement>("link[rel='icon']");
    if (!link) {
      link = doc.createElement("link");
      link.rel = "icon";
      doc.head.appendChild(link);
    }
    // The data-URI carries its own mime (`data:image/x-icon;base64,...`), so no `type` attribute.
    link.href = brand.faviconDataUri;
  }
}
