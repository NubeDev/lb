// Access-console UI features (access-console scope), driven against a REAL spawned gateway (CLAUDE
// §9 — no fake): the effective-caps provenance detail (drives `authz.resolve`), the catalog-driven
// no-widening capability picker (drives `tools.catalog` + emits canonical `mcp:<tool>:call`), the
// overview tiles (honest counts), and the live-token revoke lever (drives `authz.revoke-tokens`).
// Each test logs into a UNIQUE workspace for isolation on the shared real node.

import { describe, expect, it, beforeAll, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { EffectiveCaps } from "./EffectiveCaps";
import { CapabilityPicker } from "./CapabilityPicker";
import { AccessOverview } from "./AccessOverview";
import { RevokeTokensLever } from "./RevokeTokensLever";
import { assignGrant } from "@/lib/admin/grants.api";
import { defineRole } from "@/lib/admin/roles.api";
import { createTeam } from "@/lib/admin/teams.api";
import { createUser } from "@/lib/admin/users.api";
import { addMember } from "@/lib/members/members.api";
import { CAP } from "@/lib/session/admin-caps";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `access-${n++}`;

beforeAll(() => useRealGateway());

describe("EffectiveCaps provenance (real gateway)", () => {
  it("tags a direct grant, a role, and a team-inherited cap with their sources", async () => {
    const ws = nextWs();
    await signInReal("user:alice", ws);
    await createUser("bob");
    // `operator` bundles a cap the dev admin holds (no-widening at define).
    await defineRole("operator", [CAP.grantsList]);
    await createTeam("facilities", "Facilities");
    await addMember("facilities", "bob");
    // bob's own DIRECT cap (a cap the admin holds) + bob's own role.
    await assignGrant("user:bob", CAP.teamsList);
    await assignGrant("user:bob", "role:operator");
    // the team's role grant → bob inherits the operator cap via team.
    await assignGrant("team:facilities", "role:operator");

    render(<EffectiveCaps subject="user:bob" />);
    // The teams.list cap is sourced `direct`.
    expect(await screen.findByText(CAP.teamsList)).toBeInTheDocument();
    expect(await screen.findByText("direct")).toBeInTheDocument();
    // grants.list (from the operator role) is sourced BOTH `role: operator` (bob's own role) AND
    // `via team: facilities` (the team's role grant).
    await waitFor(() => {
      expect(screen.getByText(CAP.grantsList)).toBeInTheDocument();
      expect(screen.getByText(/role: operator/)).toBeInTheDocument();
      expect(screen.getByText(/via team: facilities/)).toBeInTheDocument();
    });
  });

  it("renders an honest empty state for a subject with no caps", async () => {
    const ws = nextWs();
    await signInReal("user:alice", ws);
    await createUser("carol");
    render(<EffectiveCaps subject="user:carol" />);
    expect(
      await screen.findByText(/No capabilities\. Assign a role or a direct grant/),
    ).toBeInTheDocument();
  });
});

describe("CapabilityPicker (real gateway)", () => {
  it("offers the caller-authorized catalog and emits a canonical mcp:<tool>:call cap", async () => {
    const ws = nextWs();
    await signInReal("user:alice", ws);
    const onPick = vi.fn();
    render(<CapabilityPicker onPick={onPick} />);

    // The select populates from tools.catalog (caller-authorized → no widening).
    const select = await screen.findByLabelText("pick a capability to grant");
    await waitFor(() => {
      expect((select as HTMLSelectElement).options.length).toBeGreaterThan(1);
    });

    // Pick the first real tool option and confirm — onPick receives `mcp:<tool>:call`.
    const firstTool = (select as HTMLSelectElement).options[1].value;
    await userEvent.selectOptions(select, firstTool);
    await userEvent.click(screen.getByRole("button", { name: "Grant" }));
    expect(onPick).toHaveBeenCalledWith(`mcp:${firstTool}:call`);
    // The emitted cap is canonical (mcp:<tool>:call shape).
    expect(onPick.mock.calls[0][0]).toMatch(/^mcp:.+:call$/);
  });
});

describe("AccessOverview tiles (real gateway)", () => {
  it("renders honest counts for people/teams/roles seeded via the real write path", async () => {
    const ws = nextWs();
    await signInReal("user:alice", ws);
    await createUser("dan");
    await createUser("eve");
    await createTeam("eng", "Engineering");
    await defineRole("auditor", [CAP.userManage]);

    render(<AccessOverview ws={ws} caps={[CAP.authzResolve, CAP.userManage]} />);
    // People tile shows 2 (dan + eve); Teams 1; Roles 1 — wait for the async counts to settle.
    await waitFor(() => expect(screen.getByLabelText("People")).toHaveTextContent("2"));
    await waitFor(() => expect(screen.getByLabelText("Teams")).toHaveTextContent("1"));
    await waitFor(() => expect(screen.getByLabelText("Roles")).toHaveTextContent("1"));
  });
});

describe("RevokeTokensLever (real gateway)", () => {
  it("applies the live-token revoke and reports the consequence", async () => {
    const ws = nextWs();
    await signInReal("user:alice", ws);
    await createUser("bob");
    // Grant a cap the admin holds (no-widening at assign).
    await assignGrant("user:bob", CAP.userManage);

    render(
      <RevokeTokensLever
        subject="user:bob"
        caps={[CAP.authzRevokeTokens]}
        context="role revoke"
      />,
    );
    const btn = await screen.findByRole("button", { name: /Apply now — end active sessions/ });
    await userEvent.click(btn);
    // Success note: bob's token refused on next request + the grant count.
    await waitFor(() => {
      expect(screen.getByText(/current token is refused on the next request/)).toBeInTheDocument();
      expect(screen.getByText(/1 grant/)).toBeInTheDocument();
    });
  });

  it("is hidden when the session lacks the revoke cap", async () => {
    const ws = nextWs();
    await signInReal("user:alice", ws);
    const { container } = render(
      <RevokeTokensLever subject="user:bob" caps={[]} />,
    );
    await waitFor(() => expect(container).toBeEmptyDOMElement());
  });
});
