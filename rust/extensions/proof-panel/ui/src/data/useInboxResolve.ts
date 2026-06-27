import { useCallback, useState } from "react";

import { useBridge } from "@/app/useBridge";
import type { Decision } from "./workflow.types";

/** The status of a one-shot resolve action, keyed implicitly by the caller's UI — idle until a click,
 *  then resolving → ok / error. The page disables the buttons while `resolving`. */
export type ResolveState =
  | { status: "idle" }
  | { status: "resolving" }
  | { status: "ok" }
  | { status: "error"; error: string };

/** Resolve an inbox item (approve/reject/defer) through the granted `inbox.resolve` verb — the page's
 *  first WRITE that mutates durable workflow state. The host forces the deciding actor to the
 *  authenticated principal's `sub` (un-spoofable). Returns true on success so the caller can refresh.
 *  A rejected call (out of scope / denied) surfaces honestly as an error. */
export function useInboxResolve() {
  const bridge = useBridge();
  const [state, setState] = useState<ResolveState>({ status: "idle" });

  const resolve = useCallback(
    async (itemId: string, decision: Decision, ts: number): Promise<boolean> => {
      setState({ status: "resolving" });
      try {
        await bridge.call("inbox.resolve", { item_id: itemId, decision, ts });
        setState({ status: "ok" });
        return true;
      } catch (e: unknown) {
        setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
        return false;
      }
    },
    [bridge],
  );

  return { state, resolve };
}
