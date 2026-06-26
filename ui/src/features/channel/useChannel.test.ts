// The S3 live-feed proof, at the hook level: a message arriving over the SSE stream is folded
// into the channel's items via the SAME `setItems` sink — without anyone posting locally. We
// mock `channel.stream` so the test is deterministic (no real EventSource/gateway) and capture
// the `onMessage` handler the hook registers, then fire an item through it.
//
// This is the unit counterpart to the gateway's `the_sse_stream_pushes_a_live_message` Rust
// test: that proves the server emits the event; this proves the hook consumes it into the view.

import { renderHook, act, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Item } from "@/lib/channel/channel.types";
import type { ChannelStreamHandlers } from "@/lib/channel/channel.stream";

// Capture the handlers the hook passes to the stream, and a close spy.
let captured: ChannelStreamHandlers | null = null;
const close = vi.fn();

vi.mock("@/lib/channel/channel.stream", () => ({
  openChannelStream: (_ws: string, _ch: string, handlers: ChannelStreamHandlers) => {
    captured = handlers;
    return { close };
  },
}));

// The api client: empty history, post is a no-op (this test is about the LIVE feed, not posting).
vi.mock("@/lib/channel/channel.api", () => ({
  history: vi.fn(async () => [] as Item[]),
  post: vi.fn(async () => {}),
}));

import { useChannel } from "./useChannel";

afterEach(() => {
  captured = null;
  close.mockClear();
});

describe("useChannel live SSE feed", () => {
  it("folds an item pushed over the stream into items (others' messages appear)", async () => {
    const { result } = renderHook(() =>
      useChannel("acme", "general", "user:me", () => 1),
    );

    // History loaded (empty), and the hook registered a stream handler.
    await waitFor(() => expect(result.current.loading).toBe(false));
    await waitFor(() => expect(captured).not.toBeNull());

    // A message arrives LIVE from another user — no local post.
    act(() => {
      captured!.onMessage({
        id: "x1",
        channel: "general",
        author: "user:other",
        body: "live from elsewhere",
        ts: 5,
      });
    });

    expect(result.current.items.map((i) => i.body)).toEqual(["live from elsewhere"]);
  });

  it("is idempotent — the same live id upserts, never duplicates", async () => {
    const { result } = renderHook(() =>
      useChannel("acme", "general", "user:me", () => 1),
    );
    await waitFor(() => expect(captured).not.toBeNull());

    const item: Item = {
      id: "dup",
      channel: "general",
      author: "user:other",
      body: "once",
      ts: 2,
    };
    act(() => captured!.onMessage(item));
    act(() => captured!.onMessage({ ...item, body: "once (updated)" }));

    expect(result.current.items).toHaveLength(1);
    expect(result.current.items[0].body).toBe("once (updated)");
  });
});
