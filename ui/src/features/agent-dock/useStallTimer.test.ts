// Unit tests for the stall-timer state machine (agent-dock scope) — the pure `computeStall` reducer.

import { describe, expect, it } from "vitest";

import { computeStall, STALL_AFTER_MS } from "./useStallTimer";

describe("computeStall", () => {
  it("is inert before a run starts / when inactive", () => {
    expect(computeStall(null, null, true, 5_000)).toEqual({ elapsedSec: 0, stalled: false });
    expect(computeStall(1_000, 1_000, false, 5_000)).toEqual({ elapsedSec: 0, stalled: false });
  });

  it("reports whole-second elapsed from the run start", () => {
    expect(computeStall(1_000, 1_000, true, 4_500).elapsedSec).toBe(3);
  });

  it("is NOT stalled while events keep arriving", () => {
    // Last event at 10s, now 12s (< threshold) → not stalled.
    expect(computeStall(0, 10_000, true, 12_000).stalled).toBe(false);
  });

  it("flips to stalled after the threshold of silence", () => {
    // Last event at 0, now = threshold → stalled.
    expect(computeStall(0, 0, true, STALL_AFTER_MS).stalled).toBe(true);
  });

  it("uses the run start as the baseline when no event has arrived yet", () => {
    // No events; started at 0; now just past threshold → stalled (nothing came).
    expect(computeStall(0, null, true, STALL_AFTER_MS + 1).stalled).toBe(true);
  });
});
