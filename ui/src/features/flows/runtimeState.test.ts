import { describe, expect, it } from "vitest";

import { deriveRuntimeState, relativeFuture, relativePast } from "./runtimeState";
import type { Flow, FlowNode, FlowNodeState, FlowRunSummary } from "@/lib/flows";

function flow(over: Partial<Flow>): Flow {
  return { id: "f", name: "f", version: 1, nodes: [], ...over };
}
function run(over: Partial<FlowRunSummary>): FlowRunSummary {
  return { runId: "r", flowId: "f", flowVersion: 1, status: "success", ...over };
}
function trigger(mode: string, config: Record<string, unknown> = {}): FlowNode {
  return { id: "t", type: "trigger", needs: [], config: { mode, ...config } };
}
function flipflop(): FlowNode {
  return { id: "ff", type: "flipflop", needs: [], config: { period_secs: 1 } };
}
/** The authoritative durable runtime view (`flows.node_state`). */
function nodeState(over: Partial<FlowNodeState>): FlowNodeState {
  return { flowId: "f", nodes: [], ...over };
}

describe("deriveRuntimeState — a flow is RUNNING when enabled, STOPPED when disabled (PLC model)", () => {
  it("an enabled cron flow is RUNNING, with cron + nextFireTs from node_state", () => {
    const cronFlow = flow({ nodes: [trigger("cron", { cron: "* * * * *" })] });
    const s = deriveRuntimeState(cronFlow, [], nodeState({ enabled: true, cron: "* * * * *", nextAttemptTs: 100 }));
    expect(s.state).toBe("running");
    expect(s.running).toBe(true);
    expect(s.cron).toBe("* * * * *");
    expect(s.nextFireTs).toBe(100);
  });

  it("REGRESSION: an enabled flipflop-driven flow (NO trigger node) is RUNNING — never frozen as 'idle'", () => {
    // The bug this whole change fixes: the old model inspected the graph for a `trigger` node to decide
    // if a flow was 'armed'. A flipflop flow has none, so it was classed 'idle', the canvas never
    // polled, and its live values froze even though the host advances them every second. Runtime state
    // must derive from `enabled` alone — no graph-shape guessing.
    const ffFlow = flow({ nodes: [flipflop()] });
    const s = deriveRuntimeState(ffFlow, [], nodeState({ enabled: true }));
    expect(s.state).toBe("running");
    expect(s.running).toBe(true);
  });

  it("a manual-only flow (no self-firing source) is still RUNNING when enabled — it just advances on Run", () => {
    const s = deriveRuntimeState(flow({ nodes: [trigger("manual")] }), [], nodeState({ enabled: true }));
    expect(s.state).toBe("running");
  });

  it("node_state.enabled=false makes ANY flow STOPPED (the durable Stop survives restart)", () => {
    const cronFlow = flow({ nodes: [trigger("cron", { cron: "* * * * *" })] });
    expect(deriveRuntimeState(cronFlow, [], nodeState({ enabled: false })).state).toBe("stopped");
    expect(deriveRuntimeState(flow({ nodes: [flipflop()] }), [], nodeState({ enabled: false })).state).toBe("stopped");
  });

  it("after restart with no live run, node_state still reports the flow RUNNING with its live cursor", () => {
    // node_state is the per-trigger cursor truth (not the flow record's dormant cron), so on reload the
    // banner is correct even with no run in flight.
    const s = deriveRuntimeState(
      flow({ nodes: [trigger("cron", { cron: "*/5 * * * *" })], cron: null }),
      [],
      nodeState({ enabled: true, cron: "*/5 * * * *", nextAttemptTs: 999 }),
    );
    expect(s.state).toBe("running");
    expect(s.nextFireTs).toBe(999);
  });

  it("falls back to the flow record until node_state loads (nodeState undefined)", () => {
    const s = deriveRuntimeState(
      flow({ enabled: true, nodes: [trigger("cron", { cron: "* * * * *" })], cron: "* * * * *", nextAttemptTs: 50 }),
      [],
    );
    expect(s.state).toBe("running");
    expect(s.nextFireTs).toBe(50);
  });

  it("enabled defaults true when neither node_state nor the flow record says otherwise", () => {
    expect(deriveRuntimeState(flow({}), []).state).toBe("running");
  });

  it("latestRun is runs[0] (host returns newest-first)", () => {
    const runs = [run({ runId: "newest", ts: 200 }), run({ runId: "older", ts: 100 })];
    const s = deriveRuntimeState(flow({}), runs, nodeState({ enabled: true }));
    expect(s.latestRun?.runId).toBe("newest");
  });

  it("nextFireTs is null when not yet primed (ts 0)", () => {
    const s = deriveRuntimeState(flow({}), [], nodeState({ enabled: true, nextAttemptTs: 0 }));
    expect(s.nextFireTs).toBeNull();
  });
});

describe("relative time", () => {
  it("future: seconds, minutes, any moment", () => {
    expect(relativeFuture(130, 100)).toBe("in 30s");
    expect(relativeFuture(280, 100)).toBe("in 3m");
    expect(relativeFuture(100, 100)).toBe("any moment");
    expect(relativeFuture(null, 100)).toBe("—");
  });
  it("past: seconds, minutes ago", () => {
    expect(relativePast(70, 100)).toBe("30s ago");
    expect(relativePast(100, 280)).toBe("3m ago");
    expect(relativePast(undefined, 100)).toBe("—");
  });
});
