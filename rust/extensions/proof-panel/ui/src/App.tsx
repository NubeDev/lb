import { BridgeContext } from "@/app/bridge-context";
import type { Bridge, MountCtx } from "@/app/contract";
import { Panel } from "@/pages/Panel";

interface Props {
  ctx: MountCtx;
  bridge: Bridge;
}

/** Root: provides the host `ctx`/`bridge` to the tree and renders the single proof page. */
export function App({ ctx, bridge }: Props) {
  return (
    <BridgeContext.Provider value={{ ctx, bridge }}>
      <Panel />
    </BridgeContext.Provider>
  );
}
