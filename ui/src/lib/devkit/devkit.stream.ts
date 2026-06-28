// Live devkit build logs over the generic bus SSE route. The host publishes JSON strings on
// `devkit/build/<job_id>`; the gateway authenticates by token query param because EventSource
// cannot set Authorization headers.

import { gatewayUrl } from "@/lib/ipc/http";
import { sessionToken } from "@/lib/session/session.store";

export interface BuildLogStream {
  close: () => void;
}

export function openDevkitBuildLog(
  subject: string,
  onLine: (line: string) => void,
): BuildLogStream | null {
  const base = gatewayUrl();
  if (base === "" && import.meta.env.VITE_GATEWAY_URL === undefined) return null;
  if (typeof EventSource === "undefined") return null;

  const url = `${base}/bus/stream?subject=${encodeURIComponent(subject)}&token=${encodeURIComponent(
    sessionToken(),
  )}`;
  const es = new EventSource(url);

  es.addEventListener("message", (event) => {
    try {
      const payload = JSON.parse((event as MessageEvent).data);
      if (typeof payload === "string") onLine(payload);
    } catch {
      // Ignore a malformed frame; the next build line can still arrive.
    }
  });

  return { close: () => es.close() };
}
