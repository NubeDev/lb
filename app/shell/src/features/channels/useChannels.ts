// The channel directory: `channel.list` + `channel.create`. Re-runs on workspace switch. A deny
// surfaces as text ("not permitted"), never a silent empty list (app-shell scope §Capabilities).

import { useCallback, useEffect, useState } from 'react';
import { InvokeError, type ChannelRecord } from '@nube/app-sdk';
import { gatewayClient } from '../../lib/client';

export function useChannels(activeWs: string | undefined): {
  channels: ChannelRecord[];
  create: (id: string) => Promise<void>;
  error: string;
  reload: () => void;
} {
  const [channels, setChannels] = useState<ChannelRecord[]>([]);
  const [error, setError] = useState('');
  const [rev, setRev] = useState(0);

  useEffect(() => {
    const client = gatewayClient();
    if (!client || !activeWs) return;
    let live = true;
    client
      .invoke<ChannelRecord[]>('channel_list')
      .then((rows) => live && setChannels(rows))
      .catch((e: unknown) => {
        if (!live) return;
        setChannels([]);
        setError(e instanceof InvokeError && e.isDenied ? 'not permitted' : String(e));
      });
    return () => {
      live = false;
    };
  }, [activeWs, rev]);

  const create = useCallback(async (id: string) => {
    setError('');
    try {
      await gatewayClient()?.invoke('channel_create', { channel: id });
      setRev((r) => r + 1);
    } catch (e) {
      setError(e instanceof InvokeError && e.isDenied ? 'not permitted' : String(e));
    }
  }, []);

  return { channels, create, error, reload: () => setRev((r) => r + 1) };
}
