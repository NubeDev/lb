// The members slice in the UI (collaboration scope, slice 3), driven against a REAL spawned gateway
// (no fake — CLAUDE §9). Adds a member through the real `POST /teams/{team}/members` route and reads
// the roster back over `GET /teams/{team}/members`; workspace isolation comes from each test using a
// unique workspace (the node derives the workspace from the token, the hard wall §7).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { MembersView } from "./MembersView";
import { addMember } from "@/lib/members/members.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `members-${n++}`;

beforeAll(() => useRealGateway());

describe("MembersView (real gateway)", () => {
  it("adds a member to the default team and shows it", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<MembersView ws={ws} />);

    expect(await screen.findByText(/no members in eng yet/i)).toBeInTheDocument();

    await user.type(screen.getByLabelText("add member"), "user:bob");
    await user.click(screen.getByLabelText("add"));

    expect(await screen.findByText("user:bob")).toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B never sees ws-A's members", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    // Seed a member into ws-A through the real add route.
    await addMember("eng", "user:ada");

    const wsB = nextWs();
    await signInReal("user:bob", wsB);
    render(<MembersView ws={wsB} />);
    expect(await screen.findByText(/no members in eng yet/i)).toBeInTheDocument();
    expect(screen.queryByText("user:ada")).not.toBeInTheDocument();
  });
});
