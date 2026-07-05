// The custom-persona editor (agent-personas scope #1) — creates (`agent.persona.create`) or edits
// (`agent.persona.update`) a workspace custom persona: label + identity text + the granted-tools list +
// the grounding-skills list + an `extends` multiselect from the existing personas. Built-ins are never
// edited here (no edit affordance is offered; the host rejects a `builtin.*` write too). Tool/skill/
// persona ids are OPAQUE strings (rule 10) — the editor lists them as data, never branching on one.
//
// This edits ADVERTISEMENT (what the agent sees), never the wall — a granted_tools entry the caller
// lacks stays denied at dispatch. See EffectiveTools for the live `persona ∩ agent ∩ caller` result.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Checkbox } from "@/components/ui/checkbox";
import type { Persona } from "@/lib/agent/agentPersona.api";
import { Field, FieldGroup } from "../Field";
import { StringListField } from "./StringListField";

interface Props {
  /** The persona being edited (custom only), or `null` to create a new one. */
  editing: Persona | null;
  /** The existing personas — the `extends` multiselect source (a persona cannot extend itself). */
  personas: Persona[];
  onSubmit: (persona: Persona) => Promise<void>;
  onCancel: () => void;
}

/** A slug from a label: lowercase, non-alnum → single dash. Only seeds a NEW persona's id (an edit
 *  keeps its id). Never prefixed `builtin.` — that prefix is reserved for seeded built-ins. */
function slugify(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function PersonaEditor({ editing, personas, onSubmit, onCancel }: Props) {
  const [id, setId] = useState(editing?.id ?? "");
  const [label, setLabel] = useState(editing?.label ?? "");
  const [description, setDescription] = useState(editing?.description ?? "");
  const [identity, setIdentity] = useState(editing?.identity ?? "");
  const [grantedTools, setGrantedTools] = useState<string[]>(editing?.granted_tools ?? []);
  const [groundingSkills, setGroundingSkills] = useState<string[]>(
    editing?.grounding_skills ?? [],
  );
  const [extendsIds, setExtendsIds] = useState<string[]>(editing?.extends ?? []);
  const [status, setStatus] = useState<"idle" | "saving" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  const isEdit = !!editing;
  const effectiveId = isEdit ? editing!.id : id || slugify(label);
  const canSubmit = !!label && !!identity && !!effectiveId;

  // The `extends` sources: every OTHER persona (a persona cannot extend itself; the host also
  // cycle-checks at write time, but we drop the self option up front for a clean picker).
  const extendOptions = personas.filter((p) => p.id !== effectiveId);

  const toggleExtend = (pid: string) =>
    setExtendsIds((prev) => (prev.includes(pid) ? prev.filter((x) => x !== pid) : [...prev, pid]));

  const submit = async () => {
    setStatus("saving");
    setError(null);
    try {
      await onSubmit({
        id: effectiveId,
        label,
        description: description || undefined,
        identity,
        granted_tools: grantedTools,
        grounding_skills: groundingSkills,
        extends: extendsIds,
        // policy_preset / runtimes are set by the built-in seed or a future pane — the editor carries
        // through the editing record's values so an edit never drops them.
        policy_preset: editing?.policy_preset,
        runtimes: editing?.runtimes,
        builtin: false,
      });
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : "save failed");
    }
  };

  return (
    <div className="rounded-md border border-border p-4" aria-label="custom persona editor">
      <FieldGroup title={isEdit ? `Edit "${editing!.label}"` : "New custom persona"}>
        <Field label="Label" htmlFor="persona-label">
          <Input
            id="persona-label"
            value={label}
            placeholder="e.g. Data analyst"
            onChange={(e) => setLabel(e.target.value)}
          />
        </Field>
        {!isEdit && (
          <Field
            label="Id"
            htmlFor="persona-id"
            help="A stable slug. Defaults from the label; cannot start with 'builtin.'."
          >
            <Input
              id="persona-id"
              value={id || slugify(label)}
              onChange={(e) => setId(e.target.value)}
            />
          </Field>
        )}
        <Field label="Description" htmlFor="persona-desc">
          <Input
            id="persona-desc"
            value={description}
            placeholder="Optional — shown in the picker."
            onChange={(e) => setDescription(e.target.value)}
          />
        </Field>
        <Field
          label="Identity"
          htmlFor="persona-identity"
          help="A short persona prompt — prepended to the system prompt / folded into the goal."
        >
          <Textarea
            id="persona-identity"
            value={identity}
            rows={4}
            placeholder="e.g. You are a focused data analyst. Prefer querying series over guessing."
            onChange={(e) => setIdentity(e.target.value)}
          />
        </Field>
      </FieldGroup>

      <FieldGroup title="Focus">
        <p className="mb-2 text-[11px] leading-snug text-muted">
          These narrow the <em>advertised</em> menu and pin grounding — they never grant a capability.
          A tool the caller lacks stays denied at dispatch (see Effective tools). Use a trailing{" "}
          <code>*</code> glob to include a whole area (e.g. <code>flows.*</code>).
        </p>
        <StringListField
          label="Granted tools"
          ariaLabel="granted tools"
          placeholder="e.g. series.query or flows.*"
          values={grantedTools}
          onChange={setGrantedTools}
        />
        <StringListField
          label="Grounding skills"
          ariaLabel="grounding skills"
          placeholder="e.g. analyst.playbook"
          values={groundingSkills}
          onChange={setGroundingSkills}
        />
        <Field
          label="Extends"
          help="Personas whose tool/skill lists union in (this persona's identity wins)."
        >
          {extendOptions.length === 0 ? (
            <p className="text-[11px] text-muted">No other personas to extend.</p>
          ) : (
            <div className="flex flex-col gap-1" aria-label="extends multiselect">
              {extendOptions.map((p) => (
                <label key={p.id} className="flex items-center gap-2 text-xs text-fg">
                  <Checkbox
                    aria-label={`extend ${p.id}`}
                    checked={extendsIds.includes(p.id)}
                    onChange={() => toggleExtend(p.id)}
                  />
                  <span className="truncate">
                    {p.label} <span className="text-muted">({p.id})</span>
                  </span>
                </label>
              ))}
            </div>
          )}
        </Field>
      </FieldGroup>

      <div className="flex items-center gap-3 pt-2">
        <Button
          onClick={submit}
          disabled={!canSubmit || status === "saving"}
          aria-label="save persona"
        >
          {status === "saving" ? "Saving…" : isEdit ? "Save changes" : "Create persona"}
        </Button>
        <Button variant="ghost" onClick={onCancel} aria-label="cancel persona edit">
          Cancel
        </Button>
        {status === "error" && (
          <span role="alert" className="text-xs text-red-500">
            {error}
          </span>
        )}
      </div>
    </div>
  );
}
