// RolesAdmin (admin-console redesign), driven against a REAL spawned gateway (CLAUDE §9 — no fake).
// Proves a role is built by CHECKING capabilities (not typing `role:<name>`), that the candidate caps
// are the admin's own caps (no-widening), and that a defined role shows up with its cap count. The
// checklist's candidate caps come from the `caps` prop; save goes through the real useRoles →
// roles.define → /admin/roles route, and the table re-reads the real roles.list. The dev login grants
// the full admin cap set, so a role bundling `user.manage` is accepted server-side (no-widening holds).
// Each test logs into a UNIQUE workspace for isolation on the shared node.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { RolesAdmin } from "./RolesAdmin";
import { CAP } from "@/lib/session/admin-caps";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `roles-${n++}`;

beforeAll(() => useRealGateway());

describe("RolesAdmin (real gateway)", () => {
  it("creates a role by checking caps and lists it with its cap count", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<RolesAdmin ws={ws} caps={[CAP.userManage, CAP.teamsManage]} />);

    await user.click(screen.getByLabelText("new role"));
    await user.type(screen.getByLabelText("role name"), "operator");
    // The checklist offers the admin's own caps (no-widening).
    await user.click(screen.getByLabelText(`include ${CAP.userManage}`));
    await user.click(screen.getByLabelText("save role"));

    const table = await screen.findByRole("table");
    expect(await within(table).findByText("operator")).toBeInTheDocument();
    // The cap count column reads 1 (the old UI showed nothing about a role's caps).
    expect(within(table).getByText("1")).toBeInTheDocument();
  });

  it("offers no caps to bundle when the admin holds none (no-widening)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<RolesAdmin ws={ws} caps={[]} />);
    expect(
      await screen.findByText("You hold no capabilities to bundle (no-widening)."),
    ).toBeInTheDocument();
  });

  it("deletes a custom role (cascade) over the real route", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<RolesAdmin ws={ws} caps={[CAP.rolesManage, CAP.userManage, CAP.rolesDefine]} />);

    // Create a role, then delete it.
    await user.click(screen.getByLabelText("new role"));
    await user.type(screen.getByLabelText("role name"), "operator");
    await user.click(screen.getByLabelText(`include ${CAP.userManage}`));
    await user.click(screen.getByLabelText("save role"));
    const table = await screen.findByRole("table");
    expect(await within(table).findByText("operator")).toBeInTheDocument();

    await user.click(screen.getByLabelText("delete role operator"));
    await user.click(screen.getByRole("button", { name: "confirm action" }));

    // The role is gone from the table; the result note reports the cascade (0 assignees here).
    await waitFor(() =>
      expect(within(table).queryByText("operator")).not.toBeInTheDocument(),
    );
    expect(await screen.findByText(/un-assign/i)).toBeInTheDocument();
  });
});
