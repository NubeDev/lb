// The Flows page (flows-canvas scope, Wave 3) — wraps the rail (list/open/delete/new) + the typed-node
// canvas (edit/save/run/lifecycle/patch/import/export/undo). The page is thin glue: it owns the open
// flow via `useFlows` and hands the canvas a save action that surfaces the host's validation message
// inline. A blank flow seeds a single trigger node so the author has somewhere to start.
//
// The page follows the canonical surface shape (scope/frontend/ui-standards-scope.md): an
// `<AppPageHeader>`-led `<section>` with the roster aside + the canvas body, the same shape as the
// dashboard page so the two read as one app.

import { useCallback, useState } from "react";
import { Workflow } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { AppEmptyState } from "@/components/app/empty-state";
import { Badge } from "@/components/ui/badge";
import type { Flow } from "@/lib/flows";
import { FlowCanvas } from "./FlowCanvas";
import { FlowRail } from "./FlowRail";
import { useFlows } from "./useFlows";

export interface FlowsViewProps {
  ws: string;
}

/** A fresh blank flow seeded with one trigger node (the entry point every flow needs). */
function blankFlow(): Flow {
  const id = `flow-${Date.now()}`;
  return {
    id,
    name: id,
    version: 1,
    nodes: [{ id: "start", type: "trigger", needs: [], config: {} }],
    failurePolicy: "halt",
  };
}

export function FlowsView({ ws }: FlowsViewProps) {
  const { roster, open, palette, error, load, save, remove, setOpen } = useFlows(ws);
  const [draftId, setDraftId] = useState(0); // bump to force a fresh canvas on "new"

  const onNew = useCallback(() => {
    setOpen(blankFlow());
    setDraftId((n) => n + 1);
  }, [setOpen]);

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
      <FlowRail
        roster={roster}
        openId={open?.id ?? null}
        onOpen={load}
        onDelete={remove}
        onNew={onNew}
      />
      {open ? (
        <FlowCanvas
          key={`${open.id}-${draftId}`}
          flow={open}
          palette={palette}
          onSave={save}
          onDeleted={() => setOpen(null)}
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
