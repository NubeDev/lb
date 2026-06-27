// The inbox slice in the UI (collaboration scope, slice 4): the REAL durable inbox (not the workflow
// fake) lists items and approving one persists the resolution. Driven through the real hook + api
// client + the contract-identical inbox fake.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { InboxView } from "./InboxView";
import { setSession } from "@/lib/session/session.store";
import { __seedInboxItem, __inboxResolution } from "@/lib/ipc/inbox.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace });
}

describe("InboxView", () => {
  it("lists a real durable item and approving it persists the resolution", async () => {
    const user = userEvent.setup();
    signIn("acme");
    __seedInboxItem("acme", {
      id: "appr-1",
      channel: "approvals",
      author: "ext:github",
      body: "needs:approval — open PR",
      ts: 1,
    });

    render(<InboxView channel="approvals" ws="acme" />);

    expect(await screen.findByText(/needs:approval/)).toBeInTheDocument();

    await user.click(screen.getByLabelText("approve appr-1"));

    // The resolution persisted, with the session principal as the actor (host forces this).
    const res = __inboxResolution("acme", "appr-1");
    expect(res?.decision).toBe("approved");
    expect(res?.actor).toBe("user:ada");
  });

  it("is workspace-isolated — ws-B never sees ws-A's items", async () => {
    signIn("ws-a");
    __seedInboxItem("ws-a", {
      id: "secret-1",
      channel: "approvals",
      author: "ext:gh",
      body: "ws-a only",
      ts: 1,
    });

    signIn("ws-b");
    render(<InboxView channel="approvals" ws="ws-b" />);
    expect(await screen.findByText(/no items/i)).toBeInTheDocument();
    expect(screen.queryByText("ws-a only")).not.toBeInTheDocument();
  });
});
