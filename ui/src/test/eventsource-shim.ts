// A minimal fetch-backed `EventSource` for jsdom gateway tests — jsdom ships no EventSource, so
// SSE consumers (`openBusStream`) silently no-op there. This shim reads the REAL gateway stream
// over fetch and dispatches `message` events with the same surface the browser API exposes (the
// subset our code uses: constructor(url), addEventListener, close). It is a browser-API polyfill
// against the real backend — NOT a fake backend (rule 9; same spirit as the rect-stub).
//
// Deny behavior mirrors the real EventSource: a non-2xx response (401/403 from the stream route)
// produces no events — consumers degrade silently, which is exactly the contract under test.

/** The `message` listener shape (the only event type the gateway emits). */
type Listener = (e: MessageEvent) => void;

class FetchEventSource {
  readonly url: string;
  private readonly ctrl = new AbortController();
  private readonly listeners = new Map<string, Set<Listener>>();

  constructor(url: string) {
    this.url = url;
    void this.pump();
  }

  addEventListener(type: string, fn: Listener): void {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(fn);
  }

  removeEventListener(type: string, fn: Listener): void {
    this.listeners.get(type)?.delete(fn);
  }

  close(): void {
    this.ctrl.abort();
  }

  private dispatch(type: string, data: string): void {
    for (const fn of this.listeners.get(type) ?? []) fn({ data } as MessageEvent);
  }

  private async pump(): Promise<void> {
    let res: Response;
    try {
      res = await fetch(this.url, {
        signal: this.ctrl.signal,
        headers: { accept: "text/event-stream" },
      });
    } catch {
      return; // aborted/unreachable — silent, like a browser EventSource error state
    }
    if (!res.ok || !res.body) return; // 401/403 deny — no events, consumer degrades
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) return;
        buffer += decoder.decode(value, { stream: true });
        let idx: number;
        while ((idx = buffer.indexOf("\n\n")) >= 0) {
          const frame = buffer.slice(0, idx);
          buffer = buffer.slice(idx + 2);
          let event = "message";
          const datas: string[] = [];
          for (const line of frame.split("\n")) {
            if (line.startsWith("event:")) event = line.slice(6).trim();
            else if (line.startsWith("data:")) datas.push(line.slice(5).trimStart());
          }
          if (datas.length > 0) this.dispatch(event, datas.join("\n"));
        }
      }
    } catch {
      // aborted mid-read (close()) — done
    }
  }
}

/** Install the shim as `globalThis.EventSource`; returns an uninstaller for `afterAll`. */
export function installEventSourceShim(): () => void {
  const g = globalThis as { EventSource?: unknown };
  const prev = g.EventSource;
  g.EventSource = FetchEventSource;
  return () => {
    g.EventSource = prev;
  };
}
