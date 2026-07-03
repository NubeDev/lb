// The FieldNamePicker against a REAL gateway (editor-parity scope, step 1 testing plan): the picker's
// options come from a REAL `viz.query` result over REAL seeded rows — never a hardcoded list (CLAUDE §9).
// Also pins the degrade path: with no frames yet it stays usable as a labeled free-text combobox.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useMemo, useState } from "react";

import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";
import { usePanelData } from "../../builder/usePanelData";
import { fieldNamesOf } from "./resultFields";
import { ResultFieldsProvider } from "./FieldsContext";
import { FieldNamePicker } from "./FieldNamePicker";

let n = 0;
const nextWs = () => `fnp-${n++}`;

beforeAll(() => useRealGateway());

/** A v3 cell over a real `series.read` target — the non-watch path that resolves via viz.query. */
function cellOver(series: string): Cell {
  return {
    i: "c", x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "chart", view: "timeseries",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
  };
}

/** The same wiring PanelEditor does: ONE usePanelData read feeds the provider; the picker consumes it. */
function Harness({ cell }: { cell: Cell }) {
  const data = usePanelData(cell);
  const fields = useMemo(() => fieldNamesOf(data.rows), [data.rows]);
  const [picked, setPicked] = useState("");
  return (
    <ResultFieldsProvider fields={fields}>
      <output aria-label="fields">{fields.join(",")}</output>
      <output aria-label="picked">{picked}</output>
      <FieldNamePicker aria-label="field" value={picked} onChange={setPicked} />
    </ResultFieldsProvider>
  );
}

describe("FieldNamePicker fed by a real viz.query result", () => {
  it("offers the REAL result field names and commits a click — no typing required", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSeries({ series: "picker.temp", seq: 1, payload: 21.5, key: "kind", value: "temperature" });

    const user = userEvent.setup();
    render(<Harness cell={cellOver("picker.temp")} />);

    // Wait for the REAL viz.query rows to land (debounced ~200ms + round-trip).
    const fieldsOut = await waitFor(
      () => {
        const el = screen.getByLabelText("fields");
        expect(el.textContent).not.toBe("");
        return el;
      },
      { timeout: 4000 },
    );
    const fields = fieldsOut.textContent!.split(",");
    expect(fields.length).toBeGreaterThan(0);

    // Open the picker: every REAL field name is offered as an option.
    await user.click(screen.getByRole("combobox", { name: "field" }));
    for (const f of fields) {
      expect(screen.getByRole("option", { name: new RegExp(`^${f}$`) })).toBeInTheDocument();
    }

    // Pick the first real field by CLICK — the exit-gate behavior (no remembered-and-retyped name).
    await user.click(screen.getByRole("option", { name: new RegExp(`^${fields[0]}$`) }));
    expect(screen.getByLabelText("picked").textContent).toBe(fields[0]);
  });

  it("degrades to labeled free-text when there are no frames yet (never blocks authoring)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const user = userEvent.setup();
    // A target-less cell → no rows → no offered fields; the picker must still accept typed text.
    const bare: Cell = { i: "b", x: 0, y: 0, w: 4, h: 3, widget_type: "chart", binding: { series: "" } };
    render(<Harness cell={bare} />);

    const trigger = screen.getByRole("combobox", { name: "field" });
    expect(trigger.textContent).toContain("type a field name");
    await user.click(trigger);
    await user.type(screen.getByLabelText("field search"), "hand_typed{Enter}");
    expect(screen.getByLabelText("picked").textContent).toBe("hand_typed");
  });
});
