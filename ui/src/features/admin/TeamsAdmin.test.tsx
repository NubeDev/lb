// TeamsAdmin (admin-console redesign): create a team, see it in the table, select it, add/remove a
// member inline (the old separate Members tab is folded in — no typing a team id), and delete with the
// cascade consequence. Driven through the real useDirectory hook + apis + the admin/members fakes.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { TeamsAdmin } from "./TeamsAdmin";
import { setSession } from "@/lib/session/session.store";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { __resetAdminFake } from "@/lib/ipc/admin.fake";
import { __resetMembersFake } from "@/lib/ipc/members.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace, caps: ADMIN_CAPS });
}

beforeEach(() => {
  __resetAdminFake();
  __resetMembersFake();
  signIn("acme");
});
afterEach(() => {
  __resetAdminFake();
  __resetMembersFake();
});

describe("TeamsAdmin", () => {
  it("creates a team, then adds a member inline (no typing a team id)", async () => {
    const user = userEvent.setup();
    render(<TeamsAdmin ws="acme" />);

    await user.click(screen.getByLabelText("new team"));
    await user.type(screen.getByLabelText("new team id"), "facilities");
    await user.click(screen.getByRole("button", { name: "Create" }));

    // The team appears in the table; selecting it reveals its (empty) member list + add form.
    await user.click(await screen.findByLabelText("select facilities"));
    await user.type(screen.getByLabelText("add member"), "bob");
    await user.click(screen.getByLabelText("add member to team"));

    expect(await screen.findByText("bob")).toBeInTheDocument();
    expect(screen.getByLabelText("remove bob")).toBeInTheDocument();
  });

  it("delete shows the member count + cascade and removes the team", async () => {
    const user = userEvent.setup();
    render(<TeamsAdmin ws="acme" />);

    await user.click(screen.getByLabelText("new team"));
    await user.type(screen.getByLabelText("new team id"), "facilities");
    await user.click(screen.getByRole("button", { name: "Create" }));
    await user.click(await screen.findByLabelText("select facilities"));

    // Add one member so the consequence reads "1 member".
    await user.type(screen.getByLabelText("add member"), "bob");
    await user.click(screen.getByLabelText("add member to team"));
    await screen.findByText("bob");

    await user.click(screen.getByLabelText("delete team facilities"));
    expect(await screen.findByTestId("consequence")).toHaveTextContent(/Removes 1 member/i);
    expect(screen.getByTestId("consequence")).toHaveTextContent(/cascade/i);
    await user.click(screen.getByLabelText("confirm action"));

    expect(await screen.findByText("No teams yet.")).toBeInTheDocument();
  });
});
