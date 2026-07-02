import { afterEach, describe, expect, it, vi } from "vitest";
import {
  RestError,
  newGestureId,
  setRestGestureId,
  setRestActorId,
  withGesture,
  updateNode,
  getNodeByUid,
} from "./rest";

describe("RestError.debug", () => {
  it("formats a copy-pasteable round-trip dump with request + response", () => {
    const e = new RestError(
      400,
      "target property already linked",
      "/api/v0/edge",
      "POST",
      { sourceUid: 1, targetPropUid: 2 },
      { code: "edge.linked" },
    );
    expect(e.debug).toBe(
      [
        "POST /api/v0/edge",
        "→ 400 target property already linked",
        "",
        "Request:",
        '{\n  "sourceUid": 1,\n  "targetPropUid": 2\n}',
        "",
        "Response:",
        '{\n  "code": "edge.linked"\n}',
      ].join("\n"),
    );
  });

  it("omits request/response sections when absent", () => {
    const e = new RestError(404, "not found", "/api/v0/nodes/uid/9", "GET");
    expect(e.debug).toBe("GET /api/v0/nodes/uid/9\n→ 404 not found");
  });

  it("is an Error with the message set", () => {
    const e = new RestError(500, "boom", "/x");
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toBe("boom");
    expect(e.status).toBe(500);
  });
});

describe("gesture id", () => {
  it("newGestureId is monotonic, positive, and never 0", () => {
    const a = newGestureId();
    const b = newGestureId();
    expect(a).toBeGreaterThan(0);
    expect(b).toBe(a + 1);
  });

  it("withGesture returns the body's value and runs it once", async () => {
    let calls = 0;
    const out = await withGesture(async () => { calls++; return 42; });
    expect(out).toBe(42);
    expect(calls).toBe(1);
  });

  it("withGesture propagates (and clears on) a throwing body", async () => {
    await expect(
      withGesture(async () => {
        throw new Error("boom");
      }),
    ).rejects.toThrow("boom");
  });
});

describe("request headers (undo/redo scoping)", () => {
  afterEach(() => {
    setRestActorId(null);
    setRestGestureId(null);
    vi.restoreAllMocks();
  });

  function mockFetch() {
    const calls: { url: string; headers: Record<string, string> }[] = [];
    vi.stubGlobal("fetch", (url: string, init: { headers?: Record<string, string> }) => {
      calls.push({ url, headers: (init?.headers ?? {}) as Record<string, string> });
      return Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve({ data: {} }) });
    });
    return calls;
  }

  it("stamps X-Actor-Id and X-Gesture-Id on a mutating request", async () => {
    const calls = mockFetch();
    setRestActorId(7);
    setRestGestureId(123);
    await updateNode(100, { name: "x" });
    expect(calls[0].headers["X-Actor-Id"]).toBe("7");
    expect(calls[0].headers["X-Gesture-Id"]).toBe("123");
  });

  it("does NOT stamp X-Gesture-Id on a GET read", async () => {
    const calls = mockFetch();
    setRestGestureId(123);
    await getNodeByUid(100);
    expect(calls[0].headers["X-Gesture-Id"]).toBeUndefined();
  });

  it("omits X-Gesture-Id entirely when unset (time-window coalescing)", async () => {
    const calls = mockFetch();
    setRestGestureId(null);
    await updateNode(100, { name: "x" });
    expect(calls[0].headers["X-Gesture-Id"]).toBeUndefined();
  });

  it("withGesture activates an id for writes inside and clears it after", async () => {
    const calls = mockFetch();
    await withGesture(async () => { await updateNode(100, { name: "in" }); });
    await updateNode(101, { name: "out" }); // outside the gesture
    expect(calls[0].headers["X-Gesture-Id"]).toBeDefined();
    expect(calls[1].headers["X-Gesture-Id"]).toBeUndefined();
  });
});
