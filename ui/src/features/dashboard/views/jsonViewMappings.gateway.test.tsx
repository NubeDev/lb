// Value mappings on a `jsonview` widget, END TO END against a REAL gateway (viz field-config scope;
// CLAUDE §9 — no fake backend). The regression this pins: a JSON-view panel bound to a SCALAR flow
// value ignored its `fieldConfig.mappings`, so a `false → on` value mapping rendered the raw `false`
// instead of "on" (the reported bug). JsonView now resolves `valueFieldOptions(cell)` and applies
// mappings to a scalar the same fieldConfig bridge stat/gauge use.
//
// Seed a real boolean `false` on a flow node through the real run path, save the EXACT `jsonview` cell
// the PanelEditor writes (a v3 `sources[]` flow read + a `value` mapping keyed by "false"), render it
// through `WidgetHost` (the grid's real render path), and assert the mapped text — never the raw scalar.
//
// Workspace isolation is SPECIFIED: the SAME global user has a DIFFERENT mapping in two workspaces; the
// same seeded `false` renders each workspace's OWN mapped text and never reads the other's cell.

import { describe, expect, it, beforeAll } from "vitest";
import { render, waitFor } from "@testing-library/react";

import { useRealGateway, signInReal } from "@/test/gateway-session";
import { saveFlow, injectFlow, runFlow, getFlowRun, type Flow } from "@/lib/flows";
import type { Cell } from "@/lib/dashboard";
import { WidgetHost } from "../WidgetHost";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `jsonview-map-${n++}`;

beforeAll(() => useRealGateway());

/** A flow with a `rhai` node that passes an injected `{payload}` envelope straight through, so the node
 *  records the exact scalar we seed. */
function boolFlow(): Flow {
  return {
    id: "bool-flow",
    name: "Bool Flow",
    version: 1,
    failurePolicy: "halt",
    nodes: [{ id: "gate", type: "rhai", needs: [], with: { payload: false }, config: { source: "payload" } }],
  } as Flow;
}

/** Seed a real `{payload: bool}` value on the node through the REAL run path (inject → run → settle). */
async function seedBool(value: boolean) {
  await saveFlow(boolFlow());
  await injectFlow("bool-flow", "gate", { payload: value }, "payload");
  const { run_id } = await runFlow("bool-flow");
  let snap = await getFlowRun(run_id);
  for (let i = 0; i < 40 && !["success", "partialFailure", "failed"].includes(snap.status); i++) {
    await new Promise((r) => setTimeout(r, 100));
    snap = await getFlowRun(run_id);
  }
}

/** The exact cell the PanelEditor saves for a JSON view bound to the node's `payload`, with a `value`
 *  mapping that maps the stringified scalar to display text (the `false → on` mapping from the bug). */
function jsonViewCell(mapText: string): Cell {
  return {
    i: "c1",
    x: 0,
    y: 0,
    w: 4,
    h: 3,
    v: 3,
    widget_type: "chart",
    view: "jsonview",
    binding: { series: "" },
    source: { tool: "", args: null } as unknown as Cell["source"],
    sources: [
      {
        refId: "A",
        tool: "flows.node_state",
        args: { id: "bool-flow", __flowNode: "gate", __flowPort: "payload" },
        datasource: { type: "flows" },
      },
    ],
    fieldConfig: {
      defaults: {
        mappings: [{ type: "value", options: { false: { text: mapText, color: "green" } } }],
      },
      overrides: [],
    },
    options: {},
  } as Cell;
}

describe("jsonview value mappings apply to a scalar (real gateway)", () => {
  it("a `false → on` value mapping renders 'on', never the raw `false`", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedBool(false);

    const { getByLabelText } = render(<WithDashboardCache ws={ws}><WidgetHost cell={jsonViewCell("on")} workspace={ws} /></WithDashboardCache>);

    await waitFor(() => expect(getByLabelText("json content")).toHaveTextContent("on"));
    expect(getByLabelText("json content")).not.toHaveTextContent("false");
  });

  it("workspace isolation: the SAME user's mapping in ws-A never bleeds into ws-B", async () => {
    // Same global user, same seeded `false`; each workspace resolves its OWN cell mapping. A
    // one-workspace test would pass even with a leak; two prove the wall holds.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedBool(false);
    const a = render(<WithDashboardCache ws={wsA}><WidgetHost cell={jsonViewCell("ON-A")} workspace={wsA} /></WithDashboardCache>);
    await waitFor(() => expect(a.getByLabelText("json content")).toHaveTextContent("ON-A"));
    a.unmount();

    const wsB = nextWs();
    await signInReal("user:ada", wsB);
    await seedBool(false);
    const b = render(<WithDashboardCache ws={wsB}><WidgetHost cell={jsonViewCell("ON-B")} workspace={wsB} /></WithDashboardCache>);
    await waitFor(() => expect(b.getByLabelText("json content")).toHaveTextContent("ON-B"));
    expect(b.getByLabelText("json content")).not.toHaveTextContent("ON-A");
  });
});
