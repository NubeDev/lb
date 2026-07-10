// The brand-profile API client — one call per verb, mirroring the gateway's `brand.*` routes and the
// host verbs 1:1 (reports scope). The UI never calls `invoke` directly (FILE-LAYOUT frontend rules).
// Each is capability-gated server-side; the workspace + owner come from the session token (§7), never
// an argument. The seeded default is never empty, so `listBrands` always returns at least one profile.

import type { Brand } from "./brand.types";
import { invoke } from "@/lib/ipc/invoke";

/** Every brand profile in the workspace (BrandPicker options). Mirrors `brand.list`. */
export function listBrands(): Promise<Brand[]> {
  return invoke<{ brands: Brand[] }>("brand_list", {}).then((r) => r.brands);
}

/** Read one brand profile. Mirrors `brand.get`. */
export function getBrand(id: string): Promise<Brand> {
  return invoke<Brand>("brand_get", { id });
}

/** Create or update a brand profile (idempotent UPSERT on `id`; owner-only update). Mirrors
 *  `brand.save`. */
export function saveBrand(brand: Brand): Promise<Brand> {
  return invoke<Brand>("brand_save", {
    id: brand.id,
    name: brand.name,
    logoAssetId: brand.logoAssetId,
    colors: brand.colors,
    fonts: brand.fonts,
    headerText: brand.headerText,
    footerText: brand.footerText,
  });
}

/** Soft-delete a brand profile (idempotent tombstone; owner-only). Mirrors `brand.delete`. */
export function deleteBrand(id: string): Promise<void> {
  return invoke<void>("brand_delete", { id });
}
