// Unit tests for the run-feed reducer (channels-agent scope) — folding a run's `RunEvent`s into the
// compact shape the AgentCard renders. Pure function, so no gateway/EventSource needed.

import { describe, expect, it } from "vitest";

import { fold, type RunFeed } from "./useRunFeed";
import type { RunEvent } from "@/lib/channel/run.stream";

const EMPTY: RunFeed = { live: true, text: "", reasoning: "", tools: [], finished: false };

function run(events: RunEvent[]): RunFeed {
  return events.reduce(fold, EMPTY);
}

describe("fold", () => {
  it("accumulates text deltas in order", () => {
    const f = run([
      { type: "text-delta", turn: 0, text: "Hello " },
      { type: "text-delta", turn: 0, text: "world" },
    ]);
    expect(f.text).toBe("Hello world");
  });

  it("tracks tool calls and updates them in place on result", () => {
    const f = run([
      { type: "tool-call-start", id: "t1", name: "exec: ls" },
      { type: "tool-call-start", id: "t2", name: "federation.query" },
      { type: "tool-call-result", id: "t1", ok: "file.txt", err: null },
      { type: "tool-call-result", id: "t2", ok: null, err: "denied" },
    ]);
    expect(f.tools.map((t) => t.name)).toEqual(["exec: ls", "federation.query"]);
    expect(f.tools[0].ok).toBe("file.txt");
    expect(f.tools[1].err).toBe("denied");
  });

  it("does not duplicate a tool row when a start id repeats", () => {
    const f = run([
      { type: "tool-call-start", id: "t1", name: "x" },
      { type: "tool-call-start", id: "t1", name: "x" },
    ]);
    expect(f.tools).toHaveLength(1);
  });

  it("falls back to the run-finish answer when no text deltas streamed (per-step transport)", () => {
    const f = run([
      { type: "run-start", goal: "g" },
      { type: "step-start", turn: 0 },
      { type: "run-finish", outcome: "done", answer: "PONG" },
    ]);
    expect(f.finished).toBe(true);
    expect(f.text).toBe("PONG");
  });

  it("keeps streamed text over the finish answer when both are present", () => {
    const f = run([
      { type: "text-delta", turn: 0, text: "streamed" },
      { type: "run-finish", outcome: "done", answer: "final" },
    ]);
    expect(f.text).toBe("streamed");
  });

  it("shows the latest reasoning line", () => {
    const f = run([
      { type: "reasoning-delta", turn: 0, text: "thinking A" },
      { type: "reasoning-delta", turn: 0, text: "thinking B" },
    ]);
    expect(f.reasoning).toBe("thinking B");
  });
});
