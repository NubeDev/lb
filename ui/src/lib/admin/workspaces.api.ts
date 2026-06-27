// The workspace-lifecycle admin api client — one call per export, mirroring the host `workspaces`
// service verbs and the gateway `/admin/workspaces` routes 1:1 (admin-crud scope). Rename un-archives;
// archive is the reversible soft-delete; purge is the HARD delete and needs the typed confirm token
// (== the ws id) AND the `workspace.purge` cap server-side (defense in depth — the UI also gates).

import { invoke } from "@/lib/ipc/invoke";

/** Rename workspace `ws` to `name` (also un-archives a soft-deleted ws). Mirrors `workspace.rename`. */
export function renameWorkspace(ws: string, name: string): Promise<void> {
  return invoke<void>("workspace_rename", { ws, name });
}

/** Archive (soft-delete) workspace `ws` — hidden from the list, reversible by rename.
 *  Mirrors `workspace.delete`. */
export function archiveWorkspace(ws: string): Promise<void> {
  return invoke<void>("workspace_archive", { ws });
}

/** Hard-delete (purge) workspace `ws`. `confirm` MUST equal `ws` — the backend rejects otherwise and
 *  also requires the `workspace.purge` cap. Irreversible (tombstoned). Mirrors `workspace.purge`. */
export function purgeWorkspace(ws: string, confirm: string): Promise<void> {
  return invoke<void>("workspace_purge", { ws, confirm });
}
