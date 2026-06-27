// The presence hook — renders WHO IS ONLINE in a channel (collaboration scope, slice 3). The data
// already streams: the gateway emits an `event: presence` `{member, present}` over the same SSE
// connection the messages ride. This hook subscribes to that and folds join/leave into a roster.
//
// Presence is unordered and racy (join/leave interleave), so the roster is a SET keyed by member,
// updated idempotently — never a function of event order (scope: "rely on idempotent roster updates,
// not event order"). `mergePresence` is the pure reducer, unit-tested directly; the hook wires it to
// the live stream. One hook per file (FILE-LAYOUT), beside `useChannel`.

import { useEffect, useState } from "react";

import { openChannelStream } from "@/lib/channel/channel.stream";

/** Fold a presence change into the online set: `present` adds the member, absent removes it.
 *  Idempotent — applying the same change twice yields the same set (order-independent). */
export function mergePresence(online: Set<string>, member: string, present: boolean): Set<string> {
  const next = new Set(online);
  if (present) next.add(member);
  else next.delete(member);
  return next;
}

/** Subscribe to `(ws, channel)` presence and expose the sorted roster of online members. Empty in
 *  the Tauri shell / tests (no gateway) — there the live feed is absent by design. */
export function usePresence(ws: string, channel: string): string[] {
  const [online, setOnline] = useState<Set<string>>(new Set());

  useEffect(() => {
    setOnline(new Set());
    const stream = openChannelStream(ws, channel, {
      // Messages are useChannel's concern; here we only fold presence.
      onMessage: () => {},
      onPresence: (member, present) =>
        setOnline((prev) => mergePresence(prev, member, present)),
    });
    return () => stream?.close();
  }, [ws, channel]);

  return [...online].sort();
}
