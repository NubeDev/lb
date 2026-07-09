// The flow rail (flows-canvas scope, Wave 3) — the left list of saved flows: open one, delete one, or
// start a new named flow. Chrome/behavior live in the shared `RosterRail` (components/app/roster.tsx —
// the maintained inner-sidebar look); this file only maps flows onto it: the timestamp id scheme
// (derived here, not from the title), the version badge (`v{n}`), and a delete confirm (the rail hands
// the item back, the feature owns the destructive gate — flows previously deleted with no confirm,
// which was a gap). Rename is the shared roster's inline title editor (hover pencil), wired to a
// name-only `flows.save` in the feature (useFlows.rename). One component per file (FILE-LAYOUT).

import { useState } from "react";
import { Workflow } from "lucide-react";

import { RosterRail, type RosterItem } from "@/components/app/roster";
import { ConfirmDestructive } from "@/features/confirm/ConfirmDestructive";
import type { FlowSummary } from "@/lib/flows";

export interface FlowRailProps {
  roster: FlowSummary[];
  openId: string | null;
  onOpen: (id: string) => Promise<void> | void;
  onDelete: (id: string) => Promise<void> | void;
  /** Inline rename (the shared roster's hover pencil) — a name-only `flows.save`. */
  onRename: (id: string, name: string) => void;
  /** Name-first create: the rail's inline field supplies the name; the adapter derives the id (the
   *  flow's timestamp scheme) and hands both to the host, which seeds a blank flow with that name. */
  onCreate: (id: string, name: string) => void;
  /** Minimize the rail — the host (FlowsView) renders the symmetric `CollapsedRail` when closed. */
  onCollapse?: () => void;
}

/** The flow id scheme: a timestamp-derived handle (unchanged from the prior blank-flow seed). */
function flowId(): string {
  return `flow-${Date.now()}`;
}

export function FlowRail({ roster, openId, onOpen, onDelete, onRename, onCreate, onCollapse }: FlowRailProps) {
  // The flow pending a delete confirm — the rail hands the item back; the feature owns the gate.
  const [pendingDelete, setPendingDelete] = useState<RosterItem | null>(null);

  return (
    <>
      <RosterRail
        noun="flow"
        icon={Workflow}
        items={roster.map((f) => ({ id: f.id, title: f.name || f.id, badge: `v${f.version}` }))}
        selectedId={openId}
        onSelect={(id) => void onOpen(id)}
        emptyText="No flows yet."
        createPlaceholder="New flow…"
        onCreate={(name) => onCreate(flowId(), name)}
        onRename={onRename}
        onCollapse={onCollapse}
        onRemove={setPendingDelete}
      />

      {pendingDelete && (
        <ConfirmDestructive
          title={`Delete ${pendingDelete.title}`}
          consequence="This flow and its saved graph will be removed. It can be recreated but its current graph is not recoverable."
          reversible={false}
          escalation="none"
          confirmLabel="Delete"
          onConfirm={() => {
            void onDelete(pendingDelete.id);
            setPendingDelete(null);
          }}
          onCancel={() => setPendingDelete(null)}
        />
      )}
    </>
  );
}
