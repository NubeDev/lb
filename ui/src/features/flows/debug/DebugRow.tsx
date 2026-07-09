// One row in the flow debug panel (debug-node-scope): renders a single `DebugMessage` the
// live stream delivered. Presentation only — the panel owns the stream + the message list.
// A `debug` frame renders `value` per its `format` (json → <pre> tree, text/markdown → <pre>),
// auto-collapsing when the serialized size exceeds `collapseBytes` (0 = never). A `dropped`
// frame is the publish-governor sentinel ("N messages suppressed"), rendered as a muted note.

import { useMemo, useState } from "react";

import { cn } from "@/lib/utils";
import type { DebugMessage } from "@/lib/flows";

interface DebugRowProps {
  msg: DebugMessage;
}

/** Serialize a debug value to the string the row renders (json pretty-printed, others as-is). */
function renderValue(msg: DebugMessage): string {
  if (msg.format === "json") {
    try {
      return JSON.stringify(msg.value, null, 2);
    } catch {
      return String(msg.value);
    }
  }
  return typeof msg.value === "string" ? msg.value : JSON.stringify(msg.value);
}

export function DebugRow({ msg }: DebugRowProps) {
  const body = useMemo(() => (msg.kind === "debug" ? renderValue(msg) : ""), [msg]);
  // Collapse when the body exceeds the node's threshold (0 = never). Start collapsed; the full
  // value is always on the wire, so expanding is a pure UI toggle.
  const collapsible = msg.collapseBytes ? body.length > msg.collapseBytes : false;
  const [expanded, setExpanded] = useState(false);

  const label = msg.label ?? msg.node;

  if (msg.kind === "dropped") {
    return (
      <li className="px-3 py-2 text-xs text-muted-foreground">
        <span className="font-medium">{label}</span> — {msg.dropped ?? 0} message
        {msg.dropped === 1 ? "" : "s"} suppressed under the rate limit
      </li>
    );
  }

  return (
    <li className="border-b border-border px-3 py-2 last:border-b-0">
      <div className="flex items-center justify-between gap-2">
        <span className="truncate text-xs font-medium">{label}</span>
        <span className="shrink-0 text-[10px] uppercase tracking-wide text-muted-foreground">
          {msg.format ?? "text"}
        </span>
      </div>
      <pre
        className={cn(
          "mt-1 overflow-x-auto whitespace-pre-wrap break-words rounded bg-muted/50 p-2 text-xs",
          collapsible && !expanded && "max-h-24 overflow-y-hidden",
        )}
      >
        {body}
      </pre>
      {collapsible ? (
        <button
          type="button"
          className="mt-1 text-[11px] text-primary hover:underline"
          onClick={() => setExpanded((v) => !v)}
        >
          {expanded ? "Collapse" : "Expand"}
        </button>
      ) : null}
    </li>
  );
}
