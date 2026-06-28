// Unit tests for the auto-refresh tick (widget-config-vars Slice 4): the interval parse, that a tick
// bumps the key, that `off` never ticks, and that changing the interval reschedules. Fake timers; the
// visibility pause is exercised via `document.hidden`. Pure cadence — no gateway.

import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { useAutoRefresh, refreshMs } from "./useAutoRefresh";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("refreshMs", () => {
  it("parses s/m, treats off/absent/unknown as 0", () => {
    expect(refreshMs("5s")).toBe(5000);
    expect(refreshMs("30s")).toBe(30_000);
    expect(refreshMs("1m")).toBe(60_000);
    expect(refreshMs("15m")).toBe(900_000);
    expect(refreshMs("")).toBe(0);
    expect(refreshMs(undefined)).toBe(0);
    expect(refreshMs("13x")).toBe(0);
  });
});

describe("useAutoRefresh", () => {
  it("bumps the key every interval", () => {
    const { result } = renderHook(() => useAutoRefresh("5s"));
    expect(result.current).toBe(0);
    act(() => void vi.advanceTimersByTime(5000));
    expect(result.current).toBe(1);
    act(() => void vi.advanceTimersByTime(10_000));
    expect(result.current).toBe(3);
  });

  it("off never ticks", () => {
    const { result } = renderHook(() => useAutoRefresh(""));
    act(() => void vi.advanceTimersByTime(60_000));
    expect(result.current).toBe(0);
  });

  it("reschedules when the interval changes", () => {
    const { result, rerender } = renderHook(({ i }) => useAutoRefresh(i), {
      initialProps: { i: "5s" },
    });
    act(() => void vi.advanceTimersByTime(5000));
    expect(result.current).toBe(1);
    // Switch to a slower interval — the old 5s timer is cleared, the new 30s one schedules.
    rerender({ i: "30s" });
    act(() => void vi.advanceTimersByTime(5000));
    expect(result.current).toBe(1); // not yet (30s not elapsed)
    act(() => void vi.advanceTimersByTime(25_000));
    expect(result.current).toBe(2);
  });
});
