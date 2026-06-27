import { useContext } from "react";

import { BridgeContext } from "./bridge-context";
import type { MountCtx } from "./contract";

/** The host page context (active workspace). */
export function useCtx(): MountCtx {
  const rt = useContext(BridgeContext);
  if (!rt) throw new Error("useCtx must be used within the extension root");
  return rt.ctx;
}
