// The Persona Settings surface proven END TO END over a REAL spawned, SEEDED gateway (no mocks, no
// fake backend — CLAUDE §9): the seeded built-in `builtin.data-analyst` lists in the roster; an admin
// curates the workspace roster via the enable/disable toggle (writing `agent.config.enabled_personas`),
// sets a member default (`prefs.set { agent_persona }`) and a workspace default
// (`prefs.set_default { agent_persona }`); the Effective-tools view renders the resolved tools; the
// Permissions pane round-trips `agent.policy.set`; and a member without the roster/default-write caps
// sees the roster + defaults read-only (still allowed to set their own default).
//
// The test gateway boot-seeds `builtin.data-analyst` through the real seeder; every write hits the
// real verbs over the MCP bridge. Persona ids are opaque — no branch on a specific id here.
//
// (persona-session #5 rework: the "Use" pick + `active_persona` are GONE. The roster +
// member/ws-default setters are the new surface. The dock-side chip + per-invoke `persona` arg are
// proven in `AgentDock.gateway.test.tsx`.)

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ThemeProvider } from "@/lib/theme";
import { SettingsHarness } from "./SettingsHarness";
import {
  listPersonas,
  createPersona,
  type Persona,
} from "@/lib/agent/agentPersona.api";
import { getAgentConfig } from "@/lib/agent/config.api";
import { getAgentPolicy } from "@/lib/agent/policy.api";
import { getPrefs } from "@/lib/prefs/get";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `persona-settings-${n++}`;

const PERSONA_READ = [
  "mcp:agent.persona.list:call",
  "mcp:agent.persona.get:call",
  "mcp:agent.persona.resolve:call",
];
const POLICY_READ = ["mcp:agent.policy.get:call"];
const CATALOG_READ = ["mcp:tools.catalog:call", "mcp:agent.config.get:call"];

const ADMIN_CAPS = [
  ...PERSONA_READ,
  ...POLICY_READ,
  ...CATALOG_READ,
  "mcp:agent.config.set:call",
  "mcp:agent.persona.create:call",
  "mcp:agent.persona.update:call",
  "mcp:agent.persona.delete:call",
  "mcp:agent.policy.set:call",
  "mcp:prefs.get:call",
  "mcp:prefs.set:call",
  "mcp:prefs.set_default:call",
];
// A member with the reads + own-prefs write but NOT the roster/ws-default/policy-write caps.
const MEMBER_CAPS = [
  ...PERSONA_READ,
  ...POLICY_READ,
  ...CATALOG_READ,
  "mcp:prefs.get:call",
  "mcp:prefs.set:call",
];

beforeAll(() => useRealGateway());

/** Render the Settings harness under a ThemeProvider — the `Tabs` panel's `Reveal` reads the motion
 *  pref via `useTheme`, so the provider is required to mount the tab body. */
function renderSettings(ws: string, caps: string[] | undefined) {
  return render(
    <ThemeProvider>
      <SettingsHarness ws={ws} caps={caps} />
    </ThemeProvider>,
  );
}

/** Open Settings → Agent tab in a rendered harness. */
async function openAgentTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByLabelText("Agent"));
}

describe("Persona Settings — roster + member/ws defaults + CRUD + effective-tools + policy over a real gateway", () => {
  it("lists the seeded built-in persona (enabled by default); toggling writes enabled_personas", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    // The seeded built-in lists over the real verb, with `enabled: true` by default (no roster set).
    const seeded = await listPersonas();
    expect(seeded.some((p) => p.id === "builtin.data-analyst" && p.enabled)).toBe(true);

    renderSettings(ws, session.caps);
    await openAgentTab(user);

    // The built-in renders with a Built-in badge and NO edit/delete affordance (read-only tier).
    const row = await screen.findByLabelText("persona builtin.data-analyst");
    expect(row.getAttribute("data-enabled")).toBe("true");
    expect(screen.queryByLabelText("edit builtin.data-analyst")).toBeNull();
    expect(screen.queryByLabelText("delete builtin.data-analyst")).toBeNull();

    // Disable it via the roster toggle → writes `agent.config.enabled_personas` (every persona except
    // this one — None ⇒ all enabled, so disabling materializes the explicit all-but-this list).
    await user.click(screen.getByLabelText("disable builtin.data-analyst"));
    await waitFor(() =>
      expect(
        screen.getByLabelText("persona builtin.data-analyst").getAttribute("data-enabled"),
      ).toBe("false"),
    );

    // Re-read the live config: the roster persisted (not just component state).
    await waitFor(async () => {
      const cfg = await getAgentConfig();
      const roster = cfg?.enabled_personas;
      expect(Array.isArray(roster)).toBe(true);
      expect(roster!.includes("builtin.data-analyst")).toBe(false);
    });
    cleanup();
  });

  it("an admin creates, edits, and deletes a custom persona (with surfaces)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    renderSettings(ws, session.caps);
    await openAgentTab(user);
    await screen.findByLabelText("persona builtin.data-analyst");

    // Create a custom persona through the real editor → real `agent.persona.create`.
    await user.click(screen.getByLabelText("new custom persona"));
    await user.type(screen.getByLabelText("Label"), "My analyst");
    await user.type(
      screen.getByLabelText("Identity"),
      "You are a focused analyst.",
    );
    await user.type(screen.getByLabelText("granted tools input"), "series.query");
    await user.click(screen.getByLabelText("add granted tools"));
    // Surfaces — the dock's context-match vocabulary (a record-only edit, rule 10).
    await user.type(screen.getByLabelText("surfaces input"), "data");
    await user.click(screen.getByLabelText("add surfaces"));
    await user.click(screen.getByLabelText("save persona"));

    // It appears in the catalog (a real list read), tagged custom with edit/delete affordances.
    const custom = await screen.findByLabelText("persona my-analyst");
    expect(custom).toBeTruthy();
    expect(screen.getByLabelText("edit my-analyst")).toBeTruthy();
    expect(custom.textContent).toContain("surfaces: data");

    // Edit it: change the label → real `agent.persona.update`.
    await user.click(screen.getByLabelText("edit my-analyst"));
    const labelInput = screen.getByLabelText("Label") as HTMLInputElement;
    await user.clear(labelInput);
    await user.type(labelInput, "My analyst v2");
    await user.click(screen.getByLabelText("save persona"));
    await waitFor(async () => {
      const ps = await listPersonas();
      expect(ps.find((p) => p.id === "my-analyst")?.label).toBe("My analyst v2");
    });

    // The editor closed and the catalog re-rendered with the updated persona — wait for its row.
    await screen.findByLabelText("delete my-analyst");

    // Delete it → real `agent.persona.delete`; it leaves the catalog.
    await user.click(screen.getByLabelText("delete my-analyst"));
    await waitFor(() => expect(screen.queryByLabelText("persona my-analyst")).toBeNull());
    cleanup();

    await waitFor(async () => {
      const ps = await listPersonas();
      expect(ps.some((p) => p.id === "my-analyst")).toBe(false);
    });
  });

  it("an admin sets + clears their member default and the workspace default (prefs chain)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    renderSettings(ws, session.caps);
    await openAgentTab(user);
    await screen.findByLabelText("persona builtin.data-analyst");

    // Set my default → writes `agent_persona` to the viewer's OWN prefs.
    await user.click(screen.getByLabelText("set my default builtin.data-analyst"));
    await screen.findByLabelText("clear my default builtin.data-analyst");

    // Re-read the viewer's own stored prefs: the agent_persona axis persisted.
    await waitFor(async () => {
      const prefs = await getPrefs();
      expect(prefs?.agent_persona).toBe("builtin.data-analyst");
    });

    // Set workspace default → writes `agent_persona` to the workspace-default prefs (admin).
    await user.click(screen.getByLabelText("set workspace default builtin.data-analyst"));
    await screen.findByLabelText("clear workspace default builtin.data-analyst");
    // The ws-default id is tracked optimistically (no read verb); we only assert the UI flag updated.

    // Clear my default → writes "" (the MERGE-can't-write-null workaround). The "My default" badge drops.
    await user.click(screen.getByLabelText("clear my default builtin.data-analyst"));
    await waitFor(() =>
      expect(screen.queryByLabelText("clear my default builtin.data-analyst")).toBeNull(),
    );
    // The axis is "" (cleared), which `getPrefs` already filters out for display.
    await waitFor(async () => {
      const prefs = await getPrefs();
      expect(prefs?.agent_persona ?? "").toBe("");
    });
    cleanup();
  });

  it("a member without roster/ws-default caps sees the roster + ws-default read-only but can set their own default", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:bob", ws, MEMBER_CAPS);

    renderSettings(ws, session.caps);
    await openAgentTab(user);
    await screen.findByLabelText("persona builtin.data-analyst");

    // No "new persona", no roster toggle, no "Set as workspace default" — the member cannot curate
    // or set the workspace default. The policy pane is read-only too.
    expect(screen.queryByLabelText("new custom persona")).toBeNull();
    expect(screen.queryByLabelText("disable builtin.data-analyst")).toBeNull();
    expect(
      screen.queryByLabelText("set workspace default builtin.data-analyst"),
    ).toBeNull();
    expect(screen.queryByLabelText("save policy")).toBeNull();
    expect(screen.queryByLabelText("add rule")).toBeNull();

    // But the member CAN set their OWN default (member-level `prefs.set`).
    await user.click(screen.getByLabelText("set my default builtin.data-analyst"));
    await waitFor(async () => {
      const prefs = await getPrefs();
      expect(prefs?.agent_persona).toBe("builtin.data-analyst");
    });
    cleanup();
  });

  it("a member without agent.persona.create is denied at the verb (no widening)", async () => {
    const ws = nextWs();
    await signInWithCaps("user:bob", ws, MEMBER_CAPS);

    const persona: Persona = {
      id: "sneaky",
      label: "Sneaky",
      identity: "nope",
      granted_tools: [],
      grounding_skills: [],
      extends: [],
      surfaces: [],
      builtin: false,
    };
    // The real verb 403s — the cap-deny is the wall, not the UI gate.
    await expect(createPersona(persona)).rejects.toThrow();
    // Nothing persisted.
    const ps = await listPersonas();
    expect(ps.some((p) => p.id === "sneaky")).toBe(false);
  });

  it("a member without agent.config.set is denied a roster write at the verb", async () => {
    const ws = nextWs();
    await signInWithCaps("user:bob", ws, MEMBER_CAPS);
    // The real `agent.config.set` 403s — the member cannot curate the workspace roster.
    const { setAgentConfig } = await import("@/lib/agent/config.api");
    await expect(setAgentConfig({ enabled_personas: [] })).rejects.toThrow();
    // The roster is unchanged (still None ⇒ all enabled).
    const cfg = await getAgentConfig();
    expect(cfg?.enabled_personas ?? null).toBeNull();
  });

  it("Effective tools marks an out-of-catalog persona tool as excluded", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    // A custom persona granting a tool that is NOT in the caller's reachable catalog — the effective
    // view must mark it excluded ("not granted") without widening anything.
    await createPersona({
      id: "narrow",
      label: "Narrow",
      identity: "focused",
      granted_tools: ["definitely.not.a.real.tool"],
      grounding_skills: [],
      extends: [],
      surfaces: [],
      builtin: false,
    });

    renderSettings(ws, session.caps);
    await openAgentTab(user);
    await screen.findByLabelText("persona narrow");

    // Focus the Effective-tools view on the custom persona.
    const select = (await screen.findByLabelText(
      "effective persona select",
    )) as HTMLSelectElement;
    await user.selectOptions(select, "narrow");

    // The out-of-catalog tool renders as excluded.
    const row = await screen.findByLabelText("tool definitely.not.a.real.tool");
    await waitFor(() => expect(row.getAttribute("data-status")).toBe("excluded"));
    expect(row.textContent).toContain("not granted");
    cleanup();
  });

  it("Permissions pane round-trips an Ask rule via agent.policy.set", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    renderSettings(ws, session.caps);
    await openAgentTab(user);
    await screen.findByLabelText("persona builtin.data-analyst");

    // Add an Ask rule and save → real `agent.policy.set`.
    await user.click(screen.getByLabelText("add rule"));
    await user.type(screen.getByLabelText("rule 0 tool"), "flows.save");
    await user.selectOptions(screen.getByLabelText("rule 0 effect"), "ask");
    await user.click(screen.getByLabelText("save policy"));
    await screen.findByText("Saved.");

    // Re-read the live policy: the rule persisted.
    await waitFor(async () => {
      const rules = await getAgentPolicy();
      expect(rules.some((r) => r.tool === "flows.save" && r.effect === "ask")).toBe(true);
    });
    cleanup();
  });
});
