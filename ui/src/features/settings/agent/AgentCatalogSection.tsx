// The Agent-definition catalog section (agent-catalog scope) — the "who runs" half of the Agent tab:
// the catalog manager + active-selection picker (list built-ins ∪ custom definitions, highlight the
// active pick, and — for an admin — pick / create / edit / delete a definition, or seal a model key on
// the active pick). Extracted from `AgentTab` so BOTH the Settings tab AND the Setup "Set up the agent"
// wizard render the SAME editor (setup rule 3: extract, never fork). One responsibility per file
// (FILE-LAYOUT): this file owns the definition-catalog block; the persona half lives in `PersonaSection`.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { CAP, hasCap } from "@/lib/session";
import type { AgentDefinition } from "@/lib/agent/agentDef.api";
import { AgentCatalog } from "./AgentCatalog";
import { AgentDefinitionEditor } from "./AgentDefinitionEditor";
import { useAgentCatalog } from "./useAgentCatalog";

interface Props {
  caps: string[] | undefined;
}

type EditorState = { open: false } | { open: true; editing: AgentDefinition | null };

export function AgentCatalogSection({ caps }: Props) {
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
    <div>
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
          activeHasKey={!!catalog.config.model_endpoint?.api_key_secret}
          onPick={(def) => void guard(() => catalog.pick(def))}
          onSetActiveKey={(value) => guard(() => catalog.setActiveKey(value))}
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
