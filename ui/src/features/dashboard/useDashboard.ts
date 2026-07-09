// The dashboard hook — the one place the dashboard surface loads the roster + the selected dashboard
// and mutates it (dashboard scope). Layout edits (drag/resize/add/remove a cell) persist through
// `dashboard.save` (the layout is a SurrealDB record, never localStorage — rule 4). Every call goes
// through the real `invoke` seam to the gateway/host; no fake/demo data. One hook per file.

import { useCallback, useEffect, useState } from "react";

import {
  deleteDashboard,
  getDashboard,
  listDashboards,
  saveDashboard,
  shareDashboard,
  type Cell,
  type Dashboard,
  type DashboardMeta,
  type DashboardSummary,
  type Variable,
  type Visibility,
} from "@/lib/dashboard";

export interface DashboardState {
  roster: DashboardSummary[];
  current: Dashboard | null;
  error: string | null;
  refresh: () => Promise<void>;
  select: (id: string) => Promise<void>;
  create: (id: string, title: string) => Promise<void>;
  /** Persist the cell set (drag/resize/add/remove all funnel here → `dashboard.save`). */
  saveCells: (cells: Cell[]) => Promise<void>;
  /** Persist the variable definitions (the variable editor → `dashboard.save`, cells preserved). */
  saveVariables: (variables: Variable[]) => Promise<void>;
  /** Rename a dashboard (title-only `dashboard.save`, cells + variables preserved; owner-only). */
  rename: (id: string, title: string) => Promise<void>;
  /** Persist the current dashboard's page settings — description/icon/color (dashboard page-settings).
   *  Cells + variables preserved; omitted meta keys keep their stored value (owner-only). */
  saveMeta: (meta: DashboardMeta) => Promise<void>;
  remove: (id: string) => Promise<void>;
  share: (visibility: Visibility, team?: string) => Promise<void>;
}

export function useDashboard(ws: string): DashboardState {
  const [roster, setRoster] = useState<DashboardSummary[]>([]);
  const [current, setCurrent] = useState<Dashboard | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setRoster(await listDashboards());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    // ws is the session's hard wall; re-run when it changes (a workspace switch is a re-login).
  }, []);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    setCurrent(null);
    void refresh();
  }, [ws]);

  const select = useCallback(async (id: string) => {
    try {
      setCurrent(await getDashboard(id));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const create = useCallback(
    async (id: string, title: string) => {
      try {
        const d = await saveDashboard(id, title, []);
        setCurrent(d);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refresh],
  );

  const saveCells = useCallback(
    async (cells: Cell[]) => {
      if (!current) return;
      try {
        // Preserve the existing variable definitions across a layout save (variables and cells both
        // live on the one record; a cell save must not drop the variables).
        const d = await saveDashboard(current.id, current.title, cells, current.variables ?? []);
        setCurrent(d);
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [current],
  );

  const saveVariables = useCallback(
    async (variables: Variable[]) => {
      if (!current) return;
      try {
        const d = await saveDashboard(current.id, current.title, current.cells, variables);
        setCurrent(d);
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [current],
  );

  const rename = useCallback(
    async (id: string, title: string) => {
      const t = title.trim();
      if (!t) return;
      try {
        // Preserve cells + variables: use the loaded copy when it's the current dashboard, otherwise
        // read the target first (a title-only save must not blank its layout).
        const target = current && current.id === id ? current : await getDashboard(id);
        const d = await saveDashboard(id, t, target.cells, target.variables ?? []);
        setCurrent((c) => (c && c.id === id ? d : c));
        await refresh();
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [current, refresh],
  );

  const saveMeta = useCallback(
    async (meta: DashboardMeta) => {
      if (!current) return;
      try {
        // Cells + variables preserved; `meta` carries only the page-settings fields the dialog edited.
        const d = await saveDashboard(
          current.id,
          current.title,
          current.cells,
          current.variables ?? [],
          meta,
        );
        setCurrent(d);
        await refresh();
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [current, refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      try {
        await deleteDashboard(id);
        setCurrent((c) => (c && c.id === id ? null : c));
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refresh],
  );

  const share = useCallback(
    async (visibility: Visibility, team?: string) => {
      if (!current) return;
      try {
        const d = await shareDashboard(current.id, visibility, team);
        setCurrent(d);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [current, refresh],
  );

  return {
    roster,
    current,
    error,
    refresh,
    select,
    create,
    saveCells,
    saveVariables,
    rename,
    saveMeta,
    remove,
    share,
  };
}
