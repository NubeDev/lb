// Overrides usability gate — REAL gateway (editor-parity scope, step 4 testing plan; CLAUDE §9). Author
// a field override entirely through the pickers over REAL viz.query result fields (seeded rows) — a
// byName matcher fed by the real fields + a typed unit property from the registry — and assert the
// resolved field config for that field. No JSON, no free-typed property id.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useMemo, useState } from "react";

import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";
import { usePanelData } from "../../builder/usePanelData";
import { fieldNamesOf } from "../fields/resultFields";
import { ResultFieldsProvider } from "../fields/FieldsContext";
import { OverridesTab } from "./OverridesTab";
import { cellToEditorState, type EditorState } from "../cellEditorState";
import type { FieldConfig } from "@/lib/dashboard";
import { resolveFieldOptions } from "../../fieldconfig/resolve";

let n = 0;
const nextWs = () => `ovr-${n++}`;

beforeAll(() => useRealGateway());

function cellOver(series: string): Cell {
  return {
    i: "c", x: 0, y: 0, w: 8, h: 4, v: 3, widget_type: "chart", view: "timeseries",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
  };
}

function Harness({ cell }: { cell: Cell }) {
  const data = usePanelData(cell);
  const fields = useMemo(() => fieldNamesOf(data.rows), [data.rows]);
  const [state, setState] = useState<EditorState>(() => cellToEditorState(cell));
  return (
    <ResultFieldsProvider fields={fields}>
      <output aria-label="fields">{fields.join(",")}</output>
      <output aria-label="fieldConfig">{JSON.stringify(state.fieldConfig ?? null)}</output>
      <OverridesTab state={state} patch={(next) => setState((s) => ({ ...s, ...next }))} />
    </ResultFieldsProvider>
  );
}

describe("overrides usability (real gateway)", () => {
  it("authors a byName override via pickers over REAL fields; resolved config applies to that field", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSeries({ series: "ovr.temp", seq: 1, payload: 19, key: "kind", value: "temperature" });

    const user = userEvent.setup();
    render(<Harness cell={cellOver("ovr.temp")} />);

    const fieldsOut = await waitFor(
      () => {
        const el = screen.getByLabelText("fields");
        expect(el.textContent).not.toBe("");
        return el;
      },
      { timeout: 4000 },
    );
    const target = fieldsOut.textContent!.split(",")[0];

    await user.click(screen.getByLabelText("add override"));
    await user.click(screen.getByRole("combobox", { name: "override 0 match" }));
    await user.click(screen.getByRole("option", { name: new RegExp(`^${target}$`) }));
    await user.click(screen.getByRole("combobox", { name: "override 0 add property" }));
    await user.click(screen.getByRole("option", { name: /^Unit$/ }));
    await user.click(screen.getByRole("combobox", { name: "unit value" }));
    await user.type(screen.getByLabelText("unit value search"), "celsius");
    await user.click(screen.getByRole("option", { name: /celsius/i }));

    // The resolved field config for the targeted field carries the authored unit — proving the override
    // the pickers built is the real fieldConfig the render path merges (no JSON, no typed property id).
    const fc = await waitFor(() => {
      const raw = screen.getByLabelText("fieldConfig").textContent!;
      const parsed = JSON.parse(raw) as FieldConfig | null;
      expect(parsed?.overrides?.[0]?.properties?.[0]?.value).toBe("celsius");
      return parsed!;
    });
    expect(resolveFieldOptions(fc, { name: target, type: "number" }).unit).toBe("celsius");
    // A different field is unaffected (the byName matcher is field-scoped).
    expect(resolveFieldOptions(fc, { name: "__other__", type: "number" }).unit).toBeUndefined();
  });
});
