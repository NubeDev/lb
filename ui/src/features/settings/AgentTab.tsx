// The Agent tab (agent-config scope) — the workspace's default agent runtime + model endpoint. Reads
// the node's offered runtimes (`agent.runtimes`) and the persisted selection (`agent.config.get`), and
// (for an admin holding `mcp:agent.config.set:call`) writes it via `agent.config.set`. A member without
// the write cap sees the current selection read-only. The runtime dropdown is the SAME list the
// node offers — a workspace can never select a runtime the node can't run (the host re-validates on
// write). The model endpoint is NAMES ONLY: `api_key_env` is an env-var name, never a secret value.

import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { CAP, hasCap } from "@/lib/session";
import { agentRuntimes, type AgentRuntimes } from "@/lib/agent/runtimes.api";
import {
  getAgentConfig,
  setAgentConfig,
  type AgentConfig,
  type ModelEndpointPatch,
} from "@/lib/agent/config.api";
import { Field, FieldGroup } from "./Field";

interface Props {
  ws: string;
  caps: string[] | undefined;
}

export function AgentTab({ caps }: Props) {
  const canEdit = hasCap(caps, CAP.agentConfigSet);

  const [runtimes, setRuntimes] = useState<AgentRuntimes | null>(null);
  const [config, setConfig] = useState<AgentConfig>({});
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    void agentRuntimes()
      .then(setRuntimes)
      .catch(() => setRuntimes(null));
    void getAgentConfig()
      .then((c) => setConfig(c ?? {}))
      .catch(() => setConfig({}));
  }, []);
  useEffect(load, [load]);

  const endpoint = config.model_endpoint ?? {};
  const setEndpoint = (key: keyof ModelEndpointPatch, value: string) => {
    setStatus("idle");
    setConfig((prev) => {
      const ep: ModelEndpointPatch = { ...(prev.model_endpoint ?? {}) };
      if (value) ep[key] = value;
      else delete ep[key];
      return { ...prev, model_endpoint: Object.keys(ep).length ? ep : undefined };
    });
  };

  const save = async () => {
    setStatus("saving");
    setError(null);
    try {
      await setAgentConfig(config);
      setStatus("saved");
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : "save failed");
    }
  };

  // Flag a stored runtime the node no longer offers (registry drift) — the record outlives a transient
  // config change; the UI says so rather than erroring.
  const selected = config.default_runtime;
  const unavailable =
    !!selected && !!runtimes && !runtimes.runtimes.includes(selected);

  return (
    <div className="mx-auto max-w-3xl px-4 py-4">
      <FieldGroup title="Runtime">
        <Field
          label="Default agent runtime"
          htmlFor="agent-runtime"
          help="Used when a run doesn't name an explicit runtime. Only runtimes this node offers are selectable."
        >
          <Select
            id="agent-runtime"
            value={selected ?? ""}
            disabled={!canEdit}
            onChange={(e) => {
              setStatus("idle");
              setConfig((p) => {
                const v = e.target.value;
                return { ...p, default_runtime: v || undefined };
              });
            }}
          >
            <option value="">
              {runtimes ? `Node default — ${runtimes.default}` : "Node default"}
            </option>
            {(runtimes?.runtimes ?? []).map((id) => (
              <option key={id} value={id}>
                {id}
                {runtimes && id === runtimes.default ? " (node default)" : ""}
              </option>
            ))}
            {/* Keep a stored-but-now-unavailable id visible so it isn't silently dropped. */}
            {unavailable && <option value={selected}>{selected} — not currently available</option>}
          </Select>
          {unavailable && (
            <p role="alert" className="mt-1 text-[11px] text-amber-500">
              This runtime is stored but not offered by the current node. It will fall back to the node
              default until the runtime is available again.
            </p>
          )}
        </Field>
      </FieldGroup>

      <FieldGroup title="Model endpoint">
        <p className="mb-2 text-[11px] leading-snug text-muted">
          Where the agent's models route. Store the API key in the node environment and name its
          variable here — the key value is never stored.
        </p>
        <Field label="Provider" htmlFor="ep-provider">
          <Input id="ep-provider" value={endpoint.provider ?? ""} disabled={!canEdit} placeholder="e.g. zaicoding" onChange={(e) => setEndpoint("provider", e.target.value)} />
        </Field>
        <Field label="Model" htmlFor="ep-model">
          <Input id="ep-model" value={endpoint.model ?? ""} disabled={!canEdit} placeholder="e.g. glm-4.6" onChange={(e) => setEndpoint("model", e.target.value)} />
        </Field>
        <Field label="API key env var" htmlFor="ep-keyenv" help="The NAME of the env var holding the key — not the key.">
          <Input id="ep-keyenv" value={endpoint.api_key_env ?? ""} disabled={!canEdit} placeholder="e.g. ZAI_API_KEY" onChange={(e) => setEndpoint("api_key_env", e.target.value)} />
        </Field>
        <Field label="Base URL" htmlFor="ep-baseurl">
          <Input id="ep-baseurl" value={endpoint.base_url ?? ""} disabled={!canEdit} placeholder="e.g. https://api.z.ai/api/coding/paas/v4" onChange={(e) => setEndpoint("base_url", e.target.value)} />
        </Field>
      </FieldGroup>

      {canEdit ? (
        <div className="sticky bottom-0 flex items-center gap-3 border-t border-border bg-bg/95 py-3 backdrop-blur">
          <Button onClick={save} disabled={status === "saving"} aria-label="save agent config">
            {status === "saving" ? "Saving…" : "Save agent config"}
          </Button>
          {status === "saved" && <span className="text-xs text-accent">Saved.</span>}
          {status === "error" && (
            <span role="alert" className="text-xs text-red-500">
              {error}
            </span>
          )}
        </div>
      ) : (
        <p className="border-t border-border py-3 text-[11px] text-muted">
          You can view the workspace agent configuration. Changing it requires an administrator.
        </p>
      )}
    </div>
  );
}
