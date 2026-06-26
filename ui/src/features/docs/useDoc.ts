// The doc hook — data + state for opening one doc asset (FILE-LAYOUT: one hook per file, data
// separated from markup). Reads a doc by id through the capability- and membership-checked node
// verb; a denied read surfaces as an error (the gate-3 deny the S4 exit gate is about), never a
// silent empty.

import { useCallback, useEffect, useState } from "react";

import { getDoc } from "@/lib/assets/assets.api";
import type { Doc } from "@/lib/assets/assets.types";

export interface DocState {
  doc: Doc | null;
  loading: boolean;
  /** Set when the node refused the read (denied) or the doc is absent — shown to the user. */
  error: string | null;
  reload: () => Promise<void>;
}

/** Open doc `id` in `(ws)` as `author`. `author` is the caller principal (see assets.api). */
export function useDoc(ws: string, id: string, author: string): DocState {
  const [doc, setDoc] = useState<Doc | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setDoc(await getDoc(ws, id, author));
      setError(null);
    } catch (e) {
      setDoc(null);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [ws, id, author]);

  useEffect(() => {
    void reload();
  }, [reload]);

  return { doc, loading, error, reload };
}
