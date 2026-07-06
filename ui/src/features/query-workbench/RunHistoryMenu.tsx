// The run-history dropdown in the workbench run bar — the last 10 unique runs for this source,
// most-recent first; clicking one restores it into the editor (Code mode — the SQL is the truth
// being restored). One responsibility (FILE-LAYOUT): render + pick; the fold lives in
// `runHistory.ts`, the restore semantics in the host (QueryWorkbench).

import { useEffect, useRef, useState } from "react";
import { History } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { RunHistoryEntry } from "./runHistory";

interface Props {
  entries: RunHistoryEntry[];
  onRestore: (sql: string) => void;
}

/** "History" button + dropdown list of recent runs. Hidden entirely when there is no history. */
export function RunHistoryMenu({ entries, onRestore }: Props) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  // Close on any outside click (the usual lightweight popover discipline).
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  if (entries.length === 0) return null;

  return (
    <div ref={rootRef} className="relative">
      <Button
        type="button"
        variant="ghost"
        size="sm"
        aria-label="run history"
        title="Recent runs (last 10, unique)"
        className="gap-1.5"
        onClick={() => setOpen((o) => !o)}
      >
        <History size={13} /> History
      </Button>
      {open && (
        <div
          role="menu"
          aria-label="run history list"
          className="absolute bottom-full left-0 z-50 mb-1 max-h-72 w-[420px] overflow-y-auto rounded-md border border-border bg-panel p-1 shadow-lg"
        >
          {entries.map((e) => (
            <Button
              key={e.sql}
              type="button"
              variant="ghost"
              role="menuitem"
              className="block h-auto w-full rounded-md px-2 py-1.5 text-left"
              title={e.sql}
              onClick={() => {
                onRestore(e.sql);
                setOpen(false);
              }}
            >
              <span className="block truncate font-mono text-[11px] text-fg">{e.sql}</span>
              <span className="block text-[10px] text-muted">{new Date(e.ts).toLocaleString()}</span>
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}
