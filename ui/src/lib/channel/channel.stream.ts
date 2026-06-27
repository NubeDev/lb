// The live channel feed over SSE — the S3 server→browser push (mirrors the gateway's
// `GET /channels/{cid}/stream`). The browser opens this once per channel and receives OTHERS'
// messages and presence changes in real time, which `useChannel` folds into its `setItems` sink.
//
// One verb: `openChannelStream`. It uses the native `EventSource`, so it only runs in a real
// browser against a real gateway — in the Tauri shell and in tests there is no gateway URL, and
// the caller skips opening it (live updates there come from the post→refresh round trip, S2).

import type { Item } from "./channel.types";
import { gatewayUrl } from "@/lib/ipc/http";
import { sessionToken } from "@/lib/session/session.store";

/** Callbacks for the two SSE event kinds the gateway emits. */
export interface ChannelStreamHandlers {
  onMessage: (item: Item) => void;
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
  const base = gatewayUrl();
  if (base === "" && import.meta.env.VITE_GATEWAY_URL === undefined) return null;
  if (typeof EventSource === "undefined") return null;

  // The token rides as a query param: `EventSource` cannot set an Authorization header, and the
  // gateway's stream route authenticates by `?token=` for exactly this reason (the hard wall holds —
  // workspace + caps come from the verified token).
  const url = `${base}/channels/${encodeURIComponent(channel)}/stream?token=${encodeURIComponent(
    sessionToken(),
  )}`;
  const es = new EventSource(url);

  es.addEventListener("message", (e) => {
    try {
      handlers.onMessage(JSON.parse((e as MessageEvent).data) as Item);
    } catch {
      // a malformed frame never breaks the stream
    }
  });

  if (handlers.onPresence) {
    es.addEventListener("presence", (e) => {
      try {
        const { member, present } = JSON.parse((e as MessageEvent).data) as {
          member: string;
          present: boolean;
        };
        handlers.onPresence?.(member, present);
      } catch {
        /* ignore */
      }
    });
  }

  return { close: () => es.close() };
}
