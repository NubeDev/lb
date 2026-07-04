// One function-palette row — the verb name + signature, a one-line summary, and click-to-insert
// (rules-editor-ux scope). The card header inserts on click (the big target); a quiet "Preview" toggle
// reveals the exact snippet code the insert will drop, so a newcomer can read what they're about to add
// before committing it. Accent-bordered on hover; the disclosed code sits on the shell's code surface so
// it reads in both light and dark. One component per file (FILE-LAYOUT).

import { useState } from "react";
import { ChevronDown, Code2, Plus } from "lucide-react";

import { cn } from "@/lib/utils";
import type { FnEntry } from "../catalog";

interface FunctionEntryProps {
  entry: FnEntry;
  onInsert: (snippet: string) => void;
}

/** A single palette entry; the header inserts, the disclosure previews the exact snippet code. */
export function FunctionEntry({ entry, onInsert }: FunctionEntryProps) {
  const [open, setOpen] = useState(false);

  return (
    <div
      className={cn(
        "group rounded-md border border-transparent bg-card transition-colors",
        "hover:border-border hover:bg-muted/40",
      )}
    >
      <button
        type="button"
        aria-label={`insert ${entry.name}`}
        title={entry.summary}
        onClick={() => onInsert(entry.snippet)}
        className="flex w-full flex-col items-start gap-0.5 rounded-md px-2.5 py-1.5 text-left focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
      >
        <span className="flex w-full items-center justify-between gap-2">
          <code className="truncate font-mono text-xs text-fg">{entry.signature}</code>
          <Plus
            size={13}
            className="shrink-0 text-accent opacity-0 transition-opacity group-hover:opacity-100"
          />
        </span>
        <span className="line-clamp-2 text-[11px] leading-snug text-muted">{entry.summary}</span>
      </button>

      <div className="flex items-center px-2.5 pb-1">
        <button
          type="button"
          aria-label={`preview ${entry.name} snippet`}
          aria-expanded={open}
          onClick={() => setOpen((v) => !v)}
          className="flex items-center gap-1 rounded-md text-[10px] font-medium uppercase tracking-wide text-muted transition-colors hover:text-fg focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
        >
          <ChevronDown
            size={11}
            className={cn("transition-transform", open && "rotate-180")}
          />
          Preview
        </button>
      </div>

      {open ? (
        <div className="px-2.5 pb-2">
          <div className="flex items-start gap-1.5 rounded-md border border-border bg-muted-bg px-2 py-1.5">
            <Code2 size={11} className="mt-0.5 shrink-0 text-muted" />
            <code className="whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-fg">
              {entry.snippet}
            </code>
          </div>
        </div>
      ) : null}
    </div>
  );
}
