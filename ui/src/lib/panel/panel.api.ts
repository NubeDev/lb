// The library-panel API client — one call per export, mirroring the gateway's `panel.*` routes and the
// host verbs 1:1 (library-panels scope). The UI never calls `invoke` directly; it goes through these
// named verbs (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the workspace + owner
// come from the session token (the hard wall, §7), never an argument. Sharing a panel shares its
// DEFINITION only — the data it reads is re-checked per call under the viewer's caps at render.

import type { Panel, PanelSpec, PanelSummary, PanelUsageRow, Visibility } from "./panel.types";
import { invoke } from "@/lib/ipc/invoke";

/** The roster the caller can reach (own + team-shared + workspace), summaries only. Mirrors
 *  `panel.list`. */
export function listPanels(): Promise<PanelSummary[]> {
  return invoke<{ panels: PanelSummary[] }>("panel_list", {}).then((r) => r.panels);
}

/** Read one panel (the three-gate read, full spec). Mirrors `panel.get`. */
export function getPanel(id: string): Promise<Panel> {
  return invoke<Panel>("panel_get", { id });
}

/** Create or update a panel (idempotent UPSERT on `id`; owner-only update). Mirrors `panel.save`. */
export function savePanel(id: string, title: string, spec: PanelSpec): Promise<Panel> {
  return invoke<Panel>("panel_save", { id, title, spec });
}

/** Soft-delete a panel (idempotent tombstone; owner-only). Refused while referenced unless `force`.
 *  Mirrors `panel.delete`. */
export function deletePanel(id: string, force = false): Promise<void> {
  return invoke<void>("panel_delete", { id, force });
}

/** Set a panel's visibility / write the S4 share edge (owner-only). Mirrors `panel.share`. */
export function sharePanel(id: string, visibility: Visibility, team?: string): Promise<Panel> {
  return invoke<Panel>("panel_share", { id, visibility, team });
}

/** The dashboards referencing a panel (delete-safety + the "used on N dashboards" banner). Mirrors
 *  `panel.usage`. */
export function panelUsage(id: string): Promise<PanelUsageRow[]> {
  return invoke<{ usage: PanelUsageRow[] }>("panel_usage", { id }).then((r) => r.usage);
}
