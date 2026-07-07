// The persona roster + defaults surface (agent-personas scope #1, reworked by persona-session #5).
// Each row shows the persona, an ENABLE/DISABLE toggle (admin `agent.config.set` writes the roster),
// and two default setters: "Set as my default" (member `prefs.set` writes the viewer's own default)
// and "Set as workspace default" (admin `prefs.set_default`). The "Use" pick + the active highlight
// are GONE — the dock now resolves the focus client-side from page context, with a sticky per-tab pin,
// and the server's prefs fold (member → ws-default) is the new defaults home. Custom personas still
// carry edit/delete affordances; built-ins are read-only. Persona ids are OPAQUE (rule 10) — no branch.
//
// One responsibility: render the roster + affordances. The data lives in `usePersonaCatalog`.

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import type { PersonaListItem } from "@/lib/agent/agentPersona.api";

interface Props {
  personas: PersonaListItem[];
  /** The viewer's OWN default persona id (from `prefs.get`), or `null` when unset. */
  memberDefaultId: string | null;
  /** The workspace-default id (optimistic — null until an admin sets one in THIS session). */
  wsDefaultId: string | null;
  /** May the caller write the workspace roster (`agent.config.set`)? */
  canSetRoster: boolean;
  /** May the caller write the viewer's OWN default (`prefs.set`)? Member-level. */
  canSetMemberDefault: boolean;
  /** May the caller write the workspace default (`prefs.set_default`)? Admin-level. */
  canSetWsDefault: boolean;
  /** May the caller manage custom personas (create/update/delete)? */
  canManage: boolean;
  onToggleEnabled: (persona: PersonaListItem) => void;
  onSetMemberDefault: (id: string) => void;
  onClearMemberDefault: () => void;
  onSetWsDefault: (id: string) => void;
  onClearWsDefault: () => void;
  onEdit: (persona: PersonaListItem) => void;
  onDelete: (persona: PersonaListItem) => void;
}

export function PersonaCatalog({
  personas,
  memberDefaultId,
  wsDefaultId,
  canSetRoster,
  canSetMemberDefault,
  canSetWsDefault,
  canManage,
  onToggleEnabled,
  onSetMemberDefault,
  onClearMemberDefault,
  onSetWsDefault,
  onClearWsDefault,
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
        const isMemberDefault = persona.id === memberDefaultId;
        const isWsDefault = persona.id === wsDefaultId;
        return (
          <li
            key={persona.id}
            aria-label={`persona ${persona.id}`}
            data-enabled={persona.enabled ? "true" : "false"}
            data-member-default={isMemberDefault || undefined}
            data-ws-default={isWsDefault || undefined}
            className={[
              "flex flex-col gap-2 rounded-md border px-4 py-3",
              persona.enabled ? "border-border" : "border-border bg-panel-2/30 opacity-70",
            ].join(" ")}
          >
            <div className="flex items-start justify-between gap-4">
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="truncate text-sm font-medium text-fg">{persona.label}</span>
                  <span className="rounded-md bg-panel px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-muted">
                    {persona.builtin ? "Built-in" : "Custom"}
                  </span>
                  {isMemberDefault && (
                    <span className="rounded-md bg-accent/15 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-accent">
                      My default
                    </span>
                  )}
                  {isWsDefault && (
                    <span className="rounded-md bg-accent/15 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-accent">
                      Workspace default
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
                  {persona.surfaces.length > 0 ? ` · surfaces: ${persona.surfaces.join(", ")}` : ""}
                </p>
              </div>

              <div className="flex shrink-0 flex-col items-end gap-1.5">
                {/* Roster toggle — admin `agent.config.set { enabled_personas }`. When disabled, the
                    persona is hidden from the dock's context match + switcher; an explicit invoke of
                    one fails with a named disabled error. Built-ins + customs both toggle. */}
                {canSetRoster && (
                  <label className="flex items-center gap-1.5 text-[11px] text-fg">
                    <Checkbox
                      aria-label={`${persona.enabled ? "disable" : "enable"} ${persona.id}`}
                      checked={persona.enabled}
                      onChange={() => onToggleEnabled(persona)}
                    />
                    <span>{persona.enabled ? "Enabled" : "Disabled"}</span>
                  </label>
                )}
                {!canSetRoster && (
                  <span className="text-[10px] uppercase tracking-wide text-muted">
                    {persona.enabled ? "Enabled" : "Disabled"}
                  </span>
                )}

                {/* Default setters. "My default" is member-level (prefs.set); "Workspace default" is
                    admin (prefs.set_default). Clearing writes "" (the MERGE-can't-write-null workaround). */}
                <div className="flex items-center gap-1">
                  {canSetMemberDefault &&
                    (isMemberDefault ? (
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={onClearMemberDefault}
                        aria-label={`clear my default ${persona.id}`}
                        className="h-7 px-2 text-[11px]"
                      >
                        Clear my default
                      </Button>
                    ) : (
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => onSetMemberDefault(persona.id)}
                        aria-label={`set my default ${persona.id}`}
                        className="h-7 px-2 text-[11px]"
                      >
                        Set as my default
                      </Button>
                    ))}
                  {canSetWsDefault &&
                    (isWsDefault ? (
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={onClearWsDefault}
                        aria-label={`clear workspace default ${persona.id}`}
                        className="h-7 px-2 text-[11px]"
                      >
                        Clear ws default
                      </Button>
                    ) : (
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => onSetWsDefault(persona.id)}
                        aria-label={`set workspace default ${persona.id}`}
                        className="h-7 px-2 text-[11px]"
                      >
                        Set as workspace default
                      </Button>
                    ))}
                </div>

                {/* Built-ins have NO edit/delete affordance (read-only tier); custom personas do. */}
                {canManage && !persona.builtin && (
                  <div className="flex items-center gap-1">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => onEdit(persona)}
                      aria-label={`edit ${persona.id}`}
                      className="h-7 px-2 text-[11px]"
                    >
                      Edit
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => onDelete(persona)}
                      aria-label={`delete ${persona.id}`}
                      className="h-7 px-2 text-[11px]"
                    >
                      Delete
                    </Button>
                  </div>
                )}
              </div>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
