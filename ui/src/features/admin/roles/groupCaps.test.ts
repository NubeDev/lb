// groupCaps unit test — pins the cap-grammar parsing + bucketing the Roles cap tree relies on:
// dotted ids, single-segment ids, wildcards, non-mcp surfaces, `:call`-less degradation, and the
// deterministic `*`/`other`-last ordering.

import { describe, expect, it } from "vitest";

import { groupCaps } from "./groupCaps";

describe("groupCaps", () => {
  it("groups a dotted-id cap by its first segment and shortens the label", () => {
    const [g] = groupCaps(["mcp:agent.def.list:call"]);
    expect(g.group).toBe("agent");
    expect(g.caps[0]).toEqual({ cap: "mcp:agent.def.list:call", label: "def.list" });
  });

  it("shortens a single-verb cap to the verb", () => {
    const [g] = groupCaps(["mcp:dashboard.get:call"]);
    expect(g.group).toBe("dashboard");
    expect(g.caps[0].label).toBe("get");
  });

  it("keeps a single-segment id as its own group with a non-empty label", () => {
    const [g] = groupCaps(["mcp:roles:call"]);
    expect(g.group).toBe("roles");
    expect(g.caps[0].label).toBe("roles");
  });

  it("buckets wildcard caps under `*`", () => {
    const [g] = groupCaps(["mcp:*.create:call"]);
    expect(g.group).toBe("*");
    expect(g.caps[0].label).toBe("create");
  });

  it("puts a non-mcp cap in `other` with its full string as the label", () => {
    const [g] = groupCaps(["store:doc/*:read"]);
    expect(g.group).toBe("other");
    expect(g.caps[0]).toEqual({ cap: "store:doc/*:read", label: "store:doc/*:read" });
  });

  it("degrades gracefully for an mcp cap missing the :call suffix (non-empty label)", () => {
    const [g] = groupCaps(["mcp:agent.invoke"]);
    expect(g.group).toBe("agent");
    expect(g.caps[0].label).toBe("invoke");
  });

  it("orders named groups alphabetically, then `*`, then `other` last", () => {
    const groups = groupCaps([
      "store:doc/*:read",
      "mcp:*.create:call",
      "mcp:workspace.purge:call",
      "mcp:agent.def.get:call",
    ]);
    expect(groups.map((g) => g.group)).toEqual(["agent", "workspace", "*", "other"]);
  });

  it("sorts caps within a group by their full string", () => {
    const [g] = groupCaps(["mcp:agent.def.list:call", "mcp:agent.config.get:call"]);
    expect(g.caps.map((c) => c.cap)).toEqual([
      "mcp:agent.config.get:call",
      "mcp:agent.def.list:call",
    ]);
  });
});
