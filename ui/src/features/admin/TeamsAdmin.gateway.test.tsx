// TeamsAdmin (admin-console redesign), driven against a REAL spawned gateway (CLAUDE §9 — no fake).
// Create a team, see it in the table, select it, add/remove a member inline (the old separate Members
// tab is folded in — no typing a team id), and delete with the cascade consequence. Every action goes
// through the real useDirectory hook → real admin/members api → real /admin|/teams routes. Each test
// logs into a UNIQUE workspace for isolation on the shared node.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { TeamsAdmin } from "./TeamsAdmin";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `teams-${n++}`;

beforeAll(() => useRealGateway());

describe("TeamsAdmin (real gateway)", () => {
  it("creates a team, then adds a member inline (no typing a team id)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<TeamsAdmin ws={ws} />);

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
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<TeamsAdmin ws={ws} />);

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
