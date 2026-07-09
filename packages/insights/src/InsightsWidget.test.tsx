// The widget over a REAL in-memory client (memoryClient) — asserts the read-only vs acknowledge
// behaviour and the capability-deny surface. No mocks of node behaviour (CLAUDE §9): the client is the
// injected boundary, seeded with real records.

import { describe, expect, it } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";

import type { Insight } from "./types";
import { memoryClient, denyClient } from "./memoryClient";
import { InsightsReadWidget, InsightsAckWidget } from "./InsightsWidget";

function insight(over: Partial<Insight> = {}): Insight {
  return {
    id: "ins:1",
    dedup_key: "cpu-hot",
    severity: "critical",
    title: "CPU hot on edge-01",
    origin: { kind: "rule", ref: "cpu-hot" },
    status: "open",
    count: 3,
    first_ts: 1_000,
    last_ts: 2_000,
    producer: "edge-01",
    ...over,
  };
}

describe("InsightsReadWidget", () => {
  it("lists seeded insights and shows NO action buttons", async () => {
    render(<InsightsReadWidget client={memoryClient([insight()])} now={5_000} />);
    await screen.findByText("CPU hot on edge-01");
    expect(screen.queryByRole("button", { name: /^Ack$/i })).toBeNull();
    expect(screen.queryByText(/Resolve/i)).toBeNull();
  });
});

describe("InsightsAckWidget", () => {
  it("acks an open insight through the real client", async () => {
    const client = memoryClient([insight()]);
    render(<InsightsAckWidget client={client} now={5_000} />);
    const ack = await screen.findByRole("button", { name: /^Ack$/i });
    fireEvent.click(ack);
    // The real client flips status → acked; the widget refreshes; the Ack button disappears.
    await waitFor(() => expect(screen.queryByRole("button", { name: /^Ack$/i })).toBeNull());
    expect(await client.get("ins:1")).toMatchObject({ status: "acked" });
  });

  it("dismiss hides the row locally without a status change", async () => {
    const client = memoryClient([insight()]);
    render(<InsightsAckWidget client={client} now={5_000} />);
    fireEvent.click(await screen.findByRole("button", { name: /Dismiss/i }));
    await waitFor(() => expect(screen.queryByText("CPU hot on edge-01")).toBeNull());
    // Still open server-side — dismiss is a local hide, not a resolve.
    expect(await client.get("ins:1")).toMatchObject({ status: "open" });
  });
});

describe("capability deny", () => {
  it("surfaces an honest error, never a fabricated list", async () => {
    render(<InsightsReadWidget client={denyClient()} showRefresh={false} />);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toMatch(/Denied/);
  });
});
