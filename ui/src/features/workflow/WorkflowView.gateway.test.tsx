// The S6 approval gate, at the UI level, driven against a REAL spawned gateway node (no fake —
// CLAUDE §9). The PR spec is recorded through the real `POST /approvals/{id}/request` route (so the
// later `start` can read it back), then the view drives the real `resolve`/`start` routes through
// its buttons. Covers the genuine gate in every direction: approving lets the job start and queues
// its PR through the outbox; starting an UNAPPROVED job is refused (`started: false` → "awaiting
// approval"); a rejected approval still refuses. Each test uses a unique workspace so the shared
// node stays isolated.
//
// Dropped vs the old fake test: the "user WITHOUT the workflow grant is denied" UI case. The dev
// login (`signInReal`) always mints a FULL admin cap set, so a no-cap token cannot be obtained via
// the UI. That deny is proven server-side by the Rust route test
// `workflow_verb_without_the_cap_is_denied_server_side`
// (rust/role/gateway/tests/assets_workflow_routes_test.rs).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { WorkflowView } from "./WorkflowView";
import { requestApproval } from "@/lib/workflow/workflow.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `workflow-${n++}`;

const PR = { repo: "acme/api", head: "fix/1", base: "main", title: "Fix", body: "" };

/** Seed the PR spec for an approval via the real `request` route (the view itself never requests —
 *  it only resolves/starts, so the spec must exist before `start` can read it back). */
async function seedApproval(ws: string, approvalId: string) {
  await requestApproval(ws, approvalId, "scope:1", "eng", PR);
}

beforeAll(() => useRealGateway());

describe("WorkflowView approval gate (real gateway)", () => {
  it("approving lets the job start and queues the PR through the outbox", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedApproval(ws, "ap1");

    render(<WorkflowView ws={ws} approvalId="ap1" jobId="job1" author="user:ada" caps={[]} />);
    await userEvent.click(screen.getByRole("button", { name: "Approve" }));
    await userEvent.click(screen.getByRole("button", { name: "Start coding job" }));
    await waitFor(() =>
      expect(screen.getByText(/create_pr → github \(pending\) via outbox/)).toBeInTheDocument(),
    );
  });

  it("starting an UNAPPROVED job is refused — the genuine gate", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedApproval(ws, "ap1");

    render(<WorkflowView ws={ws} approvalId="ap1" jobId="job1" author="user:ada" caps={[]} />);
    // No approval first — straight to start.
    await userEvent.click(screen.getByRole("button", { name: "Start coding job" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Awaiting approval"));
    // And no effect was queued.
    expect(screen.queryByText(/via outbox/)).not.toBeInTheDocument();
  });

  it("a rejected approval still refuses the job (the gate, the other direction)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedApproval(ws, "ap1");

    render(<WorkflowView ws={ws} approvalId="ap1" jobId="job1" author="user:ada" caps={[]} />);
    await userEvent.click(screen.getByRole("button", { name: "Reject" }));
    await userEvent.click(screen.getByRole("button", { name: "Start coding job" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Awaiting approval"));
  });
});
