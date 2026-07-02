// simulator.ts unit tests: channel determinism (same t → same value), plausible
// ranges, the seam contract (subscribe fires immediately; unsubscribe stops the ticker).

import { describe, expect, it, vi } from "vitest";
import { CHANNELS, createSimulator, sampleChannel } from "./simulator";

describe("sampleChannel (pure, deterministic)", () => {
  it("is deterministic: same channel + time → same value", () => {
    for (const ch of CHANNELS) {
      expect(sampleChannel(ch, 123.4)).toEqual(sampleChannel(ch, 123.4));
    }
  });

  it("stays in plausible ranges over an hour", () => {
    for (let t = 0; t < 3600; t += 37) {
      const speed = sampleChannel("ahu1.sf1.speed", t) as number;
      expect(speed).toBeGreaterThan(700);
      expect(speed).toBeLessThan(1050);
      const pos = sampleChannel("ahu1.oad.position", t) as number;
      expect(pos).toBeGreaterThanOrEqual(0);
      expect(pos).toBeLessThanOrEqual(100);
      const dp = sampleChannel("ahu1.filter.dp", t) as number;
      expect(dp).toBeGreaterThan(20);
      expect(dp).toBeLessThan(240);
      const temp = sampleChannel("zone.103.temp", t) as number;
      expect(temp).toBeGreaterThan(19);
      expect(temp).toBeLessThan(27);
    }
  });

  it("faults are rare but do occur", () => {
    let faults = 0;
    for (let t = 0; t < 900; t++) {
      if (sampleChannel("ahu1.sf1.fault", t)) faults++;
    }
    expect(faults).toBeGreaterThan(0);
    expect(faults / 900).toBeLessThan(0.15);
  });

  it("unknown channels sample to null", () => {
    expect(sampleChannel("nope.nothing", 10)).toBeNull();
  });
});

describe("createSimulator (the live seam)", () => {
  it("subscribe fires immediately with the current value, then ticks", () => {
    vi.useFakeTimers();
    let t = 100;
    const sim = createSimulator(() => t);
    const seen: unknown[] = [];
    const unsub = sim.subscribe("ahu1.sf1.speed", (v) => seen.push(v));
    expect(seen).toEqual([sampleChannel("ahu1.sf1.speed", 100)]);
    t = 130;
    vi.advanceTimersByTime(300);
    expect(seen.length).toBeGreaterThan(1);
    expect(seen.at(-1)).toEqual(sampleChannel("ahu1.sf1.speed", 130));
    unsub();
    const n = seen.length;
    vi.advanceTimersByTime(1000);
    expect(seen.length).toBe(n); // ticker stopped
    vi.useRealTimers();
  });

  it("get() samples at now(); channels() lists the catalog", () => {
    const sim = createSimulator(() => 42);
    expect(sim.get("ahu1.rat")).toEqual(sampleChannel("ahu1.rat", 42));
    expect(sim.channels()).toEqual([...CHANNELS]);
  });
});
