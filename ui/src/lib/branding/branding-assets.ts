// File → data-URI helpers for the brand image uploads (workspace-branding scope). The three image
// slots (logo/icon/favicon) are embedded as data-URIs DIRECTLY in the `ui_branding` blob, so an
// upload is "read a File, validate it, hand back a data-URI string the editor splices into the
// patch." No `assets.*` round-trip — keeps the brand atomic on one prefs read AND sidesteps the S4
// membership gate that would block non-admin members from reading an admin-owned asset record.
//
// One responsibility: turn a File into a validated data-URI (or reject with a clear error).

import { BRAND_IMAGE_MIMES, MAX_BRAND_IMAGE_BYTES } from "./branding-options";

/** Read `file` as a data-URI string, after size + mime validation. Rejects:
 *   - file size > {@link MAX_BRAND_IMAGE_BYTES} ("image too large (max N KiB)"),
 *   - mime not in {@link BRAND_IMAGE_MIMES} ("unsupported image type").
 *  Returns the data-URI string (`data:image/<mime>;base64,...`). */
export function readBrandImage(file: File): Promise<string> {
  if (file.size > MAX_BRAND_IMAGE_BYTES) {
    const kb = Math.round(MAX_BRAND_IMAGE_BYTES / 1024);
    return Promise.reject(new Error(`image too large (max ${kb} KiB)`));
  }
  // The browser may hand back `image/svg+xml` for an SVG, or empty for a renamed file. Allow a
  // missing mime only when the extension is recognized (fallback below); else reject.
  const mimeOk = BRAND_IMAGE_MIMES.includes(file.type as (typeof BRAND_IMAGE_MIMES)[number]);
  const extOk = hasBrandImageExtension(file.name);
  if (!mimeOk && !extOk) {
    return Promise.reject(new Error(`unsupported image type: ${file.type || "unknown"}`));
  }
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("read failed"));
    reader.onload = () => {
      const result = reader.result;
      if (typeof result === "string" && result.startsWith("data:image/")) resolve(result);
      else reject(new Error("read returned no data-URI"));
    };
    reader.readAsDataURL(file);
  });
}

/** The accept= attribute value for the brand image inputs. */
export const BRAND_IMAGE_ACCEPT = [...BRAND_IMAGE_MIMES].join(",");

function hasBrandImageExtension(name: string): boolean {
  const ext = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
  return ["png", "jpg", "jpeg", "webp", "svg", "gif", "ico"].includes(ext);
}
