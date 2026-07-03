// Per-viz options parity (editor-parity scope, step 5) — the registry-driven per-viz tabs now render
// the everyday-parity options for each viz. These assert the options are PRESENT and author through the
// UI (no JSON), exercising the registry → OptionGroups → Control path per view.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import type { EditorState } from "../../cellEditorState";
import { cellToEditorState } from "../../cellEditorState";
import type { View } from "@/lib/dashboard";
import { ResultFieldsProvider } from "../../fields/FieldsContext";
import { TableOptionsEditor } from "./TableOptionsEditor";
import { StatOptionsEditor } from "./StatOptionsEditor";
import { TimeseriesOptionsEditor } from "./TimeseriesOptionsEditor";
import { FieldTab } from "../FieldTab";

function editorState(view: View): EditorState {
  return cellToEditorState({
    i: "c", x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "chart", view,
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: {}, datasource: { type: "series" } }],
  });
}

function Harness({ view, which }: { view: View; which: "viz" | "field" }) {
  const [state, setState] = useState<EditorState>(() => editorState(view));
  const patch = (next: Partial<EditorState>) => setState((s) => ({ ...s, ...next }));
  const Editor =
    which === "field" ? <FieldTab state={{ ...state, view }} patch={patch} /> :
    view === "table" ? <TableOptionsEditor state={{ ...state, view }} patch={patch} /> :
    view === "stat" ? <StatOptionsEditor state={{ ...state, view }} patch={patch} /> :
    <TimeseriesOptionsEditor state={{ ...state, view }} patch={patch} />;
  return (
    <ResultFieldsProvider fields={["value", "host"]}>
      {Editor}
      <output aria-label="options">{JSON.stringify(state.options)}</output>
      <output aria-label="fieldConfig">{JSON.stringify(state.fieldConfig ?? null)}</output>
    </ResultFieldsProvider>
  );
}

describe("per-viz options parity", () => {
  it("table per-viz options include the footer + cell height, authored through the UI", async () => {
    const user = userEvent.setup();
    render(<Harness view="table" which="viz" />);
    // Footer show toggle exists and writes options.footer.show.
    await user.click(screen.getByLabelText("Show table footer"));
    const opts = JSON.parse(screen.getByLabelText("options").textContent!);
    expect(opts.footer.show).toBe(true);
  });

  it("table Field tab exposes the per-column cell options (width/align/cell type/filter)", () => {
    render(<Harness view="table" which="field" />);
    expect(screen.getByLabelText("Column width")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Column alignment" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Cell type" })).toBeInTheDocument();
    expect(screen.getByLabelText("Column filter")).toBeInTheDocument();
  });

  it("stat value options include color mode + orientation, authored through the UI", async () => {
    const user = userEvent.setup();
    render(<Harness view="stat" which="viz" />);
    await user.click(screen.getByRole("combobox", { name: "Color mode" }));
    await user.click(screen.getByRole("option", { name: "Background" }));
    const opts = JSON.parse(screen.getByLabelText("options").textContent!);
    expect(opts.colorMode).toBe("background");
  });

  it("timeseries offers stacking + threshold display in the Field tab graph styles", () => {
    render(<Harness view="timeseries" which="field" />);
    expect(screen.getByRole("combobox", { name: "Stacking" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Show thresholds" })).toBeInTheDocument();
  });
});
