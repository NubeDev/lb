// TeamsAdmin (admin-console scope): create a team; delete it (single confirm, shows the member count +
// cascade consequence). The count is read live from the members fake so the copy is accurate. Driven
// through the real hook + api + the admin/members fakes.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { TeamsAdmin } from "./TeamsAdmin";
import { setSession } from "@/lib/session/session.store";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { __resetAdminFake } from "@/lib/ipc/admin.fake";
import { __resetMembersFake, membersFakeInvoke } from "@/lib/ipc/members.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace, caps: ADMIN_CAPS });
}

beforeEach(() => {
  __resetAdminFake();
  __resetMembersFake();
});
afterEach(() => {
  __resetAdminFake();
  __resetMembersFake();
});

describe("TeamsAdmin", () => {
  it("creates a team and shows it", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<TeamsAdmin ws="acme" />);

    await user.type(screen.getByLabelText("new team"), "facilities");
    await user.click(screen.getByLabelText("create team"));
    expect(await screen.findByLabelText("delete facilities")).toBeInTheDocument();
  });

  it("delete shows the member count + cascade and removes the team", async () => {
    const user = userEvent.setup();
    signIn("acme");
    // Seed two members on the team so the consequence reads "2 members".
    membersFakeInvoke("members_add", { team: "facilities", user: "user:bob" });
    membersFakeInvoke("members_add", { team: "facilities", user: "user:cleo" });
    render(<TeamsAdmin ws="acme" />);
    await user.type(screen.getByLabelText("new team"), "facilities");
    await user.click(screen.getByLabelText("create team"));
    await screen.findByLabelText("delete facilities");

    await user.click(screen.getByLabelText("delete facilities"));
    expect(await screen.findByTestId("consequence")).toHaveTextContent(/Removes 2 members/i);
    expect(screen.getByTestId("consequence")).toHaveTextContent(/cascade/i);
    await user.click(screen.getByLabelText("confirm action"));

    expect(await screen.findByText("No teams yet.")).toBeInTheDocument();
  });
});
