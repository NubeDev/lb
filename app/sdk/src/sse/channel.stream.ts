// The live channel feed — the app mirror of `ui/src/lib/channel/channel.stream.ts`, riding on the
// shared `openSse` transport. One verb: `openChannelStream`. The gateway route is
// `GET /channels/{cid}/stream?token=` (README §6.13); a malformed frame never breaks the stream.

import type { GatewayConfig } from "../client/config";
import type { Item } from "../channel/channel.types";
import { openSse, type SseStream } from "./stream";

export interface ChannelStreamHandlers {
  onMessage: (item: Item) => void;
  /** A message was deleted by its author — drop the id from the local view. */
  onDelete?: (id: string) => void;
  onPresence?: (member: string, present: boolean) => void;
  /** Fired on every (re)connect — the caller re-reads `channel.history` here to close the gap. */
  onOpen?: () => void;
}

/** Open the SSE stream for `channel` in the token's workspace. */
export function openChannelStream(
  config: GatewayConfig,
  channel: string,
  handlers: ChannelStreamHandlers,
): SseStream {
  return openSse(config, `/channels/${encodeURIComponent(channel)}/stream`, {
    onOpen: handlers.onOpen,
    onEvent(event, data) {
      try {
        if (event === "message") handlers.onMessage(JSON.parse(data) as Item);
        else if (event === "delete") handlers.onDelete?.((JSON.parse(data) as { id: string }).id);
        else if (event === "presence") {
          const { member, present } = JSON.parse(data) as { member: string; present: boolean };
          handlers.onPresence?.(member, present);
        }
      } catch {
        // a malformed frame never breaks the stream
      }
    },
  });
}
