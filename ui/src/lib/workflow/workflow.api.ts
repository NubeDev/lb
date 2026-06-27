// The workflow API client — one call per export, mirroring the Rust `workflow.*` verbs and the node
// command name one-to-one. The UI never calls `invoke` directly; it goes through these named verbs
// (FILE-LAYOUT frontend rules).
//
// `author`/`caps` are the caller's demo principal + grant (the real node derives them from the
// session token; the in-memory fake uses them to resolve the capability gate, so the UI's allow/deny
// paths are exercised exactly as the node would — same seam as the agent api).

import type { Decision, Effect, PrSpec, StartResult } from "./workflow.types";
import { invoke } from "@/lib/ipc/invoke";

/** Open a `needs:approval` item gating a coding job and record its PR coordinates (read back by
 *  `startCodingJob`). Mirrors `workflow.request_approval`. The in-memory fake does not model this
 *  step; it is the real-node producer the gateway path needs before a job can start. */
export function requestApproval(
  ws: string,
  approvalId: string,
  scopeDoc: string,
  team: string,
  pr: PrSpec,
  opts?: { author?: string; caps?: string[] },
): Promise<{ id: string }> {
  return invoke<{ id: string }>("workflow_request_approval", {
    ws,
    approvalId,
    scopeDoc,
    team,
    pr,
    author: opts?.author,
    caps: opts?.caps,
  });
}

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
 *  false` (the genuine gate). Mirrors `workflow.start_job`. On the real node the PR coordinates are
 *  read back from `requestApproval` (not re-sent); `scopeDoc`/`channel`/`prKey` name the job + the
 *  effect (the in-memory fake ignores them — it only models the gate). */
export function startCodingJob(
  ws: string,
  jobId: string,
  approvalId: string,
  opts?: {
    author?: string;
    caps?: string[];
    scopeDoc?: string;
    channel?: string;
    prKey?: string;
  },
): Promise<StartResult> {
  return invoke<StartResult>("workflow_start_job", {
    ws,
    jobId,
    approvalId,
    scopeDoc: opts?.scopeDoc ?? "",
    channel: opts?.channel ?? "",
    prKey: opts?.prKey ?? `pr:${approvalId}`,
    author: opts?.author,
    caps: opts?.caps,
  });
}

/** Read the workspace's outbox effects (so the UI can show "PR queued → delivered"). Mirrors the
 *  relay's `pending` view; the fake also exposes delivered effects for display. */
export function listEffects(ws: string): Promise<Effect[]> {
  return invoke<Effect[]>("workflow_list_effects", { ws });
}
