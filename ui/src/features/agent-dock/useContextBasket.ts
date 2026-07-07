// The context-basket STATE hook (agent-context-basket scope) — the ordered item refs the user has
// gathered to feed into the NEXT ask. Thin state over the pure ops (`contextBasket.ts`); per-session
// (the refs are channel-scoped server-side, so switching dock sessions clears the basket), cleared
// by the caller after a send (the ask consumed it).

import { useCallback, useEffect, useState } from "react";

import { toggleRef } from "./contextBasket";

export interface ContextBasket {
  /** The ordered refs the next ask will carry (`AgentPayload.context_items`). */
  ids: string[];
  has: (id: string) => boolean;
  /** Add/remove one ref. Adding past the host's 8-ref cap is a no-op. */
  toggle: (id: string) => void;
  /** Empty the basket — called after an ask consumed it, or by the chips' clear-all. */
  clear: () => void;
}

/** Drive the basket for one dock session. `cid` is the current dock channel — a session switch
 *  resets the refs (they only resolve inside their own channel). */
export function useContextBasket(cid: string): ContextBasket {
  const [ids, setIds] = useState<string[]>([]);

  useEffect(() => {
    setIds([]); // refs are channel-scoped — a different session starts empty
  }, [cid]);

  const toggle = useCallback((id: string) => setIds((prev) => toggleRef(prev, id)), []);
  const clear = useCallback(() => setIds([]), []);
  const has = useCallback((id: string) => ids.includes(id), [ids]);

  return { ids, has, toggle, clear };
}
