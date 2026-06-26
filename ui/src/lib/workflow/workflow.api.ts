// The workflow API client — one call per export, mirroring the Rust `workflow.*` verbs and the node
// command name one-to-one. The UI never calls `invoke` directly; it goes through these named verbs
// (FILE-LAYOUT frontend rules).
//
// `author`/`caps` are the caller's demo principal + grant (the real node derives them from the
// session token; the in-memory fake uses them to resolve the capability gate, so the UI's allow/deny
// paths are exercised exactly as the node would — same seam as the agent api).

import type { Decision, Effect, StartResult } from "./workflow.types";
import { invoke } from "@/lib/ipc/invoke";

/** Resolve a `needs:approval` item (approve/reject/defer). Mirrors `workflow.resolve_approval`. */
export function resolveApproval(
  ws: string,
  approvalId: string,
  decision: Decision,
  opts?: { author?: string; caps?: string[] },
): Promise<{ ok: true }> {
  return invoke<{ ok: true }>("workflow_resolve_approval", {
    ws,
    approvalId,
    decision,
    author: opts?.author,
    caps: opts?.caps,
  });
}

/** Start the gated coding job — succeeds only if the approval resolved `approved`, else `started:
 *  false` (the genuine gate). Mirrors `workflow.start_job`. */
export function startCodingJob(
  ws: string,
  jobId: string,
  approvalId: string,
  opts?: { author?: string; caps?: string[] },
): Promise<StartResult> {
  return invoke<StartResult>("workflow_start_job", {
    ws,
    jobId,
    approvalId,
    author: opts?.author,
    caps: opts?.caps,
  });
}

/** Read the workspace's outbox effects (so the UI can show "PR queued → delivered"). Mirrors the
 *  relay's `pending` view; the fake also exposes delivered effects for display. */
export function listEffects(ws: string): Promise<Effect[]> {
  return invoke<Effect[]>("workflow_list_effects", { ws });
}
