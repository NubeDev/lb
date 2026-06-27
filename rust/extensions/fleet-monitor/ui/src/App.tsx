import { BridgeContext } from "@/app/bridge-context";
import { Router } from "@/app/router";
import type { Bridge, MountCtx } from "@/app/contract";

interface Props {
  ctx: MountCtx;
  bridge: Bridge;
}

/** Root: provides the host `ctx`/`bridge` to the tree and mounts the nested router. */
export function App({ ctx, bridge }: Props) {
  return (
    <BridgeContext.Provider value={{ ctx, bridge }}>
      <Router />
    </BridgeContext.Provider>
  );
}
