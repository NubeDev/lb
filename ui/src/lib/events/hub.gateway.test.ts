// The client event hub end to end over the REAL spawned gateway (no fake — CLAUDE §9). Proves the whole
// unified-event-stream path: the hub opens ONE `/events/stream` connection, the control POST subscribes a
// `bus:` subject on the real node, a real `POST /bus/publish` drives it, and the frame arrives back through
// the mux envelope to the subscriber — with exactly ONE connection held for N subscribers.
//
// jsdom has no `EventSource`, so we install a minimal fetch-streaming shim (the browser transport jsdom
// lacks — a polyfill, like `setup-gateway.ts`'s ResizeObserver, NOT a fake backend: every byte comes from
// the real node). It counts live instances so we can assert the singleton.

import { afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";

import { eventHub } from "./hub";
import { openBusStream } from "@/lib/dashboard/bus.stream";
import { httpInvoke } from "@/lib/ipc/http";
import { useRealGateway, signInReal } from "@/test/gateway-session";

// ── A real, minimal SSE client over fetch (jsdom lacks EventSource). Parses `event:`/`data:` frames and
//    dispatches to addEventListener handlers, exactly like the browser's EventSource. Counts instances.
let liveInstances = 0;
class FetchEventSource {
  private controller = new AbortController();
  private listeners = new Map<string, Array<(e: MessageEvent) => void>>();
  constructor(url: string) {
    liveInstances++;
    void this.run(url);
  }
  addEventListener(type: string, fn: (e: MessageEvent) => void) {
    const list = this.listeners.get(type) ?? [];
    list.push(fn);
    this.listeners.set(type, list);
  }
  close() {
    liveInstances--;
    this.controller.abort();
  }
  private dispatch(event: string, data: string) {
    for (const fn of this.listeners.get(event) ?? []) fn({ data } as MessageEvent);
  }
  private async run(url: string) {
    const res = await fetch(url, { signal: this.controller.signal });
    const reader = res.body!.getReader();
    const decoder = new TextDecoder();
    let buf = "";
    for (;;) {
      const { done, value } = await reader.read();
      if (done) return;
      buf += decoder.decode(value, { stream: true });
      // SSE frames are separated by a blank line; each has `event:` and `data:` lines.
      let idx: number;
      while ((idx = buf.indexOf("\n\n")) >= 0) {
        const raw = buf.slice(0, idx);
        buf = buf.slice(idx + 2);
        let event = "message";
        let data = "";
        for (const line of raw.split("\n")) {
          if (line.startsWith("event:")) event = line.slice(6).trim();
          else if (line.startsWith("data:")) data += line.slice(5).trim();
        }
        if (data) this.dispatch(event, data);
      }
    }
  }
}

beforeAll(() => useRealGateway());

beforeEach(() => {
  liveInstances = 0;
  // The hub guards on `EventSource` being defined; install the real fetch-streaming shim.
  (globalThis as Record<string, unknown>).EventSource = FetchEventSource as never;
  eventHub.reset();
});

afterEach(() => {
  eventHub.reset();
  delete (globalThis as Record<string, unknown>).EventSource;
});

/** Wait until `pred()` is true (polling), or throw after `ms`. */
async function waitFor(pred: () => boolean, ms = 5000): Promise<void> {
  const start = Date.now();
  while (!pred()) {
    if (Date.now() - start > ms) throw new Error("timed out waiting");
    await new Promise((r) => setTimeout(r, 25));
  }
}

let n = 0;
const nextWs = () => `hub-gw-${n++}`;

describe("event hub over the real gateway", () => {
  it("multiplexes a bus subject onto ONE connection and delivers the published frame", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const got: unknown[] = [];
    const handle = openBusStream("cooler/alerts", (p) => got.push(p));
    expect(handle).not.toBeNull();

    // Exactly one EventSource for the whole session, regardless of subjects.
    await waitFor(() => eventHub.connectionCount() === 1);
    expect(liveInstances).toBe(1);

    // Give the server subscription time to declare bus interest, then publish over the REAL route.
    await waitFor(() => eventHub.subjectCount() === 1);
    await new Promise((r) => setTimeout(r, 250));
    await httpInvoke("mcp_call", { tool: "bus.publish", args: { subject: "cooler/alerts", payload: { msg: "defrost" } } });

    await waitFor(() => got.length > 0);
    expect(got[0]).toEqual({ msg: "defrost" });
    handle!.close();
  });

  it("holds exactly ONE EventSource across TWO subscribers on the same subject (dedupe)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const a: unknown[] = [];
    const b: unknown[] = [];
    const h1 = openBusStream("room", (p) => a.push(p));
    const h2 = openBusStream("room", (p) => b.push(p));
    await waitFor(() => eventHub.connectionCount() === 1);

    // Two subscribers, ONE connection, ONE server subscription.
    expect(liveInstances).toBe(1);
    await waitFor(() => eventHub.subjectCount() === 1);

    await new Promise((r) => setTimeout(r, 250));
    await httpInvoke("mcp_call", { tool: "bus.publish", args: { subject: "room", payload: { n: 7 } } });

    await waitFor(() => a.length > 0 && b.length > 0);
    expect(a[0]).toEqual({ n: 7 });
    expect(b[0]).toEqual({ n: 7 });

    // Closing one keeps the shared subscription alive for the other (refcount).
    h1!.close();
    await new Promise((r) => setTimeout(r, 50));
    expect(eventHub.subjectCount()).toBe(1);
    h2!.close();
    await waitFor(() => eventHub.subjectCount() === 0);
  });
});
