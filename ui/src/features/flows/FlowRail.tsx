// The flow rail (flows-canvas scope, Wave 3) — the left list of saved flows: open one, delete one,
// or start a new blank flow. Presentation only; the roster + actions come from the parent's `useFlows`
// hook. Styled to match the dashboard roster (the canonical aside) using shadcn primitives + tokens.

import { Plus, Trash2, Workflow } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { FlowSummary } from "@/lib/flows";

export interface FlowRailProps {
  roster: FlowSummary[];
  openId: string | null;
  onOpen: (id: string) => Promise<void> | void;
  onDelete: (id: string) => Promise<void> | void;
  onNew: () => void;
}

export function FlowRail({ roster, openId, onOpen, onDelete, onNew }: FlowRailProps) {
  return (
    <aside
      aria-label="flow rail"
      className="flex w-64 shrink-0 flex-col border-r border-border bg-panel shadow-sm shadow-black/5"
    >
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-3">
        <span className="text-xs font-semibold uppercase tracking-wide text-muted">Flows</span>
        <Button aria-label="new flow" onClick={onNew} variant="outline" size="sm" className="h-7 gap-1 px-2 text-xs">
          <Plus size={14} />
          New
        </Button>
      </div>
      <ul aria-label="flow roster" className="flex-1 space-y-1 overflow-auto p-2">
        {roster.length === 0 ? (
          <li className="rounded-md border border-dashed border-border bg-bg/60 px-3 py-3 text-xs text-muted">
            No flows yet.
          </li>
        ) : (
          roster.map((f) => {
            const selected = f.id === openId;
            return (
              <li key={f.id}>
                <div
                  className={cn(
                    "flex items-center gap-1 rounded-md border px-1.5 py-1 transition-colors",
                    selected
                      ? "border-accent/30 bg-accent/10"
                      : "border-transparent hover:border-border hover:bg-bg",
                  )}
                >
                  <Button
                    aria-label={`open flow ${f.id}`}
                    onClick={() => onOpen(f.id)}
                    variant="ghost"
                    size="sm"
                    className="h-auto flex-1 justify-start gap-2 px-1.5 py-1.5"
                  >
                    <Workflow
                      size={14}
                      className={cn("shrink-0", selected ? "text-accent" : "text-muted")}
                    />
                    <span
                      className={cn(
                        "min-w-0 flex-1 truncate text-left text-sm",
                        selected ? "font-medium text-accent" : "text-fg",
                      )}
                    >
                      {f.name || f.id}
                    </span>
                    <span className="text-[10px] uppercase text-muted">v{f.version}</span>
                  </Button>
                  <Button
                    aria-label={`delete flow ${f.id}`}
                    onClick={() => onDelete(f.id)}
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7 shrink-0 text-muted hover:text-destructive"
                  >
                    <Trash2 size={13} />
                  </Button>
                </div>
              </li>
            );
          })
        )}
      </ul>
    </aside>
  );
}
