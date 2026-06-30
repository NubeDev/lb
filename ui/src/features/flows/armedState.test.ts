import { describe, expect, it } from "vitest";

import { deriveArmedState, isScheduled, relativeFuture, relativePast } from "./armedState";
import type { Flow, FlowNode, FlowNodeState, FlowRunSummary } from "@/lib/flows";

function flow(over: Partial<Flow>): Flow {
  return { id: "f", name: "f", version: 1, nodes: [], ...over };
}
function run(over: Partial<FlowRunSummary>): FlowRunSummary {
  return { runId: "r", flowId: "f", flowVersion: 1, status: "success", ...over };
}
/** A trigger node with the given mode (the graph-derived "is this flow headless" signal). */
function trigger(mode: string, config: Record<string, unknown> = {}): FlowNode {
  return { id: "t", type: "trigger", needs: [], config: { mode, ...config } };
}
/** The authoritative durable runtime view (`flows.node_state`). */
function nodeState(over: Partial<FlowNodeState>): FlowNodeState {
  return { flowId: "f", nodes: [], ...over };
}

describe("isScheduled (graph-derived, independent of enabled)", () => {
  it("true when the flow has a cron/event/boot trigger node", () => {
    expect(isScheduled(flow({ nodes: [trigger("cron", { cron: "* * * * *" })] }))).toBe(true);
    expect(isScheduled(flow({ nodes: [trigger("event", { series: "s" })] }))).toBe(true);
    expect(isScheduled(flow({ nodes: [trigger("boot")] }))).toBe(true);
  });
  it("false for a manual/inject-only flow (runs on demand)", () => {
    expect(isScheduled(flow({ nodes: [trigger("manual")] }))).toBe(false);
    expect(isScheduled(flow({ nodes: [trigger("inject")] }))).toBe(false);
    expect(isScheduled(flow({ nodes: [] }))).toBe(false);
  });
  it("holds even when the flow is disabled (a disabled cron flow is still scheduled)", () => {
    expect(isScheduled(flow({ enabled: false, nodes: [trigger("cron", { cron: "* * * * *" })] }))).toBe(
      true,
    );
  });
});

describe("deriveArmedState — armed fields come from node_state, not the dormant flow record", () => {
  const cronFlow = flow({ nodes: [trigger("cron", { cron: "* * * * *" })] });

  it("an enabled cron flow is ARMED with cron + nextFireTs from node_state", () => {
    const s = deriveArmedState(cronFlow, [], nodeState({ enabled: true, cron: "* * * * *", nextAttemptTs: 100 }));
    expect(s.kind).toBe("armed");
    expect(s.scheduled).toBe(true);
    expect(s.cron).toBe("* * * * *");
    expect(s.nextFireTs).toBe(100);
  });

  it("after restart with no live run, node_state still reports the flow ARMED", () => {
    // The regression: reading the flow record's dormant cron showed this as idle. node_state is the
    // per-trigger cursor truth, so the banner is correct on reload.
    const s = deriveArmedState(flow({ nodes: [trigger("cron", { cron: "*/5 * * * *" })], cron: null }), [], nodeState({ enabled: true, cron: "*/5 * * * *", nextAttemptTs: 999 }));
    expect(s.kind).toBe("armed");
    expect(s.nextFireTs).toBe(999);
  });

  it("node_state.enabled=false makes a cron flow DISABLED (the durable Stop survives restart)", () => {
    const s = deriveArmedState(cronFlow, [], nodeState({ enabled: false }));
    expect(s.kind).toBe("disabled");
    expect(s.scheduled).toBe(true);
  });

  it("a manual flow is IDLE, not armed", () => {
    const s = deriveArmedState(flow({ nodes: [trigger("manual")] }), [], nodeState({ enabled: true }));
    expect(s.kind).toBe("idle");
    expect(s.scheduled).toBe(false);
  });

  it("falls back to the flow record until node_state loads (nodeState undefined)", () => {
    const s = deriveArmedState(flow({ enabled: true, nodes: [trigger("cron", { cron: "* * * * *" })], cron: "* * * * *", nextAttemptTs: 50 }), []);
    expect(s.kind).toBe("armed");
    expect(s.nextFireTs).toBe(50);
  });

  it("latestRun is runs[0] (host returns newest-first)", () => {
    const runs = [run({ runId: "newest", ts: 200 }), run({ runId: "older", ts: 100 })];
    const s = deriveArmedState(cronFlow, runs, nodeState({ enabled: true }));
    expect(s.latestRun?.runId).toBe("newest");
  });

  it("nextFireTs is null when not yet primed (ts 0)", () => {
    const s = deriveArmedState(cronFlow, [], nodeState({ enabled: true, nextAttemptTs: 0 }));
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
