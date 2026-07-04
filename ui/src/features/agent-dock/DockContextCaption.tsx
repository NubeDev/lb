// The dock CONTEXT CAPTION (agent-dock scope) — a small line showing the page context that WILL be
// captured with the next message ("asking about: dashboards · sales"). Presentation only. Reads the
// live page context so the caption updates as the user navigates (the context is captured per-send,
// and this previews what that capture will be).

import { MapPin } from "lucide-react";

import type { PageContext } from "@/lib/channel/payload.types";

interface Props {
  context: PageContext;
}

/** A compact "surface · key hint" summary of the page context. */
function summarize(ctx: PageContext): string {
  // Prefer a recognizable search value (e.g. the dashboard id `d`) as the detail, else the path tail.
  const detail =
    ctx.search.d ??
    ctx.search.id ??
    ctx.search.c ??
    ctx.path.split("/").filter(Boolean).slice(-1)[0];
  return detail && detail !== ctx.surface ? `${ctx.surface} · ${detail}` : ctx.surface;
}

export function DockContextCaption({ context }: Props) {
  return (
    <div
      className="flex items-center gap-1.5 border-b border-border bg-panel-2/50 px-3 py-1.5 text-xs text-muted"
      title={`${context.path}${Object.keys(context.search).length ? ` — ${JSON.stringify(context.search)}` : ""}`}
    >
      <MapPin size={11} className="shrink-0" />
      <span className="min-w-0 truncate">
        asking about: <span className="text-fg/80">{summarize(context)}</span>
      </span>
    </div>
  );
}
