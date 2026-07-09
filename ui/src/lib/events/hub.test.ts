// The client event hub's multiplexing invariants (unified-event-stream scope §3, part 4), the whole
// point of the fix: ONE EventSource across N subscribers, refcounted subject dedupe, refcount-zero
// unsubscribe, and re-subscribe-all on reconnect. These are hub-internal contracts, driven here with a
// counting EventSource stub (NOT a fake backend — it's the browser transport jsdom lacks, the same kind
// of polyfill `setup-gateway.ts` installs for ResizeObserver). The end-to-end path against the REAL
// node is proven separately in `hub.gateway.test.ts`.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { eventHub } from "./hub";

// ── A minimal EventSource stub that records every instance + its listeners, and lets a test push frames.
let instances: FakeEventSource[] = [];
const controlCalls: Array<{ op: string; subject: string; sid: string }> = [];

class FakeEventSource {
  static readonly instances = instances;
  url: string;
  listeners = new Map<string, Array<(e: MessageEvent) => void>>();
  closed = false;
  constructor(url: string) {
    this.url = url;
    instances.push(this);
  }
  addEventListener(type: string, fn: (e: MessageEvent) => void) {
    const list = this.listeners.get(type) ?? [];
    list.push(fn);
    this.listeners.set(type, list);
  }
  close() {
    this.closed = true;
  }
  /** Deliver one SSE frame to this connection's listeners. */
  emit(type: string, data: unknown) {
    for (const fn of this.listeners.get(type) ?? []) {
      fn({ data: JSON.stringify(data) } as MessageEvent);
    }
  }
  /** Deliver a `hello` frame (mints the sid, triggers subscribe flush). */
  hello(sid: string) {
    this.emit("hello", { sid });
  }
  /** Deliver a `mux` frame wrapping a subject payload. */
  mux(sub: string, event: string, data: unknown) {
    this.emit("mux", { sub, event, data });
  }
}

beforeEach(() => {
  instances = [];
  (FakeEventSource as unknown as { instances: FakeEventSource[] }).instances = instances;
  controlCalls.length = 0;
  // A real gateway URL so the hub is enabled; the fetch below is stubbed so no network happens.
  vi.stubEnv("VITE_GATEWAY_URL", "http://test.local");
  vi.stubGlobal("EventSource", FakeEventSource);
  // Record the subscribe/unsubscribe control POSTs (and resolve them ok).
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      const m = /\/events\/([^/]+)\/(subscribe|unsubscribe)$/.exec(url);
      if (m) {
        const body = JSON.parse((init?.body as string) ?? "{}");
        controlCalls.push({ op: m[2], subject: body.subject, sid: m[1] });
      }
      return { ok: true, status: 200, json: async () => ({ ok: true }) } as Response;
    }),
  );
  eventHub.reset();
});

afterEach(() => {
  eventHub.reset();
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
});

/** Flush microtasks so the hub's async control POSTs settle. */
const tick = () => new Promise((r) => setTimeout(r, 0));

describe("event hub multiplexing", () => {
  it("holds exactly ONE EventSource across N subscribers on different subjects", async () => {
    eventHub.subscribeSubject("run:job-1", () => {});
    eventHub.subscribeSubject("channel:general", () => {});
    eventHub.subscribeSubject("series:cpu", () => {});
    expect(instances).toHaveLength(1);
    expect(eventHub.connectionCount()).toBe(1);

    // Once the sid arrives, each distinct subject is declared to the server exactly once.
    instances[0].hello("sid-1");
    await tick();
    expect(eventHub.subjectCount()).toBe(3);
    const subs = controlCalls.filter((c) => c.op === "subscribe").map((c) => c.subject).sort();
    expect(subs).toEqual(["channel:general", "run:job-1", "series:cpu"]);
  });

  it("dedupes N subscribers on ONE subject to a single server subscription, fanning frames to all", async () => {
    const seenA: string[] = [];
    const seenB: string[] = [];
    eventHub.subscribeSubject("series:cpu", (f) => seenA.push(f.data));
    eventHub.subscribeSubject("series:cpu", (f) => seenB.push(f.data));
    instances[0].hello("sid-1");
    await tick();

    // Two subscribers, ONE EventSource, ONE server subscription.
    expect(instances).toHaveLength(1);
    expect(eventHub.subjectCount()).toBe(1);
    expect(controlCalls.filter((c) => c.op === "subscribe" && c.subject === "series:cpu")).toHaveLength(1);

    // A frame for the subject reaches BOTH subscribers.
    instances[0].mux("series:cpu", "sample", { seq: 1, value: 42 });
    expect(seenA).toHaveLength(1);
    expect(seenB).toHaveLength(1);
    expect(JSON.parse(seenA[0])).toEqual({ seq: 1, value: 42 });
  });

  it("releases the server subscription only when the LAST subscriber unsubscribes (refcount)", async () => {
    const unsubA = eventHub.subscribeSubject("series:cpu", () => {});
    const unsubB = eventHub.subscribeSubject("series:cpu", () => {});
    instances[0].hello("sid-1");
    await tick();
    expect(eventHub.subjectCount()).toBe(1);

    // First unsubscribe: refcount 2→1, no server unsubscribe yet.
    unsubA();
    await tick();
    expect(eventHub.subjectCount()).toBe(1);
    expect(controlCalls.filter((c) => c.op === "unsubscribe")).toHaveLength(0);

    // Last unsubscribe: refcount 1→0, the server subscription is released.
    unsubB();
    await tick();
    expect(eventHub.subjectCount()).toBe(0);
    expect(controlCalls.filter((c) => c.op === "unsubscribe" && c.subject === "series:cpu")).toHaveLength(1);
  });

  it("routes a mux frame only to its own subject's listeners", () => {
    const runFrames: string[] = [];
    const seriesFrames: string[] = [];
    eventHub.subscribeSubject("run:job-1", (f) => runFrames.push(f.event));
    eventHub.subscribeSubject("series:cpu", (f) => seriesFrames.push(f.event));
    instances[0].hello("sid-1");

    instances[0].mux("run:job-1", "run", { type: "text-delta" });
    instances[0].mux("series:cpu", "sample", { seq: 1 });
    expect(runFrames).toEqual(["run"]);
    expect(seriesFrames).toEqual(["sample"]);
  });

  it("re-subscribes the whole live set on reconnect (a fresh hello mints a new sid)", async () => {
    eventHub.subscribeSubject("run:job-1", () => {});
    eventHub.subscribeSubject("series:cpu", () => {});
    instances[0].hello("sid-1");
    await tick();
    expect(controlCalls.filter((c) => c.op === "subscribe" && c.sid === "sid-1")).toHaveLength(2);

    // EventSource auto-reconnects and the server mints a NEW sid; the hub re-declares every live subject
    // against it (each re-runs its snapshot catch-up server-side).
    instances[0].hello("sid-2");
    await tick();
    const reSubs = controlCalls.filter((c) => c.op === "subscribe" && c.sid === "sid-2").map((c) => c.subject).sort();
    expect(reSubs).toEqual(["run:job-1", "series:cpu"]);
    // Still ONE EventSource — reconnect is EventSource's own, not a new hub connection.
    expect(instances).toHaveLength(1);
  });
});
