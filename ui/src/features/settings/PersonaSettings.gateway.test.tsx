// The Persona Settings surface proven END TO END over a REAL spawned, SEEDED gateway (no mocks, no
// fake backend — CLAUDE §9): the seeded built-in `builtin.data-analyst` lists in the picker, an admin
// creates/edits/deletes a custom persona and picks one as the active selection (re-read
// `agent.config` asserts `active_persona`), the Effective-tools view renders the resolved tools and
// marks an out-of-catalog persona tool excluded, the Permissions pane round-trips `agent.policy.set`,
// and a member without the write caps sees the pickers + policy pane read-only.
//
// The test gateway boot-seeds `builtin.data-analyst` through the real seeder; every write hits the
// real verbs over the MCP bridge. Persona ids are opaque — no branch on a specific id here.

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
];
// A member with the reads but NONE of the write caps — the deny path.
const MEMBER_CAPS = [...PERSONA_READ, ...POLICY_READ, ...CATALOG_READ];

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

describe("Persona Settings — seeded built-in + custom CRUD + effective-tools + policy over a real gateway", () => {
  it("lists the seeded built-in persona and picking writes active_persona", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    // The seeded built-in lists over the real verb.
    const seeded = await listPersonas();
    expect(seeded.some((p) => p.id === "builtin.data-analyst")).toBe(true);

    renderSettings(ws, session.caps);
    await openAgentTab(user);

    // The built-in renders with a Built-in badge and NO edit/delete affordance (read-only tier).
    await screen.findByLabelText("persona builtin.data-analyst");
    expect(screen.queryByLabelText("edit builtin.data-analyst")).toBeNull();
    expect(screen.queryByLabelText("delete builtin.data-analyst")).toBeNull();

    // Pick it → writes `agent.config.active_persona`; the entry becomes Active.
    await user.click(screen.getByLabelText("use builtin.data-analyst"));
    await waitFor(() =>
      expect(
        screen.getByLabelText("persona builtin.data-analyst").getAttribute("data-active"),
      ).toBe("true"),
    );

    // Re-read the live config: the pointer persisted (not just component state).
    await waitFor(async () => {
      const cfg = await getAgentConfig();
      expect(cfg?.active_persona).toBe("builtin.data-analyst");
    });
    cleanup();
  });

  it("an admin creates, edits, and deletes a custom persona", async () => {
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
    await user.click(screen.getByLabelText("save persona"));

    // It appears in the catalog (a real list read), tagged custom with edit/delete affordances.
    const custom = await screen.findByLabelText("persona my-analyst");
    expect(custom).toBeTruthy();
    expect(screen.getByLabelText("edit my-analyst")).toBeTruthy();

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

  it("a member without write caps sees the personas + policy read-only (no create/pick/save)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:bob", ws, MEMBER_CAPS);

    renderSettings(ws, session.caps);
    await openAgentTab(user);
    await screen.findByLabelText("persona builtin.data-analyst");

    // No "new persona", no "use" pick — the member cannot manage or pick.
    expect(screen.queryByLabelText("new custom persona")).toBeNull();
    expect(screen.queryByLabelText("use builtin.data-analyst")).toBeNull();
    // The policy pane is read-only: no Save affordance.
    expect(screen.queryByLabelText("save policy")).toBeNull();
    expect(screen.queryByLabelText("add rule")).toBeNull();
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
      builtin: false,
    };
    // The real verb 403s — the cap-deny is the wall, not the UI gate.
    await expect(createPersona(persona)).rejects.toThrow();
    // Nothing persisted.
    const ps = await listPersonas();
    expect(ps.some((p) => p.id === "sneaky")).toBe(false);
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
