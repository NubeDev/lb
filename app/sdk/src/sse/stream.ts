// The live-feed transport: open a gateway SSE route over a streaming `fetch` and dispatch frames by
// event name. RN's stock fetch has no body streaming, so the shell installs the fetch-streams
// polyfill (`react-native-fetch-api` + web-streams over the networking bridge) — recorded in the
// app-shell session doc; in Node tests undici streams natively. The token rides as `?token=` (the
// gateway's SSE routes authenticate by query param — an SSE client cannot set headers), verified
// identically to a bearer (the hard wall holds).
//
// Reconnect story: the gateway emits no `id:` fields, so resume is NOT Last-Event-ID replay — it is
// reconnect + a durable **history catch-up read** (channels are durable-inbox-backed; the scope's
// transport decision). On every (re)open the stream fires `onOpen`, and the caller re-reads history
// there to close any gap. That makes a killed stream lose nothing.

import { fetchOf, type GatewayConfig } from "../client/config";
import { createSseParser } from "./parse";

// Structural stand-ins for the streaming globals, so the sdk compiles without the DOM lib (the RN
// shell's tsconfig has no DOM; the runtime shapes come from its polyfills / undici in tests).
type ByteReader = { read(): Promise<{ done: boolean; value?: Uint8Array }> };
type StreamingBody = { getReader(): ByteReader } | null | undefined;
type Utf8Decoder = { decode(input?: Uint8Array, options?: { stream?: boolean }): string };
const TextDecoderCtor = (globalThis as { TextDecoder?: new () => Utf8Decoder }).TextDecoder;

export interface SseHandlers {
  /** Dispatched per frame by SSE event name (`message`, `delete`, `presence`, …). */
  onEvent: (event: string, data: string) => void;
  /** Fired on every successful (re)connect — do the history catch-up read here. */
  onOpen?: () => void;
  /** Fired when a connection attempt or an open stream fails (before the retry sleep). */
  onError?: (err: unknown) => void;
}

/** A live stream handle — `close()` stops it (and any pending reconnect). */
export interface SseStream {
  close: () => void;
}

const RETRY_MS = 1000;

/** Open `path` (e.g. `/channels/general/stream`) as a live SSE feed with auto-reconnect. */
export function openSse(config: GatewayConfig, path: string, handlers: SseHandlers): SseStream {
  let closed = false;
  let abort: AbortController | null = null;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;

  async function connect(): Promise<void> {
    if (closed) return;
    abort = new AbortController();
    const sep = path.includes("?") ? "&" : "?";
    const url = `${config.baseUrl}${path}${sep}token=${encodeURIComponent(config.getToken())}`;
    try {
      const res = await fetchOf(config)(url, {
        headers: { accept: "text/event-stream" },
        signal: abort.signal,
      });
      const body = (res as { body?: StreamingBody }).body;
      if (!res.ok || !body) throw new Error(`stream failed (${res.status})`);
      if (!TextDecoderCtor) throw new Error("no TextDecoder — load the shell polyfills first");
      handlers.onOpen?.();
      const reader = body.getReader();
      const decoder = new TextDecoderCtor();
      const parser = createSseParser();
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        for (const frame of parser.feed(decoder.decode(value, { stream: true }))) {
          handlers.onEvent(frame.event, frame.data);
        }
      }
      // Server closed the stream — treat as a drop and reconnect.
      scheduleRetry(new Error("stream ended"));
    } catch (err) {
      if (!closed) scheduleRetry(err);
    }
  }

  function scheduleRetry(err: unknown): void {
    if (closed) return;
    handlers.onError?.(err);
    retryTimer = setTimeout(() => void connect(), RETRY_MS);
  }

  void connect();

  return {
    close() {
      closed = true;
      if (retryTimer) clearTimeout(retryTimer);
      abort?.abort();
    },
  };
}
