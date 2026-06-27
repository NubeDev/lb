// The session hook — React's view of the session store (collaboration scope, slice 1). Exposes the
// current session plus `signIn`/`signOut`. Components read `session` to gate the app (logged out →
// login screen) and to scope every surface to `session.workspace`. One hook per file (FILE-LAYOUT).

import { useCallback, useSyncExternalStore } from "react";

import { login as loginApi } from "@/lib/session/session.api";
import { getSession, setSession, subscribeSession } from "@/lib/session/session.store";
import type { Session } from "@/lib/session/session.types";

export interface SessionState {
  session: Session | null;
  signIn: (user: string, workspace: string) => Promise<void>;
  signOut: () => void;
}

/** Subscribe to the session and drive login/logout. The token lives in the store (so the IPC layer
 *  can read it); this hook is the React adapter + the two transitions. */
export function useSession(): SessionState {
  const session = useSyncExternalStore(subscribeSession, getSession, getSession);

  const signIn = useCallback(async (user: string, workspace: string) => {
    const next = await loginApi(user, workspace);
    setSession(next);
  }, []);

  const signOut = useCallback(() => {
    setSession(null);
  }, []);

  return { session, signIn, signOut };
}
