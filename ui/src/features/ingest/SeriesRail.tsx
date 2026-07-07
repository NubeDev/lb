// The ingest series rail (data-console scope) — the left list of series the workspace holds: select one
// to inspect. Chrome/behavior live in the shared `RosterRail` (components/app/roster.tsx); this file
// only maps the series list onto it. Series have no inline create (creation is a multi-step schema
// wizard, triggered from the page header), no rename, and no delete from this surface — so only the
// select + minimize behavior is wired. The series list is the hook's filtered `find`/`list` result.
// One component per file (FILE-LAYOUT).

import { Activity } from "lucide-react";

import { RosterRail } from "@/components/app/roster";

interface SeriesRailProps {
  series: string[];
  selectedId: string | null;
  onSelect: (series: string) => void;
  /** Name-first create: the rail's inline "New series…" field. The host opens the create-series wizard
   *  seeded with this name (Ingest's create is a multi-step schema, not an immediate insert). */
  onCreate?: (name: string) => void;
  /** Minimize the rail — the host (IngestView) renders the symmetric `CollapsedRail` when closed. */
  onCollapse?: () => void;
  /** Empty-roster copy. The host swaps this for a "no match" message when a search returns nothing. */
  emptyText?: string;
}

export function SeriesRail({ series, selectedId, onSelect, onCreate, onCollapse, emptyText }: SeriesRailProps) {
  return (
    <RosterRail
      noun="series"
      icon={Activity}
      items={series.map((s) => ({ id: s, title: s }))}
      selectedId={selectedId}
      onSelect={onSelect}
      onCreate={onCreate}
      createPlaceholder="New series…"
      onCollapse={onCollapse}
      emptyText={emptyText ?? "No series."}
    />
  );
}
