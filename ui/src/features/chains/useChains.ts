// The chains CRUD state hook (rules-workbench scope, Phase 2). Holds the workspace roster + the
// currently-open chain, and the load/save/delete actions over the real api client. Separated from the
// view markup (FILE-LAYOUT frontend: data in the hook, markup in the .tsx).

import { useCallback, useEffect, useState } from "react";

import {
  deleteChain,
  getChain,
  listChains,
  saveChain,
  type Chain,
  type ChainSummary,
} from "@/lib/chains";

export interface ChainsState {
  roster: ChainSummary[];
  open: Chain | null;
  error: string | null;
  refresh: () => Promise<void>;
  load: (id: string) => Promise<void>;
  save: (chain: Chain) => Promise<{ ok: boolean; error?: string }>;
  remove: (id: string) => Promise<void>;
  setOpen: (chain: Chain | null) => void;
}

export function useChains(ws: string): ChainsState {
  const [roster, setRoster] = useState<ChainSummary[]>([]);
  const [open, setOpen] = useState<Chain | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setRoster(await listChains());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  // Re-load the roster whenever the workspace changes (a fresh session = a fresh roster, the wall).
  useEffect(() => {
    void refresh();
  }, [ws, refresh]);

  const load = useCallback(async (id: string) => {
    setError(null);
    try {
      setOpen(await getChain(id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  /** Save a chain; surfaces the host's validation message inline (no throw to the canvas) so a cyclic
   *  / invalid DAG renders its `400` text rather than crashing. */
  const save = useCallback(
    async (chain: Chain): Promise<{ ok: boolean; error?: string }> => {
      try {
        await saveChain(chain);
        setOpen(chain);
        await refresh();
        return { ok: true };
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        return { ok: false, error: msg };
      }
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      await deleteChain(id);
      setOpen((cur) => (cur?.id === id ? null : cur));
      await refresh();
    },
    [refresh],
  );

  return { roster, open, error, refresh, load, save, remove, setOpen };
}
