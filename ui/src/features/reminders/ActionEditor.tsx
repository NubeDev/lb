// The action editor — pick + configure the ONE action a reminder fires (reminders scope). Three
// kinds at v1: channel post, MCP tool, outbox effect (the existing seams; no new transport here).
// One component per file (FILE-LAYOUT). The MCP-tool action's `args` is best-effort checked at
// create time; authoritative validation is at fire time (the host re-enters call_tool).

import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type { ReminderAction } from "@/lib/reminders/reminders.types";

interface Props {
  action: ReminderAction;
  onChange: (action: ReminderAction) => void;
}

const KINDS: { value: ReminderAction["kind"]; label: string }[] = [
  { value: "channel-post", label: "Channel post" },
  { value: "mcp-tool", label: "MCP tool" },
  { value: "outbox", label: "Outbox effect" },
];

function blank(kind: ReminderAction["kind"]): ReminderAction {
  switch (kind) {
    case "channel-post":
      return { kind: "channel-post", channel: "", body: "" };
    case "mcp-tool":
      return { kind: "mcp-tool", tool: "", args: {} };
    case "outbox":
      return { kind: "outbox", target: "", action: "", payload: "" };
  }
}

/** Edit the action. Changing the kind resets the payload to a blank of that kind. */
export function ActionEditor({ action, onChange }: Props) {
  return (
    <div className="space-y-2">
      <label className="text-xs font-medium text-muted">Action</label>
      <Select
        aria-label="action kind"
        className="bg-card text-sm"
        value={action.kind}
        onChange={(e) => onChange(blank(e.target.value as ReminderAction["kind"]))}
      >
        {KINDS.map((k) => (
          <option key={k.value} value={k.value}>
            {k.label}
          </option>
        ))}
      </Select>

      {action.kind === "channel-post" && (
        <div className="space-y-2">
          <Input
            placeholder="channel (e.g. team)"
            value={action.channel}
            onChange={(e) => onChange({ ...action, channel: e.target.value })}
          />
          <Textarea
            placeholder="message body"
            value={action.body}
            onChange={(e) => onChange({ ...action, body: e.target.value })}
          />
        </div>
      )}

      {action.kind === "mcp-tool" && (
        <div className="space-y-2">
          <Input
            placeholder="tool (e.g. store.schema)"
            value={action.tool}
            onChange={(e) => onChange({ ...action, tool: e.target.value })}
          />
          <Textarea
            placeholder="args (JSON)"
            value={typeof action.args === "string" ? action.args : JSON.stringify(action.args ?? {})}
            onChange={(e) => {
              try {
                onChange({ ...action, args: JSON.parse(e.target.value) });
              } catch {
                onChange({ ...action, args: e.target.value });
              }
            }}
          />
        </div>
      )}

      {action.kind === "outbox" && (
        <div className="space-y-2">
          <Input
            placeholder="target (e.g. email)"
            value={action.target}
            onChange={(e) => onChange({ ...action, target: e.target.value })}
          />
          <Input
            placeholder="action (e.g. notify)"
            value={action.action}
            onChange={(e) => onChange({ ...action, action: e.target.value })}
          />
          <Textarea
            placeholder="payload (string or JSON)"
            value={action.payload}
            onChange={(e) => onChange({ ...action, payload: e.target.value })}
          />
        </div>
      )}
    </div>
  );
}
