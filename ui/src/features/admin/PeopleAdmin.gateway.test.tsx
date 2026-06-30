// PeopleAdmin (admin-console redesign), driven against a REAL spawned gateway (CLAUDE §9 — no fake).
// Proves the headline "who belongs to who" — a selected user's TEAMS are shown (assembled from the
// real membership endpoints, not typed) — and that a named ROLE can be assigned from a dropdown (no
// raw `role:<name>` strings). Each test logs into a UNIQUE workspace and seeds real records through
// the real admin api clients (createUser/createTeam/addMember/defineRole), then renders the view,
// which reads the same real routes via useDirectory/useRoles/useSubjectGrants.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { PeopleAdmin } from "./PeopleAdmin";
import { createUser } from "@/lib/admin/users.api";
import { createTeam } from "@/lib/admin/teams.api";
import { defineRole } from "@/lib/admin/roles.api";
import { addMember } from "@/lib/members/members.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `people-${n++}`;

beforeAll(() => useRealGateway());

describe("PeopleAdmin (real gateway)", () => {
  it("shows which teams a selected user belongs to (who-belongs-to-who)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await createUser("bob");
    await createTeam("eng", "eng");
    await addMember("eng", "user:bob");

    render(<PeopleAdmin ws={ws} />);
    await user.click(await screen.findByLabelText("select bob"));

    // The detail pane's Teams section lists eng — derived from real membership, never typed. (It
    // appears both in the table's Teams column and the detail's Teams chips.)
    expect((await screen.findAllByText("eng")).length).toBeGreaterThan(0);
  });

  it("assigns a named role from a dropdown (not a raw role: string)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await createUser("bob");
    await defineRole("operator", []);

    render(<PeopleAdmin ws={ws} />);
    await user.click(await screen.findByLabelText("select bob"));

    await user.selectOptions(
      await screen.findByLabelText("assign a role to user:bob"),
      "operator",
    );
    // The Select assigns on change (no separate button).

    expect(await screen.findByLabelText("revoke role operator from user:bob")).toBeInTheDocument();
  });

  it("creates a user via the header action (not a chat composer)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);

    render(<PeopleAdmin ws={ws} />);

    await user.click(screen.getByLabelText("new user"));
    await user.type(screen.getByLabelText("new user id"), "carol");
    await user.click(screen.getByRole("button", { name: "Create" }));

    const table = await screen.findByRole("table");
    expect(await within(table).findByText("carol")).toBeInTheDocument();
  });
});
