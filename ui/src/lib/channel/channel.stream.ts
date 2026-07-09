// The live channel feed over SSE — the S3 server→browser push (mirrors the gateway's
// `GET /channels/{cid}/stream`). The browser opens this once per channel and receives OTHERS'
// messages and presence changes in real time, which `useChannel` folds into its `setItems` sink.
//
// One verb: `openChannelStream`. It uses the native `EventSource`, so it only runs in a real
// browser against a real gateway — in the Tauri shell and in tests there is no gateway URL, and
// the caller skips opening it (live updates there come from the post→refresh round trip, S2).

import type { Item } from "./channel.types";
import { eventHub, liveStreamAvailable } from "@/lib/events/hub";

/** Callbacks for the SSE event kinds the gateway emits. */
export interface ChannelStreamHandlers {
  onMessage: (item: Item) => void;
  /** A message was deleted by its author — drop the id from the local view. */
  onDelete?: (id: string) => void;
  onPresence?: (member: string, present: boolean) => void;
}

/** A live stream handle — call `close()` to stop (the hook does this on unmount). */
export interface ChannelStream {
  close: () => void;
}

/** Open the SSE stream for `(ws, channel)`. Returns `null` when no gateway is configured (Tauri
 *  shell / tests) — the caller simply has no live feed there, by design. */
export function openChannelStream(
  _ws: string,
  channel: string,
  handlers: ChannelStreamHandlers,
): ChannelStream | null {
  if (!liveStreamAvailable()) return null;
  // Delegates to the shared event hub: the `channel:{channel}` subject rides the one multiplexed
  // connection. The gateway merges the same three feeds (message/delete/presence) into this subject, so
  // the frame handling below is unchanged — the hub just hands each `{event, data}` back verbatim.
  const unsubscribe = eventHub.subscribeSubject(`channel:${channel}`, (frame) => {
    try {
      if (frame.event === "message") {
        handlers.onMessage(JSON.parse(frame.data) as Item);
      } else if (frame.event === "delete") {
        const { id } = JSON.parse(frame.data) as { id: string };
        handlers.onDelete?.(id);
      } else if (frame.event === "presence") {
        const { member, present } = JSON.parse(frame.data) as {
          member: string;
          present: boolean;
        };
        handlers.onPresence?.(member, present);
      }
    } catch {
      // a malformed frame never breaks the stream
    }
  });
  return { close: unsubscribe };
}
