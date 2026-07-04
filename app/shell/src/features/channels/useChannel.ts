// One channel's live state: history + the SSE stream + post. The resume story is built in — every
// (re)connect re-reads `channel.history` (`onOpen`) so a backgrounded/killed stream loses nothing
// (the gateway emits no SSE ids; durable history IS the replay — app-shell scope, transport
// decision). Items merge id-keyed, so catch-up and live frames never duplicate.

import { useCallback, useEffect, useRef, useState } from 'react';
import { InvokeError, type Item } from '@nube/app-sdk';
import { gatewayClient } from '../../lib/client';

export function useChannel(
  activeWs: string | undefined,
  channel: string,
  author: string,
): { items: Item[]; post: (body: string) => Promise<void>; error: string } {
  const [items, setItems] = useState<Item[]>([]);
  const [error, setError] = useState('');
  const seq = useRef(0);

  const merge = useCallback((incoming: Item[]) => {
    setItems((held) => {
      const byId = new Map(held.map((i) => [i.id, i]));
      for (const item of incoming) byId.set(item.id, item);
      return [...byId.values()].sort((a, b) => a.ts - b.ts);
    });
  }, []);

  useEffect(() => {
    const client = gatewayClient();
    if (!client || !activeWs || !channel) return;
    setItems([]);
    const stream = client.streamChannel(channel, {
      onMessage: (item) => merge([item]),
      onDelete: (id) => setItems((held) => held.filter((i) => i.id !== id)),
      onOpen: () => {
        // The durable catch-up read — closes any gap from a dropped stream.
        void client
          .invoke<Item[]>('channel_history', { channel })
          .then(merge)
          .catch((e: unknown) =>
            setError(e instanceof InvokeError && e.isDenied ? 'not permitted' : String(e)),
          );
      },
    });
    return () => stream.close();
  }, [activeWs, channel, merge]);

  const post = useCallback(
    async (body: string) => {
      setError('');
      const item: Item = {
        id: `${Date.now()}-${seq.current++}`,
        channel,
        author,
        body,
        ts: Date.now(),
      };
      try {
        await gatewayClient()?.invoke('channel_post', { channel, item });
        merge([item]);
      } catch (e) {
        setError(e instanceof InvokeError && e.isDenied ? 'not permitted' : String(e));
      }
    },
    [channel, author, merge],
  );

  return { items, post, error };
}
