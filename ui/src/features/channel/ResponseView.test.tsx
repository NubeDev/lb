// Unit test for the rich-result adapter (channel rich responses scope). NO real gateway — the deep
// end-to-end (a posted reminder.list table whose per-row controls drive the real write verbs) is Agent
// C's gateway test. Here we unit-test the PURE pieces + a shallow render:
//   - `buildCell` maps the render envelope to a v2 Cell (view/source/action), folding the declared
//     `tools` into the cell so `cellTools(cell)` (the bridge leash) = render.tools.
//   - the per-row-control scope binding: `interpolateArgs({id:"${id}", enabled:"{{value}}"}, {values:row})`
//     yields the row's id + the interaction bool (the `${id}` row-field / `{{value}}` interaction split).
//   - a shallow render: a `rich_result` table with rowControls mounts and shows the reminder rows plus a
//     per-row switch + run/delete buttons (the interactive-list piece). The ipc seam is stubbed to real
//     rows — a thin ipc stub, NOT a node re-implementation (rule 9).

import { render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { interpolateArgs } from "@/lib/vars";
import { cellTools } from "@/features/dashboard/views/WidgetView";
import type { RichResultPayload } from "@/lib/channel/payload.types";
import type { RowControl } from "./ResponseTable";

// A GENERIC row-control fixture (was imported from the deleted reminderArgs — that domain knowledge now
// lives in the backend descriptor). The `${id}` row-field / `{{value}}` interaction split is what
// ResponseView renders, tool-agnostic; the tool names here are just fixture data.
const ROW_CONTROLS: RowControl[] = [
  { kind: "switch", label: "enabled", action: { tool: "reminder.update", argsTemplate: { id: "${id}", enabled: "{{value}}" } } },
  { kind: "button", label: "run", buttonLabel: "Run now", action: { tool: "reminder.fire", argsTemplate: { id: "${id}" } } },
  { kind: "button", label: "delete", buttonLabel: "Delete", action: { tool: "reminder.delete", argsTemplate: { id: "${id}" } } },
];

// Stub the ONE ipc seam so usePanelData resolves rows without a gateway (a thin stub, not a fake node).
// A non-watch panel resolves through `viz.query`, which returns `{ rows }` — so the stub answers that
// shape (usePanelData maps `rows` → the SourceState the table reads).
const rows = [
  { id: "r1", schedule: "0 8 * * 1", enabled: true },
  { id: "r2", schedule: "0 9 * * *", enabled: false },
];
vi.mock("@/lib/ipc/invoke", () => ({
  invoke: vi.fn(async () => ({ rows })),
}));

import { buildCell, ResponseView } from "./ResponseView";

describe("buildCell — envelope → v2 Cell", () => {
  it("maps view/source/action and folds the declared tools into the cell's leash", () => {
    const payload: RichResultPayload = {
      kind: "rich_result",
      v: 2,
      view: "table",
      source: { tool: "reminder.list", args: {} },
      options: { rowControls: ROW_CONTROLS },
      tools: ["reminder.list", "reminder.update", "reminder.fire", "reminder.delete"],
    };
    const cell = buildCell(payload, "item-1");
    expect(cell.v).toBe(2);
    expect(cell.view).toBe("table");
    expect(cell.source?.tool).toBe("reminder.list");
    // The bridge leash covers EVERY declared tool (render.tools) — the host intersects with grant.
    expect(new Set(cellTools(cell))).toEqual(new Set(payload.tools));
  });
});

describe("the per-row control scope binding", () => {
  it("binds ${id} from the row object and {{value}} from the interaction", () => {
    // The row object is the control's VarScope.values; `${id}` resolves from it, `{{value}}` from the
    // interaction (the switch bool). This is the load-bearing row-object + {{value}} split.
    const out = interpolateArgs(
      { id: "${id}", enabled: "{{value}}" },
      { values: { id: "r1", enabled: ["true"] }, builtins: {} },
      true,
    );
    expect(out).toEqual({ id: "r1", enabled: true });
  });
});

describe("ResponseView — a rich_result table with row controls", () => {
  it("mounts the shipped table with per-row switch + buttons over the source rows", async () => {
    const payload: RichResultPayload = {
      kind: "rich_result",
      v: 2,
      view: "table",
      source: { tool: "reminder.list", args: {} },
      options: { rowControls: ROW_CONTROLS },
      tools: ["reminder.list", "reminder.update", "reminder.fire", "reminder.delete"],
    };
    render(<ResponseView payload={payload} workspace="acme" itemKey="item-1" />);

    // The source rows land (through the shipped usePanelData path).
    await waitFor(() => expect(screen.getByText("r1")).toBeInTheDocument());
    expect(screen.getByText("r2")).toBeInTheDocument();

    // Each row carries the per-row controls: a pause switch + a run-now + a delete button.
    const table = screen.getByLabelText("response table");
    expect(within(table).getAllByRole("switch").length).toBe(rows.length);
    expect(within(table).getAllByLabelText(/button reminder\.fire/).length).toBe(rows.length);
    expect(within(table).getAllByLabelText(/button reminder\.delete/).length).toBe(rows.length);
  });

  it("degrades a newer envelope version at render (fallback, not crash)", () => {
    const payload = {
      kind: "rich_result",
      v: 3,
      view: "table",
    } as unknown as RichResultPayload;
    render(<ResponseView payload={payload} workspace="acme" />);
    expect(screen.getByRole("status")).toHaveTextContent(/newer app/i);
  });
});
