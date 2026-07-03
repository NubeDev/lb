// The catalog list/picker (agent-catalog scope) — renders the pickable presets from `agent.def.list`:
// seeded built-ins (read-only, no edit/delete affordance) + the workspace's custom entries (admin can
// edit/delete). The active selection (resolved from `agent.config`) is highlighted. Picking a preset
// writes `agent.config` (admin only); a member sees the list + active pick read-only.
//
// Registry drift is reused: a definition whose runtime the node no longer offers is shown disabled
// with the shipped "not currently available" note, never silently dropped.

import { Button } from "@/components/ui/button";
import type { AgentDefinition } from "@/lib/agent/agentDef.api";
import type { AgentRuntimes } from "@/lib/agent/runtimes.api";
import { AgentTestButton } from "./AgentTestButton";

interface Props {
  definitions: AgentDefinition[];
  runtimes: AgentRuntimes | null;
  activeId: string | null;
  canPick: boolean;
  canManage: boolean;
  /** May the caller run the context-proving `agent.def.test` (admin-tier — it spends a model turn)? */
  canTest: boolean;
  onPick: (def: AgentDefinition) => void;
  onEdit: (def: AgentDefinition) => void;
  onDelete: (def: AgentDefinition) => void;
}

/** Is `def`'s runtime one the node currently offers? A drifted custom entry is shown disabled. */
function runnable(def: AgentDefinition, runtimes: AgentRuntimes | null): boolean {
  return !runtimes || runtimes.runtimes.includes(def.runtime);
}

export function AgentCatalog({
  definitions,
  runtimes,
  activeId,
  canPick,
  canManage,
  canTest,
  onPick,
  onEdit,
  onDelete,
}: Props) {
  if (definitions.length === 0) {
    return (
      <p className="rounded-md border border-dashed border-border px-4 py-6 text-center text-sm text-muted">
        No agent definitions available for this node.
      </p>
    );
  }

  return (
    <ul className="flex flex-col gap-2" aria-label="agent catalog">
      {definitions.map((def) => {
        const active = def.id === activeId;
        const available = runnable(def, runtimes);
        return (
          <li
            key={def.id}
            aria-label={`definition ${def.id}`}
            data-active={active || undefined}
            className={[
              "flex flex-col gap-2 rounded-md border px-4 py-3",
              active ? "border-accent bg-accent/5" : "border-border",
              !available ? "opacity-60" : "",
            ].join(" ")}
          >
           <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <span className="truncate text-sm font-medium text-fg">{def.label}</span>
                {def.builtin ? (
                  <span className="rounded bg-panel px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-muted">
                    Built-in
                  </span>
                ) : (
                  <span className="rounded bg-panel px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-muted">
                    Custom
                  </span>
                )}
                {active && (
                  <span className="rounded bg-accent/15 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-accent">
                    Active
                  </span>
                )}
              </div>
              {def.description && (
                <p className="mt-0.5 truncate text-xs text-muted">{def.description}</p>
              )}
              <p className="mt-0.5 text-[11px] text-muted">
                {def.runtime} · {def.model_endpoint.provider}/{def.model_endpoint.model}
                {def.model_endpoint.api_key_env ? ` · ${def.model_endpoint.api_key_env}` : ""}
              </p>
              {!available && (
                <p role="alert" className="mt-1 text-[11px] text-amber-500">
                  This runtime is not currently available on this node.
                </p>
              )}
            </div>

            <div className="flex shrink-0 items-center gap-2">
              {canPick && !active && available && (
                <Button size="sm" onClick={() => onPick(def)} aria-label={`pick ${def.id}`}>
                  Use
                </Button>
              )}
              {/* Built-ins have NO edit/delete affordance (read-only tier); custom entries do. */}
              {canManage && !def.builtin && (
                <>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => onEdit(def)}
                    aria-label={`edit ${def.id}`}
                  >
                    Edit
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => onDelete(def)}
                    aria-label={`delete ${def.id}`}
                  >
                    Delete
                  </Button>
                </>
              )}
            </div>
           </div>

            {/* The context-proving Test: available per entry (admin-tier). Renders the model's reply
                + a "context: N tools, M skills" line inline. Absent when the runtime isn't runnable
                (a drifted entry can't be tested) or the caller lacks the test cap. */}
            {canTest && available && (
              <div className="border-t border-border/50 pt-2">
                <AgentTestButton id={def.id} />
              </div>
            )}
          </li>
        );
      })}
    </ul>
  );
}
