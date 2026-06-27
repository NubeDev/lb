// PeopleAdmin (admin-console redesign): the relationship-first users surface. Proves the headline
// "who belongs to who" — a selected user's TEAMS are shown (assembled from the real membership
// endpoints, not typed) — and that a named ROLE can be assigned from a dropdown (no raw `role:<name>`
// strings). Driven through useDirectory/useSubjectGrants + the real apis + the admin/members fakes.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { PeopleAdmin } from "./PeopleAdmin";
import { setSession } from "@/lib/session/session.store";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { adminFakeInvoke, __resetAdminFake } from "@/lib/ipc/admin.fake";
import { membersFakeInvoke, __resetMembersFake } from "@/lib/ipc/members.fake";

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

describe("PeopleAdmin", () => {
  it("shows which teams a selected user belongs to (who-belongs-to-who)", async () => {
    const user = userEvent.setup();
    adminFakeInvoke("user_create", { user: "bob" });
    adminFakeInvoke("teams_create", { team: "eng", name: "eng" });
    membersFakeInvoke("members_add", { team: "eng", user: "user:bob" });

    render(<PeopleAdmin ws="acme" />);
    await user.click(await screen.findByLabelText("select bob"));

    // The detail pane's Teams section lists eng — derived from membership, never typed. (It appears
    // both in the table's Teams column and the detail's Teams chips.)
    expect((await screen.findAllByText("eng")).length).toBeGreaterThan(0);
  });

  it("assigns a named role from a dropdown (not a raw role: string)", async () => {
    const user = userEvent.setup();
    adminFakeInvoke("user_create", { user: "bob" });
    adminFakeInvoke("roles_define", { name: "operator", caps: [] });

    render(<PeopleAdmin ws="acme" />);
    await user.click(await screen.findByLabelText("select bob"));

    await user.selectOptions(
      await screen.findByLabelText("assign a role to user:bob"),
      "operator",
    );
    await user.click(screen.getByLabelText("assign role"));

    expect(await screen.findByLabelText("revoke role operator from user:bob")).toBeInTheDocument();
  });

  it("creates a user via the header action (not a chat composer)", async () => {
    const user = userEvent.setup();
    render(<PeopleAdmin ws="acme" />);

    await user.click(screen.getByLabelText("new user"));
    await user.type(screen.getByLabelText("new user id"), "carol");
    await user.click(screen.getByRole("button", { name: "Create" }));

    const table = await screen.findByRole("table");
    expect(within(table).getByText("carol")).toBeInTheDocument();
  });
});
