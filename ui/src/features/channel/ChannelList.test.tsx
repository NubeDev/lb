// The channel registry slice in the UI (collaboration scope, slice 2): create a channel and see it
// listed; posting registers a channel; the list is workspace-isolated. Driven through the real hook
// + api client + the contract-identical fake (workspace from the session store, as the real gateway
// derives it from the token).

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ChannelList } from "./ChannelList";
import { post } from "@/lib/channel/channel.api";
import { setSession } from "@/lib/session/session.store";
import type { Session } from "@/lib/session/session.types";

function signIn(workspace: string): Session {
  const s: Session = { token: `t:${workspace}`, principal: "user:ada", workspace };
  setSession(s);
  return s;
}

describe("ChannelList", () => {
  it("creates a channel and shows it in the list", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<ChannelList ws="acme" selected="" onSelect={() => {}} />);

    await user.type(screen.getByLabelText("new channel"), "hvac-alerts");
    await user.click(screen.getByLabelText("create channel"));

    expect(await screen.findByText("hvac-alerts")).toBeInTheDocument();
  });

  it("lists a channel created by posting to it (create-on-post)", async () => {
    signIn("acme");
    // A post registers the channel even with no explicit create.
    await post("acme", "general", {
      id: "m1",
      channel: "general",
      author: "user:ada",
      body: "hi",
      ts: 1,
    });
    render(<ChannelList ws="acme" selected="" onSelect={() => {}} />);
    expect(await screen.findByText("general")).toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B never sees ws-A's channels", async () => {
    // Create in ws-A.
    signIn("ws-a");
    const { unmount } = render(<ChannelList ws="ws-a" selected="" onSelect={() => {}} />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("new channel"), "secret-room");
    await user.click(screen.getByLabelText("create channel"));
    await screen.findByText("secret-room");
    unmount();

    // Switch the session to ws-B and render again — the ws-A channel is gone.
    signIn("ws-b");
    render(<ChannelList ws="ws-b" selected="" onSelect={() => {}} />);
    // Give the list a tick to load.
    await screen.findByText("Channels");
    expect(screen.queryByText("secret-room")).not.toBeInTheDocument();
  });
});
