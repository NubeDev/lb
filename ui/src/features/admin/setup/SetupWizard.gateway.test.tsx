// The onboarding wizard, driven against a REAL spawned gateway (CLAUDE §9 — no fake). Proves the flow
// the setup scope describes: create a person, put them on a team, grant the team a role, share a nav
// to the team, and PREVIEW the exact access — every step a real host verb re-checked server-side. The
// wizard is pure orchestration over the same `user.*` / `teams.*` / `members.*` / `grants.*` / `nav.*`
// verbs the People/Teams/Roles/Nav tabs use; the nav grants nothing (it's a lens). Mandatory
// capability-deny + workspace-isolation coverage included. Each test logs into a UNIQUE workspace.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SetupWizard } from "./SetupWizard";
import { CAP } from "@/lib/session/admin-caps";
import { listUsers } from "@/lib/admin/users.api";
import { listMembers } from "@/lib/members/members.api";
import { listGrants, resolveCaps } from "@/lib/admin/grants.api";
import { defineRole } from "@/lib/admin/roles.api";
import { getNav, listNavShares } from "@/lib/nav";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `setup-${n++}`;

// The admin caps the wizard's writes need (the dev login carries these; we pass them for display).
const ADMIN_CAPS = [
  CAP.userManage,
  CAP.teamsManage,
  CAP.grantsAssign,
  CAP.navList,
  CAP.navGet,
  CAP.navSave,
  CAP.navShare,
  CAP.navResolve,
];

beforeAll(() => useRealGateway());

describe("SetupWizard (real gateway)", () => {
  it("onboards a new person end to end: create → team → role + nav → preview", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // Seed the REAL role the Access step grants — deliberately BROAD (dashboards + rules + flows) so
    // that the 1-item nav we build is what limits the rail, proving the nav REPLACES the surface list
    // (the bug: the preview used to show every cap-allowed page regardless of the nav).
    await defineRole("ops", [CAP.dashboardList, CAP.rulesRun, CAP.flowsList]);

    render(<SetupWizard ws={ws} caps={ADMIN_CAPS} />);

    // ── Step 1 — create a new person "bianca" ──
    await user.click(await screen.findByText("New user"));
    await user.type(screen.getByLabelText("New user id"), "bianca");
    await user.click(screen.getByLabelText("Create user"));
    // The real user.create landed — bianca is in the workspace roster.
    await waitFor(async () => expect((await listUsers()).some((u) => u.user === "bianca")).toBe(true));
    // Continue is disabled until the created user is selected in state — wait for it to enable, then
    // advance (a disabled-button click is a no-op that would silently strand us on step 1).
    await waitFor(() => expect(screen.getByLabelText("Continue")).toBeEnabled());
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 2 — create team "ops-team" and add bianca ──
    await screen.findByText("Put bianca on a team"); // step advanced
    await user.click(await screen.findByText("New team"));
    await user.type(screen.getByLabelText("New team id"), "ops-team");
    await user.click(screen.getByLabelText("Create team"));
    // Wait for the created team to be selected (button enables) before advancing — same commit race
    // as step 1 (a disabled click is a no-op that strands us with no membership edge written).
    await waitFor(() =>
      expect(screen.getByLabelText("Add to team and continue")).toBeEnabled(),
    );
    await user.click(screen.getByLabelText("Add to team and continue"));
    // The real members_add edge is written. The wizard stores BARE ids (`bianca`, not `user:bianca`)
    // because the authz resolver matches team membership on the bare user name (`m == user`, where
    // `Subject::User("bianca")` drops the prefix) — so bare storage is what makes cap-inheritance work.
    await waitFor(async () => expect(await listMembers("ops-team")).toContain("bianca"));

    // ── Step 3 — grant the team the "ops" role AND BUILD a new nav inline (page access), then apply ──
    await user.selectOptions(await screen.findByLabelText("Role"), "ops");
    // Switch to "Build a new nav" — the SAME shared composer the Nav tab uses is embedded here.
    await user.click(screen.getByLabelText("Build a new nav"));
    await user.type(await screen.findByLabelText("Nav title"), "Ops Menu");
    // Add a Dashboards surface via the composer's add-item form (default kind=surface).
    await user.selectOptions(screen.getByLabelText("Surface"), "dashboards");
    await user.click(screen.getByLabelText("Add item"));
    await waitFor(() =>
      expect(within(screen.getByTestId("nav-items")).getByText("dashboards")).toBeInTheDocument(),
    );
    await user.click(screen.getByLabelText("Apply access and preview"));

    // The real grant landed on the TEAM (so bianca inherits it), the nav was CREATED via nav.save
    // (slug id "ops-menu"), and shared to the team — all through the wizard, no pre-seeding.
    await waitFor(async () =>
      expect(await listGrants("team:ops-team")).toContain("role:ops"),
    );
    // The nav is created + shared as part of Apply (async) — wait for the persisted record.
    await waitFor(async () => {
      const created = await getNav("ops-menu");
      expect(created.title).toBe("Ops Menu");
      expect(created.items.map((i) => i.surface)).toContain("dashboards");
    });
    await waitFor(async () =>
      expect(await listNavShares("ops-menu")).toContain("team:ops-team"),
    );

    // ── Step 4 — the preview resolves bianca's EFFECTIVE caps and shows the Dashboards page ──
    // (she inherits dashboard.list via role:ops via team:ops-team — the honest lens).
    const wizard = screen.getByTestId("setup-wizard");
    await waitFor(() => expect(within(wizard).getByText("bianca is ready")).toBeInTheDocument());
    await waitFor(() =>
      expect(within(wizard).getByText("bianca’s sidebar")).toBeInTheDocument(),
    );
    // The provenance chip proves WHERE it came from (via the team's role).
    await waitFor(() => expect(within(wizard).getByText("role: ops")).toBeInTheDocument());

    // ── THE FIX ── the preview rail renders the NAV (its 1 item), not the whole cap-allowed set.
    // bianca's role grants rules + flows too, but the applied nav lists only Dashboards — so the rail
    // shows Dashboards and NOT Rules/Flows. The nav replaces the surface list (matches real NavRail).
    const rail = await within(wizard).findByTestId("preview-rail");
    await waitFor(() => expect(within(rail).getByText("Dashboards")).toBeInTheDocument());
    expect(within(rail).queryByText("Rules")).not.toBeInTheDocument();
    expect(within(rail).queryByText("Flows")).not.toBeInTheDocument();
    // Exactly one row — the single dashboards item survived the cap-strip.
    expect(within(rail).getAllByRole("listitem")).toHaveLength(1);

    // Cross-check the resolver directly: bianca DOES hold the broad caps (the nav hides, never grants).
    const caps = (await resolveCaps("user:bianca")).map((c) => c.cap);
    expect(caps).toContain(CAP.dashboardList);
    expect(caps).toContain(CAP.rulesRun);
  });

  it("denies the writes without the caps — the gateway is the boundary, not the UI", async () => {
    // A caller with ONLY navResolve (no user/teams/grants management) opens the wizard: the display
    // gate hides it, AND a direct verb call is refused server-side (defense in depth).
    const ws = nextWs();
    await signInWithCaps("user:ben", ws, [CAP.navResolve]);

    render(<SetupWizard ws={ws} caps={[CAP.navResolve]} />);
    // The wizard chrome is withheld for a non-authoring caller (display gate).
    expect(screen.queryByTestId("setup-wizard")).not.toBeInTheDocument();
    expect(
      screen.getByText(/people, teams, or grants management capabilities/),
    ).toBeInTheDocument();

    // And the boundary is the server: a raw grants.assign is refused for this caller.
    const { assignGrant } = await import("@/lib/admin/grants.api");
    await expect(assignGrant("team:x", "role:ops")).rejects.toThrow();
  });

  it("isolates workspaces — a person onboarded in ws-A is invisible in ws-B", async () => {
    // Onboard "carl" in ws-A.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    const { createUser } = await import("@/lib/admin/users.api");
    await createUser("carl");
    expect((await listUsers()).some((u) => u.user === "carl")).toBe(true);

    // ws-B (same operator identity, different workspace wall) never sees carl.
    const wsB = nextWs();
    await signInReal("user:ada", wsB);
    expect((await listUsers()).some((u) => u.user === "carl")).toBe(false);
    // And resolving carl in ws-B yields no caps (he doesn't exist here).
    expect(await resolveCaps("user:carl")).toEqual([]);
  });
});
