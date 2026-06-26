// The registry hook — data + state for the catalog + install/rollback (FILE-LAYOUT: one hook per
// file, data separated from markup). It drives the capability-checked node verbs: list the catalog,
// then install a version (or roll back to a prior one) — which the node refuses if the artifact does
// not verify (the signature gate) or the caller lacks the grant (the capability gate). Both refusals
// are surfaced to the user, never a silent no-op.

import { useCallback, useEffect, useState } from "react";

import { listCatalog, installExtension } from "@/lib/registry/registry.api";
import type { CatalogEntry } from "@/lib/registry/registry.types";

export interface RegistryState {
  /** The catalog entries (versions) available for the extension. */
  entries: CatalogEntry[];
  /** The currently-installed version (after an install/rollback), or null. */
  installedVersion: string | null;
  /** Set when an install was refused because the artifact failed verification (the signature gate). */
  unverified: boolean;
  /** Set when the node denied a verb (missing capability) — shown to the user. */
  error: string | null;
  /** Install (or roll back to) a specific version. */
  install: (version: string) => Promise<void>;
}

/** Drive the registry catalog + install/rollback in `(ws)` for `extId` as `author` holding `caps`
 *  (the demo session identity + grant until real login lands — see registry.api). */
export function useRegistry(
  ws: string,
  extId: string,
  author: string,
  caps: string[],
): RegistryState {
  const [entries, setEntries] = useState<CatalogEntry[]>([]);
  const [installedVersion, setInstalledVersion] = useState<string | null>(null);
  const [unverified, setUnverified] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listCatalog(ws, extId, { author, caps })
      .then((e) => {
        setEntries(e);
        setError(null);
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, [ws, extId, author, caps]);

  const install = useCallback(
    async (version: string) => {
      try {
        const result = await installExtension(ws, extId, version, { author, caps });
        setUnverified(!result.verified);
        setError(null);
        if (result.verified) setInstalledVersion(result.version);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [ws, extId, author, caps],
  );

  return { entries, installedVersion, unverified, error, install };
}
