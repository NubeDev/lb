// DataInspector — the Panel Inspect drawer (data-studio-ux). It's a pure view over the SourceState's
// `meta.inspect`: Data (rows grid), JSON (raw + shaped frames), Query (the resolved request that ran). It
// fetches nothing — these tests hand it a state and assert what each tab shows.

import { describe, it, expect } from "vitest";
import { render as rtlRender, screen, fireEvent } from "@testing-library/react";
import type { ReactElement } from "react";

import { ThemeProvider } from "@/lib/theme";
import { DataInspector } from "./DataInspector";
import type { SourceState } from "@/features/dashboard/builder/useSource";

// The Sheet drawer pulls the themed portal; wrap every render in the ThemeProvider (the shell always has one).
const render = (ui: ReactElement) => rtlRender(<ThemeProvider>{ui}</ThemeProvider>);

const state: SourceState = {
  rows: [
    { ts: 1, value: 10 },
    { ts: 2, value: 20 },
  ],
  latest: null,
  loading: false,
  denied: false,
  meta: {
    frames: 1,
    ms: 42,
    source: "shaped",
    inspect: {
      request: { sources: [{ tool: "store.query", args: { sql: "SELECT ts, value FROM series" } }] },
      rawFrames: [{ refId: "A", fields: [{ name: "value", values: [10, 20] }], length: 2 }],
      shapedFrames: [{ refId: "A", fields: [{ name: "value", values: [20, 10] }], length: 2 }],
    },
  },
};

describe("DataInspector", () => {
  it("shows the rows as a grid on the Data tab (default)", () => {
    render(<DataInspector open onOpenChange={() => {}} state={state} />);
    // Column headers = the row keys.
    expect(screen.getByRole("columnheader", { name: "ts" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "value" })).toBeTruthy();
    // The row values render.
    expect(screen.getAllByText("20").length).toBeGreaterThan(0);
  });

  it("shows the resolved request (the real SQL) on the Query tab", () => {
    render(<DataInspector open onOpenChange={() => {}} state={state} />);
    fireEvent.click(screen.getByRole("tab", { name: "Query" }));
    expect(screen.getByText(/SELECT ts, value FROM series/)).toBeTruthy();
  });

  it("shows raw AND shaped frames on the JSON tab", () => {
    render(<DataInspector open onOpenChange={() => {}} state={state} />);
    fireEvent.click(screen.getByRole("tab", { name: "JSON" }));
    expect(screen.getByText(/Raw frames/i)).toBeTruthy();
    expect(screen.getByText(/Shaped frames/i)).toBeTruthy();
  });

  it("surfaces the error text in the header when the query failed", () => {
    const errored: SourceState = {
      rows: [],
      latest: null,
      loading: false,
      denied: true,
      meta: { error: "Denied", source: "fetch", inspect: { request: { sources: [] } } },
    };
    render(<DataInspector open onOpenChange={() => {}} state={errored} />);
    expect(screen.getByText(/Denied/)).toBeTruthy();
  });
});
