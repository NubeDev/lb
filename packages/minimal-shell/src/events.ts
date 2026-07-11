// Minimal SSE hub — one EventSource per tab, refcounted. Opens GET /events/stream?token=...
import { gatewayUrl, sessionToken } from "./ipc";

type FrameHandler = (data: string) => void;

let es: EventSource | null = null;
let sid: string | null = null;
const subjects = new Map<string, Set<FrameHandler>>();

function ensureOpen() {
  if (es) return;
  const token = sessionToken();
  if (!token) return;
  es = new EventSource(`${gatewayUrl()}/events/stream?token=${encodeURIComponent(token)}`);
  es.addEventListener("hello", (e) => {
    sid = JSON.parse((e as MessageEvent).data).sid;
    // Re-declare subscriptions on reconnect.
    for (const sub of subjects.keys()) {
      fetch(`${gatewayUrl()}/events/${sid}/subscribe`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ subject: sub }),
      });
    }
  });
  es.addEventListener("mux", (e) => {
    const frame = JSON.parse((e as MessageEvent).data);
    const handlers = subjects.get(frame.sub);
    if (handlers) handlers.forEach((h) => h(frame.data));
  });
  es.onerror = () => {
    es?.close();
    es = null;
    sid = null;
    setTimeout(ensureOpen, 2000);
  };
}

export function subscribe(subject: string, handler: FrameHandler): () => void {
  let set = subjects.get(subject);
  if (!set) {
    set = new Set();
    subjects.set(subject, set);
  }
  set.add(handler);
  ensureOpen();
  if (sid) {
    fetch(`${gatewayUrl()}/events/${sid}/subscribe`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ subject }),
    });
  }
  return () => {
    set!.delete(handler);
    if (set!.size === 0) subjects.delete(subject);
  };
}
