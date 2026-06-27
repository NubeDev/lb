// The outbox status slice in the UI (collaboration scope, slice 4): the read-only view reflects an
// effect moving pending → delivered. Driven through the real hook + api client + the contract-
// identical outbox fake. No mutation surface — the view only reads.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { OutboxView } from "./OutboxView";
import { setSession } from "@/lib/session/session.store";
import { __seedEffect, __markDelivered } from "@/lib/ipc/outbox.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace });
}

function effect(id: string, status: "pending" | "delivered") {
  return { id, target: "github", action: "create_pr", status, attempts: 0, ts: 1 } as const;
}

describe("OutboxView", () => {
  it("shows a pending effect, then reflects it delivered after a re-read", async () => {
    signIn("acme");
    __seedEffect("acme", effect("e1", "pending"));

    const { unmount } = render(<OutboxView ws="acme" />);
    // It appears under Pending (count 1), delivered empty.
    expect(await screen.findByText(/Pending · 1/)).toBeInTheDocument();
    expect(screen.getByText(/Delivered · 0/)).toBeInTheDocument();
    expect(screen.getByText(/create_pr/)).toBeInTheDocument();
    unmount();

    // The relay delivers it; a fresh render reflects the new status.
    __markDelivered("acme", "e1");
    render(<OutboxView ws="acme" />);
    expect(await screen.findByText(/Pending · 0/)).toBeInTheDocument();
    expect(screen.getByText(/Delivered · 1/)).toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B sees none of ws-A's effects", async () => {
    signIn("ws-a");
    __seedEffect("ws-a", effect("e1", "pending"));

    signIn("ws-b");
    render(<OutboxView ws="ws-b" />);
    expect(await screen.findByText(/Pending · 0/)).toBeInTheDocument();
  });
});
