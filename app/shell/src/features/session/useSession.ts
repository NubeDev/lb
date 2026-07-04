// Adapt the sdk session store to React (`useSyncExternalStore`) — the app mirror of
// ui/src/lib/session/useSession.ts.
//
// Two signals drive a re-render: (1) the node-url store — the FIRST time a URL is set the client
// becomes available (before that `gatewayClient()` is null and there is nothing to subscribe to),
// and (2) the client's session store — login/switch/logout. We fan both into the one `onChange`
// so the login screen actually advances once the session lands.

import { useCallback, useSyncExternalStore } from 'react';
import type { Session } from '@nube/app-sdk';
import { gatewayClient } from '../../lib/client';
import { subscribeNodeUrl } from '../../lib/node-url.store';

export function useSession(): Session | null {
  const subscribe = useCallback((onChange: () => void) => {
    const unsubUrl = subscribeNodeUrl(onChange);
    const unsubSession = gatewayClient()?.session.subscribe(onChange) ?? (() => {});
    return () => {
      unsubUrl();
      unsubSession();
    };
  }, []);
  return useSyncExternalStore(subscribe, () => gatewayClient()?.session.current() ?? null);
}
