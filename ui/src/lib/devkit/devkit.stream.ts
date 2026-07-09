// Live devkit build logs over the generic bus SSE route. The host publishes JSON strings on
// `devkit/build/<job_id>`; the gateway authenticates by token query param because EventSource
// cannot set Authorization headers.

import { eventHub, liveStreamAvailable } from "@/lib/events/hub";

export interface BuildLogStream {
  close: () => void;
}

export function openDevkitBuildLog(
  subject: string,
  onLine: (line: string) => void,
): BuildLogStream | null {
  if (!liveStreamAvailable()) return null;
  // Delegates to the shared event hub via the generic `bus:{subject}` subject (the build log rides
  // `devkit/build/<job_id>` on the bus). `event: message` payload is unchanged; only string lines fold in.
  const unsubscribe = eventHub.subscribeSubject(`bus:${subject}`, (frame) => {
    if (frame.event !== "message") return;
    try {
      const payload = JSON.parse(frame.data);
      if (typeof payload === "string") onLine(payload);
    } catch {
      // Ignore a malformed frame; the next build line can still arrive.
    }
  });
  return { close: unsubscribe };
}
