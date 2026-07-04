// Extension discovery for the nav: `ext.list` folded through the sdk's cap gate (`extNavEntries` —
// the app mirror of the web `useExtensionPages`). This slice LISTS entries only; the federated
// mount is the app-extensions slice. Re-runs on workspace switch.

import { useEffect, useState } from 'react';
import { extNavEntries, type ExtNavEntry, type ExtRow } from '@nube/app-sdk';
import { gatewayClient } from '../../lib/client';

export function useExtensionNav(activeWs: string | undefined, caps: string[]): {
  entries: ExtNavEntry[];
  loading: boolean;
} {
  const [entries, setEntries] = useState<ExtNavEntry[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const client = gatewayClient();
    if (!client || !activeWs) {
      setEntries([]);
      return;
    }
    let live = true;
    setLoading(true);
    client
      .invoke<ExtRow[]>('ext_list')
      .then((rows) => live && setEntries(extNavEntries(rows, caps)))
      .catch(() => live && setEntries([]))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
    // caps travel with the session; keying on the workspace is keying on the token.
  }, [activeWs]); // eslint-disable-line react-hooks/exhaustive-deps

  return { entries, loading };
}
