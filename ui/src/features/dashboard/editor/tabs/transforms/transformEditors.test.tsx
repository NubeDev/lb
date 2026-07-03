// Typed transform editors — the deep ids (editor-parity scope, step 3). Each authors the EXACT backend
// option shape (`rust/crates/viz`) through the UI, no JSON. filterByValue condition rows; groupBy
// per-field group/calc; calculateField binary operands.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { ResultFieldsProvider } from "../../fields/FieldsContext";
import { FilterByValueEditor } from "./FilterByValueEditor";
import { GroupByEditor } from "./GroupByEditor";
import { CalculateFieldEditor } from "./CalculateFieldEditor";

function harness(
  Editor: (p: { options: Record<string, unknown>; onChange: (o: Record<string, unknown>) => void }) => JSX.Element,
  fields: string[],
  initial: Record<string, unknown> = {},
) {
  function H() {
    const [options, setOptions] = useState(initial);
    return (
      <ResultFieldsProvider fields={fields}>
        <Editor options={options} onChange={setOptions} />
        <output aria-label="json">{JSON.stringify(options)}</output>
      </ResultFieldsProvider>
    );
  }
  return render(<H />);
}
const json = () => JSON.parse(screen.getByLabelText("json").textContent!);

describe("FilterByValueEditor", () => {
  it("authors a real condition row (field + operator + operand)", async () => {
    const user = userEvent.setup();
    harness(FilterByValueEditor, ["value"], { type: "include", match: "all", filters: [] });

    await user.click(screen.getByLabelText("add filter condition"));
    // pick the field via the combobox
    await user.click(screen.getByRole("combobox", { name: "filter 0 field" }));
    await user.click(screen.getByRole("option", { name: /^value$/ }));
    await user.selectOptions(screen.getByLabelText("filter 0 matcher"), "greater");
    await user.clear(screen.getByLabelText("filter 0 value"));
    await user.type(screen.getByLabelText("filter 0 value"), "10");

    expect(json().filters).toEqual([
      { fieldName: "value", config: { id: "greater", options: { value: 10 } } },
    ]);
  });
});

describe("GroupByEditor", () => {
  it("sets a group-by field and a calculate field with an aggregation", async () => {
    const user = userEvent.setup();
    harness(GroupByEditor, ["host", "value"], { fields: {} });

    await user.selectOptions(screen.getByLabelText("host operation"), "groupby");
    await user.selectOptions(screen.getByLabelText("value operation"), "aggregate");
    await user.click(screen.getByLabelText("value agg mean"));

    expect(json().fields).toEqual({
      host: { operation: "groupby" },
      value: { operation: "aggregate", aggregations: ["sum", "mean"] },
    });
  });
});

describe("CalculateFieldEditor", () => {
  it("authors a binary field/field calculation with an alias", async () => {
    const user = userEvent.setup();
    harness(CalculateFieldEditor, ["a", "b"], { mode: "binary", binary: { left: { field: "" }, operator: "+", right: { field: "" } } });

    await user.click(screen.getByRole("combobox", { name: "left operand field" }));
    await user.click(screen.getByRole("option", { name: /^a$/ }));
    await user.selectOptions(screen.getByLabelText("calc operator"), "/");
    await user.click(screen.getByRole("combobox", { name: "right operand field" }));
    await user.click(screen.getByRole("option", { name: /^b$/ }));
    await user.type(screen.getByLabelText("calc alias"), "ratio");

    const out = json();
    expect(out.binary).toEqual({ left: { field: "a" }, operator: "/", right: { field: "b" } });
    expect(out.alias).toBe("ratio");
  });
});
