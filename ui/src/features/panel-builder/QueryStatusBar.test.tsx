// QueryStatusBar — the honest-loop feedback line (data-studio-ux). It must distinguish the states the
// old silent "no data yet" collapsed: never-ran (no source) vs. ran-and-empty vs. running vs. error, and
// surface rows/frames/duration on success plus the "shaped from cached data" provenance chip.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { QueryStatusBar } from "./QueryStatusBar";
import type { SourceState } from "@/features/dashboard/builder/useSource";

const base: SourceState = { rows: [], latest: null, loading: false, denied: false };

describe("QueryStatusBar", () => {
  it("says what's missing when no source is selected (never ran)", () => {
    render(<QueryStatusBar state={base} hasTarget={false} />);
    expect(screen.getByText(/No source selected/i)).toBeTruthy();
  });

  it("distinguishes ran-and-empty from never-ran, naming the range", () => {
    render(<QueryStatusBar state={base} hasTarget rangeLabel="last 6h" />);
    expect(screen.getByText(/returned 0 rows for last 6h/i)).toBeTruthy();
  });

  it("shows the error text inline on a denied/failed query", () => {
    const state: SourceState = { ...base, denied: true, meta: { error: "Denied" } };
    render(<QueryStatusBar state={state} hasTarget />);
    expect(screen.getByText(/Query error — Denied/i)).toBeTruthy();
  });

  it("shows rows + frames + duration on success", () => {
    const state: SourceState = {
      rows: [{ a: 1 }, { a: 2 }],
      latest: null,
      loading: false,
      denied: false,
      meta: { frames: 2, ms: 42, source: "fetch" },
    };
    render(<QueryStatusBar state={state} hasTarget />);
    expect(screen.getByText(/2 rows · 2 frames · 42 ms/i)).toBeTruthy();
  });

  it("shows the 'shaped from cached data' chip when a reshape did not re-fetch", () => {
    const state: SourceState = {
      rows: [{ a: 1 }],
      latest: null,
      loading: false,
      denied: false,
      meta: { frames: 1, source: "shaped" },
    };
    render(<QueryStatusBar state={state} hasTarget />);
    expect(screen.getByText(/shaped from cached data/i)).toBeTruthy();
  });

  it("shows a running state while loading", () => {
    render(<QueryStatusBar state={{ ...base, loading: true }} hasTarget />);
    expect(screen.getByText(/Running query/i)).toBeTruthy();
  });

  it("shows the frozen chip when frozen", () => {
    render(<QueryStatusBar state={{ ...base, rows: [{ a: 1 }] }} hasTarget frozen />);
    expect(screen.getByText("frozen")).toBeTruthy();
  });
});
