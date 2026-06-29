// The reminder view types — mirror the Rust `Reminder`/`Action` record one-to-one (camelCase wire).
// These are the UI's view of `reminder.*` MCP results (the host `reminder/tool.rs` `reminder_json`).

export type ReminderStatus = "active" | "done";

export type ReminderAction =
  | { kind: "channel-post"; channel: string; body: string }
  | { kind: "mcp-tool"; tool: string; args: unknown }
  | { kind: "outbox"; target: string; action: string; payload: string };

/** A reminder — a durable, workspace-scoped schedule that fires one action when due. */
export interface Reminder {
  id: string;
  schedule: string; // 5-field cron (storage format)
  maxRuns: number | null; // null = recurring forever
  runs: number;
  enabled: boolean;
  status: ReminderStatus;
  action: ReminderAction;
  principalSub: string;
  nextAttemptTs: number;
  ts: number;
}
