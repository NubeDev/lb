// The catalog "Test" button + DB-sealed per-workspace model key, proven END TO END over a REAL
// spawned, SEEDED gateway (no mocks, no fake backend — CLAUDE §9; agent-catalog test-and-secrets
// scope). The test gateway boots with the in-house `default` runtime over the UNCONFIGURED placeholder
// (no provider adapter is wired in the test node), so `provider_configured` is honestly false and the
// answer is the placeholder line — the CONTEXT LINE is what proves the agent was given its context.
//
// Covers the scope's UI testing plan:
//   - Test button: clicking it runs the real `agent.def.test`, shows the reply + the "context: N tools,
//     M skills" line, and the honest "no model provider is wired" note (placeholder, not a real LLM).
//   - Model-key field: entering a value seals a real `secret.set` (Private) and stores ONLY the path on
//     the definition — a fresh read shows `api_key_secret` is the PATH, never the value (names-only),
//     and re-opening the editor shows the "key is set ✓ · rotate" affordance without any readback.
//   - A built-in has NO Model-key field (read-only tier — no edit affordance opens the editor for it).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SettingsHarness } from "./SettingsHarness";
import { getAgentDef } from "@/lib/agent/agentDef.api";
import { getAgentConfig } from "@/lib/agent/config.api";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `agent-test-key-${n++}`;

const ADMIN_CAPS = [
  "mcp:agent.def.list:call",
  "mcp:agent.def.get:call",
  "mcp:agent.def.create:call",
  "mcp:agent.def.update:call",
  "mcp:agent.def.delete:call",
  "mcp:agent.config.get:call",
  "mcp:agent.config.set:call",
  "mcp:agent.runtimes:call",
  "mcp:tools.catalog:call",
  // The context-proving diagnostic (admin-tier — spends a model turn).
  "mcp:agent.def.test:call",
  // Sealing the model key: the shipped secrets gate for the `agent/` path space.
  "mcp:secret.set:call",
  "secret:agent/*:write",
];

beforeAll(() => useRealGateway());

describe("Agent catalog — Test button + sealed model key over a real seeded gateway", () => {
  it("the Test button runs the real diagnostic and shows the reply + context line", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    render(<SettingsHarness ws={ws} caps={session.caps} />);
    await user.click(screen.getByLabelText("Agent"));

    // A built-in is runnable (runtime `default`) → it carries a Test button.
    const testBtn = await screen.findByLabelText("test builtin.in-house-glm-4.6");
    await user.click(testBtn);

    // The real `agent.def.test` result renders: the answer panel + the context line. The test node
    // runs the UNCONFIGURED placeholder, so the honest "no model provider is wired" note appears.
    const panel = await screen.findByLabelText(
      "test result builtin.in-house-glm-4.6",
      {},
      { timeout: 15_000 },
    );
    expect(panel.textContent).toMatch(/context:\s*\d+\s*tools,\s*\d+\s*skills/);
    expect(panel.textContent).toMatch(/no model provider is wired/i);
    cleanup();
  });

  it("the Model-key field seals a secret and stores only the path (names-only, no readback)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    render(<SettingsHarness ws={ws} caps={session.caps} />);
    await user.click(screen.getByLabelText("Agent"));

    // Create a custom definition WITH a sealed model key. The editor seals the value via `secret.set`
    // FIRST, then writes the definition carrying only the resulting path (`agent/<id>-key`).
    await user.click(await screen.findByLabelText("new custom definition"));
    await user.type(screen.getByLabelText("Label"), "Custom — sealed key");
    await user.type(screen.getByLabelText("Provider"), "zaicoding");
    await user.type(screen.getByLabelText("Model"), "glm-sealed-x");
    await user.type(screen.getByLabelText("Model key"), "sk-super-secret-VALUE-999");
    await user.click(screen.getByLabelText("save definition"));

    // It appears in the catalog after a real round-trip.
    await screen.findByLabelText("definition custom-sealed-key");

    // NAMES-ONLY, proven at the SERVER: a fresh `agent.def.get` shows the path, never the value.
    await waitFor(async () => {
      const def = await getAgentDef("custom-sealed-key");
      expect(def.model_endpoint.api_key_secret).toBe("agent/custom-sealed-key-key");
      // The value never lands on the record.
      expect(JSON.stringify(def)).not.toContain("sk-super-secret-VALUE-999");
    });

    // Re-open the editor: the field shows the "key is set ✓ · rotate" affordance WITHOUT reading the
    // value back (the input is empty; only the placeholder signals the sealed state).
    await user.click(screen.getByLabelText("edit custom-sealed-key"));
    const keyField = (await screen.findByLabelText("Model key")) as HTMLInputElement;
    expect(keyField.value).toBe("");
    expect(keyField.placeholder).toMatch(/key is set ✓/);
    cleanup();
  });

  it("an admin seals a model key on the ACTIVE built-in pick without cloning it", async () => {
    // The headline UX fix: a built-in is read-only, but the workspace's SELECTION of it can own a
    // sealed key. Pick a built-in, then use the active-pick "Set model key" affordance — the value
    // seals via `secret.set` and only the PATH lands on `agent.config` (names-only). This is the
    // self-serve "add a token to the in-house model" path — no clone needed.
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    render(<SettingsHarness ws={ws} caps={session.caps} />);
    await user.click(screen.getByLabelText("Agent"));

    // Pick a built-in as the active selection.
    await user.click(await screen.findByLabelText("pick builtin.in-house-glm-4.6"));
    await waitFor(() =>
      expect(
        screen.getByLabelText("definition builtin.in-house-glm-4.6").getAttribute("data-active"),
      ).toBe("true"),
    );

    // The active pick shows a "Set model key" affordance (no clone). Open it and seal a token.
    await user.click(await screen.findByLabelText("set model key"));
    await user.type(screen.getByLabelText("active model key"), "sk-active-TOKEN-777");
    await user.click(screen.getByLabelText("save model key"));

    // NAMES-ONLY at the SERVER: `agent.config` now carries a secret PATH, never the token value.
    await waitFor(async () => {
      const cfg = await getAgentConfig();
      expect(cfg?.model_endpoint?.api_key_secret).toBeTruthy();
      expect(JSON.stringify(cfg)).not.toContain("sk-active-TOKEN-777");
    });
    cleanup();
  });

  it("a built-in has no Model-key field (read-only tier — no editor opens for it)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    render(<SettingsHarness ws={ws} caps={session.caps} />);
    await user.click(screen.getByLabelText("Agent"));

    // A built-in renders with NO edit affordance, so the editor (and its Model-key field) never opens.
    await screen.findByLabelText("definition builtin.in-house-glm-4.6");
    expect(screen.queryByLabelText("edit builtin.in-house-glm-4.6")).toBeNull();
    // The Model-key field only exists inside the editor — absent while the catalog is shown read-only.
    expect(screen.queryByLabelText("Model key")).toBeNull();
    cleanup();
  });
});
