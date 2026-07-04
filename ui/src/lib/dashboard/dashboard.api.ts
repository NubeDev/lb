// The dashboard API client — one call per export, mirroring the gateway's `dashboard.*` routes and
// the host verbs 1:1 (dashboard scope). The UI never calls `invoke` directly; it goes through these
// named verbs (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the workspace +
// owner come from the session token (the hard wall, §7), never an argument.

import type { Cell, Dashboard, DashboardSummary, Variable, Visibility } from "./dashboard.types";
import { invoke } from "@/lib/ipc/invoke";

/** The roster the caller can reach (own + team-shared + workspace), summaries only. Mirrors
 *  `dashboard.list`. */
export function listDashboards(): Promise<DashboardSummary[]> {
  return invoke<{ dashboards: DashboardSummary[] }>("dashboard_list", {}).then((r) => r.dashboards);
}

/** Read one dashboard (the three-gate read). Mirrors `dashboard.get`. */
export function getDashboard(id: string): Promise<Dashboard> {
  return invoke<Dashboard>("dashboard_get", { id });
}

/** Create or update a dashboard (idempotent UPSERT on `id`; owner-only update). Mirrors
 *  `dashboard.save`. `variables` is additive (widget-config-vars Slice 2) — omit it for a layout-only
 *  save. Returns the persisted record. */
export function saveDashboard(
  id: string,
  title: string,
  cells: Cell[],
  variables?: Variable[],
): Promise<Dashboard> {
  return invoke<Dashboard>("dashboard_save", { id, title, cells, variables: variables ?? [] });
}

/** Soft-delete a dashboard (idempotent tombstone; owner-only). Mirrors `dashboard.delete`. */
export function deleteDashboard(id: string): Promise<void> {
  return invoke<void>("dashboard_delete", { id });
}

/** Set a dashboard's visibility / write the S4 share edge (owner-only). Mirrors `dashboard.share`. */
export function shareDashboard(
  id: string,
  visibility: Visibility,
  team?: string,
): Promise<Dashboard> {
  return invoke<Dashboard>("dashboard_share", { id, visibility, team });
}

/** Pin a tool result-render envelope to a dashboard (widget-platform scope, Slice B). The host mints a
 *  v3 cell from the `x-lb-render` envelope (a tool's `descriptor.result`, or a channel `rich_result`
 *  body minus `kind`/`v`), upserts it (idempotent on `pin-{slug(source.tool||view)}`; owner-only on an
 *  existing dashboard), and persists via the Slice A validation chain. Generic over the tool id —
 *  `envelope.source.tool` is opaque data, never branched on. `title` is used only when creating a fresh
 *  dashboard; an existing dashboard keeps its title. Returns the updated dashboard (hydrated). */
export function pinDashboard(
  id: string,
  envelope: Record<string, unknown>,
  title?: string,
): Promise<Dashboard> {
  return invoke<Dashboard>("dashboard_pin", { id, envelope, title: title ?? "" });
}
