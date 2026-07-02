// THE one declared fake in this package (thecrew-scope.md §reuse #2): a deterministic
// plant-value simulator behind the ValueSource seam. Allowed because there is no node
// here at all; the framework replaces this file with the bridge. Do not add others.
//
// Determinism: every value is a pure function of (channel, tSec) — `sampleChannel` is
// exported for tests. The live source just samples it on a ticker.

import type { ValueSource, Unsubscribe } from "./value-source";

/** Small stable string hash → [0, 1) — gives each channel its own phase/character. */
function hash01(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0) / 4294967296;
}

function wave(tSec: number, periodSec: number, phase01: number): number {
  return Math.sin((tSec / periodSec + phase01) * Math.PI * 2); // -1..1
}

/** The channel catalog: everything the PropertyRail's binding picker offers.
 * ahu1.* feeds the AHU demo; zone.* feeds the floor-plan demo's temp tints. */
export const CHANNELS = [
  "ahu1.sf1.running",
  "ahu1.sf1.speed",
  "ahu1.sf1.fault",
  "ahu1.oad.position",
  "ahu1.filter.dp",
  "ahu1.chwv.valve",
  "ahu1.sat",
  "ahu1.rat",
  "zone.101.temp",
  "zone.102.temp",
  "zone.103.temp",
  "zone.104.temp",
  "zone.105.temp",
  "zone.106.temp",
  "zone.101.occupied",
  "zone.102.occupied",
  "zone.103.occupied",
  "zone.104.occupied",
  "zone.105.occupied",
  "zone.106.occupied",
] as const;

/** Pure, deterministic: the value of `channel` at `tSec`. Unknown channels → null. */
export function sampleChannel(channel: string, tSec: number): unknown {
  const p = hash01(channel);
  if (channel.endsWith(".running")) return true; // the demo plant runs
  if (channel.endsWith(".fault")) {
    // a fault window ~8 s out of every 90 s, offset per channel — rare, visible
    return (tSec + p * 90) % 90 < 8;
  }
  if (channel.endsWith(".speed")) {
    return Math.round(880 + wave(tSec, 47, p) * 90 + wave(tSec, 7, p + 0.3) * 18); // rpm
  }
  if (channel.endsWith(".position") || channel.endsWith(".valve")) {
    return Math.round(50 + wave(tSec, 61, p) * 35 + wave(tSec, 11, p + 0.5) * 6); // 0-100 %
  }
  if (channel.endsWith(".dp")) {
    // filter loading: slow sawtooth 40 → 220 Pa over ~10 min, plus breathing
    const saw = ((tSec / 600 + p) % 1) * 180 + 40;
    return Math.round(saw + wave(tSec, 13, p) * 6);
  }
  if (channel.endsWith(".sat")) {
    return Math.round((13 + wave(tSec, 53, p) * 1.2) * 10) / 10; // °C supply
  }
  if (channel.endsWith(".rat")) {
    return Math.round((23.5 + wave(tSec, 71, p) * 0.8) * 10) / 10; // °C return
  }
  if (channel.endsWith(".temp")) {
    return Math.round((23 + wave(tSec, 89, p) * 2.5 + wave(tSec, 17, p + 0.7) * 0.4) * 10) / 10;
  }
  if (channel.endsWith(".occupied")) {
    return (tSec + p * 120) % 120 < 80; // occupied ~2/3 of the time
  }
  return null;
}

const TICK_MS = 250;

/** The live source: one ticker, all subscribers sampled from the pure function. */
export function createSimulator(now: () => number = () => performance.now() / 1000): ValueSource {
  const subs = new Map<string, Set<(value: unknown) => void>>();
  let timer: ReturnType<typeof setInterval> | undefined;

  function tick() {
    const t = now();
    for (const [channel, fns] of subs) {
      const v = sampleChannel(channel, t);
      for (const fn of fns) fn(v);
    }
  }

  return {
    get(channel: string): unknown {
      return sampleChannel(channel, now());
    },
    subscribe(channel: string, onValue: (value: unknown) => void): Unsubscribe {
      let set = subs.get(channel);
      if (!set) {
        set = new Set();
        subs.set(channel, set);
      }
      set.add(onValue);
      onValue(sampleChannel(channel, now())); // fires immediately, per the seam contract
      if (!timer) timer = setInterval(tick, TICK_MS);
      return () => {
        set.delete(onValue);
        if (set.size === 0) subs.delete(channel);
        if (subs.size === 0 && timer) {
          clearInterval(timer);
          timer = undefined;
        }
      };
    },
    channels(): string[] {
      return [...CHANNELS];
    },
  };
}
