import { createContext } from "react";

import type { Bridge, MountCtx } from "./contract";

/** What the root provides to every page: the host `ctx` (workspace) and the data `bridge`. */
export interface ExtRuntime {
  ctx: MountCtx;
  bridge: Bridge;
}

/** Carries the host-provided runtime down the tree. Null until the root provider wraps the app. */
export const BridgeContext = createContext<ExtRuntime | null>(null);
