// The workflow hook — data + state for the approval gate + the coding job (FILE-LAYOUT: one hook
// per file, data separated from markup). It drives the capability-checked node verbs: a reviewer
// resolves an approval, then starts the job — which the node refuses unless approved (the genuine
// S6 gate, surfaced to the user, never a silent no-op).

import { useCallback, useState } from "react";

import { resolveApproval, startCodingJob, listEffects } from "@/lib/workflow/workflow.api";
import type { Decision, Effect } from "@/lib/workflow/workflow.types";

export interface WorkflowState {
  /** The outbox effects queued by the started job (so the UI shows "PR queued"). */
  effects: Effect[];
  /** Set when the gate refused the job start (awaiting approval) — shown to the user. */
  gated: boolean;
  /** Set when the node denied a verb (missing capability) — shown to the user. */
  error: string | null;
  /** Resolve the approval item with a decision. */
  resolve: (decision: Decision) => Promise<void>;
  /** Start the coding job; reflects whether the gate let it through. */
  start: () => Promise<void>;
}

/** Drive the gated coding workflow in `(ws)` for `approvalId`/`jobId` as `author` holding `caps`
 *  (the demo session identity + grant until real login lands — see workflow.api). */
export function useWorkflow(
  ws: string,
  approvalId: string,
  jobId: string,
  author: string,
  caps: string[],
): WorkflowState {
  const [effects, setEffects] = useState<Effect[]>([]);
  const [gated, setGated] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const resolve = useCallback(
    async (decision: Decision) => {
      try {
        await resolveApproval(ws, approvalId, decision, { author, caps });
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [ws, approvalId, author, caps],
  );

  const start = useCallback(async () => {
    try {
      const result = await startCodingJob(ws, jobId, approvalId, { author, caps });
      setGated(!result.started);
      setError(null);
      if (result.started) setEffects(await listEffects(ws));
      else setEffects([]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [ws, jobId, approvalId, author, caps]);

  return { effects, gated, error, resolve, start };
}
