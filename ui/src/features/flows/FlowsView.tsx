// The Flows page (flows-canvas scope, Wave 3) — wraps the rail (list/open/delete/new) + the typed-node
// canvas (edit/save/run/lifecycle/patch/import/export/undo). The page is thin glue: it owns the open
// flow via `useFlows` and hands the canvas a save action that surfaces the host's validation message
// inline. A blank flow seeds a single trigger node so the author has somewhere to start.
//
// The page follows the canonical surface shape (scope/frontend/ui-standards-scope.md): an
// `<AppPageHeader>`-led `<section>` with the roster aside + the canvas body, the same shape as the
// dashboard page so the two read as one app.

import { useCallback, useEffect, useState } from "react";
import { Workflow } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { AppEmptyState } from "@/components/app/empty-state";
import { CollapsedRail } from "@/components/app/rail-collapsed";
import { Badge } from "@/components/ui/badge";
import type { Flow } from "@/lib/flows";
import { FlowCanvas } from "./FlowCanvas";
import { FlowRail } from "./FlowRail";
import { useFlows } from "./useFlows";

export interface FlowsViewProps {
  ws: string;
  /** The flow id from the URL (`/flows/$id`), or null on the bare `/flows` surface. */
  flowId?: string | null;
  /** Sync the URL when the open flow changes (open/new/delete). */
  onSelectFlow?: (id: string | null) => void;
}

/** A fresh blank flow seeded with one trigger node (the entry point every flow needs). The id comes
 *  from the rail adapter (the timestamp scheme, `flowId()`); the name is the title the author typed
 *  into the inline create field (name-first create, same shape as Dashboards/Rules). */
function blankFlow(id: string, name: string): Flow {
  return {
    id,
    name,
    version: 1,
    nodes: [{ id: "start", type: "trigger", needs: [], config: {} }],
    failurePolicy: "halt",
  };
}

export function FlowsView({ ws, flowId, onSelectFlow }: FlowsViewProps) {
  const { roster, open, palette, error, load, save, rename, remove, setOpen } = useFlows(ws);
  const [draftId, setDraftId] = useState(0); // bump to force a fresh canvas on "new"
  // The flow rail folds to the shared thin strip (same affordance as the dashboard/rules rosters).
  const [railOpen, setRailOpen] = useState(true);

  // Deep-link: open the flow named in the URL. Runs when the id changes (paste/back/forward) but not
  // for a blank draft (no id in the URL) or when that flow is already open — the load re-fetches.
  useEffect(() => {
    if (flowId && open?.id !== flowId) void load(flowId);
    if (!flowId && open) setOpen(null);
    // `open` is intentionally omitted: reacting to it would re-close on every open. Only the URL drives.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [flowId, load, setOpen]);

  const onOpen = useCallback(
    (id: string) => {
      onSelectFlow?.(id);
      void load(id);
    },
    [onSelectFlow, load],
  );

  const onCreate = useCallback(
    (id: string, name: string) => {
      onSelectFlow?.(null);
      setOpen(blankFlow(id, name));
      setDraftId((n) => n + 1);
    },
    [onSelectFlow, setOpen],
  );

  const onRemove = useCallback(
    async (id: string) => {
      await remove(id);
      if (open?.id === id) onSelectFlow?.(null);
    },
    [remove, open, onSelectFlow],
  );

  return (
    <AppPage
      label="flows view"
      icon={Workflow}
      title={open?.name || open?.id || "Flows"}
      description="Author, run, and watch typed node graphs."
      workspace={ws}
      error={error}
      actions={
        open ? (
          <Badge variant="outline" className="rounded-full">
            v{open.version}
          </Badge>
        ) : null
      }
    >
      {railOpen ? (
        <FlowRail
          roster={roster}
          openId={open?.id ?? null}
          onOpen={onOpen}
          onDelete={onRemove}
          onRename={(id, name) => void rename(id, name)}
          onCreate={onCreate}
          onCollapse={() => setRailOpen(false)}
        />
      ) : (
        <CollapsedRail noun="flow" onExpand={() => setRailOpen(true)} />
      )}
      {open ? (
        <FlowCanvas
          key={`${open.id}-${draftId}`}
          flow={open}
          palette={palette}
          onSave={async (flow) => {
            const res = await save(flow);
            if (res.ok) onSelectFlow?.(flow.id); // a saved draft now has a real, linkable id
            return res;
          }}
          onDeleted={() => {
            setOpen(null);
            onSelectFlow?.(null);
          }}
        />
      ) : (
        <AppEmptyState
          icon={Workflow}
          title="Select or create a flow."
          description="Flows are typed node graphs scoped to the current workspace. Drag nodes from the palette, wire them, and run."
        />
      )}
    </AppPage>
  );
}
