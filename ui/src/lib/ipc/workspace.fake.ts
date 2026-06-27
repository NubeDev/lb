// The in-memory workspace-directory fake (TEST-ONLY) — mirrors the gateway `workspace_list` /
// `workspace_create`. The directory is node-level (not workspace-scoped), so this is one flat map of
// `ws → WorkspaceRecord`, exactly like the reserved-namespace record the real node keeps. Returns
// `null` for unowned commands (fake-chain convention).

import type { WorkspaceRecord } from "@/lib/workspace/workspace.types";

const directory = new Map<string, WorkspaceRecord>();
let seq = 0;

export function workspaceFakeInvoke<T>(cmd: string, args?: Record<string, unknown>): T | null {
  switch (cmd) {
    case "workspace_list":
      return [...directory.values()].sort((a, b) => a.ts - b.ts) as T;
    case "workspace_create": {
      const { ws, name } = args as { ws: string; name: string };
      const record: WorkspaceRecord = { ws, name, kind: "workspace", ts: ++seq };
      directory.set(ws, record);
      return record as T;
    }
    default:
      return null;
  }
}

/** Test helper: clear the fake directory between tests. */
export function __resetWorkspaceFake(): void {
  directory.clear();
  seq = 0;
}
