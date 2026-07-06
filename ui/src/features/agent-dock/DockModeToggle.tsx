// The dock's ASK/TOOLS mode toggle (agent-context-basket scope) — presentation only. "Ask" shows the
// free-text composer; "Tools" mounts the shared channel CommandPalette so the user can run catalog
// tools (query a datasource, list reminders, …) whose results land as durable dock-channel items —
// gatherable into the context basket for the next ask.

import { MessageSquare, Wrench } from "lucide-react";

import { Button } from "@/components/ui/button";

export type DockMode = "ask" | "tools";

interface Props {
  mode: DockMode;
  onMode: (mode: DockMode) => void;
}

export function DockModeToggle({ mode, onMode }: Props) {
  const btn = (m: DockMode, label: string, Icon: typeof Wrench) => (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      role="tab"
      aria-selected={mode === m}
      aria-label={`${label} mode`}
      onClick={() => onMode(m)}
      className={`h-7 gap-1.5 px-2 text-xs font-medium ${
        mode === m ? "bg-accent/15 text-accent hover:text-accent" : "text-muted hover:text-fg"
      }`}
    >
      <Icon size={13} />
      {label}
    </Button>
  );
  return (
    <div
      role="tablist"
      aria-label="dock input mode"
      className="flex items-center gap-1 border-t border-border bg-panel-2/40 px-3 py-1.5"
    >
      {btn("ask", "Ask", MessageSquare)}
      {btn("tools", "Tools", Wrench)}
    </div>
  );
}
