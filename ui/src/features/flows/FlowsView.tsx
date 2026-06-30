// The Flows page (flows-canvas scope, Wave 3) — wraps the rail (list/open/delete/new) + the typed-node
// canvas (edit/save/run/lifecycle/patch/import/export/undo). The page is thin glue: it owns the open
// flow via `useFlows` and hands the canvas a save action that surfaces the host's validation message
// inline. A blank flow seeds a single trigger node so the author has somewhere to start.

import { useCallback, useState } from "react";

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
    <div aria-label="flows view" className="flex h-full">
      <FlowRail
        roster={roster}
        openId={open?.id ?? null}
        onOpen={load}
        onDelete={remove}
        onNew={onNew}
      />
      {error ? (
        <div aria-label="flows error" className="p-2 text-sm text-denied">
          {error}
        </div>
      ) : null}
      {open ? (
        <FlowCanvas
          key={`${open.id}-${draftId}`}
          flow={open}
          palette={palette}
          onSave={save}
          onDeleted={() => setOpen(null)}
        />
      ) : (
        <div className="p-4 text-sm text-muted">Select or create a flow.</div>
      )}
    </div>
  );
}
