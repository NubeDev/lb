// The custom-definition editor (agent-catalog scope) — the repurposed raw endpoint form. Creates
// (`agent.def.create`) or edits (`agent.def.update`) a workspace custom definition: label + a runtime
// dropdown (from `agent.runtimes`) + provider / model / api-key-env NAME / base URL. Built-ins are
// never edited here (no edit affordance is offered for them; the host rejects a `builtin.*` write too).
// NAMES ONLY — `api_key_env` is an env-var name, never a secret value.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { AgentRuntimes } from "@/lib/agent/runtimes.api";
import { setModelKey, type AgentDefinition } from "@/lib/agent/agentDef.api";
import { Field, FieldGroup } from "../Field";

interface Props {
  runtimes: AgentRuntimes | null;
  /** The definition being edited (custom only), or `null` to create a new one. */
  editing: AgentDefinition | null;
  onSubmit: (def: AgentDefinition) => Promise<void>;
  onCancel: () => void;
}

/** A slug from a label: lowercase, non-alnum → single dash. Only used to seed a NEW definition's id
 *  (an edit keeps its id). Never prefixed `builtin.` — that prefix is reserved for seeded built-ins. */
function slugify(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function AgentDefinitionEditor({ runtimes, editing, onSubmit, onCancel }: Props) {
  const [id, setId] = useState(editing?.id ?? "");
  const [label, setLabel] = useState(editing?.label ?? "");
  const [description, setDescription] = useState(editing?.description ?? "");
  const [runtime, setRuntime] = useState(editing?.runtime ?? runtimes?.default ?? "default");
  const [provider, setProvider] = useState(editing?.model_endpoint.provider ?? "");
  const [model, setModel] = useState(editing?.model_endpoint.model ?? "");
  const [apiKeyEnv, setApiKeyEnv] = useState(editing?.model_endpoint.api_key_env ?? "");
  const [baseUrl, setBaseUrl] = useState(editing?.model_endpoint.base_url ?? "");
  // The sealed model KEY is write-only: the field holds a fresh value to seal (never the stored key,
  // which is never read back). `keySecret` is the PATH already on the definition (a name) — if set,
  // we show "key is set ✓ · rotate" instead of implying we know the value.
  const [modelKey, setModelKey_] = useState("");
  const keySecret = editing?.model_endpoint.api_key_secret ?? "";
  const [status, setStatus] = useState<"idle" | "saving" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  const isEdit = !!editing;
  const effectiveId = isEdit ? editing!.id : id || slugify(label);
  const canSubmit = !!label && !!provider && !!model && !!runtime && !!effectiveId;

  const submit = async () => {
    setStatus("saving");
    setError(null);
    try {
      // If the admin entered a fresh model key, seal it FIRST through `secret.set` (the value flows
      // only there), then store just the resulting PATH on the definition — names-only. An unchanged
      // field keeps the existing path (or none). The value is never placed on the record.
      let apiKeySecret = keySecret || undefined;
      if (modelKey) {
        const path = `agent/${effectiveId}-key`;
        apiKeySecret = await setModelKey(path, modelKey);
      }
      await onSubmit({
        id: effectiveId,
        label,
        description: description || undefined,
        runtime,
        model_endpoint: {
          provider,
          model,
          api_key_env: apiKeyEnv || undefined,
          api_key_secret: apiKeySecret,
          base_url: baseUrl || undefined,
        },
        builtin: false,
      });
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : "save failed");
    }
  };

  return (
    <div className="rounded-md border border-border p-4" aria-label="custom definition editor">
      <FieldGroup title={isEdit ? `Edit "${editing!.label}"` : "New custom definition"}>
        <Field label="Label" htmlFor="def-label">
          <Input
            id="def-label"
            value={label}
            placeholder="e.g. In-house — GLM-5.2 (staging key)"
            onChange={(e) => setLabel(e.target.value)}
          />
        </Field>
        {!isEdit && (
          <Field
            label="Id"
            htmlFor="def-id"
            help="A stable slug. Defaults from the label; cannot start with 'builtin.'."
          >
            <Input
              id="def-id"
              value={id || slugify(label)}
              onChange={(e) => setId(e.target.value)}
            />
          </Field>
        )}
        <Field label="Description" htmlFor="def-desc">
          <Input
            id="def-desc"
            value={description}
            placeholder="Optional — shown in the picker."
            onChange={(e) => setDescription(e.target.value)}
          />
        </Field>
        <Field
          label="Runtime"
          htmlFor="def-runtime"
          help="Only runtimes this node offers are selectable (validated on save)."
        >
          <Select id="def-runtime" value={runtime} onChange={(e) => setRuntime(e.target.value)}>
            {(runtimes?.runtimes ?? ["default"]).map((rid) => (
              <option key={rid} value={rid}>
                {rid}
                {runtimes && rid === runtimes.default ? " (node default)" : ""}
              </option>
            ))}
          </Select>
        </Field>
      </FieldGroup>

      <FieldGroup title="Model endpoint">
        <p className="mb-2 text-[11px] leading-snug text-muted">
          Names only on the record. Set the key here to seal it in the workspace store, or name a node
          env var. The key value is written through the sealed secret store — never onto the definition.
        </p>
        <Field label="Provider" htmlFor="def-provider">
          <Input
            id="def-provider"
            value={provider}
            placeholder="e.g. zaicoding"
            onChange={(e) => setProvider(e.target.value)}
          />
        </Field>
        <Field label="Model" htmlFor="def-model">
          <Input
            id="def-model"
            value={model}
            placeholder="e.g. glm-4.6"
            onChange={(e) => setModel(e.target.value)}
          />
        </Field>
        <Field
          label="Model key"
          htmlFor="def-modelkey"
          help={
            keySecret
              ? "A sealed key is set. Enter a new value to rotate it — the current value is never shown."
              : "Sealed in the workspace store on save. Written once; never read back. Optional — env var below still works."
          }
        >
          <Input
            id="def-modelkey"
            type="password"
            value={modelKey}
            placeholder={keySecret ? "key is set ✓ · enter a new value to rotate" : "Paste the API key to seal it"}
            onChange={(e) => setModelKey_(e.target.value)}
          />
        </Field>
        <Field
          label="API key env var"
          htmlFor="def-keyenv"
          help="Fallback: the NAME of a node env var holding the key — used only if no sealed key is set."
        >
          <Input
            id="def-keyenv"
            value={apiKeyEnv}
            placeholder="e.g. ZAI_API_KEY"
            onChange={(e) => setApiKeyEnv(e.target.value)}
          />
        </Field>
        <Field label="Base URL" htmlFor="def-baseurl">
          <Input
            id="def-baseurl"
            value={baseUrl}
            placeholder="e.g. https://api.z.ai/api/coding/paas/v4"
            onChange={(e) => setBaseUrl(e.target.value)}
          />
        </Field>
      </FieldGroup>

      <div className="flex items-center gap-3 pt-2">
        <Button onClick={submit} disabled={!canSubmit || status === "saving"} aria-label="save definition">
          {status === "saving" ? "Saving…" : isEdit ? "Save changes" : "Create definition"}
        </Button>
        <Button variant="ghost" onClick={onCancel} aria-label="cancel definition edit">
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
