// The nav-builder data hook (nav scope) — loads the nav roster + the three REAL item sources
// (surfaces, dashboards, ext pages) the builder picks from, and exposes save/delete/share/set-default
// against the real gateway. No mock data: every list is a real `*.list` call, every write a real
// `nav.*` verb re-checked server-side. The nav grants nothing — the builder can never write a cap.

import { useCallback, useEffect, useState } from "react";

import {
  deleteNav,
  listNavs,
  saveNav,
  setDefaultNav,
  shareNav,
  type NavItem,
  type NavSummary,
  type Visibility,
} from "@/lib/nav";
import { listDashboards, type DashboardSummary } from "@/lib/dashboard";
import { listExtensions, type ExtRow } from "@/lib/ext/ext.api";

export interface NavSources {
  dashboards: DashboardSummary[];
  extensions: ExtRow[];
}

/** The nav roster + the pickable sources + the write actions. Reloads the roster after every write. */
export function useNavs(ws: string) {
  const [navs, setNavs] = useState<NavSummary[]>([]);
  const [sources, setSources] = useState<NavSources>({ dashboards: [], extensions: [] });
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const reloadNavs = useCallback(async () => {
    try {
      setNavs(await listNavs());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      // The three real sources the builder picks from — dashboards + ext pages (surfaces are the
      // static core set the builder ships inline). Each is a real list call, workspace-walled.
      const [dashboards, extensions] = await Promise.all([
        listDashboards().catch(() => [] as DashboardSummary[]),
        listExtensions().catch(() => [] as ExtRow[]),
      ]);
      setSources({ dashboards, extensions: extensions.filter((e: ExtRow) => e.ui?.entry) });
      await reloadNavs();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [reloadNavs]);

  useEffect(() => {
    void reload();
  }, [ws, reload]);

  const save = useCallback(
    async (id: string, title: string, items: NavItem[]) => {
      await saveNav(id, title, items);
      await reloadNavs();
    },
    [reloadNavs],
  );

  const remove = useCallback(
    async (id: string) => {
      await deleteNav(id);
      await reloadNavs();
    },
    [reloadNavs],
  );

  const share = useCallback(
    async (id: string, visibility: Visibility, team?: string) => {
      await shareNav(id, visibility, team);
      await reloadNavs();
    },
    [reloadNavs],
  );

  const setDefault = useCallback(async (id: string) => {
    await setDefaultNav(id);
  }, []);

  return { navs, sources, error, loading, reload, save, remove, share, setDefault };
}
