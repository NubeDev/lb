// The workflow view — review an approval, then start the gated coding job and watch the outbox.
// Layout + wiring only; data lives in useWorkflow (FILE-LAYOUT). This is the UI face of the S6
// story: approving lets the job start (and queue its PR through the outbox); starting an unapproved
// job is REFUSED — the same gate the Rust `workflow_test` proves on the backend, surfaced to the
// user.

import { GitPullRequest } from "lucide-react";

import { useWorkflow } from "./useWorkflow";

interface Props {
  ws: string;
  approvalId: string;
  jobId: string;
  /** The current user's principal (demo session identity until real login lands). */
  author: string;
  /** The caller's held capabilities (the grant the node checks; demo until real tokens). */
  caps: string[];
}

export function WorkflowView({ ws, approvalId, jobId, author, caps }: Props) {
  const { effects, gated, error, resolve, start } = useWorkflow(
    ws,
    approvalId,
    jobId,
    author,
    caps,
  );

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <GitPullRequest size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Coding workflow</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      <div className="flex gap-2 border-b border-border px-4 py-3">
        <button
          type="button"
          onClick={() => void resolve("approved")}
          className="rounded-md bg-accent px-3 py-1 text-sm text-bg"
        >
          Approve
        </button>
        <button
          type="button"
          onClick={() => void resolve("rejected")}
          className="rounded-md border border-border px-3 py-1 text-sm"
        >
          Reject
        </button>
        <button
          type="button"
          onClick={() => void start()}
          className="ml-auto rounded-md bg-accent px-3 py-1 text-sm text-bg"
        >
          Start coding job
        </button>
      </div>

      {error ? (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error === "denied" ? "You don't have access to this workflow." : error}
        </div>
      ) : gated ? (
        <div role="status" className="bg-panel px-4 py-2 text-xs text-accent">
          Awaiting approval — the job can't start yet.
        </div>
      ) : effects.length > 0 ? (
        <ul className="flex-1 px-4 py-3 text-sm">
          {effects.map((e) => (
            <li key={e.idempotencyKey} className="py-1">
              {e.action} → {e.target} ({e.status}) via outbox
            </li>
          ))}
        </ul>
      ) : (
        <div className="flex flex-1 items-center justify-center text-sm text-muted">
          Approve the scope, then start the job.
        </div>
      )}
    </section>
  );
}
