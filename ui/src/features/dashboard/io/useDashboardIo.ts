// The dashboard IMPORT orchestration hook (dashboard scope, import/export UX). It REPLAYS a parsed
// {@link DashboardBundle} into the store through the SHIPPED write verbs — `dashboard.save` /
// `panel.save`. No new host surface: import IS a sequence of ordinary owner-scoped writes, each
// capability-gated + workspace-walled server-side (§5/§6). The workspace/owner are never read from the
// file — they come from the caller's token (rule 6), so a bundle exported on node A imports cleanly on
// node B under B's authority. One responsibility: the replay + the collision-safe id policy.
//
// (EXPORT is not here: it's a pure read — build a bundle from records and show it in the shared
// `JsonPopout` (view / copy / download). No orchestration state is needed for that, so it lives at the
// call site next to the popout, keeping this hook to the one side that mutates.)

import { useCallback, useState } from "react";

import {
  saveDashboard,
  listDashboards,
  uniqueId,
  type DashboardBundle,
} from "@/lib/dashboard";
import { savePanel, listPanels, type PanelSpec } from "@/lib/panel";

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

  return { busy, error, importBundle };
}

function bare(id: string): string {
  const i = id.indexOf(":");
  return i >= 0 ? id.slice(i + 1) : id;
}
