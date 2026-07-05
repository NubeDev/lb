// The run-control client (agent-dock scope, run controls) — STOP / PAUSE / RESUME a live agent run.
// One verb per action over the host-mediated bridge (`agent_control` → `POST /runs/{job}/{op}`), gated
// by `mcp:agent.control:call` server-side. Sibling of `run.stream.ts` (which only WATCHES a run).
//
// FILE-LAYOUT: one responsibility — the three control calls. The dock's buttons call these; a rejected
// call (403 without the cap, 400 on a bad state) rejects the promise, which the dock surfaces.

import { invoke } from "@/lib/ipc/invoke";

/** The lifecycle op the run-control route accepts. `cancel` is the stop verb (matches `lb_jobs::cancel`). */
export type RunControlOp = "cancel" | "pause" | "resume";

/** Drive one run-control op on `job`. Resolves on `204`; rejects (via `invoke`'s typed error) on a
 *  deny (403) or a bad state (400). */
export function runControl(job: string, op: RunControlOp): Promise<void> {
  return invoke<void>("agent_control", { job, op });
}

/** Stop (cancel) a run — terminal, non-restartable. */
export const stopRun = (job: string) => runControl(job, "cancel");
/** Pause a run — suspend it; a later resume continues from where it paused. */
export const pauseRun = (job: string) => runControl(job, "pause");
/** Resume a paused run — the reactor re-drives it from the durable cursor. */
export const resumeRun = (job: string) => runControl(job, "resume");
