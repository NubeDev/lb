// The Chains canvas, driven against a REAL in-process gateway (rules-workbench scope, Phase 2; CLAUDE
// §9 / testing §0 — no fake backend). Each test logs in to a UNIQUE workspace, seeds a real saved
// rule + chain through the real write path (the chains api + the always-available `mcp.call` bridge
// for `rules.save`), and drives the real `ChainCanvas`/`useChainRun` over the real HTTP transport.
//
// Covers: a CRUD round-trip (save a chain → get loads it → delete drops it); a CYCLIC edge at save →
// the host's `400` validation message rendered INLINE (no crash); a run then poll → nodes colour as
// steps settle (a node reaches ok/green + the banner shows a terminal status); and a late
// `getChainRun` rebuilds the same colours from the records. No mocks — real host responses.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ChainCanvas } from "./ChainCanvas";
import { deleteChain, getChain, getChainRun, saveChain, type Chain } from "@/lib/chains";
import { snapshotColours } from "./chainGraph";
import { invoke } from "@/lib/ipc/invoke";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

// The full cap set a chain canvas needs over the gateway: the six chains MCP caps + `rules.save`/
// `rules.run` (a step runs a saved rule) + the `store:rule|chain` surface caps the host's save/run
// verbs check (defense-in-depth beyond the MCP gate). NOTE for the lead: the dev login
// (credentials.rs) grants the MCP caps but NOT `store:rule/*`/`store:chain/*`, so the live
// Playground/canvas needs those added there too — until then this test mints them explicitly.
const CAPS = [
  "mcp:chains.save:call",
  "mcp:chains.run:call",
  "mcp:chains.get:call",
  "mcp:chains.list:call",
  "mcp:chains.delete:call",
  "mcp:chains.runs.get:call",
  "mcp:rules.save:call",
  "mcp:rules.run:call",
  "store:rule:read",
  "store:rule:write",
  "store:chain:read",
  "store:chain:write",
];

async function signIn(user: string, ws: string) {
  return signInWithCaps(user, ws, CAPS);
}

/** A save wrapper returning the host's validation outcome (mirrors `useChains.save` — the shape the
 *  canvas's `onSave` expects, surfacing a `400` inline rather than throwing). */
async function saveResult(c: Chain): Promise<{ ok: boolean; error?: string }> {
  try {
    await saveChain(c);
    return { ok: true };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

let n = 0;
const nextWs = () => `chains-ui-${n++}`;

beforeAll(() => useRealGateway());

/** Seed a saved rule the chain's steps reference, through the always-available `/mcp/call` bridge
 *  (`rules.save` is a shipped host verb). The dev login carries the rules + chains caps. */
async function seedRule(name: string, body: string): Promise<void> {
  await invoke("mcp_call", { tool: "rules.save", args: { id: name, name, body } });
}

const EMIT_OK = `emit(#{ level: "info", msg: "ok" });`;

describe("Chains canvas (real gateway)", () => {
  it("CRUD round-trip: save a chain, get loads it, delete drops it", async () => {
    const ws = nextWs();
    await signIn("user:ada", ws);
    await seedRule("r", EMIT_OK);

    const chain: Chain = {
      id: "pipe",
      name: "Pipe",
      steps: [{ id: "a", rule: "r", needs: [] }],
    };
    const saved = await saveChain(chain);
    expect(saved.id).toBe("pipe");

    // get loads the same DAG.
    const loaded = await getChain("pipe");
    expect(loaded.id).toBe("pipe");
    expect(loaded.steps[0].id).toBe("a");

    // delete drops it → get now rejects (404).
    await deleteChain("pipe");
    await expect(getChain("pipe")).rejects.toThrow();
  });

  it("a cyclic edge at save renders the host's validation error INLINE (no crash)", async () => {
    const ws = nextWs();
    await signIn("user:ada", ws);
    await seedRule("r", EMIT_OK);

    // Open the canvas on a chain whose two steps will be wired into a cycle by the save action.
    const cyclic: Chain = {
      id: "bad",
      name: "Bad",
      steps: [
        { id: "a", rule: "r", needs: ["b"] },
        { id: "b", rule: "r", needs: ["a"] },
      ],
    };
    // The canvas Save serializes the current nodes/edges; render with the cyclic chain so its edges
    // round-trip into the cyclic DAG the host rejects.
    render(<ChainCanvas chain={cyclic} onSave={saveResult} />);

    const user = userEvent.setup();
    await user.click(screen.getByLabelText("save chain"));

    // The host's `400` message is shown inline — and the canvas did NOT crash (the toolbar survives).
    const err = await screen.findByLabelText("chain save error");
    expect(err.textContent?.toLowerCase()).toContain("cycle");
    expect(screen.getByLabelText("run chain")).toBeInTheDocument();
  });

  it("run a chain then poll → nodes colour as steps settle + a terminal banner", async () => {
    const ws = nextWs();
    await signIn("user:ada", ws);
    await seedRule("r", EMIT_OK);

    const chain: Chain = {
      id: "run-me",
      name: "Run Me",
      steps: [
        { id: "a", rule: "r", needs: [] },
        { id: "b", rule: "r", needs: ["a"] },
      ],
    };
    await saveChain(chain);

    render(<ChainCanvas chain={chain} onSave={saveResult} />);

    const user = userEvent.setup();
    await user.click(screen.getByLabelText("run chain"));

    // A node reaches ok/green (data-colour="ok") and the banner shows a terminal status.
    await waitFor(
      async () => {
        const node = await screen.findByLabelText("step a");
        expect(node.getAttribute("data-colour")).toBe("ok");
      },
      { timeout: 10_000 },
    );
    const banner = await screen.findByLabelText("run status");
    expect(["success", "partialFailure"]).toContain(banner.getAttribute("data-status"));
  });

  it("a late getChainRun rebuilds the same colours from the records", async () => {
    const ws = nextWs();
    await signIn("user:ada", ws);
    await seedRule("r", EMIT_OK);

    const chain: Chain = {
      id: "late",
      name: "Late",
      steps: [{ id: "a", rule: "r", needs: [] }],
    };
    await saveChain(chain);
    const { run_id } = await invoke<{ run_id: string }>("chains_run", { id: "late", params: {} });

    // A late open reads ONE snapshot from the durable records and rebuilds the same colour.
    await waitFor(
      async () => {
        const snap = await getChainRun("late", run_id);
        expect(snap.status).toBe("success");
        expect(snapshotColours(snap).a).toBe("ok");
      },
      { timeout: 10_000 },
    );
  });
});
