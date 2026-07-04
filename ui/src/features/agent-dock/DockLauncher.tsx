// The dock LAUNCHER (agent-dock scope) — the StatusBar button that toggles the agent dock, plus a
// run-state pip that lights while a dock run is in flight. Presentation only (FILE-LAYOUT); the toggle
// + running signal come from the shell's dock chrome. The `mod+j` shortcut toggles the same state (one
// shell listener) — the button is the discoverable, pointer affordance for it.

import { forwardRef } from "react";
import { Bot } from "lucide-react";

import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  running: boolean;
  onToggle: () => void;
}

/** The launcher button. `forwardRef` so the shell can return focus here when the dock closes on Esc. */
export const DockLauncher = forwardRef<HTMLButtonElement, Props>(function DockLauncher(
  { open, running, onToggle },
  ref,
) {
  return (
    <button
      ref={ref}
      type="button"
      aria-label="toggle agent dock"
      aria-pressed={open}
      aria-keyshortcuts="Meta+J Control+J"
      title="Agent dock (⌘/Ctrl+J)"
      onClick={onToggle}
      className={cn(
        "relative inline-flex h-5 items-center gap-1 rounded-md px-1.5 text-[11px] transition-colors",
        open ? "bg-accent/15 text-accent" : "text-muted hover:text-fg",
      )}
    >
      <Bot size={12} className="shrink-0" />
      <span className="hidden sm:inline">Agent</span>
      {running && (
        <span
          aria-label="run in progress"
          className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent"
        />
      )}
    </button>
  );
});
