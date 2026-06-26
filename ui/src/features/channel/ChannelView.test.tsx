// The S2 exit-gate proof, in the UI: type a message, send it, and see it appear — driven
// through the real hook + api client + IPC seam (the in-memory node fake stands in for the
// node until SSE lands at S3). Deterministic: a fake clock is injected (no wall-clock).

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ChannelView } from "./ChannelView";

function fixedClock() {
  let t = 0;
  return () => ++t;
}

describe("ChannelView", () => {
  it("shows a posted message in the channel", async () => {
    const user = userEvent.setup();
    render(
      <ChannelView ws="acme" channel="general" author="user:me" now={fixedClock()} />,
    );

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
    render(
      <ChannelView ws="acme" channel="general" author="user:me" now={fixedClock()} />,
    );
    await screen.findByText(/no messages yet/i);

    await user.type(screen.getByLabelText("message"), "first");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("first");
    await user.type(screen.getByLabelText("message"), "second");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("second");

    const items = screen.getAllByRole("listitem").map((li) => li.textContent);
    expect(items[0]).toContain("first");
    expect(items[1]).toContain("second");
  });

  it("ignores an empty message", async () => {
    const user = userEvent.setup();
    render(<ChannelView ws="acme" channel="general" author="user:me" now={fixedClock()} />);
    await screen.findByText(/no messages yet/i);

    await user.click(screen.getByLabelText("send"));

    // Still empty — a blank submit posts nothing.
    expect(screen.getByText(/no messages yet/i)).toBeInTheDocument();
  });
});
