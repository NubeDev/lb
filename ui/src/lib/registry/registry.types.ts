// View/DTO types for the extension-registry surface — mirror the Rust registry contract (the catalog
// entry + the install/rollback result). One name across the Rust model, the DTO, and the client
// (FILE-LAYOUT): `CatalogEntry`/`Visibility` match `lb_registry`.

/** Where a catalog entry is visible (mirrors `lb_registry::Visibility`). "public" = discoverable
 *  cross-workspace, never more privileged. */
export type Visibility = "public" | "private";

/** A catalog entry — an installable `(extId, version)` with its content digest, no bytes attached
 *  (mirrors `lb_registry::CatalogEntry`). The basis for the install/rollback version picker. */
export interface CatalogEntry {
  extId: string;
  version: string;
  /** The content digest (lowercase hex) the signature bound — shown so a user can confirm identity. */
  digestHex: string;
  publisherKeyId: string;
  visibility: Visibility;
}

/** The result of an install/rollback: the installed version, and whether the artifact verified.
 *  `verified: false` means a tampered/unsigned/untrusted artifact was refused — nothing installed
 *  (the signature gate, surfaced to the user, distinct from a capability denial). */
export interface InstallResult {
  extId: string;
  version: string;
  verified: boolean;
}
