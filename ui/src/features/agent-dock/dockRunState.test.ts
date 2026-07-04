// Unit tests for the dock run-state fold (agent-dock scope) — the six-state feedback contract, driven
// over REAL folded `RunFeed` shapes (the same shape `fold` produces from real `RunEvent`s). This is the
// state machine the scope asks to assert: Sent → Working → Answering → Stalled → Done → Error.

import { describe, expect, it } from "vitest";

import type { RunFeed } from "@/features/channel/useRunFeed";
import { fold } from "@/features/channel/useRunFeed";
import { dockRunPhase, isTerminalPhase } from "./dockRunState";

const EMPTY: RunFeed = { live: true, text: "", reasoning: "", tools: [], finished: false };

describe("dockRunPhase", () => {
  it("Sent — a live feed with nothing observed yet", () => {
    expect(dockRunPhase({ feed: EMPTY, hasResult: false, hasError: false, stalled: false })).toBe("sent");
  });

  it("Working — a reasoning-delta arrived (folded from a real RunEvent)", () => {
    const feed = fold(EMPTY, { type: "reasoning-delta", turn: 1, text: "checking metrics" });
    expect(dockRunPhase({ feed, hasResult: false, hasError: false, stalled: false })).toBe("working");
  });

  it("Working — a tool-call started", () => {
    const feed = fold(EMPTY, { type: "tool-call-start", id: "c1", name: "series.query" });
    expect(dockRunPhase({ feed, hasResult: false, hasError: false, stalled: false })).toBe("working");
  });

  it("Answering — a text-delta appended", () => {
    const feed = fold(EMPTY, { type: "text-delta", turn: 1, text: "throughput dipped because" });
    expect(dockRunPhase({ feed, hasResult: false, hasError: false, stalled: false })).toBe("answering");
  });

  it("Stalled — live and quiet, no text yet (a hint, not an error)", () => {
    const feed = fold(EMPTY, { type: "reasoning-delta", turn: 1, text: "thinking" });
    expect(dockRunPhase({ feed, hasResult: false, hasError: false, stalled: true })).toBe("stalled");
  });

  it("Answering overrides Stalled once text is streaming", () => {
    const feed = fold(EMPTY, { type: "text-delta", turn: 1, text: "here" });
    expect(dockRunPhase({ feed, hasResult: false, hasError: false, stalled: true })).toBe("answering");
  });

  it("Done — a durable agent_result reconciled (terminal wins over any live state)", () => {
    const feed = fold(EMPTY, { type: "text-delta", turn: 1, text: "partial" });
    expect(dockRunPhase({ feed, hasResult: true, hasError: false, stalled: false })).toBe("done");
  });

  it("Error — a durable agent_error / transport failure (wins over Done)", () => {
    expect(dockRunPhase({ feed: EMPTY, hasResult: true, hasError: true, stalled: false })).toBe("error");
  });

  it("a full real-frame progression folds Sent → Working → Answering → Done", () => {
    let feed = EMPTY;
    expect(dockRunPhase({ feed, hasResult: false, hasError: false, stalled: false })).toBe("sent");
    feed = fold(feed, { type: "run-start", goal: "why?" });
    feed = fold(feed, { type: "reasoning-delta", turn: 1, text: "looking" });
    expect(dockRunPhase({ feed, hasResult: false, hasError: false, stalled: false })).toBe("working");
    feed = fold(feed, { type: "text-delta", turn: 1, text: "because…" });
    expect(dockRunPhase({ feed, hasResult: false, hasError: false, stalled: false })).toBe("answering");
    feed = fold(feed, { type: "run-finish", outcome: "ok", answer: "because…" });
    // run-finish keeps the streamed text (still "answering" on the live feed) UNTIL the DURABLE
    // agent_result lands — only then does the phase become the terminal Done (the message of record).
    expect(dockRunPhase({ feed, hasResult: false, hasError: false, stalled: false })).toBe("answering");
    expect(dockRunPhase({ feed, hasResult: true, hasError: false, stalled: false })).toBe("done");
  });
});

describe("isTerminalPhase", () => {
  it("done and error are terminal; the rest are not", () => {
    expect(isTerminalPhase("done")).toBe(true);
    expect(isTerminalPhase("error")).toBe(true);
    expect(isTerminalPhase("working")).toBe(false);
    expect(isTerminalPhase("stalled")).toBe(false);
  });
});
