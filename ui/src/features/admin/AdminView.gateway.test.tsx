// Cap-gated VISIBILITY (admin-console redesign), driven against a REAL spawned gateway (CLAUDE §9 — no
// fake). Toggling the `caps` PROP shows/hides the four admin tabs (People · Teams · Roles ·
// Workspaces). This is a CONVENIENCE gate only — the gateway re-checks every verb server-side, and the
// server deny on a forged call is proven in Rust (role/gateway/tests/admin_routes_test.rs). Here we
// assert only that the UI HIDES controls a session lacks the cap for — never that hiding is the
// boundary. The visible tab's child view mounts and loads from the real backend, so each test signs
// into a UNIQUE real workspace; the gating itself is a pure function of the `caps` prop.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";

import { AdminView } from "./AdminView";
import { CAP } from "@/lib/session/admin-caps";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `adminview-${n++}`;

beforeAll(() => useRealGateway());

describe("AdminView cap-gated tab visibility (real gateway)", () => {
  it("a full-admin session shows every tab", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const caps = [CAP.workspaceDelete, CAP.userManage, CAP.teamsManage, CAP.grantsAssign];
    render(<AdminView ws={ws} caps={caps} />);
    expect(screen.getByRole("tab", { name: "People" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Teams" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Roles" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Workspaces" })).toBeInTheDocument();
  });

  it("a session with only user.manage shows ONLY the People tab", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<AdminView ws={ws} caps={[CAP.userManage]} />);
    expect(screen.getByRole("tab", { name: "People" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Workspaces" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Teams" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Roles" })).not.toBeInTheDocument();
  });

  it("the Roles tab is gated on grants.assign", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<AdminView ws={ws} caps={[CAP.grantsAssign]} />);
    expect(screen.getByRole("tab", { name: "Roles" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "People" })).not.toBeInTheDocument();
  });
});
