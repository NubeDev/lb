// The report API client — one call per verb, mirroring the gateway's `report.*` routes and the host
// verbs 1:1 (reports scope). The UI never calls `invoke` directly; it goes through these named verbs
// (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the workspace + owner come from
// the session token (the hard wall, §7), never an argument. Sharing a report shares its DEFINITION
// only — every embedded panel's data re-checks per call under the viewer's caps at render.
//
// `exportReport` is the odd one out: it returns bytes, not JSON. Track C adds a `postBytes`/blob path
// for the `report_export` case (binary response + a snapshot payload don't fit the JSON MCP envelope);
// we call `invoke<Blob>` and hand back the raw PDF Blob for the caller to download.

import type { Block, Report, ReportSummary, ReportSnapshots, Visibility } from "./report.types";
import { invoke } from "@/lib/ipc/invoke";

/** The roster the caller can reach (own + team-shared + workspace), summaries only. Mirrors
 *  `report.list`. */
export function listReports(): Promise<ReportSummary[]> {
  return invoke<{ reports: ReportSummary[] }>("report_list", {}).then((r) => r.reports);
}

/** Read one report (the three-gate read; blocks hydrated — `panel:{id}` refs expanded host-side).
 *  Mirrors `report.get`. */
export function getReport(id: string): Promise<Report> {
  return invoke<Report>("report_get", { id });
}

/** Create or update a report (idempotent UPSERT on `id`; owner-only update; whole-record LWW + undo).
 *  Mirrors `report.save`. */
export function saveReport(
  id: string,
  title: string,
  blocks: Block[],
  brandId: string,
  toolbar: unknown,
): Promise<Report> {
  return invoke<Report>("report_save", { id, title, blocks, brandId, toolbar });
}

/** Soft-delete a report (idempotent tombstone; owner-only). Mirrors `report.delete`. */
export function deleteReport(id: string): Promise<void> {
  return invoke<void>("report_delete", { id });
}

/** Set a report's visibility / write the S4 share edge (owner-only). Mirrors `report.share`. */
export function shareReport(id: string, visibility: Visibility, team?: string): Promise<Report> {
  return invoke<Report>("report_share", { id, visibility, team });
}

/** Export a report to a branded PDF. The browser captures each panel block to a PNG under the
 *  exporter's caps and sends the snapshots in the request; the node composes markdown + images +
 *  snapshots into the PDF. Returns the raw PDF Blob for download. Mirrors `report.export` (the
 *  `POST /reports/{id}/export.pdf` bytes route). */
export function exportReport(id: string, snapshots: ReportSnapshots): Promise<Blob> {
  return invoke<Blob>("report_export", { id, snapshots });
}
