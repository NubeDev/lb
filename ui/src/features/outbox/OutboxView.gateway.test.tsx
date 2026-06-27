// The outbox status view, driven against a REAL in-process gateway (no fake — CLAUDE §9). Effects are
// seeded as **real outbox records** through the test gateway's `/_seed/outbox` route (a real
// `lb_outbox::enqueue` write, the same path `start_job` uses), then read back over the real
// `GET /outbox` route. Covers: a pending effect renders under Pending; a delivered one under
// Delivered; and workspace isolation (ws-B sees none of ws-A's effects).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { OutboxView } from "./OutboxView";
import { useRealGateway, signInReal, seedOutbox } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `outbox-${n++}`;

beforeAll(() => useRealGateway());

describe("OutboxView (real gateway)", () => {
  it("shows a pending effect and a delivered effect in their groups", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOutbox({ id: "e1", target: "github", action: "create_pr", status: "pending", ts: 1 });
    await seedOutbox({ id: "e2", target: "github", action: "comment", status: "delivered", ts: 2 });

    render(<OutboxView ws={ws} />);
    await waitFor(() => expect(screen.getByText(/Pending · 1/)).toBeInTheDocument());
    expect(screen.getByText(/Delivered · 1/)).toBeInTheDocument();
    expect(screen.getByText(/create_pr/)).toBeInTheDocument();
  });

  it("is workspace-isolated — ws-B sees none of ws-A's effects", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedOutbox({ id: "e1", target: "github", action: "create_pr", status: "pending", ts: 1 });

    const wsB = nextWs();
    await signInReal("user:bob", wsB);
    render(<OutboxView ws={wsB} />);
    await waitFor(() => expect(screen.getByText(/Pending · 0/)).toBeInTheDocument());
    expect(screen.getByText(/Delivered · 0/)).toBeInTheDocument();
  });
});
