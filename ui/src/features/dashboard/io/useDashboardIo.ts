// The dashboard import/export orchestration hook (dashboard scope, import/export UX). It bridges the
// pure {@link DashboardBundle} format to the SHIPPED store verbs — export READS the selected records
// (`dashboard.get` / `panel.get`) and builds a bundle; import REPLAYS a bundle through `dashboard.save`
// / `panel.save`. No new host surface: import IS a sequence of ordinary owner-scoped writes, each
// capability-gated + workspace-walled server-side (§5/§6). The workspace/owner are never read from the
// file — they come from the caller's token (rule 6), so a bundle exported on node A imports cleanly on
// node B under B's authority. One responsibility: the two directions + the collision-safe id policy.

import { useCallback, useState } from "react";

import {
  getDashboard,
  saveDashboard,
  listDashboards,
  dashboardToPortable,
  makeBundle,
  serializeBundle,
  uniqueId,
  type DashboardBundle,
  type PortableDashboard,
  type PortablePanel,
} from "@/lib/dashboard";
import { getPanel, savePanel, listPanels, type PanelSpec } from "@/lib/panel";
import { downloadText } from "./downloadText";
import { BUNDLE_EXT } from "@/lib/dashboard";

/** How an import resolves an id collision with an existing record. */
export type CollisionMode = "rename" | "overwrite";

export interface ImportOutcome {
  dashboards: { id: string; title: string; renamedFrom?: string }[];
  panels: { id: string; title: string; renamedFrom?: string }[];
  errors: string[];
}

export interface DashboardIo {
  busy: boolean;
  error: string | null;
  /** Export the given dashboards + standalone panels as a downloaded bundle file. Reads each full
   *  record first (the roster only carries summaries). `nowIso` stamps `exportedAt`. */
  exportBundle: (
    dashboardIds: string[],
    panelIds: string[],
    filename?: string,
    nowIso?: string,
  ) => Promise<void>;
  /** Build (but don't download) a bundle from already-loaded portable entries — used to export the
   *  dashboard currently open in the grid, or a single widget, without a round-trip read. */
  downloadBundle: (
    dashboards: PortableDashboard[],
    panels: PortablePanel[],
    filename: string,
    nowIso?: string,
  ) => void;
  /** Replay a parsed bundle into the store under the caller's authority. `mode` decides collision
   *  behavior; overwrite is owner-only + re-checked host-side (a non-owner's save is denied). */
  importBundle: (
    bundle: DashboardBundle,
    mode: CollisionMode,
  ) => Promise<ImportOutcome>;
}

export function useDashboardIo(): DashboardIo {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const exportBundle = useCallback(
    async (
      dashboardIds: string[],
      panelIds: string[],
      filename?: string,
      nowIso?: string,
    ) => {
      setBusy(true);
      setError(null);
      try {
        const dashboards: PortableDashboard[] = [];
        for (const id of dashboardIds) {
          const full = await getDashboard(id);
          dashboards.push(dashboardToPortable(full));
        }
        const panels: PortablePanel[] = [];
        for (const id of panelIds) {
          const p = await getPanel(id);
          panels.push({
            id: bare(p.id),
            title: p.title,
            spec: p.spec,
            schemaVersion: p.schemaVersion,
          });
        }
        const bundle = makeBundle(dashboards, panels, nowIso);
        const name = filename ?? defaultFilename(dashboards, panels);
        downloadText(name, serializeBundle(bundle));
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const downloadBundle = useCallback(
    (
      dashboards: PortableDashboard[],
      panels: PortablePanel[],
      filename: string,
      nowIso?: string,
    ) => {
      downloadText(
        filename,
        serializeBundle(makeBundle(dashboards, panels, nowIso)),
      );
    },
    [],
  );

  const importBundle = useCallback(
    async (
      bundle: DashboardBundle,
      mode: CollisionMode,
    ): Promise<ImportOutcome> => {
      setBusy(true);
      setError(null);
      const outcome: ImportOutcome = { dashboards: [], panels: [], errors: [] };
      try {
        // Collision policy: gather the ids already present so `rename` can pick a fresh, non-colliding
        // id (never a silent overwrite). `overwrite` keeps the desired id — the host still enforces
        // owner-only on an existing record, so an overwrite of someone else's dashboard is denied.
        const takenDash = new Set<string>();
        const takenPanel = new Set<string>();
        if (mode === "rename") {
          try {
            (await listDashboards()).forEach((d) => takenDash.add(bare(d.id)));
          } catch {
            /* a list failure just means no known collisions to avoid; save still enforces the wall */
          }
          try {
            (await listPanels()).forEach((p) => takenPanel.add(bare(p.id)));
          } catch {
            /* as above */
          }
        }

        // Panels first, so a dashboard that references a panel could resolve it (ref cells hydrate
        // host-side; inline cells don't need this, but the order is harmless and future-friendly).
        for (const p of bundle.panels) {
          const desired = bare(p.id);
          const id =
            mode === "rename" ? uniqueId(desired, takenPanel) : desired;
          takenPanel.add(id);
          try {
            const saved = await savePanel(id, p.title, p.spec as PanelSpec);
            outcome.panels.push({
              id: bare(saved.id),
              title: saved.title,
              renamedFrom: id !== desired ? desired : undefined,
            });
          } catch (e) {
            outcome.errors.push(
              `Widget "${p.title}": ${e instanceof Error ? e.message : String(e)}`,
            );
          }
        }

        for (const d of bundle.dashboards) {
          const desired = bare(d.id);
          const id = mode === "rename" ? uniqueId(desired, takenDash) : desired;
          takenDash.add(id);
          try {
            const saved = await saveDashboard(
              id,
              d.title,
              d.cells,
              d.variables ?? [],
            );
            outcome.dashboards.push({
              id: bare(saved.id),
              title: saved.title,
              renamedFrom: id !== desired ? desired : undefined,
            });
          } catch (e) {
            outcome.errors.push(
              `Dashboard "${d.title}": ${e instanceof Error ? e.message : String(e)}`,
            );
          }
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        outcome.errors.push(e instanceof Error ? e.message : String(e));
      } finally {
        setBusy(false);
      }
      return outcome;
    },
    [],
  );

  return { busy, error, exportBundle, downloadBundle, importBundle };
}

function bare(id: string): string {
  const i = id.indexOf(":");
  return i >= 0 ? id.slice(i + 1) : id;
}

/** A sensible default filename for an export — the single record's title when there's exactly one,
 *  else a generic "dashboards" name. Always ends in {@link BUNDLE_EXT}. */
export function defaultFilename(
  dashboards: PortableDashboard[],
  panels: PortablePanel[],
): string {
  const only =
    dashboards.length === 1 && panels.length === 0
      ? dashboards[0].title
      : panels.length === 1 && dashboards.length === 0
        ? panels[0].title
        : "dashboards";
  const slug =
    only
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "export";
  return `${slug}${BUNDLE_EXT}`;
}
