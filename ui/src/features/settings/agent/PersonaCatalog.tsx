// The persona list/picker (agent-personas scope #1) — renders the pickable personas from
// `agent.persona.list`: seeded built-ins (read-only, no edit/delete affordance) + the workspace's
// custom personas (admin can edit/delete). The active selection (`agent.config.active_persona`) is
// highlighted. Picking a persona writes `agent.config` (needs `agent.config.set`); a member sees the
// list + active pick read-only. Persona ids are OPAQUE (rule 10) — no branch on a specific id.

import { Button } from "@/components/ui/button";
import type { Persona } from "@/lib/agent/agentPersona.api";

interface Props {
  personas: Persona[];
  activeId: string | null;
  /** May the caller set `agent.config.active_persona` (the "Use" pick)? */
  canPick: boolean;
  /** May the caller manage custom personas (create/update/delete)? */
  canManage: boolean;
  onPick: (persona: Persona) => void;
  onEdit: (persona: Persona) => void;
  onDelete: (persona: Persona) => void;
}

export function PersonaCatalog({
  personas,
  activeId,
  canPick,
  canManage,
  onPick,
  onEdit,
  onDelete,
}: Props) {
  if (personas.length === 0) {
    return (
      <p className="rounded-md border border-dashed border-border px-4 py-6 text-center text-sm text-muted">
        No personas available for this workspace.
      </p>
    );
  }

  return (
    <ul className="flex flex-col gap-2" aria-label="persona catalog">
      {personas.map((persona) => {
        const active = persona.id === activeId;
        return (
          <li
            key={persona.id}
            aria-label={`persona ${persona.id}`}
            data-active={active || undefined}
            className={[
              "flex flex-col gap-2 rounded-md border px-4 py-3",
              active ? "border-accent bg-accent/5" : "border-border",
            ].join(" ")}
          >
            <div className="flex items-start justify-between gap-4">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="truncate text-sm font-medium text-fg">{persona.label}</span>
                  <span className="rounded-md bg-panel px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-muted">
                    {persona.builtin ? "Built-in" : "Custom"}
                  </span>
                  {active && (
                    <span className="rounded-md bg-accent/15 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-accent">
                      Active
                    </span>
                  )}
                </div>
                {persona.description && (
                  <p className="mt-0.5 truncate text-xs text-muted">{persona.description}</p>
                )}
                <p className="mt-0.5 text-[11px] text-muted">
                  {persona.granted_tools.length} tool
                  {persona.granted_tools.length === 1 ? "" : "s"} ·{" "}
                  {persona.grounding_skills.length} skill
                  {persona.grounding_skills.length === 1 ? "" : "s"}
                  {persona.extends.length > 0 ? ` · extends ${persona.extends.join(", ")}` : ""}
                </p>
              </div>

              <div className="flex shrink-0 items-center gap-2">
                {canPick && !active && (
                  <Button
                    size="sm"
                    onClick={() => onPick(persona)}
                    aria-label={`use ${persona.id}`}
                  >
                    Use
                  </Button>
                )}
                {/* Built-ins have NO edit/delete affordance (read-only tier); custom personas do. */}
                {canManage && !persona.builtin && (
                  <>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => onEdit(persona)}
                      aria-label={`edit ${persona.id}`}
                    >
                      Edit
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => onDelete(persona)}
                      aria-label={`delete ${persona.id}`}
                    >
                      Delete
                    </Button>
                  </>
                )}
              </div>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
