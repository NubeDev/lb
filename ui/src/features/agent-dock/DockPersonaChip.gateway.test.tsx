// The dock persona chip proven END TO END over a REAL spawned gateway (no mocks, no fake backend —
// CLAUDE §9, persona-session #5). The chip + the per-invoke `persona` arg must never disagree (one
// gateway test pins this), so every case asserts BOTH what the chip displays AND what the wire
// payload carries:
//
//   - context match: dashboards → widget-builder, chip "from this page", payload.persona == id
//   - pin overrides: pinning data-analyst flips chip + payload to data-analyst
//   - pin survives a remount (durable within the tab via sessionStorage)
//   - pin in tab A never changes tab B (sessionStorage is per-tab — fresh storage ⇒ fresh focus)
//   - disabled personas absent from the picker (roster filter — curation working as intended)
//   - explicit disabled invoke surfaces a named error (the wall, not a silent degrade)
//   - second member (addMember first) sees their own focus — server fold is per-member
//
// Real records, real dispatch, real gateway. The dock's pin rides client-side (sessionStorage) by
// design (scope non-goal: "no server-side session/tab identity"); the durable job record is the audit.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { AgentDock } from "./AgentDock";
import { PageContextProvider, type PageContextSource } from "./PageContextProvider";
import { history, listChannels } from "@/lib/channel/channel.api";
import { parsePayload } from "@/lib/channel/payload.types";
import { setAgentConfig } from "@/lib/agent/config.api";
import { setDefaultPrefs } from "@/lib/prefs/set";
import { addMember } from "@/lib/membership/membership.api";
import {
  useRealGateway,
  signInReal,
  signInWithCaps,
} from "@/test/gateway-session";
import { setSession } from "@/lib/session/session.store";

let n = 0;
const nextWs = () => `dock-persona-${n++}`;

beforeAll(() => useRealGateway());

/** A page-context source pinned to one surface — the dock's resolution reads `surface` for the match. */
function surfaceCtx(surface: string, path = `/${surface}`): PageContextSource {
  return { capture: () => ({ surface, path, search: {} }) };
}

function fixedClock() {
  let t = 0;
  return () => ++t;
}

function renderDock(ws: string, principal: string, surface: string) {
  return render(
    <PageContextProvider source={surfaceCtx(surface)}>
      <AgentDock
        ws={ws}
        principal={principal}
        width={480}
        onWidth={() => {}}
        onClose={() => {}}
        now={fixedClock()}
      />
    </PageContextProvider>,
  );
}

/** Find the user's own dock channel for the session and read its history. */
async function readDockPayloads(ws: string, principal: string) {
  const rows = await listChannels(ws);
  const prefix = `dock-${principal.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")}-`;
  const dockCids = rows.map((r) => r.id).filter((id) => id.startsWith(prefix));
  if (dockCids.length === 0) return [];
  const items = await history(ws, dockCids[0]);
  return items
    .map((it) => parsePayload(it.body))
    .filter((p): p is Extract<typeof p, { kind: "agent" }> => p?.kind === "agent");
}

describe("Dock persona chip (real gateway — persona-session #5)", () => {
  it("chip matches the page surface AND the wire payload carries the same id (no divergence)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    sessionStorage.clear();
    await signInReal("user:ada", ws);
    // dashboards → widget-builder (its only surfaces-match). Real seeded roster, no roster constraint.
    renderDock(ws, "user:ada", "dashboards");
    await screen.findByLabelText("ask the agent");

    // The chip shows widget-builder + "from this page" caption.
    const chip = await screen.findByLabelText("persona focus");
    expect(chip.getAttribute("data-persona-id")).toBe("builtin.widget-builder");
    expect(chip.getAttribute("data-focus-reason")).toBe("context");
    expect(chip.getAttribute("title")).toContain("from this page");

    // Send → the durable agent payload's persona == widget-builder (chip and run agree).
    await user.type(screen.getByLabelText("ask the agent"), "summarize this dashboard");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("summarize this dashboard");

    await waitFor(async () => {
      const reqs = await readDockPayloads(ws, "user:ada");
      expect(reqs.length).toBeGreaterThan(0);
      expect(reqs[reqs.length - 1].persona).toBe("builtin.widget-builder");
    });
    cleanup();
  });

  it("a pin overrides the context suggestion AND survives a remount (sticky within the tab)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    sessionStorage.clear();
    await signInReal("user:ada", ws);
    renderDock(ws, "user:ada", "dashboards");
    await screen.findByLabelText("ask the agent");

    // Open the switcher and pin data-analyst (which does NOT match dashboards — proves pin overrides).
    await user.click(await screen.findByLabelText("persona focus"));
    await user.click(screen.getByLabelText("pin builtin.data-analyst"));

    // The chip now shows data-analyst + "pinned".
    await waitFor(() => {
      const chip = screen.getByLabelText("persona focus");
      expect(chip.getAttribute("data-persona-id")).toBe("builtin.data-analyst");
      expect(chip.getAttribute("data-focus-reason")).toBe("pinned");
    });

    // Pin survives a remount — the storage-backed focus restores on the next mount (durable in-tab).
    cleanup();
    renderDock(ws, "user:ada", "dashboards");
    const restored = await screen.findByLabelText("persona focus");
    await waitFor(() =>
      expect(restored.getAttribute("data-persona-id")).toBe("builtin.data-analyst"),
    );

    // Send → payload carries data-analyst (the pin), NOT widget-builder (the context match).
    await user.type(screen.getByLabelText("ask the agent"), "from a pinned focus");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("from a pinned focus");
    await waitFor(async () => {
      const reqs = await readDockPayloads(ws, "user:ada");
      expect(reqs.length).toBeGreaterThan(0);
      expect(reqs[reqs.length - 1].persona).toBe("builtin.data-analyst");
    });
    cleanup();
  });

  it("a pin in tab A never changes tab B (sessionStorage is per-tab — fresh storage ⇒ fresh focus)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();

    // Tab A: sign in, render, pin data-analyst.
    sessionStorage.clear();
    await signInReal("user:ada", ws);
    renderDock(ws, "user:ada", "dashboards");
    await screen.findByLabelText("ask the agent");
    await user.click(await screen.findByLabelText("persona focus"));
    await user.click(screen.getByLabelText("pin builtin.data-analyst"));
    await waitFor(() => {
      const chip = screen.getByLabelText("persona focus");
      expect(chip.getAttribute("data-persona-id")).toBe("builtin.data-analyst");
    });
    cleanup();

    // Tab B is a NEW browsing context ⇒ its OWN (empty) sessionStorage. Real browsers give each tab
    // its own session context; we model that by clearing the shared jsdom storage before the remount.
    sessionStorage.clear();
    renderDock(ws, "user:ada", "dashboards");
    const tabBChip = await screen.findByLabelText("persona focus");
    // No pin in tab B → the context match wins (widget-builder), NOT tab A's data-analyst pin.
    await waitFor(() =>
      expect(tabBChip.getAttribute("data-persona-id")).toBe("builtin.widget-builder"),
    );
    expect(tabBChip.getAttribute("data-persona-id")).not.toBe("builtin.data-analyst");
    cleanup();
  });

  it("disabled personas are absent from the picker (roster curation)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    sessionStorage.clear();
    // Admin signs in and disables data-analyst + extension-builder via the roster.
    await signInWithCaps("user:ada", ws, [
      "mcp:agent.config.set:call",
      "mcp:agent.config.get:call",
      "mcp:agent.persona.list:call",
      "mcp:agent.persona.get:call",
      "mcp:agent.persona.resolve:call",
      "bus:chan/*:pub",
      "bus:chan/*:sub",
      "mcp:agent.invoke:call",
    ]);
    // Disable data-analyst (set enabled_personas to all-built-ins EXCEPT data-analyst — materializes
    // the explicit list since the default None ⇒ all-enabled).
    await setAgentConfig({
      enabled_personas: [
        "builtin.flow-author",
        "builtin.widget-builder",
        "builtin.rules-author",
        "builtin.workspace-admin",
        "builtin.channels-operator",
        "builtin.system-manager",
        "builtin.extension-builder",
      ],
    });

    // Render the dock at the datasources surface — data-analyst WOULD match, but it's disabled, so the
    // chip falls through to "Workspace default" (no enabled persona matches "datasources" — only
    // data-analyst had it).
    renderDock(ws, "user:ada", "datasources");
    await screen.findByLabelText("ask the agent");
    const chip = await screen.findByLabelText("persona focus");
    expect(chip.getAttribute("data-persona-id")).toBe("");
    expect(chip.getAttribute("title")).toContain("Workspace default");

    // Open the switcher — data-analyst is NOT listed (curation hides it from the picker).
    await user.click(chip);
    const switcher = await screen.findByLabelText("persona switcher");
    expect(switcher.textContent).not.toContain("data-analyst");
    expect(switcher.textContent).toContain("widget-builder");
    cleanup();
  });
  it("an explicit invoke of a disabled persona surfaces a named error (the wall, not a silent degrade)", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, [
      "mcp:agent.config.set:call",
      "mcp:agent.config.get:call",
      "mcp:agent.persona.list:call",
      "mcp:agent.persona.get:call",
      "mcp:agent.persona.resolve:call",
      "mcp:agent.invoke:call",
    ]);
    // Disable data-analyst (a real persona the workspace would otherwise allow).
    await setAgentConfig({ enabled_personas: ["builtin.flow-author", "builtin.widget-builder"] });

    // Drive a direct `/agent/invoke` with a disabled persona id — the host's resolve_persona rejects
    // with a named disabled error (scope: curation must not be silently bypassable). The 400 body
    // surfaces verbatim as the error message (the route's `agent_status` mapping).
    const { invokeAgent } = await import("@/lib/agent/agent.api");
    await expect(
      invokeAgent(ws, "job-disabled", "test", { persona: "builtin.data-analyst" }),
    ).rejects.toThrow(/disabled/);
  });

  it("a second member (addMember first) gets their own server fold (per-member isolation)", async () => {
    const ws = nextWs();
    sessionStorage.clear();
    // Admin (user:ada) sets the WORKSPACE default to system-manager + adds bob to the workspace roster.
    await signInWithCaps("user:ada", ws, [
      "mcp:prefs.set_default:call",
      "mcp:members.manage:call",
    ]);
    await setDefaultPrefs({ agent_persona: "builtin.system-manager" });
    await addMember("user:bob");

    // user:bob signs in (NO member default of their own). The server fold lands on the ws default.
    const bobSession = await signInWithCaps("user:bob", ws, [
      "mcp:agent.persona.resolve:call",
      "mcp:prefs.set:call",
    ]);
    setSession(bobSession);

    const { resolveEffectivePersona } = await import("@/lib/agent/agentPersona.api");
    // No-id resolve returns the server's per-caller fold: bob has no member default → ws default wins.
    const eff1 = await resolveEffectivePersona();
    expect(eff1?.id).toBe("builtin.system-manager");

    // Bob sets their OWN default — member axis wins over ws default. The fold is per-member: bob's
    // record doesn't touch ada's ws default (per scope: the chain is structural, not a shared value).
    const { setPrefs } = await import("@/lib/prefs/set");
    await setPrefs({ agent_persona: "builtin.flow-author" });
    const eff2 = await resolveEffectivePersona();
    expect(eff2?.id).toBe("builtin.flow-author");
    cleanup();
  });
});
