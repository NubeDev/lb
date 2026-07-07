// TeamsAdmin (admin-console redesign), driven against a REAL spawned gateway (CLAUDE §9 — no fake).
// Create a team, see it in the table, select it, add/remove a member inline (the old separate Members
// tab is folded in — no typing a team id), and delete with the cascade consequence. Every action goes
// through the real useDirectory hook → real admin/members api → real /admin|/teams routes. Each test
// logs into a UNIQUE workspace for isolation on the shared node.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { TeamsAdmin } from "./TeamsAdmin";
import { createUser } from "@/lib/admin/users.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `teams-${n++}`;

beforeAll(() => useRealGateway());

/** Workaround for jsdom + React 18 controlled `<select>`: user-event's selectOptions mutates the
 *  DOM `value` but the synthetic `change` doesn't reach React's root listener, so the controlled
 *  state never updates. Set through the prototype setter and dispatch a native `change` (the path
 *  React listens on) so `onChange` fires and state updates. */
function selectValue(sel: HTMLElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!;
  setter.call(sel as HTMLSelectElement, value);
  fireEvent.change(sel);
}

describe("TeamsAdmin (real gateway)", () => {
  it("creates a team, then adds a member from a user dropdown (no typing a user id)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await createUser("bob");

    render(<TeamsAdmin ws={ws} />);

    await user.click(screen.getByLabelText("new team"));
    await user.type(screen.getByLabelText("new team id"), "facilities");
    await user.click(screen.getByRole("button", { name: "create team" }));

    // The team appears in the table; selecting it reveals its (empty) member list + add dropdown.
    await user.click(await screen.findByLabelText("select facilities"));
    const sel = await screen.findByLabelText("add member") as HTMLSelectElement;
    await screen.findByRole("option", { name: "bob" });
    selectValue(sel, "bob");
    expect(sel.value).toBe("bob");
    const addBtn = screen.getByLabelText("add member to team");
    expect((addBtn as HTMLButtonElement).disabled).toBe(false);
    await user.click(addBtn);
    // Wait for the form's refresh to flush: bob should appear with a Remove button.
    expect(await screen.findByLabelText("remove bob", undefined, { timeout: 3000 })).toBeInTheDocument();
  });

  it("delete shows the member count + cascade and removes the team", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await createUser("bob");

    render(<TeamsAdmin ws={ws} />);

    await user.click(screen.getByLabelText("new team"));
    await user.type(screen.getByLabelText("new team id"), "facilities");
    await user.click(screen.getByRole("button", { name: "create team" }));
    await user.click(await screen.findByLabelText("select facilities"));

    // Add one member so the consequence reads "1 member".
    const sel = await screen.findByLabelText("add member") as HTMLSelectElement;
    await screen.findByRole("option", { name: "bob" });
    selectValue(sel, "bob");
    await user.click(screen.getByLabelText("add member to team"));
    await screen.findByLabelText("remove bob");

    await user.click(screen.getByLabelText("delete team facilities"));
    expect(await screen.findByTestId("consequence")).toHaveTextContent(/Removes 1 member/i);
    expect(screen.getByTestId("consequence")).toHaveTextContent(/cascade/i);
    await user.click(screen.getByLabelText("confirm action"));

    expect(await screen.findByText("No teams yet.")).toBeInTheDocument();
  });
});
