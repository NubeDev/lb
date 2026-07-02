// The React context carrying the picker's `SourceLoaders` from the mount shell (which holds the
// bridge) down to the PropertyRail's bind picker — without threading the bridge through the prop-less
// `App` tree. One responsibility: provide/consume the loaders. The default is EMPTY loaders (no reads),
// so a build without a bridge (tests, the inert default) shows a picker with no series — never a crash.

import { createContext, useContext } from "react";
import type { SourceLoaders } from "@nube/source-picker";

/** Default = no loaders (every picker group empty). The page mount injects the bridge-backed set. */
export const SourceLoadersContext = createContext<SourceLoaders>({});

export function useSourceLoaders(): SourceLoaders {
  return useContext(SourceLoadersContext);
}
