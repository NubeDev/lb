// The Agent tab (agent-catalog scope) — a catalog manager + active-selection picker. Renders the
// pickable presets from `agent.def.list` (seeded read-only built-ins ∪ the workspace's custom
// definitions), highlights the active selection (resolved from the shipped `agent.config`), and — for
// an admin — lets you pick one (writes `agent.config`), or create/edit/delete a custom definition.
// A member without the write caps sees the catalog + active pick read-only.
//
// Picking a definition sets the workspace default RUNTIME today; the invoke path's
// `resolve_effective_runtime` honors it. Routing the in-house loop to a per-workspace model ENDPOINT
// is gated on the ai-gateway provider adapter (default-agent-wiring) — the copy says so honestly and
// does not over-promise that the key/model is live per workspace beyond what is wired.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { CAP, hasCap } from "@/lib/session";
import type { AgentDefinition } from "@/lib/agent/agentDef.api";
import { AgentCatalog } from "./agent/AgentCatalog";
import { AgentDefinitionEditor } from "./agent/AgentDefinitionEditor";
import { useAgentCatalog } from "./agent/useAgentCatalog";

interface Props {
  ws: string;
  caps: string[] | undefined;
}

type EditorState = { open: false } | { open: true; editing: AgentDefinition | null };

export function AgentTab({ caps }: Props) {
  // Picking writes `agent.config` — the admin-only set cap. Managing custom definitions needs the
  // create/update/delete caps. `list`/`get` are member-level (the read the catalog renders on).
  const canPick = hasCap(caps, CAP.agentConfigSet);
  const canManage =
    hasCap(caps, CAP.agentDefCreate) ||
    hasCap(caps, CAP.agentDefUpdate) ||
    hasCap(caps, CAP.agentDefDelete);
  // The context-proving Test spends a model turn — its own admin-tier cap.
  const canTest = hasCap(caps, CAP.agentDefTest);

  const catalog = useAgentCatalog();
  const [editor, setEditor] = useState<EditorState>({ open: false });
  const [error, setError] = useState<string | null>(null);

  const guard = async (fn: () => Promise<void>) => {
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : "action failed");
    }
  };

  return (
    <div className="mx-auto max-w-3xl px-4 py-4">
      <div className="mb-3 flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold text-fg">Agent catalog</h2>
          <p className="text-[11px] leading-snug text-muted">
            Named runtime + model presets. Pick one as the workspace default, or add your own. The
            selected runtime applies to agent runs now; a per-workspace model endpoint applies once a
            provider adapter is configured.
          </p>
        </div>
        {canManage && !editor.open && (
          <Button
            size="sm"
            onClick={() => setEditor({ open: true, editing: null })}
            aria-label="new custom definition"
          >
            New definition
          </Button>
        )}
      </div>

      {error && (
        <p role="alert" className="mb-3 text-xs text-red-500">
          {error}
        </p>
      )}

      {editor.open ? (
        <AgentDefinitionEditor
          runtimes={catalog.runtimes}
          editing={editor.editing}
          onCancel={() => setEditor({ open: false })}
          onSubmit={async (def) => {
            await guard(async () => {
              if (editor.editing) {
                await catalog.update(def.id, {
                  label: def.label,
                  description: def.description,
                  runtime: def.runtime,
                  model_endpoint: def.model_endpoint,
                });
              } else {
                await catalog.create(def);
              }
              setEditor({ open: false });
            });
          }}
        />
      ) : catalog.loading ? (
        <p className="text-sm text-muted">Loading…</p>
      ) : (
        <AgentCatalog
          definitions={catalog.definitions}
          runtimes={catalog.runtimes}
          activeId={catalog.activeId}
          canPick={canPick}
          canManage={canManage}
          canTest={canTest}
          onPick={(def) => void guard(() => catalog.pick(def))}
          onEdit={(def) => setEditor({ open: true, editing: def })}
          onDelete={(def) => void guard(() => catalog.remove(def.id))}
        />
      )}

      {!canPick && !canManage && (
        <p className="mt-3 border-t border-border pt-3 text-[11px] text-muted">
          You can view the workspace agent catalog. Changing it requires an administrator.
        </p>
      )}
    </div>
  );
}
