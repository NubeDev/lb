// RolesAdmin (admin-console redesign): the REAL role editor. Proves a role is built by CHECKING
// capabilities (not typing `role:<name>`), that the candidate caps are the admin's own session caps
// (no-widening), and that a defined role shows up with its cap count. Driven through useRoles + the
// real roles api + the admin fake.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { RolesAdmin } from "./RolesAdmin";
import { setSession } from "@/lib/session/session.store";
import { ADMIN_CAPS, CAP } from "@/lib/session/admin-caps";
import { __resetAdminFake } from "@/lib/ipc/admin.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace, caps: ADMIN_CAPS });
}

beforeEach(() => {
  __resetAdminFake();
  signIn("acme");
});
afterEach(() => __resetAdminFake());

describe("RolesAdmin", () => {
  it("creates a role by checking caps and lists it with its cap count", async () => {
    const user = userEvent.setup();
    render(<RolesAdmin ws="acme" caps={[CAP.userManage, CAP.teamsManage]} />);

    await user.click(screen.getByLabelText("new role"));
    await user.type(screen.getByLabelText("role name"), "operator");
    // The checklist offers the admin's own caps (no-widening).
    await user.click(screen.getByLabelText(`include ${CAP.userManage}`));
    await user.click(screen.getByLabelText("save role"));

    const table = await screen.findByRole("table");
    expect(within(table).getByText("operator")).toBeInTheDocument();
    // The cap count column reads 1 (the old UI showed nothing about a role's caps).
    expect(within(table).getByText("1")).toBeInTheDocument();
  });

  it("offers no caps to bundle when the admin holds none (no-widening)", () => {
    render(<RolesAdmin ws="acme" caps={[]} />);
    expect(
      screen.getByText("You hold no capabilities to bundle (no-widening)."),
    ).toBeInTheDocument();
  });
});
