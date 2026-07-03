// Overrides tab usability (editor-parity scope, step 4): author an override entirely through the
// pickers — a byName matcher fed by real fields, then an "add property" picker over the registry, each
// property rendering its normal typed control — and assert the produced `overrides[]`. No JSON, no
// free-typed property id. Also covers multiple properties per override.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { cellToEditorState } from "@/lib/panel-kit/cellEditorState";
import { ResultFieldsProvider } from "../fields/FieldsContext";
import { OverridesTab } from "./OverridesTab";

function Harness({ fields }: { fields: string[] }) {
  const [state, setState] = useState<EditorState>(() =>
    cellToEditorState({
      i: "c", x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "chart", view: "timeseries",
      binding: { series: "" },
      sources: [{ refId: "A", tool: "series.read", args: {}, datasource: { type: "series" } }],
    }),
  );
  return (
    <ResultFieldsProvider fields={fields}>
      <OverridesTab state={state} patch={(next) => setState((s) => ({ ...s, ...next }))} />
      <output aria-label="overrides">{JSON.stringify(state.fieldConfig?.overrides ?? [])}</output>
    </ResultFieldsProvider>
  );
}

describe("OverridesTab", () => {
  it("authors a byName override + two typed properties through the pickers — no JSON", async () => {
    const user = userEvent.setup();
    render(<Harness fields={["cpu", "mem"]} />);

    await user.click(screen.getByLabelText("add override"));
    // Matcher value: pick the real field `cpu` (a picker, not a free-typed name).
    await user.click(screen.getByRole("combobox", { name: "override 0 match" }));
    await user.click(screen.getByRole("option", { name: /^cpu$/ }));

    // Add the `unit` property via the registry picker, then set it to celsius via its normal control.
    await user.click(screen.getByRole("combobox", { name: "override 0 add property" }));
    await user.click(screen.getByRole("option", { name: /^Unit$/ }));
    await user.click(screen.getByRole("combobox", { name: "unit value" }));
    await user.type(screen.getByLabelText("unit value search"), "celsius");
    await user.click(screen.getByRole("option", { name: /celsius/i }));

    // Add a SECOND property (decimals) — multiple properties per override.
    await user.click(screen.getByRole("combobox", { name: "override 0 add property" }));
    await user.click(screen.getByRole("option", { name: /^Decimals$/ }));
    await user.type(screen.getByLabelText("decimals value"), "2");

    const overrides = JSON.parse(screen.getByLabelText("overrides").textContent!);
    expect(overrides).toEqual([
      {
        matcher: { id: "byName", options: "cpu" },
        properties: [
          { id: "unit", value: "celsius" },
          { id: "decimals", value: 2 },
        ],
      },
    ]);
  });

  it("switches the matcher kind to a typed control (byType dropdown)", async () => {
    const user = userEvent.setup();
    render(<Harness fields={["cpu"]} />);
    await user.click(screen.getByLabelText("add override"));
    await user.selectOptions(screen.getByLabelText("override 0 matcher"), "byType");
    await user.selectOptions(screen.getByLabelText("override 0 match"), "number");

    const overrides = JSON.parse(screen.getByLabelText("overrides").textContent!);
    expect(overrides[0].matcher).toEqual({ id: "byType", options: "number" });
  });
});
