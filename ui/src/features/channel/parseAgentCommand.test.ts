// Unit tests for the `/agent` composer command parser (channels-agent scope). The UI parses the
// slash command and builds the structured `kind:"agent"` payload — the host never parses chat text.

import { describe, expect, it } from "vitest";

import { parseAgentCommand } from "./useChannel";

describe("parseAgentCommand", () => {
  it("returns null for ordinary chat", () => {
    expect(parseAgentCommand("hello team")).toBeNull();
    expect(parseAgentCommand("agent without a slash")).toBeNull();
  });

  it("parses `/agent <goal>` with the default (in-house) runtime", () => {
    expect(parseAgentCommand("/agent summarize the deploy logs")).toEqual({
      goal: "summarize the deploy logs",
      runtime: undefined,
    });
  });

  it("parses `/agent @runtime <goal>` selecting an external agent", () => {
    expect(parseAgentCommand("/agent @open-interpreter-default what changed?")).toEqual({
      goal: "what changed?",
      runtime: "open-interpreter-default",
    });
  });

  it("trims whitespace and tolerates extra spaces", () => {
    expect(parseAgentCommand("  /agent   @vtcode-default   hi there  ")).toEqual({
      goal: "hi there",
      runtime: "vtcode-default",
    });
  });

  it("an empty goal is surfaced (caller decides not to post)", () => {
    expect(parseAgentCommand("/agent")).toEqual({ goal: "", runtime: undefined });
  });
});
