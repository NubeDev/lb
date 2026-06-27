// MembersAdmin (admin-console scope): add a member, remove it (the freshness-asymmetry consequence —
// docs unreadable immediately, inherited caps drop on next sign-in), and the remove is
// workspace-isolated. Driven through the real hook + api + the members fake. Mirrors MembersView.test.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { MembersAdmin } from "./MembersAdmin";
import { setSession } from "@/lib/session/session.store";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { __resetMembersFake } from "@/lib/ipc/members.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace, caps: ADMIN_CAPS });
}

beforeEach(() => __resetMembersFake());
afterEach(() => __resetMembersFake());

describe("MembersAdmin", () => {
  it("removes a member through a confirm showing the freshness asymmetry", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<MembersAdmin ws="acme" />);

    await user.type(screen.getByLabelText("add member"), "user:bob");
    await user.click(screen.getByLabelText("add"));
    await screen.findByText("user:bob");

    await user.click(screen.getByLabelText("remove user:bob"));
    expect(screen.getByTestId("consequence")).toHaveTextContent(
      /team-shared docs become unreadable immediately/i,
    );
    expect(screen.getByTestId("consequence")).toHaveTextContent(
      /inherited caps drop on next sign-in/i,
    );
    await user.click(screen.getByLabelText("confirm action"));

    expect(await screen.findByText(/no members in eng yet/i)).toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B never sees ws-A's members", async () => {
    const user = userEvent.setup();
    signIn("ws-a");
    const { unmount } = render(<MembersAdmin ws="ws-a" />);
    await user.type(screen.getByLabelText("add member"), "user:ada");
    await user.click(screen.getByLabelText("add"));
    await screen.findByText("user:ada");
    unmount();

    signIn("ws-b");
    render(<MembersAdmin ws="ws-b" />);
    expect(await screen.findByText(/no members in eng yet/i)).toBeInTheDocument();
    expect(screen.queryByText("user:ada")).not.toBeInTheDocument();
  });
});
