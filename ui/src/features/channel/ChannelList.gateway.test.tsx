// The channel registry slice in the UI (collaboration scope, slice 2), driven against a REAL spawned
// gateway (no fake — CLAUDE §9). Creates a channel through the real `POST /channels` route and reads
// it back over `GET /channels`; create-on-post is exercised through the real `POST
// /channels/{cid}/messages`. Isolation comes from a unique workspace per test (the node derives the
// workspace from the token, the hard wall §7).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ChannelList } from "./ChannelList";
import { createChannel, post } from "@/lib/channel/channel.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `chanlist-${n++}`;

beforeAll(() => useRealGateway());

describe("ChannelList (real gateway)", () => {
  it("creates a channel and shows it in the list", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<ChannelList ws={ws} selected="" onSelect={() => {}} />);

    await user.type(screen.getByLabelText("new channel"), "hvac-alerts");
    await user.click(screen.getByLabelText("create channel"));

    expect(await screen.findByText("hvac-alerts")).toBeInTheDocument();
  });

  it("lists a channel created by posting to it (create-on-post)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // A post registers the channel even with no explicit create.
    await post(ws, "general", {
      id: "m1",
      channel: "general",
      author: "user:ada",
      body: "hi",
      ts: 1,
    });
    render(<ChannelList ws={ws} selected="" onSelect={() => {}} />);
    expect(await screen.findByText("general")).toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B never sees ws-A's channels", async () => {
    // Create in ws-A through the real route.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await createChannel(wsA, "secret-room");

    // Switch the session to ws-B and render — the ws-A channel is gone.
    const wsB = nextWs();
    await signInReal("user:bob", wsB);
    render(<ChannelList ws={wsB} selected="" onSelect={() => {}} />);
    // Give the list a tick to load.
    await screen.findByText("Channels");
    expect(screen.queryByText("secret-room")).not.toBeInTheDocument();
  });
});
