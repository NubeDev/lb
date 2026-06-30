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

import { AppPageHeader } from "@/components/app/page-header";
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
    <section aria-label="flows view" className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Workflow}
        title={open?.name || open?.id || "Flows"}
        description="Author, run, and watch typed node graphs."
        workspace={ws}
        actions={
          open ? (
            <Badge variant="outline" className="rounded-full">
              v{open.version}
            </Badge>
          ) : null
        }
      />

      {error ? (
        <div
          role="alert"
          aria-label="flows error"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {error}
        </div>
      ) : null}

      <div className="flex min-h-0 flex-1">
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
          <div className="flex flex-1 items-center justify-center p-6">
            <div className="flex max-w-sm flex-col items-center rounded-lg border border-dashed border-border bg-card/70 px-6 py-7 text-center shadow-sm shadow-black/5">
              <div className="mb-3 flex h-10 w-10 items-center justify-center rounded-md border border-border bg-bg text-accent">
                <Workflow size={20} />
              </div>
              <p className="text-sm font-medium text-fg">Select or create a flow.</p>
              <p className="mt-1 text-xs leading-5 text-muted">
                Flows are typed node graphs scoped to the current workspace. Drag nodes from the
                palette, wire them, and run.
              </p>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
