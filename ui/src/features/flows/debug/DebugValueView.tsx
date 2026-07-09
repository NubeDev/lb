// DebugValueView — the type-aware value renderer for one debug message (debug-node-scope). Dispatches
// by the message's `format`: json → JsonTreeView, markdown → the shared MarkdownView (reused, no
// duplicate helper — rule 8), text → TextView. The long-content **auto-collapse** (Decision 6) lives
// here: a value whose rendered text size exceeds `collapseBytes` renders collapsed behind a
// "show more / show less" disclosure (shadcn Collapsible); the full value is always on the wire,
// collapse is presentation only.

import { useState } from "react";

import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { MarkdownView } from "@/features/channel/MarkdownView";

import { JsonTreeView } from "./JsonTreeView";
import { TextView } from "./TextView";

interface Props {
  /** The wire value the debug node captured. */
  value: unknown;
  /** `json` | `text` | `markdown` — resolved host-side. */
  format: "json" | "text" | "markdown";
  /** The collapse threshold (bytes); 0 or absent = never collapse. */
  collapseBytes?: number;
}

/** Estimate the rendered byte size of a value (the metric the collapse threshold compares against). */
function renderedSize(value: unknown): number {
  if (typeof value === "string") return value.length;
  if (value === null || value === undefined) return 4;
  try {
    return JSON.stringify(value).length;
  } catch {
    return String(value).length;
  }
}

export function DebugValueView({ value, format, collapseBytes }: Props) {
  const limit = collapseBytes && collapseBytes > 0 ? collapseBytes : Infinity;
  const oversized = renderedSize(value) > limit;
  const [open, setOpen] = useState(false);

  if (!oversized) {
    return <DebugValueBody value={value} format={format} />;
  }
  // Long content: collapsed by default with a "show more / show less" disclosure (Decision 6).
  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleContent hidden={false} className="data-[state=closed]:hidden">
        <DebugValueBody value={value} format={format} />
      </CollapsibleContent>
      <CollapsibleTrigger
        aria-label={open ? "show less" : "show more"}
        className="mt-1 text-xs text-accent hover:underline"
      >
        {open ? "show less" : "show more"}
      </CollapsibleTrigger>
    </Collapsible>
  );
}

/** Render the value body in the declared format, with no collapse (the collapse wraps this). */
function DebugValueBody({ value, format }: Props) {
  switch (format) {
    case "json":
      return <JsonTreeView value={value} />;
    case "markdown":
      return (
        <div className="text-sm">
          <MarkdownView>{typeof value === "string" ? value : String(value ?? "")}</MarkdownView>
        </div>
      );
    case "text":
    default:
      return <TextView value={value} />;
  }
}
