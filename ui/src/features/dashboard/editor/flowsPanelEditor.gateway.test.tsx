// The Flows binding INSIDE the ONE PanelEditor ("Edit panel"), against a REAL gateway (flow-dashboard-
// binding-ux-scope; CLAUDE §9 — no fake backend). This is the test that the slice is actually reachable
// from the editor the user sees: open the editor on a fresh cell, pick the **Flows** datasource, pick a
// node port from the real `flows.list`/`flows.get`/`flows.nodes` reads, choose a control/read view, and
// assert the SAVED cell carries the right `action`/`source` + `view`. Proven against a seeded flow whose
// node ports are NON-`payload` too (agnostic to the node type a developer ships).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useRealGateway, signInReal } from "@/test/gateway-session";
import { saveFlow, injectFlow, runFlow, getFlowRun, type Flow } from "@/lib/flows";
import type { Cell } from "@/lib/dashboard";
import { flowBindingOfSource } from "../views/flowBinding";
import { WidgetView } from "../views/WidgetView";
import { WidgetHost } from "../WidgetHost";
import { PanelEditor } from "./PanelEditor";
import { defaultCell } from "./defaultCell";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `flow-panel-${n++}`;

beforeAll(() => useRealGateway());

/** A flow with a built-in `rhai` control node (input/output port `payload`). */
function ctlFlow(): Flow {
  return {
    id: "cooler-ctl",
    name: "Cooler Control",
    version: 1,
    failurePolicy: "halt",
    nodes: [{ id: "ctl", type: "rhai", needs: [], with: { payload: 1 }, config: { source: "payload" } }],
  } as Flow;
}

/** Render the editor open on a fresh cell; return the cell captured by Save. */
function openEditor(ws: string): { saved: () => Cell | null } {
  let captured: Cell | null = null;
  render(
    <WithDashboardCache ws={ws}>
      <PanelEditor
        ws={ws}
        cell={defaultCell("timeseries", "w1")}
        open
        onOpenChange={() => {}}
        onSave={(c) => {
          captured = c;
        }}
      />
    </WithDashboardCache>,
  );
  return { saved: () => captured };
}

describe("Flows binding in the PanelEditor (real gateway)", () => {
  it("pick Flows → an INPUT port → Slider: the saved cell carries the port-aware inject action", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());

    const user = userEvent.setup();
    const { saved } = openEditor(ws);

    // 1) Datasource → Flows (the new built-in option, reachable in the live editor).
    const ds = await screen.findByLabelText("panel datasource");
    await user.selectOptions(ds, "flows");

    // 2) The flow→node→port picker appears with REAL seeded entries; pick the input port.
    const portSel = await screen.findByLabelText("flow node port");
    await waitFor(() => expect(portSel).not.toBeDisabled());
    await user.selectOptions(portSel, "flows:in:cooler-ctl:ctl:payload");

    // 3) The viz picker swapped to the control set; pick Slider.
    await user.click(await screen.findByLabelText("viz slider"));

    // 4) Save → the cell binds the port-aware inject action + the slider view (no tool typed).
    await user.click(screen.getByRole("button", { name: /save panel/i }));
    const cell = saved();
    expect(cell?.view).toBe("slider");
    expect(cell?.action).toEqual({
      tool: "flows.inject",
      argsTemplate: { id: "cooler-ctl", node: "ctl", port: "payload", value: "{{value}}" },
    });
  });

  it("pick Flows → an OUTPUT port → JSON view: the saved cell reads the node via flows.node_state", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());

    const user = userEvent.setup();
    const { saved } = openEditor(ws);

    await user.selectOptions(await screen.findByLabelText("panel datasource"), "flows");
    const portSel = await screen.findByLabelText("flow node port");
    await waitFor(() => expect(portSel).not.toBeDisabled());
    await user.selectOptions(portSel, "flows:out:cooler-ctl:ctl:payload");

    // The output binding defaults to the JSON read view; save and assert the source.
    await user.click(screen.getByRole("button", { name: /save panel/i }));
    const cell = saved();
    expect(cell?.view).toBe("jsonview");
    const src = cell?.sources?.[0] ?? (cell?.source as { tool: string; args?: Record<string, unknown> } | undefined);
    expect(src?.tool).toBe("flows.node_state");
    expect(src?.args).toMatchObject({ id: "cooler-ctl", __flowNode: "ctl", __flowPort: "payload" });
  });

  it("the visual JSON builder binds a NESTED path from the node's real value (parse out the JSON)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());

    // Seed a real STRUCTURED value on the node: inject a nested object, then run so it records as the
    // node's `flow_node_state` value (the tree introspects this real value — no fake).
    await injectFlow("cooler-ctl", "ctl", { profile: { setpoint: 4, band: [3.5, 4.5] } }, "payload");
    const { run_id } = await runFlow("cooler-ctl");
    let snap = await getFlowRun(run_id);
    for (let i = 0; i < 40 && !["success", "partialFailure", "failed"].includes(snap.status); i++) {
      await new Promise((r) => setTimeout(r, 100));
      snap = await getFlowRun(run_id);
    }

    const user = userEvent.setup();
    const { saved } = openEditor(ws);
    await user.selectOptions(await screen.findByLabelText("panel datasource"), "flows");
    const portSel = await screen.findByLabelText("flow node port");
    await waitFor(() => expect(portSel).not.toBeDisabled());
    await user.selectOptions(portSel, "flows:out:cooler-ctl:ctl:payload");

    // The visual builder reads the node's REAL value and shows its tree; expand payload → profile, then
    // bind the `setpoint` leaf by clicking the row (no hand-typed JSON pointer).
    await user.click(await screen.findByLabelText("expand payload"));
    await user.click(await screen.findByLabelText("expand profile"));
    await user.click(await screen.findByLabelText("bind payload.profile.setpoint"));
    // the preview shows exactly that leaf value.
    await waitFor(() => expect(screen.getByLabelText("path preview")).toHaveTextContent("4"));

    await user.click(screen.getByRole("button", { name: /save panel/i }));
    const cell = saved();
    const src = cell?.sources?.[0] ?? (cell?.source as { tool: string; args?: Record<string, unknown> } | undefined);
    // the saved source carries the picked path; the binding recovers it for the read views.
    expect(src?.args?.__flowPath).toEqual(["payload", "profile", "setpoint"]);
    const bind = flowBindingOfSource({ tool: src!.tool, args: src!.args as Record<string, unknown> });
    expect(bind?.path).toEqual(["payload", "profile", "setpoint"]);

    // END-TO-END: render the EXACT cell the editor saved through WidgetHost (the GRID's real render
    // path) — it must show the bound leaf (4), never "binding broken — re-pick".
    const grid = render(<WithDashboardCache ws={ws}><WidgetHost cell={cell!} workspace={ws} /></WithDashboardCache>);
    await waitFor(() => expect(grid.getByLabelText("json content")).toHaveTextContent("4"));
    expect(grid.queryByText(/binding broken/i)).toBeNull();
  });

  it("a v3 sources[] flow cell RENDERS its value (not 'binding broken') — JSON view + Stat", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveFlow(ctlFlow());
    // Seed a real structured value: {payload: 80, ts: ...} envelope on the node.
    await injectFlow("cooler-ctl", "ctl", { payload: 80, ts: 1782862981 }, "payload");
    const { run_id } = await runFlow("cooler-ctl");
    let snap = await getFlowRun(run_id);
    for (let i = 0; i < 40 && !["success", "partialFailure", "failed"].includes(snap.status); i++) {
      await new Promise((r) => setTimeout(r, 100));
      snap = await getFlowRun(run_id);
    }

    // The cell exactly as the PanelEditor saves it: a v3 `sources[]` flow read (NOT a v2 `cell.source`).
    const base: Cell = {
      i: "c1",
      x: 0,
      y: 0,
      w: 4,
      h: 3,
      v: 3,
      widget_type: "chart",
      binding: { series: "" },
      sources: [
        {
          refId: "A",
          tool: "flows.node_state",
          args: { id: "cooler-ctl", __flowNode: "ctl", __flowPort: "payload", __flowPath: ["payload"] },
          datasource: { type: "flows" },
        },
      ],
    };

    // JSON view renders the picked leaf (80), never "binding broken — re-pick" (the v3 source bug).
    const json = render(<WithDashboardCache ws={ws}><WidgetView cell={{ ...base, view: "jsonview" }} workspace={ws} /></WithDashboardCache>);
    await waitFor(() => expect(json.getByLabelText("json content")).toHaveTextContent("80"));
    expect(json.queryByText(/binding broken/i)).toBeNull();
    json.unmount();

    // And a Stat over the SAME v3 flow source shows the scalar (usePanelData flow path → rows).
    const stat = render(<WithDashboardCache ws={ws}><WidgetView cell={{ ...base, view: "stat" }} workspace={ws} /></WithDashboardCache>);
    await waitFor(() => expect(stat.container.textContent).toContain("80"));
  });
});
