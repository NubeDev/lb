import { useCallback, useState } from "react";

import { useBridge } from "@/app/useBridge";

/** The result shape the guest's `proof.simulate` returns — a summary of each step that landed. */
export interface SimulateResult {
  inbox_id: string;
  resolved: boolean;
  outbox_pending: number;
}

/** The status of the one-shot simulation — idle until the user clicks, then running → ok / error. */
export type SimulateState =
  | { status: "idle" }
  | { status: "running" }
  | { status: "ok"; result: SimulateResult }
  | { status: "error"; error: string };

/** Run the extension's OWN backend tool `proof.simulate` (proof-workflow-sim scope): the wasm GUEST
 *  drives a full inbox→approval→outbox round-trip ENTIRELY through the host-mediated `host.call-tool`
 *  callback — it PRODUCES the workflow motion (records an inbox item, resolves it Approved, enqueues an
 *  outbox effect), instead of only reading something else seeded. Unlike the host-native hooks here this
 *  calls the qualified EXTENSION tool `proof-panel.proof.simulate`. A denied inner callback (narrowed by
 *  `caller ∩ grant`, or the simulate verb itself ungranted) surfaces honestly as an error — never a
 *  fabricated summary. Returns the result on success so the caller can refresh the inbox/outbox views. */
export function useSimulate() {
  const bridge = useBridge();
  const [state, setState] = useState<SimulateState>({ status: "idle" });

  const simulate = useCallback(async (): Promise<SimulateResult | null> => {
    setState({ status: "running" });
    try {
      const res = await bridge.call<SimulateResult>("proof-panel.proof.simulate", {});
      const result: SimulateResult = {
        inbox_id: res?.inbox_id ?? "",
        resolved: Boolean(res?.resolved),
        outbox_pending: res?.outbox_pending ?? 0,
      };
      setState({ status: "ok", result });
      return result;
    } catch (e: unknown) {
      setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
      return null;
    }
  }, [bridge]);

  return { state, simulate };
}
