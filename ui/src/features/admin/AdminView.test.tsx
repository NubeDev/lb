// Cap-gated VISIBILITY (admin-console redesign): toggling the session's caps shows/hides the four
// admin tabs (People · Teams · Roles · Workspaces). This is a CONVENIENCE gate only — the gateway
// re-checks every verb server-side, and the server deny on a forged call is proven in Rust
// (role/gateway/tests/admin_routes_test.rs). Here we assert only that the UI HIDES controls a session
// lacks the cap for — never that hiding is the boundary.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { AdminView } from "./AdminView";
import { CAP } from "@/lib/session/admin-caps";
import { setSession } from "@/lib/session/session.store";
import { __resetAdminFake } from "@/lib/ipc/admin.fake";
import { __resetWorkspaceFake } from "@/lib/ipc/workspace.fake";
import { __resetMembersFake } from "@/lib/ipc/members.fake";

beforeEach(() => {
  setSession({ token: "t", principal: "user:ada", workspace: "acme" });
  __resetAdminFake();
  __resetWorkspaceFake();
  __resetMembersFake();
});
afterEach(() => __resetAdminFake());

describe("AdminView cap-gated tab visibility", () => {
  it("a full-admin session shows every tab", () => {
    const caps = [CAP.workspaceDelete, CAP.userManage, CAP.teamsManage, CAP.grantsAssign];
    render(<AdminView ws="acme" caps={caps} />);
    expect(screen.getByRole("tab", { name: "People" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Teams" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Roles" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Workspaces" })).toBeInTheDocument();
  });

  it("a session with only user.manage shows ONLY the People tab", () => {
    render(<AdminView ws="acme" caps={[CAP.userManage]} />);
    expect(screen.getByRole("tab", { name: "People" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Workspaces" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Teams" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Roles" })).not.toBeInTheDocument();
  });

  it("the Roles tab is gated on grants.assign", () => {
    render(<AdminView ws="acme" caps={[CAP.grantsAssign]} />);
    expect(screen.getByRole("tab", { name: "Roles" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "People" })).not.toBeInTheDocument();
  });
});
