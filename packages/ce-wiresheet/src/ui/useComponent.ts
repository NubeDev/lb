import { useEffect, useState } from "react";
import { useStructural } from "../lib/store";
import { getNodeByUid } from "../lib/rest";
import type { Component } from "../lib/engine-types";

/** Resolve a component by uid: from the structural store when it's in the current
 *  folder, else fetched once. Lets panels work for off-folder / global selections
 *  (e.g. a component opened from a cross-folder "All" index). */
export function useComponent(uid: number | undefined): Component | undefined {
  const inStore = useStructural((s) => (uid != null ? s.components.get(uid) : undefined));
  const [fetched, setFetched] = useState<Component | undefined>(undefined);
  useEffect(() => {
    if (uid == null || inStore) { setFetched(undefined); return; }
    let live = true;
    getNodeByUid(uid, { depth: 0 }).then((r) => { if (live) setFetched(r.nodes[0]); }).catch(() => {});
    return () => { live = false; };
  }, [uid, inStore]);
  return inStore ?? fetched;
}
