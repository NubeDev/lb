// The app shell — a real collaboration app over a real session (collaboration scope). The hardcoded
// S2 demo identity (`WS`/`CHANNEL`/`AUTHOR`) is gone: identity now comes from `useSession` (a verified
// token). Logged out → the login screen; logged in → the nav rail + workspace/channel switchers + the
// selected route. Layout + wiring only; each surface owns its data (FILE-LAYOUT).

import { useMemo } from "react";
import { RouterProvider } from "@tanstack/react-router";

import { useSession, CAP, hasCap } from "@/lib/session";
import { LoginView } from "./features/session";
import { useExtensionPages } from "./features/ext-host";
import { allowedSurfaces } from "./features/routing/allowed";
import { createAppRouter } from "./features/routing/createAppRouter";
import { RoutingContextProvider } from "./features/routing/RoutingContextProvider";
import { ThemeProvider } from "./lib/theme";

export function App() {
  const { session, signIn, signOut } = useSession();
  const router = useMemo(() => createAppRouter(), []);

  // Extension PAGES (ui-federation scope): installed extensions that declare a `[ui]` block contribute
  // a cap-gated sidebar slot. Discovered from `ext.list` (only visible to a session that can list
  // extensions — the gateway re-checks the page's bridged calls regardless). Called unconditionally
  // (before the logged-out early return) so the hook order is stable; the empty `ws` disables it.
  const extPages = useExtensionPages(
    session && hasCap(session.caps, CAP.extList) ? session.workspace : "",
  );

  let content = <LoginView onSignIn={signIn} />;

  if (session) {
    const { workspace, principal, caps } = session;
    // Switching workspace is a re-login (the workspace is the token's hard wall §7), keeping identity.
    const switchWorkspace = (ws: string) => void signIn(principal, ws);
    const allowed = allowedSurfaces(caps);
    const routingContext = {
      workspace,
      principal,
      caps,
      allowed,
      extPages: extPages.pages,
      extPagesLoading: extPages.loading,
      onSignOut: signOut,
      switchWorkspace,
    };

    content = (
      <RoutingContextProvider value={routingContext}>
        <RouterProvider router={router} context={routingContext} />
      </RoutingContextProvider>
    );
  }

  return (
    <ThemeProvider>
      {content}
    </ThemeProvider>
  );
}
