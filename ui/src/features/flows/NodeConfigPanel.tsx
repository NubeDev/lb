// The selected-node config panel (flows-canvas scope; redesigned per flow-ui-polish). Renders the
// `SchemaForm` for the selected node from its descriptor's inline JSON-Schema (Decision 3 — no
// hardcoded UI), validates with ajv, and during an active run renders the **executed-node-lock**: an
// executed node is read-only, an unexecuted node offers a config-only `flows.patch_run` (Decision
// 1/12). Lives as the right dock's Config tab — the dock owns close/width; the canvas owns the edit
// buffer + save/patch. ONE context-aware primary action: `Save node` normally, `Patch run` while a
// run is active on an unexecuted node (whole-flow writes are the header's Deploy — `Save flow` was
// dropped here deliberately; see the scope's open question 1).

import { useState } from "react";

import { Button } from "@/components/ui/button";
import type { FlowNode, NodeDescriptor } from "@/lib/flows";
import { nodeIcon } from "./flowIcons";
import { SchemaForm, useSchemaValidity, type JsonSchema } from "./SchemaForm";
import { TriggerConfigFields } from "./TriggerConfigFields";

interface NodeConfigPanelProps {
  node: FlowNode | null;
  descriptor: NodeDescriptor | null;
  /** True when a run is active AND this node has already executed → render read-only (the lock). */
  locked: boolean;
  /** True when a run is active (the panel offers patch_run on unexecuted nodes). */
  runActive: boolean;
  /** The live (edited) config — the canvas owns it so a tab switch never drops an edit. */
  config: Record<string, unknown>;
  onConfigChange: (next: Record<string, unknown>) => void;
  /** Persist JUST this node's config (`flows.node.update`) — no whole-flow post (flow-runtime-control
   *  scope). Validated against the node's descriptor schema; bumps the flow version. */
  onSaveNode: () => Promise<{ ok: boolean; error?: string }>;
  /** Config-only patch to THIS unexecuted node of the live run (validated against the pinned schema). */
  onPatch: () => Promise<{ ok: boolean; error?: string }>;
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
  onSaveNode,
  onPatch,
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

  if (!node) {
    return (
      <div className="p-3 text-xs text-muted">
        Select a node on the canvas to configure it.
      </div>
    );
  }

  const Icon = nodeIcon({ kind: descriptor?.kind ?? "transform", icon: undefined });

  return (
    <div aria-label="node config panel" className="flex min-h-0 flex-1 flex-col">
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-3">
        <div className="flex flex-col gap-0.5">
          <strong className="text-sm text-fg">{node.id}</strong>
          <span className="flex items-center gap-1.5 text-xs text-muted">
            <Icon size={12} aria-hidden />
            {descriptor ? descriptor.title : node.type}
            <span className="font-mono text-[10px]">{node.type}</span>
          </span>
        </div>
        {locked ? (
          <div
            className="rounded-md bg-muted/30 px-2 py-1.5 text-xs text-fg"
            title="A structural change becomes a new version for the next run; a config tweak here needs a fresh run."
          >
            Executed in the active run — read-only.
          </div>
        ) : runActive ? (
          <div
            className="rounded-md bg-accent/10 px-2 py-1.5 text-xs text-fg"
            title="Patch run applies a config-only tweak to this unexecuted node of the live run; Save node writes a new version for the next run."
          >
            Run active — Patch applies to the live run.
          </div>
        ) : null}
        {descriptor?.type === "trigger" ? (
          <TriggerConfigFields config={config} onChange={onConfigChange} disabled={locked || busy} />
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
      </div>
      {/* Sticky footer — one context-aware primary action (flow-ui-polish). */}
      {!locked ? (
        <div className="flex items-center gap-2 border-t border-border bg-card/60 px-3 py-2">
          {canPatch ? (
            <>
              <Button aria-label="patch run" onClick={() => run(onPatch)} size="sm" disabled={busy}>
                Patch run
              </Button>
              <Button
                aria-label="save node"
                onClick={() => run(onSaveNode)}
                variant="outline"
                size="sm"
                disabled={!canSave || busy}
              >
                Save node
              </Button>
            </>
          ) : (
            <Button
              aria-label="save node"
              onClick={() => run(onSaveNode)}
              size="sm"
              disabled={!canSave || busy}
            >
              Save node
            </Button>
          )}
          {!validity.ok ? (
            <span className="text-xs text-destructive">Fix the invalid field(s) first.</span>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
