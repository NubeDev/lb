import { useContext } from "react";

import { BridgeContext } from "./bridge-context";
import type { Bridge } from "./contract";

/** The host-mediated data bridge — the ONLY way a page reaches platform data. */
export function useBridge(): Bridge {
  const rt = useContext(BridgeContext);
  if (!rt) throw new Error("useBridge must be used within the extension root");
  return rt.bridge;
}
