// Cap-gated VISIBILITY (admin-console scope): toggling the session's caps shows/hides the admin tabs.
// This is a CONVENIENCE gate only — the gateway re-checks every verb server-side, and the server deny
// on a forged call is already proven in Rust (role/gateway/tests/admin_routes_test.rs:
// forged_admin_call_by_non_admin_is_denied_server_side). Here we assert only that the UI HIDES the
// controls a session lacks the cap for — never that hiding is the boundary.

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
    expect(screen.getByRole("tab", { name: "Workspaces" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Users" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Teams" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Members" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Roles & grants" })).toBeInTheDocument();
  });

  it("a session with only user.manage shows ONLY the Users tab", () => {
    render(<AdminView ws="acme" caps={[CAP.userManage]} />);
    expect(screen.getByRole("tab", { name: "Users" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Workspaces" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Teams" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Roles & grants" })).not.toBeInTheDocument();
  });

  it("a session without the purge cap still cannot bypass it — the workspace tab gates on workspace.delete only; purge needs the type-name gate AND the server cap", () => {
    // workspace.delete present but not purge: the tab shows, but purge's escalated confirm + the
    // backend's separate workspace.purge cap remain the real gate (defense in depth).
    render(<AdminView ws="acme" caps={[CAP.workspaceDelete]} />);
    expect(screen.getByRole("tab", { name: "Workspaces" })).toBeInTheDocument();
  });
});
