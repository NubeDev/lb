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
  type DashboardSummary,
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
        const d = await saveDashboard(current.id, current.title, cells);
        setCurrent(d);
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [current],
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

  return { roster, current, error, refresh, select, create, saveCells, remove, share };
}
