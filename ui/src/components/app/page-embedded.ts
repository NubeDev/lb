// The embedded-page seam (data-studio-10x scope, phase 2 "pages-as-panes") — a context that tells
// `AppPage` it is mounted inside a dock pane rather than on its own route. Embedded, the page
// suppresses its full-width header (the dock tab is the title bar); everything else — caps, error
// strip, body — is identical: embedding changes WHERE a view mounts, not its authority. A pane host
// wraps its content in the provider; the routed shell never does, so standalone routes keep their
// header. One responsibility: the context handle.

import { createContext, useContext } from "react";

export const EmbeddedPageContext = createContext(false);

/** True when the nearest host declared this subtree embedded (mounted inside a dock pane). */
export function useEmbeddedPage(): boolean {
  return useContext(EmbeddedPageContext);
}
