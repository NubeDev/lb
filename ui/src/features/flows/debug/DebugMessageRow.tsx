// One debug message row (debug-node-scope) — attribution line (node label, time, run) + the value
// rendered type-aware: `json` → pretty-printed tree, `markdown` → rendered (react-markdown +
// remark-gfm, the channel MarkdownView precedent), `text` → <pre>. Auto-collapses when the rendered
// payload exceeds the node's `collapseBytes` hint (the full value is always present — expand to see
// it). A `dropped` sentinel renders as a muted governor line, no value. One responsibility: render
// one message; the buffer lives in useDebugStream, the list in DebugPanel.

import { useMemo, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

import { Button } from "@/components/ui/button";
import type { DebugMessage } from "@/lib/flows";
import { cn } from "@/lib/utils";

export function DebugMessageRow({ msg }: { msg: DebugMessage }) {
  const rendered = useMemo(() => renderValue(msg.value), [msg.value]);
  const collapseAt = msg.collapseBytes ?? 0;
  const collapsible = collapseAt > 0 && rendered.length > collapseAt;
  const [expanded, setExpanded] = useState(false);

  const when = msg.ts ? new Date(msg.ts).toLocaleTimeString() : "";

  if (msg.kind === "dropped") {
    return (
      <div
        aria-label="debug dropped"
        className="border-b border-border/60 px-3 py-1.5 text-[11px] italic text-muted"
      >
        {when ? `${when} · ` : ""}
        {msg.dropped ?? "some"} message{(msg.dropped ?? 2) === 1 ? "" : "s"} suppressed by the
        rate limit
      </div>
    );
  }

  return (
    <div aria-label="debug message" className="border-b border-border/60 px-3 py-1.5">
      <div className="flex items-baseline gap-2 text-[10px] text-muted">
        <span className="font-medium text-fg">{msg.label ?? msg.node}</span>
        {when ? <span>{when}</span> : null}
        {msg.runId ? <span className="truncate font-mono">{msg.runId}</span> : null}
        {collapsible ? (
          <Button
            type="button"
            variant="ghost"
            aria-label={expanded ? "collapse value" : "expand value"}
            aria-expanded={expanded}
            onClick={() => setExpanded((e) => !e)}
            className="ml-auto h-auto gap-0.5 rounded-sm px-1 py-0 text-[10px] font-normal text-muted hover:text-fg"
          >
            {expanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
            {expanded ? "collapse" : `${rendered.length} bytes`}
          </Button>
        ) : null}
      </div>
      <div
        className={cn(
          "mt-1 overflow-x-auto text-xs text-fg",
          collapsible && !expanded && "max-h-16 overflow-y-hidden",
        )}
      >
        {msg.format === "markdown" && typeof msg.value === "string" ? (
          <div className="prose prose-sm dark:prose-invert max-w-none">
            <Markdown remarkPlugins={[remarkGfm]}>{msg.value}</Markdown>
          </div>
        ) : (
          <pre className="whitespace-pre-wrap break-words font-mono text-[11px] leading-snug">
            {rendered}
          </pre>
        )}
      </div>
    </div>
  );
}

/** Render a wire value to its display string: strings verbatim, everything else pretty JSON. */
function renderValue(value: unknown): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
