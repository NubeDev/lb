// The dock COMPOSER (agent-dock scope) — the ask box at the foot of the panel. Presentation + local
// draft only (FILE-LAYOUT: the send goes through the callback; no data here). Enter sends, Shift+Enter
// newlines; disabled while a run is in flight so a session can't fan out concurrent runs from the dock.

import { useState } from "react";
import { SendHorizontal } from "lucide-react";

import { Button } from "@/components/ui/button";

interface Props {
  onAsk: (goal: string) => void;
  /** True while a run is pending — the composer is disabled (one run at a time in the dock). */
  busy: boolean;
}

export function DockComposer({ onAsk, busy }: Props) {
  const [draft, setDraft] = useState("");

  const submit = () => {
    const goal = draft.trim();
    if (!goal || busy) return;
    onAsk(goal);
    setDraft("");
  };

  return (
    <form
      className="flex items-end gap-2 border-t border-border bg-panel p-3"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <textarea
        aria-label="ask the agent"
        rows={1}
        value={draft}
        disabled={busy}
        placeholder={busy ? "Working…" : "Ask about this page…"}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            submit();
          }
        }}
        className="max-h-32 min-h-[2.25rem] flex-1 resize-none rounded-md border border-border bg-bg px-2.5 py-1.5 text-sm text-fg outline-none placeholder:text-muted focus-visible:border-accent focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-60"
      />
      <Button
        type="submit"
        size="sm"
        aria-label="send"
        disabled={busy || !draft.trim()}
        className="h-9 shrink-0 px-2.5"
      >
        <SendHorizontal size={15} />
      </Button>
    </form>
  );
}
