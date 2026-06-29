// One function-palette row — the verb name + signature, a one-line summary, and a click-to-insert
// affordance (rules-editor-ux scope). The whole row is the insert button (a big click target); the
// signature is monospaced, the summary muted. Hover reveals the full summary via the title attr for
// long lines. One component per file (FILE-LAYOUT).

import { Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { FnEntry } from "../catalog";

interface FunctionEntryProps {
  entry: FnEntry;
  onInsert: (snippet: string) => void;
}

/** A single palette entry; clicking inserts its snippet at the editor cursor. */
export function FunctionEntry({ entry, onInsert }: FunctionEntryProps) {
  return (
    <Button
      type="button"
      variant="ghost"
      aria-label={`insert ${entry.name}`}
      title={entry.summary}
      onClick={() => onInsert(entry.snippet)}
      className="group h-auto w-full flex-col items-start gap-0.5 rounded-md px-2 py-1.5 text-left hover:bg-muted"
    >
      <span className="flex w-full items-center justify-between gap-2">
        <code className="font-mono text-xs text-fg">{entry.signature}</code>
        <Plus size={12} className="shrink-0 text-muted opacity-0 transition-opacity group-hover:opacity-100" />
      </span>
      <span className="line-clamp-2 text-[11px] font-normal leading-snug text-muted">
        {entry.summary}
      </span>
    </Button>
  );
}
