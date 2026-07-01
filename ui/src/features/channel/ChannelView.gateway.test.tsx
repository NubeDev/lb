// The S2 exit-gate proof, in the UI, driven against a REAL spawned gateway (no fake — CLAUDE §9):
// type a message, send it (real `POST /channels/{cid}/messages`), and see it appear via the
// post→refresh round trip (real `GET /channels/{cid}/messages`). A fake logical clock keeps the ids
// deterministic. Isolation comes from a unique workspace per test (the node derives the workspace
// from the token, the hard wall §7).
//
// Note: live SSE append is a separate stream; these assertions ride the durable history re-read the
// hook performs after each post — the same path the original fake-backed test exercised.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ChannelView } from "./ChannelView";
import { history } from "@/lib/channel/channel.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `chanview-${n++}`;

function fixedClock() {
  let t = 0;
  return () => ++t;
}

beforeAll(() => useRealGateway());

async function signedInWs(): Promise<string> {
  const ws = nextWs();
  await signInReal("user:me", ws);
  return ws;
}

describe("ChannelView (real gateway)", () => {
  it("shows a posted message in the channel", async () => {
    const user = userEvent.setup();
    const ws = await signedInWs();
    render(<ChannelView ws={ws} channel="general" author="user:me" now={fixedClock()} />);

    // Empty channel first.
    expect(await screen.findByText(/no messages yet/i)).toBeInTheDocument();

    // Post a message.
    await user.type(screen.getByLabelText("message"), "hello world");
    await user.click(screen.getByLabelText("send"));

    // It appears — the post → refresh-from-history round trip.
    expect(await screen.findByText("hello world")).toBeInTheDocument();
    expect(screen.getByText("user:me")).toBeInTheDocument();
  });

  it("keeps messages ordered oldest→newest", async () => {
    const user = userEvent.setup();
    const ws = await signedInWs();
    render(<ChannelView ws={ws} channel="general" author="user:me" now={fixedClock()} />);
    await screen.findByText(/no messages yet/i);

    await user.type(screen.getByLabelText("message"), "first");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("first");
    await user.type(screen.getByLabelText("message"), "second");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("second");

    // Scope to the message list — the surface also renders the channel roster (its own <li>s).
    const messages = within(screen.getByLabelText("messages"));
    const items = messages.getAllByRole("listitem").map((li) => li.textContent);
    expect(items[0]).toContain("first");
    expect(items[1]).toContain("second");
  });

  it("ignores an empty message", async () => {
    const user = userEvent.setup();
    const ws = await signedInWs();
    render(<ChannelView ws={ws} channel="general" author="user:me" now={fixedClock()} />);
    await screen.findByText(/no messages yet/i);

    await user.click(screen.getByLabelText("send"));

    // Still empty — a blank submit posts nothing.
    expect(screen.getByText(/no messages yet/i)).toBeInTheDocument();
  });

  it("does not overflow horizontally at a narrow (phone) viewport", async () => {
    // Responsive regression guard for the shadcn migration (ui-standards-scope): the canonical
    // `flex h-full min-w-0 flex-col` surface must never exceed its container on a phone.
    const ws = await signedInWs();
    const { container } = render(
      <div style={{ width: 360 }}>
        <ChannelView ws={ws} channel="general" author="user:me" now={fixedClock()} />
      </div>,
    );
    const section = await screen.findByLabelText("channel view");
    expect(container.querySelector("section")).toBe(section);
    expect(section.scrollWidth).toBeLessThanOrEqual(360);
  });

  it("re-posting the same message id is idempotent (durable history shows one)", async () => {
    const user = userEvent.setup();
    const ws = await signedInWs();
    // A fixed clock makes the hook mint the SAME id (`author-1`) on each first post — but two distinct
    // sends mint distinct ids, so re-post idempotency is asserted directly against the real history.
    render(<ChannelView ws={ws} channel="general" author="user:me" now={() => 7} />);
    await screen.findByText(/no messages yet/i);

    await user.type(screen.getByLabelText("message"), "echo");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("echo");
    // Second send with the same clock → same id → idempotent upsert, not a duplicate.
    await user.type(screen.getByLabelText("message"), "echo");
    await user.click(screen.getByLabelText("send"));

    const hist = await history(ws, "general");
    expect(hist.filter((m) => m.body === "echo")).toHaveLength(1);
  });
});
