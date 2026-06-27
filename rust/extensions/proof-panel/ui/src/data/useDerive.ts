import { useCallback, useState } from "react";

import { useBridge } from "@/app/useBridge";

/** The status of the one-shot derive action — idle until the user clicks, then deriving → ok / error. */
export type DeriveState =
  | { status: "idle" }
  | { status: "deriving" }
  | { status: "ok"; derived: number; sourceSeq: number }
  | { status: "error"; error: string };

/** Run the extension's OWN backend tool `proof.derive` (host-callback scope): the wasm GUEST reads the
 *  latest `proof.demo` and writes `proof.derived = value*2`, ALL through the host-mediated `host.call-tool`
 *  callback — a guest doing real platform work, not just echoing input. Unlike every other hook here
 *  (which call host-native verbs), this calls the qualified EXTENSION tool `proof-panel.proof.derive`.
 *  A denied call (the guest's callback narrowed away by `caller ∩ grant`, or the derive verb itself
 *  ungranted) surfaces honestly as an error — never a fabricated value. */
export function useDerive() {
  const bridge = useBridge();
  const [state, setState] = useState<DeriveState>({ status: "idle" });

  const derive = useCallback(async (): Promise<number | null> => {
    setState({ status: "deriving" });
    try {
      const res = await bridge.call<{ derived: number; source_seq: number }>(
        "proof-panel.proof.derive",
        {},
      );
      const derived = res?.derived ?? 0;
      setState({ status: "ok", derived, sourceSeq: res?.source_seq ?? 0 });
      return derived;
    } catch (e: unknown) {
      setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
      return null;
    }
  }, [bridge]);

  return { state, derive };
}
