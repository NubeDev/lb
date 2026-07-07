// The Persona section of the Agent tab (agent-personas scope #1, reworked by persona-session #5) —
// the three panes below the definition catalog. One mental model: *agent = who runs (definition) ×
// what for (persona)*. The roster (enable/disable per persona) curates the dock's advertisement +
// context-match layer; the per-member and workspace defaults live in the prefs chain (member → ws);
// the dock's per-tab pin is the last-mile override. The definition catalog stays in `AgentTab`.
//
// The boundary (load-bearing): every pane here edits ADVERTISEMENT + SUPERVISION, never the wall. No
// control grants or revokes a capability; Effective-tools shows the live `persona ∩ agent ∩ caller`.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { CAP, hasCap } from "@/lib/session";
import type { Persona } from "@/lib/agent/agentPersona.api";
import { PersonaCatalog } from "./PersonaCatalog";
import { PersonaEditor } from "./PersonaEditor";
import { EffectiveTools } from "./EffectiveTools";
import { PolicyPane } from "./PolicyPane";
import { usePersonaCatalog } from "./usePersonaCatalog";

interface Props {
  caps: string[] | undefined;
}

type EditorState = { open: false } | { open: true; editing: Persona | null };

export function PersonaSection({ caps }: Props) {
  // Roster writes need `agent.config.set` (admin) — the same gate the definition/runtime pick uses.
  const canSetRoster = hasCap(caps, CAP.agentConfigSet);
  // "My default" is member-level — every member may write their own prefs record.
  const canSetMemberDefault = hasCap(caps, CAP.prefsSet);
  // "Workspace default" needs the admin `prefs.set_default` cap.
  const canSetWsDefault = hasCap(caps, CAP.prefsSetDefault);
  const canManage =
    hasCap(caps, CAP.agentPersonaCreate) ||
    hasCap(caps, CAP.agentPersonaUpdate) ||
    hasCap(caps, CAP.agentPersonaDelete);
  const canEditPolicy = hasCap(caps, CAP.agentPolicySet);

  const catalog = usePersonaCatalog();
  const [editor, setEditor] = useState<EditorState>({ open: false });
  const [error, setError] = useState<string | null>(null);
  // Which persona the Effective-tools + Permissions panes reflect: an explicit selection, else the
  // first enabled persona (a sensible default for inspection). Kept as an id (opaque — no branch).
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const firstEnabledId = catalog.personas.find((p) => p.enabled)?.id;
  const focusId = selectedId ?? firstEnabledId ?? catalog.personas[0]?.id;
  const focusPersona = catalog.personas.find((p) => p.id === focusId);

  const guard = async (fn: () => Promise<void>) => {
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : "action failed");
    }
  };

  return (
    <div className="mt-8 border-t border-border pt-6">
      <div className="mb-3 flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold text-fg">Persona — what the agent runs for</h2>
          <p className="text-[11px] leading-snug text-muted">
            The definition above picks <em>who runs</em> (runtime + model); a persona picks{" "}
            <em>what for</em> — its focused tool menu, pinned skills, and identity. The roster curates
            which personas the dock can suggest; defaults live in your preferences. Editing a persona
            changes what the agent is <em>shown</em>, never what it may reach.
          </p>
        </div>
        {canManage && !editor.open && (
          <Button
            size="sm"
            onClick={() => setEditor({ open: true, editing: null })}
            aria-label="new custom persona"
          >
            New persona
          </Button>
        )}
      </div>

      {error && (
        <p role="alert" className="mb-3 text-xs text-red-500">
          {error}
        </p>
      )}

      {editor.open ? (
        <PersonaEditor
          editing={editor.editing}
          personas={catalog.personas}
          onCancel={() => setEditor({ open: false })}
          onSubmit={async (persona) => {
            await guard(async () => {
              if (editor.editing) {
                await catalog.update(persona.id, {
                  label: persona.label,
                  description: persona.description,
                  identity: persona.identity,
                  granted_tools: persona.granted_tools,
                  grounding_skills: persona.grounding_skills,
                  extends: persona.extends,
                  surfaces: persona.surfaces,
                  policy_preset: persona.policy_preset,
                  runtimes: persona.runtimes,
                });
              } else {
                await catalog.create(persona);
              }
              setEditor({ open: false });
            });
          }}
        />
      ) : catalog.loading ? (
        <p className="text-sm text-muted">Loading…</p>
      ) : (
        <PersonaCatalog
          personas={catalog.personas}
          memberDefaultId={catalog.memberDefaultId}
          wsDefaultId={catalog.wsDefaultId}
          canSetRoster={canSetRoster}
          canSetMemberDefault={canSetMemberDefault}
          canSetWsDefault={canSetWsDefault}
          canManage={canManage}
          onToggleEnabled={(persona) => void guard(() => catalog.toggleEnabled(persona))}
          onSetMemberDefault={(id) => void guard(() => catalog.setMemberDefault(id))}
          onClearMemberDefault={() => void guard(() => catalog.clearMemberDefault())}
          onSetWsDefault={(id) => void guard(() => catalog.setWsDefault(id))}
          onClearWsDefault={() => void guard(() => catalog.clearWsDefault())}
          onEdit={(persona) => setEditor({ open: true, editing: persona })}
          onDelete={(persona) => void guard(() => catalog.remove(persona.id))}
        />
      )}

      {!canSetRoster && !canManage && !canSetWsDefault && (
        <p className="mt-3 text-[11px] text-muted">
          You can view the workspace personas and set your own default. Changing the roster or the
          workspace default requires an administrator.
        </p>
      )}

      {/* Effective tools — the live `persona ∩ agent ∩ caller` for the selected persona. */}
      <section className="mt-8" aria-label="effective tools section">
        <div className="mb-2 flex items-center justify-between gap-3">
          <h3 className="text-sm font-semibold text-fg">Effective tools</h3>
          {catalog.personas.length > 0 && (
            <Select
              aria-label="effective persona select"
              className="h-8 w-auto"
              value={focusId ?? ""}
              onChange={(e) => setSelectedId(e.target.value || null)}
            >
              {catalog.personas.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                  {p.enabled ? "" : " (disabled)"}
                </option>
              ))}
            </Select>
          )}
        </div>
        <EffectiveTools personaId={focusId} />
      </section>

      {/* Permissions — the Allow/Ask/Deny supervision editor, with the selected persona's preset floor. */}
      <section className="mt-8" aria-label="permissions section">
        <h3 className="mb-2 text-sm font-semibold text-fg">Permissions — how the agent is supervised</h3>
        <PolicyPane canEdit={canEditPolicy} preset={focusPersona?.policy_preset} />
      </section>
    </div>
  );
}
