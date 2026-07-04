// The agent-definition catalog proven END TO END over a REAL spawned, SEEDED gateway (no mocks, no
// fake backend — CLAUDE §9): the seeded built-ins list, a member sees them read-only, an admin
// creates/edits/deletes a custom definition and picks one as the active selection, and a built-in has
// no edit/delete affordance. The test gateway seeds the six built-ins through the real boot seeder.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SettingsHarness } from "./SettingsHarness";
import { listAgentDefs } from "@/lib/agent/agentDef.api";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `agent-catalog-${n++}`;

const READ_CAPS = ["mcp:agent.def.list:call", "mcp:agent.def.get:call"];
const ADMIN_CAPS = [
  ...READ_CAPS,
  "mcp:agent.config.get:call",
  "mcp:agent.config.set:call",
  "mcp:agent.runtimes:call",
  "mcp:agent.def.create:call",
  "mcp:agent.def.update:call",
  "mcp:agent.def.delete:call",
];

beforeAll(() => useRealGateway());

describe("Agent catalog — seeded built-ins + custom CRUD over a real seeded gateway", () => {
  it("lists the node-runnable built-ins; the open-interpreter ones are filtered", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    const defs = await listAgentDefs();
    const ids = defs.map((d) => d.id);
    // The in-house three seed AND list (runtime `default`, always offered).
    expect(ids).toContain("builtin.in-house-glm-4.6");
    expect(ids).toContain("builtin.in-house-glm-5.1");
    expect(ids).toContain("builtin.in-house-glm-5.2");
    // The open-interpreter three are seeded but filtered (runtime not offered on the default node).
    expect(ids.some((id) => id.startsWith("builtin.open-interpreter"))).toBe(false);
    // NAMES-ONLY: the seeded endpoint carries the env NAME, never a secret value.
    const inhouse = defs.find((d) => d.id === "builtin.in-house-glm-4.6")!;
    expect(inhouse.model_endpoint.api_key_env).toBe("ZAI_API_KEY");
  });

  it("an admin creates, edits, picks, and deletes a custom definition; built-ins are read-only", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    render(<SettingsHarness ws={ws} caps={session.caps} />);
    await user.click(screen.getByLabelText("Agent"));

    // A built-in renders with NO edit/delete affordance (read-only tier).
    await screen.findByLabelText("definition builtin.in-house-glm-4.6");
    expect(screen.queryByLabelText("edit builtin.in-house-glm-4.6")).toBeNull();
    expect(screen.queryByLabelText("delete builtin.in-house-glm-4.6")).toBeNull();

    // Create a custom definition through the real editor → real `agent.def.create`.
    await user.click(screen.getByLabelText("new custom definition"));
    await user.type(screen.getByLabelText("Label"), "Custom — staging");
    await user.type(screen.getByLabelText("Provider"), "zaicoding");
    // A model id distinct from every built-in so the active-selection resolves to THIS entry
    // unambiguously (the copy-based pick matches on runtime+provider+model — see the copy-vs-reference
    // open question; two entries with identical resolved fields would tie).
    await user.type(screen.getByLabelText("Model"), "glm-custom-x");
    await user.type(screen.getByLabelText("API key env var"), "ZAI_STAGING_KEY");
    await user.click(screen.getByLabelText("save definition"));

    // It appears in the catalog (a real list read), tagged custom with edit/delete affordances.
    const custom = await screen.findByLabelText("definition custom-staging");
    expect(custom).toBeTruthy();
    expect(screen.getByLabelText("edit custom-staging")).toBeTruthy();

    // Pick it → writes `agent.config`; the entry becomes Active.
    await user.click(screen.getByLabelText("pick custom-staging"));
    await waitFor(() =>
      expect(
        screen.getByLabelText("definition custom-staging").getAttribute("data-active"),
      ).toBe("true"),
    );

    // Delete it → real `agent.def.delete`; it leaves the catalog.
    await user.click(screen.getByLabelText("delete custom-staging"));
    await waitFor(() =>
      expect(screen.queryByLabelText("definition custom-staging")).toBeNull(),
    );
    cleanup();

    // A read against the live gateway confirms the custom entry is gone (not just component state).
    await waitFor(async () => {
      const defs = await listAgentDefs();
      expect(defs.some((d) => d.id === "custom-staging")).toBe(false);
    });
  });

  it("a member (read caps only) sees the catalog read-only — no pick/manage affordances", async () => {
    const ws = nextWs();
    const session = await signInWithCaps("user:bob", ws, READ_CAPS);

    render(<SettingsHarness ws={ws} caps={session.caps} />);
    const user = userEvent.setup();
    await user.click(screen.getByLabelText("Agent"));

    await screen.findByLabelText("definition builtin.in-house-glm-4.6");
    // No "new definition", no "use" pick button — the member cannot manage or pick.
    expect(screen.queryByLabelText("new custom definition")).toBeNull();
    expect(screen.queryByLabelText("pick builtin.in-house-glm-4.6")).toBeNull();
    cleanup();
  });
});
