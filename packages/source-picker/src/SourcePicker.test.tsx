// Component tests: the grouped <select> renders every non-empty group and fires onSelect with the
// chosen entry's selection (or null when cleared).

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SourcePicker } from "./SourcePicker";
import { buildSourceEntries } from "./sourcePicker";
import type { ExtRow } from "./types";

const ext: ExtRow = {
  ext: "thecrew",
  enabled: true,
  widgets: [{ entry: "r.js", label: "Scene", icon: "box", scope: ["assets.get_doc"] }],
};
const entries = buildSourceEntries({ series: ["ahu1.speed"], extensions: [ext] });

describe("SourcePicker", () => {
  it("renders a group per non-empty origin and its options", () => {
    render(<SourcePicker entries={entries} onSelect={() => {}} />);
    expect((screen.getByRole("option", { name: "ahu1.speed" }) as HTMLOptionElement).closest("optgroup")?.label).toBe("Series");
    expect((screen.getByRole("option", { name: "thecrew · Scene" }) as HTMLOptionElement).closest("optgroup")?.label).toBe("Extension widgets");
  });

  it("fires onSelect with the chosen entry's selection", () => {
    const onSelect = vi.fn();
    render(<SourcePicker entries={entries} onSelect={onSelect} />);
    fireEvent.change(screen.getByLabelText("source"), { target: { value: "series:ahu1.speed" } });
    expect(onSelect).toHaveBeenCalledWith({
      id: "series:ahu1.speed",
      source: { tool: "series.read", args: { series: "ahu1.speed" } },
      action: undefined,
      viewKey: undefined,
    });
  });

  it("fires onSelect(null) when cleared to the placeholder", () => {
    const onSelect = vi.fn();
    render(<SourcePicker entries={entries} value="series:ahu1.speed" onSelect={onSelect} />);
    fireEvent.change(screen.getByLabelText("source"), { target: { value: "" } });
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it("shows a loading placeholder while entries load", () => {
    render(<SourcePicker entries={[]} loading onSelect={() => {}} />);
    expect(screen.getByText("loading sources…")).toBeTruthy();
  });
});
