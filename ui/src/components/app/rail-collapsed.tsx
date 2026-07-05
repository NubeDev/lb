// CollapsedRail — the thin strip a minimized `RosterRail` folds to: a 2.5rem raised column with a
// single expand affordance. Extracted from the dashboard surface so every rail collapses to the
// identical strip; a host renders `RosterRail` (with `onCollapse`) or this, and owns the open state.
// `noun` seeds the aria-labels ("dashboard rail collapsed", "expand rule rail", …). One component
// per file (FILE-LAYOUT).

import { PanelLeftOpen } from "lucide-react";

import { Button } from "@/components/ui/button";

interface CollapsedRailProps {
  /** The surface's noun, used in the aria-labels: "dashboard", "rule", "flow". */
  noun: string;
  onExpand: () => void;
}

export function CollapsedRail({ noun, onExpand }: CollapsedRailProps) {
  return (
    <aside
      aria-label={`${noun} rail collapsed`}
      data-panel=""
      className="flex w-10 shrink-0 flex-col items-center border-r border-border bg-panel-2 py-2"
    >
      <Button
        aria-label={`expand ${noun} rail`}
        variant="ghost"
        size="icon"
        className="h-8 w-8"
        title="Expand"
        onClick={onExpand}
      >
        <PanelLeftOpen size={14} />
      </Button>
    </aside>
  );
}
