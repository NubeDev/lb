// Component tests for the SEARCHABLE combobox (data-studio-ux). The list opens on focus, filters by
// typed query across every group, and picking an option fires BOTH onSelect (the folded selection) and
// onSelectEntry (the raw entry, for a host that keys on id). Same model as the <select>.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SourceCombobox } from "./SourceCombobox";
import { buildSourceEntries } from "./sourcePicker";
import type { ExtRow } from "./types";

const ext: ExtRow = {
  ext: "thecrew",
  enabled: true,
  widgets: [{ entry: "r.js", label: "Scene", icon: "box", scope: ["assets.get_doc"] }],
};
const entries = buildSourceEntries({
  series: ["ahu1.speed", "chiller.temp"],
  extensions: [ext],
  rules: [{ id: "r1", name: "Hourly mean" }],
});

describe("SourceCombobox", () => {
  it("opens on focus and lists options grouped by origin", () => {
    render(<SourceCombobox entries={entries} onSelect={() => {}} />);
    fireEvent.focus(screen.getByLabelText("source"));
    expect(screen.getByRole("option", { name: "ahu1.speed" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "thecrew · Scene" })).toBeTruthy();
    // The group headers render.
    expect(screen.getByText("Series")).toBeTruthy();
    expect(screen.getByText("Rules")).toBeTruthy();
  });

  it("filters options by the typed query across groups", () => {
    render(<SourceCombobox entries={entries} onSelect={() => {}} />);
    const input = screen.getByLabelText("source");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "chiller" } });
    expect(screen.getByRole("option", { name: "chiller.temp" })).toBeTruthy();
    expect(screen.queryByRole("option", { name: "ahu1.speed" })).toBeNull();
    expect(screen.queryByRole("option", { name: "Hourly mean" })).toBeNull();
  });

  it("shows an empty state when nothing matches", () => {
    render(<SourceCombobox entries={entries} onSelect={() => {}} />);
    const input = screen.getByLabelText("source");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "zzz-nope" } });
    expect(screen.getByText("No matching sources")).toBeTruthy();
  });

  it("fires onSelect (folded) and onSelectEntry (raw) when an option is chosen", () => {
    const onSelect = vi.fn();
    const onSelectEntry = vi.fn();
    render(<SourceCombobox entries={entries} onSelect={onSelect} onSelectEntry={onSelectEntry} />);
    fireEvent.focus(screen.getByLabelText("source"));
    // onMouseDown (not click) fires before the input blur closes the list.
    fireEvent.mouseDown(screen.getByRole("option", { name: "Hourly mean" }));
    expect(onSelect).toHaveBeenCalledWith(
      expect.objectContaining({ id: "rule:r1", source: { tool: "rules.run", args: { rule_id: "r1", route: false } } }),
    );
    expect(onSelectEntry).toHaveBeenCalledWith(expect.objectContaining({ id: "rule:r1", group: "rules" }));
  });

  it("shows the selected entry's label when one is set", () => {
    render(<SourceCombobox entries={entries} value="series:ahu1.speed" onSelect={() => {}} />);
    expect((screen.getByLabelText("source") as HTMLInputElement).value).toBe("ahu1.speed");
  });
});
