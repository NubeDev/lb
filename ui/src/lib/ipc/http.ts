// The real HTTP transport to a node's SSE/HTTP gateway (S3). This is what replaces the S2
// in-memory fake when the app runs as a browser against a real node: the same command verbs
// (`channel_post`, `channel_history`) map onto the gateway's REST routes one-to-one.
//
// One verb per command, mapped to the gateway routes built in `role/gateway`:
//   channel_post    → POST /channels/{cid}/messages
//   channel_history → GET  /channels/{cid}/messages
//
// The gateway base URL comes from `VITE_GATEWAY_URL` (set for the browser build). The feature
// code never sees this — it goes through `invoke`, exactly as it does for Tauri and the fake.

import type { Item } from "@/lib/channel/channel.types";

/** The gateway base URL, e.g. `http://127.0.0.1:8080`. Empty string = same origin. */
export function gatewayUrl(): string {
  return (import.meta.env.VITE_GATEWAY_URL as string | undefined) ?? "";
}

export async function httpInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const base = gatewayUrl();
  switch (cmd) {
    case "channel_post": {
      const { channel, item } = args as { channel: string; item: Item };
      const res = await fetch(`${base}/channels/${encodeURIComponent(channel)}/messages`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(item),
      });
      if (!res.ok) throw new Error(await errorText(res));
      return (await res.json()) as T;
    }
    case "channel_history": {
      const { channel } = args as { channel: string };
      const res = await fetch(`${base}/channels/${encodeURIComponent(channel)}/messages`);
      if (!res.ok) throw new Error(await errorText(res));
      return (await res.json()) as T;
    }
    default:
      throw new Error(`unknown command: ${cmd}`);
  }
}

/** A 403 from the gateway is the host's capability `Denied`; surface its body as the message. */
async function errorText(res: Response): Promise<string> {
  const body = await res.text().catch(() => "");
  return body || `request failed (${res.status})`;
}
