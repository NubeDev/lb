// FlowsQuerySection "binding broken — re-pick" detection (flow-dashboard-binding-ux-scope). Regression
// for the false-positive: the hint must NEVER flash while the picker entries are still loading or when
// the workspace granted no flows — only when a HELD binding's flow/node/port is genuinely absent from
// the loaded entries (renamed/deleted). Pure component logic; no gateway.

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { FlowsQuerySection, suggestTimestampUnit } from "./FlowsQuerySection";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import type { SourceEntry } from "@/features/dashboard/builder/sourcePicker";

const entries: SourceEntry[] = [
  { id: "flows:out:f1:counter-2:payload", group: "flows", label: "F1 › counter-2 › payload (output)",
    source: { tool: "flows.node_state", args: { id: "f1", __flowNode: "counter-2", __flowPort: "payload" } }, writes: false },
  { id: "flows:in:f1:start:payload", group: "flows", label: "F1 › start › payload (input)",
    action: { tool: "flows.inject", argsTemplate: { id: "f1", node: "start", port: "payload", value: "{{value}}" } }, writes: true },
];

function baseState(over: Partial<EditorState>): EditorState {
  return {
    view: "", title: "", description: "", targets: [], options: {}, transformations: [],
    carry: { v: 3, widget_type: "chart", binding: { series: "" }, action: undefined, pluginVersion: undefined, targetRepr: "sources", extraOptions: {} },
    ...over,
  } as EditorState;
}

describe("FlowsQuerySection broken detection", () => {
  it("a VALID output binding is NOT broken", () => {
    const state = baseState({
      view: "jsonview",
      targets: [{ refId: "A", tool: "flows.node_state", args: { id: "f1", __flowNode: "counter-2", __flowPort: "payload" }, datasource: { type: "flows" } }],
    });
    render(<FlowsQuerySection state={state} patch={() => {}} entries={entries} loading={false} />);
    expect(screen.queryByText(/binding broken/i)).toBeNull();
  });
  it("a VALID input binding is NOT broken", () => {
    const state = baseState({
      view: "slider",
      carry: { v: 3, widget_type: "chart", binding: { series: "" }, action: { tool: "flows.inject", argsTemplate: { id: "f1", node: "start", port: "payload", value: "{{value}}" } }, pluginVersion: undefined, targetRepr: "none", extraOptions: {} },
    });
    render(<FlowsQuerySection state={state} patch={() => {}} entries={entries} loading={false} />);
    expect(screen.queryByText(/binding broken/i)).toBeNull();
  });
});

describe("FlowsQuerySection broken — loading race", () => {
  it("does NOT flash broken while entries are still loading", () => {
    const state = baseState({
      view: "jsonview",
      targets: [{ refId: "A", tool: "flows.node_state", args: { id: "f1", __flowNode: "counter-2", __flowPort: "payload" }, datasource: { type: "flows" } }],
    });
    // entries empty + loading (the async useSourcePicker race)
    render(<FlowsQuerySection state={state} patch={() => {}} entries={[]} loading={true} />);
    expect(screen.queryByText(/binding broken/i)).toBeNull();
  });
  it("does NOT flash broken when entries loaded empty (no flows granted) — shows the empty hint instead", () => {
    const state = baseState({
      view: "jsonview",
      targets: [{ refId: "A", tool: "flows.node_state", args: { id: "f1", __flowNode: "counter-2", __flowPort: "payload" }, datasource: { type: "flows" } }],
    });
    render(<FlowsQuerySection state={state} patch={() => {}} entries={[]} loading={false} />);
    expect(screen.queryByText(/binding broken/i)).toBeNull();
  });
  it("DOES show broken when a held binding is absent from loaded entries (renamed/deleted)", () => {
    const state = baseState({
      view: "jsonview",
      targets: [{ refId: "A", tool: "flows.node_state", args: { id: "f1", __flowNode: "GONE", __flowPort: "payload" }, datasource: { type: "flows" } }],
    });
    render(<FlowsQuerySection state={state} patch={() => {}} entries={entries} loading={false} />);
    expect(screen.getByText(/binding broken/i)).toBeTruthy();
  });
});

describe("timestamp-unit suggestion (flow-ts-display scope)", () => {
  it("suggests the flow-seconds datetime unit for a `ts` / `*_ts` / `*_at` leaf", () => {
    for (const leaf of ["ts", "cron_ts", "created_at"]) {
      const fc = suggestTimestampUnit(["payload", leaf], undefined);
      expect(fc?.defaults.unit).toBe("time:flow-seconds");
    }
  });
  it("does NOT suggest for a non-timestamp leaf, or an array index", () => {
    expect(suggestTimestampUnit(["payload"], undefined)).toBeNull();
    expect(suggestTimestampUnit(["value", "count"], undefined)).toBeNull();
    expect(suggestTimestampUnit(["items", 0], undefined)).toBeNull();
  });
  it("NEVER clobbers an author's explicit unit", () => {
    const existing = { defaults: { unit: "dateTimeAsIso" }, overrides: [] };
    expect(suggestTimestampUnit(["ts"], existing)).toBeNull();
  });
});
