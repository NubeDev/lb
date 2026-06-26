// The registry API client — one call per export, mirroring the Rust `registry.*` verbs and the node
// command name one-to-one. The UI never calls `invoke` directly; it goes through these named verbs
// (FILE-LAYOUT frontend rules).
//
// `author`/`caps` are the caller's demo principal + grant (the real node derives them from the
// session token; the in-memory fake uses them to resolve the capability gate, so the UI's allow/deny
// paths are exercised exactly as the node would — same seam as the workflow/agent api).

import type { CatalogEntry, InstallResult } from "./registry.types";
import { invoke } from "@/lib/ipc/invoke";

/** List the catalog entries for `extId` visible to this workspace. Mirrors `registry.list`. */
export function listCatalog(
  ws: string,
  extId: string,
  opts?: { author?: string; caps?: string[] },
): Promise<CatalogEntry[]> {
  return invoke<CatalogEntry[]>("registry_list", {
    ws,
    extId,
    author: opts?.author,
    caps: opts?.caps,
  });
}

/** Install (or roll back to) a specific `(extId, version)` from the registry — pull · verify · cache
 *  · install. `verified: false` if the artifact was tampered/unsigned/untrusted (refused, nothing
 *  installed). Rollback is the SAME verb with a prior version. Mirrors `registry.install`. */
export function installExtension(
  ws: string,
  extId: string,
  version: string,
  opts?: { author?: string; caps?: string[] },
): Promise<InstallResult> {
  return invoke<InstallResult>("registry_install", {
    ws,
    extId,
    version,
    author: opts?.author,
    caps: opts?.caps,
  });
}
