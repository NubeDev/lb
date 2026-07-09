// The Agent wizard, driven against a REAL in-process seeded gateway (setup scope; CLAUDE §9 — no fake
// backend). Proves the wizard is pure orchestration over the real Settings › Agent editors: the intro
// page names the four key parts; the Definition step renders the SAME catalog the Settings tab does
// (the seeded built-ins, read back over the real gateway) and an admin can pick one as active; the
// Persona step renders the real persona section. A fresh workspace per test isolates the shared node.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { AgentWizard } from "./AgentWizard";
import { getAgentConfig } from "@/lib/agent/config.api";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `agent-wiz-${n++}`;

const ADMIN_CAPS = [
  "mcp:agent.def.list:call",
  "mcp:agent.def.get:call",
  "mcp:agent.config.get:call",
  "mcp:agent.config.set:call",
  "mcp:agent.runtimes:call",
  "mcp:agent.persona.list:call",
];

beforeAll(() => useRealGateway());

describe("AgentWizard (real seeded gateway)", () => {
  it("intros the four parts, then reuses the real catalog to pick an active definition", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ADMIN_CAPS);
    render(<AgentWizard ws={ws} caps={ADMIN_CAPS} />);

    // ── Step 1: the intro names the four key parts. ──
    await screen.findByText("Set up the workspace agent");
    expect(screen.getByText(/Definition — who runs/)).toBeInTheDocument();
    expect(screen.getByText(/Persona — what for/)).toBeInTheDocument();
    expect(screen.getByText(/Tools — what it can reach/)).toBeInTheDocument();
    expect(screen.getByText(/Permissions — how it's supervised/)).toBeInTheDocument();
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 2: the Definition step renders the REAL seeded catalog; pick one as active. ──
    const list = await screen.findByLabelText("agent catalog");
    const firstDef = (await within(list).findAllByLabelText(/^definition /))[0];
    const id = firstDef.getAttribute("aria-label")!.replace("definition ", "");
    await user.click(within(firstDef).getByLabelText(`pick ${id}`));

    // The pick landed on the REAL workspace agent config — read it back over the gateway.
    await waitFor(async () => {
      const cfg = await getAgentConfig();
      expect(cfg?.default_runtime).toBeTruthy();
    });
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 3: the Persona step renders the real persona section. ──
    await screen.findByText(/Shape what it's for/);
  });
});
