// The S6 approval gate, at the UI level: approving lets the coding job start and queue its PR
// through the outbox; starting an UNAPPROVED job is refused ("awaiting approval"); and a user
// without the workflow grant is denied — the same gates the Rust `workflow_test` proves on the
// backend, surfaced through the real api client + the faithful in-memory fake.
//
// We drive the actual `workflow.api` → `invoke` → fake path (no mock of the api): the fake mirrors
// the node's capability + approval gates, so this exercises the allow/deny/gated branches the user
// actually hits.

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { WorkflowView } from "./WorkflowView";
import { __resetWorkflowFake } from "@/lib/ipc/workflow.fake";

const WS = "acme";
const RESOLVE = "mcp:workflow.resolve_approval:call";
const START = "mcp:workflow.start_job:call";

beforeEach(() => __resetWorkflowFake());
afterEach(() => __resetWorkflowFake());

describe("WorkflowView approval gate", () => {
  it("approving lets the job start and queues the PR through the outbox", async () => {
    render(
      <WorkflowView
        ws={WS}
        approvalId="ap1"
        jobId="job1"
        author="user:ada"
        caps={[RESOLVE, START]}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Approve" }));
    await userEvent.click(screen.getByRole("button", { name: "Start coding job" }));
    await waitFor(() =>
      expect(screen.getByText(/create_pr → github \(pending\) via outbox/)).toBeInTheDocument(),
    );
  });

  it("starting an UNAPPROVED job is refused — the genuine gate", async () => {
    render(
      <WorkflowView
        ws={WS}
        approvalId="ap1"
        jobId="job1"
        author="user:ada"
        caps={[RESOLVE, START]}
      />,
    );
    // No approval first — straight to start.
    await userEvent.click(screen.getByRole("button", { name: "Start coding job" }));
    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("Awaiting approval"),
    );
    // And no effect was queued.
    expect(screen.queryByText(/via outbox/)).not.toBeInTheDocument();
  });

  it("a user WITHOUT the workflow grant is denied", async () => {
    render(
      <WorkflowView ws={WS} approvalId="ap1" jobId="job1" author="user:cleo" caps={[]} />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Approve" }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "You don't have access to this workflow.",
      ),
    );
  });

  it("a rejected approval still refuses the job (the gate, the other direction)", async () => {
    render(
      <WorkflowView
        ws={WS}
        approvalId="ap1"
        jobId="job1"
        author="user:ada"
        caps={[RESOLVE, START]}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Reject" }));
    await userEvent.click(screen.getByRole("button", { name: "Start coding job" }));
    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("Awaiting approval"),
    );
  });
});
