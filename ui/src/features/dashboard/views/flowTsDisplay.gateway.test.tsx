// Flow timestamp → viewer's wall-clock, END TO END against a REAL gateway (flow-ts-display scope;
// CLAUDE §9 — no fake backend). The one test that proves the slice is actually reachable from the grid
// the user sees: seed a flow whose node records a canonical `{payload, ts}` envelope (ts = epoch
// SECONDS, the flow clock), seed the VIEWER's real `user_prefs` (timezone + date/time style), then
// render the EXACT Stat cell the PanelEditor saves — a v3 `sources[]` flow read bound to the `ts` field
// with the `time:flow-seconds` datetime unit — through `WidgetHost` (the grid's real render path). The
// cell must show the viewer's wall-clock, never 1970 (the seconds-vs-ms bug) and never the raw epoch.
//
// Workspace isolation is SPECIFIED, not generic: the SAME global user has DIFFERENT tz prefs in two
// workspaces; the same canonical `ts` renders in EACH workspace's tz and never reads the other's record.

import { describe, expect, it, beforeAll } from "vitest";
import { render, waitFor } from "@testing-library/react";

import { useRealGateway, signInReal } from "@/test/gateway-session";
import { saveFlow, injectFlow, runFlow, getFlowRun, type Flow } from "@/lib/flows";
import { setPrefs } from "@/lib/prefs/set";
import type { Cell } from "@/lib/dashboard";
import { WidgetHost } from "../WidgetHost";

let n = 0;
const nextWs = () => `flow-ts-${n++}`;

beforeAll(() => useRealGateway());

/** A flow with a `rhai` node that passes an injected `{payload, ts}` envelope straight through, so the
 *  node records the exact canonical value we seed (ts = epoch seconds — the flow clock's unit). */
function tsFlow(): Flow {
  return {
    id: "ts-flow",
    name: "TS Flow",
    version: 1,
    failurePolicy: "halt",
    nodes: [{ id: "clock", type: "rhai", needs: [], with: { payload: 0 }, config: { source: "payload" } }],
  } as Flow;
}

/** Seed a real `{payload, ts}` value on the node through the REAL run path (inject → run → settle). */
async function seedTsValue(ts: number) {
  await saveFlow(tsFlow());
  await injectFlow("ts-flow", "clock", { payload: 42, ts }, "payload");
  const { run_id } = await runFlow("ts-flow");
  let snap = await getFlowRun(run_id);
  for (let i = 0; i < 40 && !["success", "partialFailure", "failed"].includes(snap.status); i++) {
    await new Promise((r) => setTimeout(r, 100));
    snap = await getFlowRun(run_id);
  }
}

/** The exact cell the PanelEditor saves for a Stat bound to the node's `ts` field with the flow-seconds
 *  datetime unit (incl. the empty v2 `source` placeholder the gateway round-trips — the shape that broke
 *  the earlier "binding broken" bug). */
function tsStatCell(): Cell {
  return {
    i: "c1",
    x: 0,
    y: 0,
    w: 4,
    h: 3,
    v: 3,
    widget_type: "chart",
    view: "stat",
    binding: { series: "" },
    // The gateway round-trips a v3 cell with this empty placeholder alongside `sources[]` — the reader
    // must ignore it and use the real flow target (the regression the WidgetView `.tool ?` fix pins).
    source: { tool: "", args: null } as unknown as Cell["source"],
    sources: [
      {
        refId: "A",
        tool: "flows.node_state",
        args: { id: "ts-flow", __flowNode: "clock", __flowPort: "payload", __flowPath: ["ts"] },
        datasource: { type: "flows" },
      },
    ],
    fieldConfig: { defaults: { unit: "time:flow-seconds" }, overrides: [] },
    // A stat reduces one row to one value; `last` over the single recorded value.
    options: { reduceOptions: { calcs: ["last"] } },
  } as Cell;
}

// 2026-07-01T00:05:00Z as epoch SECONDS (10 digits — what the flow clock stamps).
const TS_SECONDS = 1782864300;

describe("flow timestamp renders in the viewer's resolved prefs (real gateway)", () => {
  it("EU/Madrid viewer sees their wall-clock date; the canonical ts stays epoch seconds", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await setPrefs({ timezone: "Europe/Madrid", date_style: "eu", time_style: "h24" });
    await seedTsValue(TS_SECONDS);

    const { getByLabelText, queryByText } = render(<WidgetHost cell={tsStatCell()} workspace={ws} />);

    // Madrid is UTC+2 in July → 02:05 on 01/07/2026 (EU order, 24h). Never 1970, never the raw epoch.
    await waitFor(() => expect(getByLabelText("stat value")).toHaveTextContent("01/07/2026 02:05"));
    expect(queryByText(/1970/)).toBeNull();
    expect(queryByText(String(TS_SECONDS))).toBeNull();
  });

  it("USA/New_York viewer sees the SAME instant in their own style (12h, USA order)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await setPrefs({ timezone: "America/New_York", date_style: "usa", time_style: "h12" });
    await seedTsValue(TS_SECONDS);

    const { getByLabelText } = render(<WidgetHost cell={tsStatCell()} workspace={ws} />);

    // New York is UTC-4 in July → 2026-06-30 20:05 → USA order + 12h.
    await waitFor(() => expect(getByLabelText("stat value")).toHaveTextContent("06/30/2026 8:05 PM"));
  });

  it("workspace isolation: the SAME user's tz in ws-A never bleeds into ws-B", async () => {
    // ws-A: Madrid. ws-B: Tokyo. Same global user, same canonical ts — each workspace resolves its OWN
    // prefs. A one-workspace test would pass even with a leak; two prove the wall holds.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await setPrefs({ timezone: "Europe/Madrid", date_style: "eu", time_style: "h24" });
    await seedTsValue(TS_SECONDS);
    const a = render(<WidgetHost cell={tsStatCell()} workspace={wsA} />);
    await waitFor(() => expect(a.getByLabelText("stat value")).toHaveTextContent("01/07/2026 02:05"));
    a.unmount();

    const wsB = nextWs();
    await signInReal("user:ada", wsB);
    await setPrefs({ timezone: "Asia/Tokyo", date_style: "iso", time_style: "h24" });
    await seedTsValue(TS_SECONDS);
    const b = render(<WidgetHost cell={tsStatCell()} workspace={wsB} />);
    // Tokyo is UTC+9 → 2026-07-01 09:05, ISO order — NOT Madrid's 02:05.
    await waitFor(() => expect(b.getByLabelText("stat value")).toHaveTextContent("2026-07-01 09:05"));
  });
});
