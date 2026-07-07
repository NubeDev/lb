// RuleRail — the left rail of saved rules (rules-workbench scope): list via `rules.list`, open via
// `rules.get` (the parent hook), delete via `rules.delete`. Chrome/behavior live in the shared
// `RosterRail` (components/app/roster.tsx — the maintained inner-sidebar look); this file only maps
// rules onto it: name-first create (the rail's inline field; the parent derives the id and opens the
// new rule) and a delete confirm (the rail hands the item back, the feature owns the destructive
// gate — rules previously deleted with no confirm, which was a gap). One component per file
// (FILE-LAYOUT).

import { useState } from "react";
import { FileCode2 } from "lucide-react";

import { RosterRail, type RosterItem } from "@/components/app/roster";
import { ConfirmDestructive } from "@/features/confirm/ConfirmDestructive";
import type { SavedRule } from "@/lib/rules";

interface RuleRailProps {
  roster: SavedRule[];
  selectedId: string | null;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  /** Name-first create: derive an id from `name`, save the current buffer, open the new rule. */
  onCreate: (name: string) => Promise<string | null>;
  /** Minimize the rail — the host (RulesView) renders the symmetric `CollapsedRail` when closed. */
  onCollapse?: () => void;
}

export function RuleRail({ roster, selectedId, onOpen, onDelete, onCreate, onCollapse }: RuleRailProps) {
  // The rule pending a delete confirm — the rail hands the item back; the feature owns the gate.
  const [pendingDelete, setPendingDelete] = useState<RosterItem | null>(null);

  return (
    <>
      <RosterRail
        noun="rule"
        icon={FileCode2}
        items={roster.map((r) => ({ id: r.id, title: r.name || r.id }))}
        selectedId={selectedId}
        onSelect={onOpen}
        emptyText="No saved rules yet."
        createPlaceholder="New rule…"
        onCreate={(name) => void onCreate(name)}
        onCollapse={onCollapse}
        onRemove={setPendingDelete}
      />

      {pendingDelete && (
        <ConfirmDestructive
          title={`Delete ${pendingDelete.title}`}
          consequence="This saved rule will be removed and can't be recovered from the workbench."
          reversible={false}
          escalation="none"
          confirmLabel="Delete"
          onConfirm={() => {
            onDelete(pendingDelete.id);
            setPendingDelete(null);
          }}
          onCancel={() => setPendingDelete(null)}
        />
      )}
    </>
  );
}
