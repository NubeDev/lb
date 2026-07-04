// The channels hook — data + state for the channel switcher (collaboration scope, slice 2). Lists
// the workspace's registered channels and creates new ones. Distinct from `useChannel` (singular),
// which drives ONE channel's messages; this is the registry list. One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import { createChannel, listChannels } from "@/lib/channel/channel.api";
import type { ChannelRecord } from "@/lib/channel/channel.types";
import { isDockChannel } from "@/features/agent-dock/dockId";

export interface ChannelsState {
  channels: ChannelRecord[];
  error: string | null;
  refresh: () => Promise<void>;
  create: (channel: string) => Promise<void>;
}

/** Drive the channel registry list + create for workspace `ws`. Reloads after a create. */
export function useChannels(ws: string): ChannelsState {
  const [channels, setChannels] = useState<ChannelRecord[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      // Filter `dock.*` sessions OUT of the channels surface (agent-dock scope non-goal): a dock session
      // is the dock's STORAGE, not another room in the channel list. The prefix is a UI convention only
      // (the host never knows it — the wall is caps, not the name).
      setChannels((await listChannels(ws)).filter((c) => !isDockChannel(c.id)));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [ws]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const create = useCallback(
    async (channel: string) => {
      try {
        await createChannel(ws, channel);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [ws, refresh],
  );

  return { channels, error, refresh, create };
}
