// The members slice in the UI (collaboration scope, slice 3): add a member and see the roster; the
// list is workspace-isolated. Driven through the real hook + api client + the contract-identical fake.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { MembersView } from "./MembersView";
import { setSession } from "@/lib/session/session.store";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace });
}

describe("MembersView", () => {
  it("adds a member to the default team and shows it", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<MembersView ws="acme" />);

    expect(await screen.findByText(/no members in eng yet/i)).toBeInTheDocument();

    await user.type(screen.getByLabelText("add member"), "user:bob");
    await user.click(screen.getByLabelText("add"));

    expect(await screen.findByText("user:bob")).toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B never sees ws-A's members", async () => {
    const user = userEvent.setup();
    signIn("ws-a");
    const { unmount } = render(<MembersView ws="ws-a" />);
    await screen.findByText(/no members in eng yet/i);
    await user.type(screen.getByLabelText("add member"), "user:ada");
    await user.click(screen.getByLabelText("add"));
    await screen.findByText("user:ada");
    unmount();

    signIn("ws-b");
    render(<MembersView ws="ws-b" />);
    expect(await screen.findByText(/no members in eng yet/i)).toBeInTheDocument();
    expect(screen.queryByText("user:ada")).not.toBeInTheDocument();
  });
});
