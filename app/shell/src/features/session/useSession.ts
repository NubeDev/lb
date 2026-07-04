// Adapt the sdk session store to React (`useSyncExternalStore`) — the app mirror of
// ui/src/lib/session/useSession.ts. Re-subscribes when the client (node URL) changes.

import { useCallback, useSyncExternalStore } from 'react';
import type { Session } from '@nube/app-sdk';
import { gatewayClient, subscribeClient } from '../../lib/client';

/** The active session, or null when logged out / no node configured. */
export function useSession(): Session | null {
  const subscribe = useCallback((onChange: () => void) => {
    const unsubClient = subscribeClient(onChange);
    const unsubSession = gatewayClient()?.session.subscribe(onChange) ?? (() => {});
    return () => {
      unsubClient();
      unsubSession();
    };
  }, []);
  return useSyncExternalStore(subscribe, () => gatewayClient()?.session.current() ?? null);
}
