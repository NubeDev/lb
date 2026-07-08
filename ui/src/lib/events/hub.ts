// The client-side **event hub** — the browser leg of the unified event stream (unified-event-stream
// scope §3, part 4). ONE `EventSource` per app session multiplexes every live feed; features subscribe
// by *subject* and the hub fans mux frames out to them. This replaces the per-feature "one EventSource
// per run/channel/series cell" architecture that saturated the browser's ~6-connection HTTP/1.1 pool
// (the "agent dock blocks navigation" defect — see
// `debugging/frontend/agent-dock-blocks-navigation-sse-pool-exhaustion.md`).
//
// The hub is a singleton (one connection for the whole tab). It:
//   - opens `GET /events/stream?token=` lazily on the first subscribe and reads `event: hello {sid}`;
//   - `subscribeSubject(subject, onFrame)` POSTs `/events/{sid}/subscribe` and returns an unsubscribe fn.
//     N callers on the SAME subject share ONE server subscription (refcounted dedupe) — closing the
//     deferred `useSeries` "one EventSource per series, fanned to N cells" follow-up for free;
//   - routes each `event: mux {sub, event, data}` frame to every listener on `sub`, handing them the
//     ORIGINAL `{event, data}` so the dedicated-route fold logic (run/channel/series) is untouched;
//   - on an `EventSource` reconnect, a fresh `hello {sid}` arrives and the hub re-subscribes the entire
//     live subject set (each subject then re-runs its snapshot catch-up server-side — same healing as a
//     per-stream reconnect today).
//
// The `*.stream.ts` openers delegate here; feature code keeps its existing callbacks (`stream/*` adapters).

import { gatewayUrl } from "@/lib/ipc/http";
import { sessionToken } from "@/lib/session/session.store";

/** One de-multiplexed frame handed to a subject listener: the ORIGINAL SSE event name + its raw JSON
 *  `data` string (exactly what the dedicated route emitted). The adapter parses `data` as it did before. */
export interface MuxFrame {
  event: string;
  data: string;
}

/** A listener on a subject — called once per frame that subject produces. */
type FrameListener = (frame: MuxFrame) => void;

/** The per-subject fan-out state: the set of listeners (refcount = size) sharing one server subscription. */
interface SubjectState {
  listeners: Set<FrameListener>;
}

/** The mux envelope shape on the wire (`event: mux` frames). */
interface MuxEnvelope {
  sub: string;
  event: string;
  data: unknown;
}

/** The singleton hub. Constructed once per module (per tab). All state is in-memory and connection-scoped. */
class EventHub {
  private es: EventSource | null = null;
  private sid: string | null = null;
  private readonly subjects = new Map<string, SubjectState>();
  /** True once we've decided there is no gateway (Tauri shell / tests without a URL) — then the hub is a
   *  no-op and every subscribe resolves to a no-op unsubscribe, exactly like the old openers returning null. */
  private disabled = false;

  /** Subscribe `subject`; `onFrame` fires for every frame it produces. Returns an unsubscribe fn that
   *  drops this listener and, at refcount zero, tells the server to release the subject. Idempotent. */
  subscribeSubject(subject: string, onFrame: FrameListener): () => void {
    if (!this.ensureConnected()) return () => {};

    let state = this.subjects.get(subject);
    const firstListener = !state;
    if (!state) {
      state = { listeners: new Set() };
      this.subjects.set(subject, state);
    }
    state.listeners.add(onFrame);

    // Only the FIRST listener on a subject opens the server subscription (refcounted dedupe). If the
    // connection isn't up yet (`hello` pending), `flushSubscribes` will POST it once `sid` arrives.
    if (firstListener && this.sid) void this.serverSubscribe(subject);

    return () => {
      const s = this.subjects.get(subject);
      if (!s) return;
      s.listeners.delete(onFrame);
      if (s.listeners.size === 0) {
        this.subjects.delete(subject);
        if (this.sid) void this.serverUnsubscribe(subject);
      }
    };
  }

  /** Open the single `EventSource` if it isn't already. Returns false when there is no gateway (the hub
   *  is then a permanent no-op — the caller degrades exactly as the old opener's `null` did). */
  private ensureConnected(): boolean {
    if (this.disabled) return false;
    if (this.es) return true;

    const base = gatewayUrl();
    if (base === "" && import.meta.env.VITE_GATEWAY_URL === undefined) {
      this.disabled = true;
      return false;
    }
    if (typeof EventSource === "undefined") {
      this.disabled = true;
      return false;
    }

    const url = `${base}/events/stream?token=${encodeURIComponent(sessionToken())}`;
    const es = new EventSource(url);
    this.es = es;

    // `hello` carries the server-minted sid. It arrives on connect AND on every reconnect — so this is
    // also the re-subscribe trigger: EventSource auto-reconnects, the server mints a fresh sid, and we
    // re-declare the whole live subject set (each re-runs its snapshot catch-up server-side).
    es.addEventListener("hello", (e) => {
      try {
        const { sid } = JSON.parse((e as MessageEvent).data) as { sid: string };
        this.sid = sid;
        this.flushSubscribes();
      } catch {
        // a malformed hello never breaks the stream
      }
    });

    // Every live frame rides as `event: mux`. De-multiplex to the subject's listeners, handing them the
    // ORIGINAL {event, data} so their fold logic is unchanged.
    es.addEventListener("mux", (e) => {
      try {
        const env = JSON.parse((e as MessageEvent).data) as MuxEnvelope;
        const state = this.subjects.get(env.sub);
        if (!state) return;
        // `data` was embedded verbatim as JSON in the envelope; re-serialize it to the string shape the
        // adapters expect (they `JSON.parse` it, mirroring the dedicated route's `e.data`).
        const frame: MuxFrame = { event: env.event, data: JSON.stringify(env.data) };
        for (const listener of state.listeners) listener(frame);
      } catch {
        // a malformed frame never breaks the stream
      }
    });

    return true;
  }

  /** Re-declare every currently-subscribed subject to the (possibly new) sid — on first connect and on
   *  every reconnect. */
  private flushSubscribes(): void {
    for (const subject of this.subjects.keys()) void this.serverSubscribe(subject);
  }

  /** POST `/events/{sid}/subscribe`. A gate DENY is not an HTTP error here — it arrives as an opaque
   *  `error` mux frame on the stream (the connection lives on). A network error is swallowed; the next
   *  reconnect's `flushSubscribes` retries. */
  private async serverSubscribe(subject: string): Promise<void> {
    await this.control("subscribe", subject);
  }

  private async serverUnsubscribe(subject: string): Promise<void> {
    await this.control("unsubscribe", subject);
  }

  private async control(op: "subscribe" | "unsubscribe", subject: string): Promise<void> {
    if (!this.sid) return;
    const base = gatewayUrl();
    try {
      await fetch(`${base}/events/${encodeURIComponent(this.sid)}/${op}`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          authorization: `Bearer ${sessionToken()}`,
        },
        body: JSON.stringify({ subject }),
      });
    } catch {
      // best-effort; reconnect re-subscribes. Unsubscribe failures are harmless (server drops the
      // connection's subjects on close anyway).
    }
  }

  /** Test-only: how many `EventSource`s the hub has open (must be ≤ 1 across N subscribers). */
  connectionCount(): number {
    return this.es ? 1 : 0;
  }

  /** Test-only: the count of distinct live subjects (server subscriptions), for the dedupe assertion. */
  subjectCount(): number {
    return this.subjects.size;
  }

  /** Test-only: reset the singleton between test cases (close the connection, drop all state). */
  reset(): void {
    this.es?.close();
    this.es = null;
    this.sid = null;
    this.subjects.clear();
    this.disabled = false;
  }
}

/** The process-wide singleton every `*.stream.ts` opener delegates to. */
export const eventHub = new EventHub();

/** Is a live stream possible here? False in the Tauri shell / tests with no gateway URL (or no
 *  `EventSource`) — the openers return `null` then, preserving their long-standing "no feed, degrade
 *  gracefully" contract (callers key off `null`). Mirrors each old opener's guard exactly. */
export function liveStreamAvailable(): boolean {
  const base = gatewayUrl();
  if (base === "" && import.meta.env.VITE_GATEWAY_URL === undefined) return false;
  if (typeof EventSource === "undefined") return false;
  return true;
}
