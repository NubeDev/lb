import { describe, expect, it } from "vitest";

import type { Item } from "@/lib/channel/channel.types";
import { exportTranscript } from "./exportTranscript";

function item(author: string, body: string, ts: number): Item {
  return { id: `i${ts}`, channel: "dock:ada", author, body, ts };
}

/** An `agent` (ask) envelope as posted by the dock. */
function ask(goal: string, ts: number): Item {
  return item("user:ada", JSON.stringify({ kind: "agent", goal, job: `run-${ts}` }), ts);
}

/** An `agent_result` envelope as posted by the worker. */
function answer(text: string, runtime: string, ts: number): Item {
  return item(
    "system:agent-worker",
    JSON.stringify({ kind: "agent_result", goal: "g", runtime, job: `run-${ts}`, answer: text }),
    ts,
  );
}

describe("exportTranscript", () => {
  const ctx = {
    ws: "acme",
    principal: "user:ada",
    personaId: "builtin.widget-builder",
    surface: "dashboards",
  };

  it("renders a context header with ws, user, persona and surface", () => {
    const md = exportTranscript(ctx, [ask("add a widget", 1)]);
    expect(md).toContain("# Agent dock transcript");
    expect(md).toContain("**workspace:** acme");
    expect(md).toContain("**user:** user:ada");
    expect(md).toContain("**persona focus:** builtin.widget-builder");
    expect(md).toContain("**page surface:** dashboards");
  });

  it("parses agent/agent_result envelopes into readable turns (not raw JSON)", () => {
    const md = exportTranscript(ctx, [
      ask("list the datasources", 1),
      answer("You have: timescale, warehouse.", "default", 2),
    ]);
    // The goal + answer text are surfaced…
    expect(md).toContain("list the datasources");
    expect(md).toContain("You have: timescale, warehouse.");
    expect(md).toContain("runtime: default");
    // …and the raw envelope keys are NOT dumped.
    expect(md).not.toContain('"kind":"agent"');
    expect(md).not.toContain('"job"');
  });

  it("renders an agent_error turn", () => {
    const err = item(
      "system:agent-worker",
      JSON.stringify({ kind: "agent_error", goal: "g", error: "denied" }),
      1,
    );
    const md = exportTranscript(ctx, [err]);
    expect(md).toContain("agent error");
    expect(md).toContain("denied");
  });

  it("renders a plain (untagged) chat message as-is", () => {
    const md = exportTranscript(ctx, [item("user:ada", "just a note", 1)]);
    expect(md).toContain("just a note");
  });

  it("falls back to friendly text when no persona/surface and no items", () => {
    const md = exportTranscript({ ws: "acme", principal: "user:ada" }, []);
    expect(md).toContain("**persona focus:** (none — workspace default)");
    expect(md).toContain("**page surface:** (none)");
    expect(md).toContain("_(no messages yet)_");
  });
});

describe("exportTranscript tool calls", () => {
  it("appends the latest run's live-captured tool calls with honest statuses", () => {
    const md = exportTranscript(
      {
        ws: "acme",
        principal: "user:ada",
        latestRunTools: [
          { id: "c1", name: "datasource.list", ok: "{}", err: null },
          { id: "c2", name: "viz.query", ok: null, err: "denied" },
          { id: "c3", name: "dashboard.pin" },
        ],
      },
      [item("user:ada", "hi", 1)],
    );
    expect(md).toContain("tool calls (latest run, live-captured)");
    expect(md).toContain("- `datasource.list` — ✓");
    expect(md).toContain("- `viz.query` — ✗ denied");
    expect(md).toContain("- `dashboard.pin` — … (still running)");
  });

  it("omits the tool section when none were captured", () => {
    const md = exportTranscript({ ws: "acme", principal: "user:ada" }, [item("user:ada", "hi", 1)]);
    expect(md).not.toContain("tool calls (latest run");
  });
});
