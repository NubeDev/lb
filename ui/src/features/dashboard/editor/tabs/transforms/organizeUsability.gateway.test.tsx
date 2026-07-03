// Organize-fields usability gate — REAL gateway (editor-parity scope, step 3 headline; CLAUDE §9). The
// organize row list is fed by the live preview's REAL viz.query result fields (seeded rows), and the
// config authored through the UI (rename + hide + reorder) is asserted on the saved `transformations[]`.
// No JSON typed anywhere — this is the exact "Organize fields is a raw JSON textarea" complaint, closed.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useMemo, useState } from "react";

import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";
import { usePanelData } from "../../../builder/usePanelData";
import { fieldNamesOf } from "../../fields/resultFields";
import { ResultFieldsProvider } from "../../fields/FieldsContext";
import { TransformTab } from "../TransformTab";
import { cellToEditorState, type EditorState } from "../../cellEditorState";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `org-${n++}`;

beforeAll(() => useRealGateway());

function cellOver(series: string): Cell {
  return {
    i: "c", x: 0, y: 0, w: 8, h: 4, v: 3, widget_type: "chart", view: "table",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    transformations: [{ id: "organize", options: {} }],
  };
}

/** Mirror PanelEditor: ONE usePanelData feeds the result fields the organize editor offers. */
function Harness({ cell }: { cell: Cell }) {
  const data = usePanelData(cell);
  const fields = useMemo(() => fieldNamesOf(data.rows), [data.rows]);
  const [state, setState] = useState<EditorState>(() => cellToEditorState(cell));
  return (
    <ResultFieldsProvider fields={fields}>
      <output aria-label="fields">{fields.join(",")}</output>
      <output aria-label="transformations">{JSON.stringify(state.transformations)}</output>
      <TransformTab state={state} patch={(next) => setState((s) => ({ ...s, ...next }))} />
    </ResultFieldsProvider>
  );
}

describe("organize usability (real gateway)", () => {
  it("builds an organize config (rename + hide + reorder) over REAL result fields — no JSON typed", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSeries({ series: "org.temp", seq: 1, payload: 21, key: "kind", value: "temperature" });

    const user = userEvent.setup();
    render(<WithDashboardCache ws={ws}><Harness cell={cellOver("org.temp")} /></WithDashboardCache>);

    // Wait for the REAL viz.query frames to populate the organize row list.
    const fieldsOut = await waitFor(
      () => {
        const el = screen.getByLabelText("fields");
        expect(el.textContent).not.toBe("");
        return el;
      },
      { timeout: 4000 },
    );
    const fields = fieldsOut.textContent!.split(",");
    expect(fields.length).toBeGreaterThan(1);
    const [first, second] = fields;

    // Rename the first field, hide the second, move the second up — all through the UI.
    await user.type(screen.getByLabelText(`rename ${first}`), "Renamed");
    await user.click(screen.getByLabelText(`hide ${second}`));
    await user.click(screen.getByLabelText(`move up ${second}`));

    const trs = JSON.parse(screen.getByLabelText("transformations").textContent!);
    const organize = trs.find((t: { id: string }) => t.id === "organize");
    expect(organize.options.renameByName).toEqual({ [first]: "Renamed" });
    expect(organize.options.excludeByName).toEqual({ [second]: true });
    // `second` moved above `first` → explicit order second=0, first=1.
    expect(organize.options.indexByName[second]).toBe(0);
    expect(organize.options.indexByName[first]).toBe(1);
  });
});
