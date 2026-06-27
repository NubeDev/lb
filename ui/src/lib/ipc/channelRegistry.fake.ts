// The in-memory channel-registry fake (TEST-ONLY) — mirrors the gateway `channel_list` /
// `channel_create`, and the create-on-post behaviour (the fake's `channel_post` calls
// `registerOnPost`). Workspace-scoped: the workspace comes from the session store, exactly as the
// real gateway derives it from the token. Returns `null` for unowned commands (fake-chain convention).

import type { ChannelRecord } from "@/lib/channel/channel.types";
import { getSession } from "@/lib/session/session.store";

const registry = new Map<string, Map<string, ChannelRecord>>(); // ws → (channel → record)
let seq = 0;

function ws(): string {
  return getSession()?.workspace ?? "";
}

function upsert(ws: string, channel: string, author: string): ChannelRecord {
  const byChannel = registry.get(ws) ?? new Map<string, ChannelRecord>();
  const record: ChannelRecord = byChannel.get(channel) ?? {
    id: channel,
    created_by: author,
    kind: "channel",
    ts: ++seq,
  };
  byChannel.set(channel, record);
  registry.set(ws, byChannel);
  return record;
}

/** Called by the channel fake's `channel_post` so a posted channel becomes listable (create-on-post). */
export function registerOnPost(workspace: string, channel: string, author: string): void {
  upsert(workspace, channel, author);
}

export function channelRegistryFakeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): T | null {
  switch (cmd) {
    case "channel_list": {
      const byChannel = registry.get(ws());
      const list = byChannel ? [...byChannel.values()].sort((a, b) => a.ts - b.ts) : [];
      return list as T;
    }
    case "channel_create": {
      const { channel } = args as { channel: string };
      return upsert(ws(), channel, getSession()?.principal ?? "user:test") as T;
    }
    default:
      return null;
  }
}

/** Test helper: clear the fake registry between tests. */
export function __resetChannelRegistryFake(): void {
  registry.clear();
  seq = 0;
}
