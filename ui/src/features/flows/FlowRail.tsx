// The flow rail (flows-canvas scope, Wave 3) — the left list of saved flows: open one, delete one,
// or start a new blank flow. Presentation only; the roster + actions come from the parent's `useFlows`
// hook.

import { Button } from "@/components/ui/button";
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
    <div
      aria-label="flow rail"
      className="flex w-52 flex-col gap-2 border-r border-border p-2"
    >
      <Button aria-label="new flow" onClick={onNew} variant="outline" size="sm" className="w-full">
        + New flow
      </Button>
      {roster.length === 0 ? (
        <div className="text-xs text-muted">No flows yet.</div>
      ) : (
        <ul className="m-0 list-none p-0">
          {roster.map((f) => (
            <li key={f.id} className="flex items-center gap-1">
              <Button
                aria-label={`open flow ${f.id}`}
                onClick={() => onOpen(f.id)}
                variant="ghost"
                size="sm"
                className="flex-1 justify-start"
                style={{ fontWeight: f.id === openId ? 700 : 400 }}
              >
                {f.name || f.id}{" "}
                <span className="text-muted">v{f.version}</span>
              </Button>
              <Button
                aria-label={`delete flow ${f.id}`}
                onClick={() => onDelete(f.id)}
                variant="ghost"
                size="sm"
              >
                ✕
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
