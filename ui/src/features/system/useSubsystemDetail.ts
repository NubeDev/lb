// The subsystem-detail hook (system-map scope) — loads `system.subsystem` for the selected card and
// exposes its detail/loading/error. Re-fetches whenever the selected id changes; clears when the id
// is null (the sheet closed). READ-ONLY: no mutation (the map is a read). One hook per file
// (FILE-LAYOUT). Runs against the real gateway, admin-gated server-side.

import { useEffect, useState } from "react";

import { systemSubsystem } from "@/lib/system/system.api";
import type { SubsystemDetail } from "@/lib/system/system.types";

export interface SubsystemDetailState {
  detail: SubsystemDetail | null;
  error: string | null;
  loading: boolean;
}

/** Load the detail for subsystem `id` (or nothing when `id` is null). */
export function useSubsystemDetail(id: string | null): SubsystemDetailState {
  const [detail, setDetail] = useState<SubsystemDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (id === null) {
      setDetail(null);
      setError(null);
      return;
    }
    let live = true;
    setLoading(true);
    setDetail(null);
    setError(null);
    void systemSubsystem(id)
      .then((d) => {
        if (live) setDetail(d);
      })
      .catch((e) => {
        if (live) setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (live) setLoading(false);
      });
    return () => {
      live = false;
    };
  }, [id]);

  return { detail, error, loading };
}
