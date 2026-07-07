// The data table rail (data-console scope) — the left list of raw store tables with their row counts:
// select one to browse its rows. Chrome/behavior live in the shared `RosterRail`
// (components/app/roster.tsx); this file only maps the table list onto it, surfacing each table's row
// count as the trailing badge. This surface is READ-ONLY — there is no create/rename/delete here (the
// raw grid never edits; edits go through the domain verbs), so only select + minimize is wired. One
// component per file (FILE-LAYOUT).

import { Table2 } from "lucide-react";

import { RosterRail } from "@/components/app/roster";
import type { TableCount } from "@/lib/data/data.types";

interface TableRailProps {
  tables: TableCount[];
  selectedId: string | null;
  onSelect: (table: string) => void;
  /** Minimize the rail — the host (DataView) renders the symmetric `CollapsedRail` when closed. */
  onCollapse?: () => void;
}

export function TableRail({ tables, selectedId, onSelect, onCollapse }: TableRailProps) {
  return (
    <RosterRail
      noun="table"
      icon={Table2}
      items={tables.map((t) => ({ id: t.table, title: t.table, badge: String(t.count) }))}
      selectedId={selectedId}
      onSelect={onSelect}
      onCollapse={onCollapse}
      emptyText="No tables found."
    />
  );
}
