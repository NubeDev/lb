// Unit tests for the auto-format hook (rules-editor-ux): it reads the persisted flag on mount,
// `toggle` flips it AND persists through to localStorage (so a reload / a second mount sees the new
// value). jsdom supplies a real localStorage — we clear the key between tests.

import { describe, expect, it, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { useAutoFormat } from "./useAutoFormat";
import { AUTO_FORMAT_KEY } from "./autoFormatPref";

beforeEach(() => localStorage.removeItem(AUTO_FORMAT_KEY));

describe("useAutoFormat", () => {
  it("starts disabled when nothing is persisted", () => {
    const { result } = renderHook(() => useAutoFormat());
    expect(result.current.enabled).toBe(false);
  });

  it("toggle enables and persists", () => {
    const { result } = renderHook(() => useAutoFormat());
    act(() => result.current.toggle());
    expect(result.current.enabled).toBe(true);
    expect(localStorage.getItem(AUTO_FORMAT_KEY)).toBe("1");
  });

  it("reads the persisted flag on a fresh mount", () => {
    localStorage.setItem(AUTO_FORMAT_KEY, "1");
    const { result } = renderHook(() => useAutoFormat());
    expect(result.current.enabled).toBe(true);
  });

  it("toggle off clears it back to disabled", () => {
    localStorage.setItem(AUTO_FORMAT_KEY, "1");
    const { result } = renderHook(() => useAutoFormat());
    act(() => result.current.toggle());
    expect(result.current.enabled).toBe(false);
    expect(localStorage.getItem(AUTO_FORMAT_KEY)).toBe("0");
  });
});
