// The dock SESSION PICKER (agent-dock scope) — the header control that switches between the user's
// past dock sessions and mints a new one. Presentation + wiring only (FILE-LAYOUT); the list + current
// + mint come from `useDockSessions`. Sessions are `dock.{user-slug}.{ulid}`; we show a short, humane
// label (the trailing ulid, truncated) — the id itself is opaque.

import { MessagesSquare, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";

interface Props {
  sessions: string[];
  current: string;
  onSelect: (cid: string) => void;
  onNew: () => void;
}

/** A short, humane label for a `dock-{slug}-{ulid}` id — the last dashed segment (the ulid), tail. */
function sessionLabel(cid: string): string {
  const ulid = cid.split("-").slice(-1)[0] ?? cid;
  return ulid.slice(-6);
}

export function DockSessionPicker({ sessions, current, onSelect, onNew }: Props) {
  return (
    <div className="flex items-center gap-2">
      <label className="sr-only" htmlFor="dock-session">
        dock session
      </label>
      <div className="relative min-w-0 flex-1">
        <MessagesSquare
          size={13}
          className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted"
        />
        <select
          id="dock-session"
          aria-label="dock session"
          value={current}
          onChange={(e) => onSelect(e.target.value)}
          className="h-8 w-full min-w-0 truncate rounded-md border border-border bg-bg pl-7 pr-2 text-xs text-fg outline-none focus-visible:border-accent"
        >
          {sessions.map((cid) => (
            <option key={cid} value={cid}>
              session {sessionLabel(cid)}
            </option>
          ))}
        </select>
      </div>
      <Button
        type="button"
        size="sm"
        variant="outline"
        aria-label="new session"
        onClick={onNew}
        className="h-8 shrink-0 px-2"
      >
        <Plus size={14} />
      </Button>
    </div>
  );
}
