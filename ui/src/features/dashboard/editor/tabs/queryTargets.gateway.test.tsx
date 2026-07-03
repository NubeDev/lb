// Multi-target Query rows — REAL gateway (editor-parity scope, step 6; CLAUDE §9). The A/B/C rows
// operate on the `targets[]` model over a real source picker (real `series.list`); add/duplicate/
// hide/reorder/delete + the query-options row are asserted on the editor state. Uses the real gateway
// so the source picker is populated for real (no fake backend).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { useRealGateway, signInReal } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";
import { QueryTargets } from "./QueryTargets";
import { cellToEditorState, editorStateToCell, type EditorState } from "../cellEditorState";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `qt-${n++}`;

beforeAll(() => useRealGateway());

function baseCell(): Cell {
  return {
    i: "c", x: 0, y: 0, w: 8, h: 4, v: 3, widget_type: "chart", view: "timeseries",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series: "x" }, datasource: { type: "series" } }],
  };
}

function Harness({ ws }: { ws: string }) {
  const [state, setState] = useState<EditorState>(() => cellToEditorState(baseCell()));
  return (
    <>
      <QueryTargets ws={ws} state={state} patch={(next) => setState((s) => ({ ...s, ...next }))} />
      <output aria-label="targets">{JSON.stringify(state.targets)}</output>
      <output aria-label="queryOptions">{JSON.stringify(state.queryOptions ?? null)}</output>
    </>
  );
}

const targets = () => JSON.parse(screen.getByLabelText("targets").textContent!) as Array<{ refId: string; hide?: boolean }>;

describe("multi-target query rows (real gateway)", () => {
  it("adds, duplicates, hides, reorders, and deletes A/B/C queries over targets[]", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const user = userEvent.setup();
    render(<WithDashboardCache ws={ws}><Harness ws={ws} /></WithDashboardCache>);

    // Starts with one query A.
    expect(targets().map((t) => t.refId)).toEqual(["A"]);

    // Add → B.
    await user.click(screen.getByLabelText("add query"));
    expect(targets().map((t) => t.refId)).toEqual(["A", "B"]);

    // Duplicate A → a new C inserted after A.
    await user.click(screen.getByLabelText("duplicate query A"));
    expect(targets().map((t) => t.refId)).toEqual(["A", "C", "B"]);

    // Hide B.
    await user.click(screen.getByLabelText("hide query B"));
    expect(targets().find((t) => t.refId === "B")?.hide).toBe(true);

    // Move A right (swap with C).
    await user.click(screen.getByLabelText("move query A right"));
    expect(targets().map((t) => t.refId)).toEqual(["C", "A", "B"]);

    // Delete C.
    await user.click(screen.getByLabelText("delete query C"));
    expect(targets().map((t) => t.refId)).toEqual(["A", "B"]);
  });

  it("query options (max data points / min interval / relative time) round-trip on the cell", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const user = userEvent.setup();
    render(<WithDashboardCache ws={ws}><Harness ws={ws} /></WithDashboardCache>);

    await user.type(screen.getByLabelText("max data points"), "500");
    await user.type(screen.getByLabelText("min interval"), "1m");
    await user.type(screen.getByLabelText("relative time"), "now-6h");

    const qo = JSON.parse(screen.getByLabelText("queryOptions").textContent!);
    expect(qo).toEqual({ maxDataPoints: 500, minInterval: "1m", relativeTime: "now-6h" });

    // And they survive the cell round-trip (absent stays absent otherwise).
    const cell = editorStateToCell({ ...cellToEditorState(baseCell()), queryOptions: qo }, baseCell());
    expect(cell.queryOptions).toEqual(qo);
    expect(editorStateToCell(cellToEditorState(cell), cell)).toEqual(cell);
  });
});
