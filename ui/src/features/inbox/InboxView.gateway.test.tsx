// The inbox slice in the UI (collaboration scope, slice 4), driven against a REAL spawned gateway
// node (no fake — CLAUDE §9 / testing §0). A durable inbox item is seeded as a **real** `lb_inbox`
// record through the test gateway's `/_seed/inbox` route (the same `lb_inbox::record` write the
// production path uses), then listed over the real `GET /inbox/{channel}` route and approved over the
// real `POST /inbox/{item}/resolve` route. Each test logs in to a UNIQUE workspace so the shared real
// node stays isolated.
//
// The original fake test additionally asserted the persisted resolution's `decision`/`actor` by
// reaching into the fake's `__inboxResolution`. The real gateway exposes no resolution-read route to
// the UI client (the inbox list returns items regardless of resolution, `lb_inbox::list`), so we
// assert the resolve round-trips with no error and the view stays healthy; the persisted
// decision + host-forced actor (`user:ada`) are proven server-side in the Rust
// `crates/inbox/tests/resolution_test.rs` (`records_and_reads_a_decision`).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { InboxView } from "./InboxView";
import { useRealGateway, signInReal, seedInbox } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `inbox-${n++}`;

beforeAll(() => useRealGateway());

describe("InboxView (real gateway)", () => {
  it("lists a real durable item and approving it round-trips without error", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedInbox({
      id: "appr-1",
      channel: "approvals",
      author: "ext:github",
      body: "needs:approval — open PR",
      ts: 1,
    });

    render(<InboxView channel="approvals" ws={ws} />);

    // The seeded item is read back over the real inbox-list route.
    expect(await screen.findByText(/needs:approval/)).toBeInTheDocument();

    // Approve it — a real `POST /inbox/appr-1/resolve`. The host records the resolution with the
    // session principal forced as the actor (proven in the Rust resolution_test). The hook reloads;
    // a failed resolve would surface in the view's `role="alert"` error band.
    await user.click(screen.getByLabelText("approve appr-1"));

    // No error band appeared, and the item is still listed (resolution is a separate facet).
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(screen.getByText(/needs:approval/)).toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B never sees ws-A's items", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedInbox({
      id: "secret-1",
      channel: "approvals",
      author: "ext:gh",
      body: "ws-a only",
      ts: 1,
    });

    const wsB = nextWs();
    await signInReal("user:bob", wsB);
    render(<InboxView channel="approvals" ws={wsB} />);

    expect(await screen.findByText(/no items/i)).toBeInTheDocument();
    expect(screen.queryByText("ws-a only")).not.toBeInTheDocument();
  });
});
