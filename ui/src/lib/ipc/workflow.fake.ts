// The in-memory workflow stand-in used when NOT in the Tauri shell (plain browser, tests). It
// mirrors the node's `workflow.*` contract faithfully enough that the UI behaves identically here
// and against the real node (the verb names + shapes match the Rust commands one-to-one).
//
// Faithful to the gates the user actually hits:
//   - the CAPABILITY gate: each verb needs its `mcp:workflow.<verb>:call` grant, else "denied"
//     (the same opaque deny the Rust `workflow_test` proves);
//   - the APPROVAL gate (the headline S6 behavior): `workflow_start_job` starts the job ONLY if the
//     approval resolved `approved` — otherwise it returns `started: false` and queues NO effect.
// On a started job it queues the PR effect in the (workspace-scoped) outbox, exactly as the node's
// transactional `emit_effect` does, so the UI can show "PR queued via outbox".
//
// One file per concern (FILE-LAYOUT): the workflow fake lives beside the channel + agent fakes.

import type { Decision, Effect } from "@/lib/workflow/workflow.types";

const RESOLVE_CAP = "mcp:workflow.resolve_approval:call";
const START_CAP = "mcp:workflow.start_job:call";

// Workspace-scoped state (key prefix = ws) — the hard wall, mirrored.
const resolutions = new Map<string, Decision>(); // `${ws}/${approvalId}` -> decision
const effects = new Map<string, Effect[]>(); // `${ws}` -> the outbox

const k = (ws: string, x: string) => `${ws}/${x}`;

function capMatches(held: string[], cap: string): boolean {
  return held.some((h) => h === cap || h === "mcp:workflow.*:call" || h === "mcp:*:call");
}

export function workflowFakeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> | null {
  switch (cmd) {
    case "workflow_resolve_approval": {
      const { ws, approvalId, decision, caps } = args as {
        ws: string;
        approvalId: string;
        decision: Decision;
        caps?: string[];
      };
      if (!capMatches(caps ?? [], RESOLVE_CAP)) return Promise.reject(new Error("denied"));
      resolutions.set(k(ws, approvalId), decision);
      return Promise.resolve({ ok: true } as T);
    }
    case "workflow_start_job": {
      const { ws, jobId, approvalId, caps } = args as {
        ws: string;
        jobId: string;
        approvalId: string;
        caps?: string[];
      };
      if (!capMatches(caps ?? [], START_CAP)) return Promise.reject(new Error("denied"));
      // THE GATE: start only on an `approved` resolution; otherwise the job does not start and no
      // effect is queued (the genuine S6 gate, mirrored).
      if (resolutions.get(k(ws, approvalId)) !== "approved") {
        return Promise.resolve({ jobId, started: false } as T);
      }
      // Started → queue the PR effect through the (mirrored) transactional outbox.
      const list = effects.get(ws) ?? [];
      const key = `pr:${approvalId}`;
      if (!list.some((e) => e.idempotencyKey === key)) {
        list.push({
          target: "github",
          action: "create_pr",
          idempotencyKey: key,
          status: "pending",
        });
      }
      effects.set(ws, list);
      return Promise.resolve({ jobId, started: true } as T);
    }
    case "workflow_list_effects": {
      const { ws } = args as { ws: string };
      return Promise.resolve([...(effects.get(ws) ?? [])] as T);
    }
    default:
      return null; // not a workflow command — let the caller fall through
  }
}

/** Test helper: clear all workflow fake state. */
export function __resetWorkflowFake(): void {
  resolutions.clear();
  effects.clear();
}
