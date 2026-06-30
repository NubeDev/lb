// The selected-node config panel (flows-canvas scope, Wave 3). Renders the `SchemaForm` for the
// selected node from its descriptor's inline JSON-Schema (Decision 3 — no hardcoded UI), validates
// with ajv before Save, and during an active run renders the **executed-node-lock**: an executed node
// is read-only, an unexecuted node offers a config-only `flows.patch_run` (Decision 1/12). The panel
// is presentation + the edit buffer; the canvas owns save/patch.

import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import type { FlowNode, NodeDescriptor } from "@/lib/flows";
import { SchemaForm, useSchemaValidity, type JsonSchema } from "./SchemaForm";
import { TriggerConfigFields } from "./TriggerConfigFields";

interface NodeConfigPanelProps {
  node: FlowNode | null;
  descriptor: NodeDescriptor | null;
  /** True when a run is active AND this node has already executed → render read-only (the lock). */
  locked: boolean;
  /** True when a run is active (the panel offers patch_run on unexecuted nodes). */
  runActive: boolean;
  /** The live (edited) config — the canvas owns it so Save persists the whole flow. */
  config: Record<string, unknown>;
  onConfigChange: (next: Record<string, unknown>) => void;
  /** Persist the whole flow (a new version — Decision 1). Returns the host outcome for inline error. */
  onSave: () => Promise<{ ok: boolean; error?: string }>;
  /** Persist JUST this node's config (`flows.node.update`) — no whole-flow post (flow-runtime-control
   *  scope). Validated against the node's descriptor schema; bumps the flow version. */
  onSaveNode: () => Promise<{ ok: boolean; error?: string }>;
  /** Config-only patch to THIS unexecuted node of the live run (validated against the pinned schema). */
  onPatch: () => Promise<{ ok: boolean; error?: string }>;
  /** Clear the selection (close the panel). */
  onClose: () => void;
  /** The last inline error from a save/patch (rendered verbatim — the host's validation message). */
  error: string | null;
}

export function NodeConfigPanel({
  node,
  descriptor,
  locked,
  runActive,
  config,
  onConfigChange,
  onSave,
  onSaveNode,
  onPatch,
  onClose,
  error,
}: NodeConfigPanelProps) {
  const schema = (descriptor?.config ?? {}) as JsonSchema;
  const validity = useSchemaValidity(schema, config);
  const [busy, setBusy] = useState(false);

  const canSave = !locked && validity.ok && !!node;
  const canPatch = runActive && !locked && validity.ok && !!node;

  async function run(fn: () => Promise<{ ok: boolean; error?: string }>) {
    setBusy(true);
    try {
      await fn();
    } finally {
      setBusy(false);
    }
  }

  const title = useMemo(() => {
    if (!node) return "";
    return descriptor ? `${node.id} (${descriptor.title})` : `${node.id} (${node.type})`;
  }, [node, descriptor]);

  if (!node) {
    return (
      <div className="flex w-72 flex-col gap-2 border-l border-border p-3 text-xs text-muted">
        <div>Select a node to configure it.</div>
      </div>
    );
  }

  return (
    <div
      aria-label="node config panel"
      className="flex w-80 flex-col gap-3 overflow-y-auto border-l border-border p-3"
    >
      <div className="flex items-center justify-between">
        <strong className="text-sm text-fg">{title}</strong>
        <Button aria-label="close config" onClick={onClose} variant="ghost" size="sm">
          ✕
        </Button>
      </div>
      {locked ? (
        <div className="rounded-md bg-muted/30 p-2 text-xs text-fg">
          This node has executed in the active run — it is read-only. A structural change is a new
          version for the next run; a config tweak here is unavailable (use a fresh run).
        </div>
      ) : null}
      {runActive && !locked ? (
        <div className="rounded-md bg-accent/10 p-2 text-xs text-fg">
          A run is active. Save writes a new version (the next run); <strong>Patch</strong> applies a
          config-only tweak to this unexecuted node of the live run.
        </div>
      ) : null}
      {descriptor?.type === "trigger" ? (
        <TriggerConfigFields
          config={config}
          onChange={onConfigChange}
          disabled={locked || busy}
        />
      ) : (
        <SchemaForm
          schema={schema}
          value={config}
          onChange={onConfigChange}
          disabled={locked || busy}
          errors={validity.errors}
        />
      )}
      {error ? (
        <span aria-label="config error" className="text-xs text-destructive">
          {error}
        </span>
      ) : null}
      <div className="flex gap-2">
        {/* The common case: persist JUST this node's config (`flows.node.update`) — no whole-flow
            post. Available when the node isn't run-locked; the live-run patch is offered separately. */}
        <Button
          aria-label="save node"
          onClick={() => run(onSaveNode)}
          size="sm"
          disabled={!canSave || busy}
        >
          Save node
        </Button>
        <Button
          aria-label="save flow"
          onClick={() => run(onSave)}
          variant="outline"
          size="sm"
          disabled={!canSave || busy}
        >
          Save flow
        </Button>
        {canPatch ? (
          <Button
            aria-label="patch run"
            onClick={() => run(onPatch)}
            size="sm"
            disabled={busy}
          >
            Patch run
          </Button>
        ) : null}
      </div>
      {!validity.ok ? (
        <span className="text-xs text-destructive">Fix the invalid field(s) before saving.</span>
      ) : null}
    </div>
  );
}
