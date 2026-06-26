// The in-memory agent stand-in used when NOT in the Tauri shell (plain browser, tests). It mirrors
// the node's `agent.invoke` contract faithfully enough for the UI to behave identically here and
// against the real node (the verb name + shapes match the Rust command one-to-one).
//
// Faithful to the gates the user actually hits:
//   - the INVOKE gate: without `mcp:agent.invoke:call` in the caller's caps, it rejects "denied"
//     (the same opaque deny the Rust `agent_test` proves before the loop runs);
//   - the SUBSTRATE grant: invoking with a skill that was not granted rejects "denied" (the S4
//     grant gate the agent inherits).
// It persists a durable "job" (so the UI can show the session id survives) and returns the answer.
//
// One file per concern (FILE-LAYOUT): the agent fake lives beside the channel + asset fakes.

import type { AgentResult } from "@/lib/agent/agent.types";

const INVOKE_CAP = "mcp:agent.invoke:call";

// Workspace-scoped state (key prefix = ws) — the wall, mirrored.
const jobs = new Map<string, AgentResult>(); // `${ws}/${jobId}` -> last result (durable session)
const grantedSkills = new Set<string>(); // `${ws}/${skill}` -> granted to the workspace

const k = (ws: string, x: string) => `${ws}/${x}`;

/** Test seam: grant a skill to the workspace so the agent may load it as substrate (mirrors
 *  `host::grant_skill`). Without this, invoking with that skill is denied. */
export function __grantAgentSkill(ws: string, skill: string): void {
  grantedSkills.add(k(ws, skill));
}

function capMatches(held: string[], cap: string): boolean {
  // The fake mirrors the grammar coarsely: an exact cap, or a `*`/`**` wildcard on the resource.
  return held.some((h) => h === cap || h === "mcp:agent.*:call" || h === "mcp:*:call");
}

export function agentFakeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> | null {
  if (cmd !== "agent_invoke") return null; // not the agent command — let the caller fall through
  const { ws, jobId, goal, skill, caps } = args as {
    ws: string;
    jobId: string;
    goal: string;
    skill?: string;
    caps?: string[];
  };

  // Gate 1: the invoke capability. No grant → opaque denied, before any "loop" runs.
  if (!capMatches(caps ?? [], INVOKE_CAP)) {
    return Promise.reject(new Error("denied"));
  }
  // Substrate grant gate: an ungranted skill is invisible to the agent (S4 grant gate).
  if (skill && !grantedSkills.has(k(ws, skill))) {
    return Promise.reject(new Error("denied"));
  }

  // The "loop" is scripted: the answer echoes the goal (deterministic, like the mock provider).
  const result: AgentResult = { answer: `agent: ${goal}`, jobId };
  jobs.set(k(ws, jobId), result); // persist the durable session
  return Promise.resolve(result as T);
}

/** Test helper: clear all agent fake state. */
export function __resetAgentFake(): void {
  jobs.clear();
  grantedSkills.clear();
}
