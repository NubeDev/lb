// The Data Studio rail — the studio's LEFT RAIL on the shared `AppRail` chrome (the RosterRail kit
// look every surface uses: Dashboards, Rules, Flows, Ingest, Data). It replaces the v2 FlexLayout
// border dock (the rotated Sources/Library strip) so the studio's source picker + library roster get
// the same raised column, header row, and minimize affordance as every other roster. Two rail tabs:
//   - Sources — the shipped `@nube/source-picker` (pick → open a stacked builder tab)
//   - Library — the workspace's `panel.*` library roster (open → edit in a builder tab)
// The host (`DataStudioView`) owns the open state and renders the symmetric `CollapsedRail`. One
// responsibility: the rail chrome + the tab switch; the tab bodies live in `panes/`.

import { useState } from "react";
import { PanelLeftClose } from "lucide-react";

import { AppRail } from "@/components/app/rail";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { specToCell } from "@/lib/panel";
import type { SourceSelection } from "@nube/source-picker";

import { SourcesPane } from "./panes/SourcesPane";
import { LibraryPane } from "./panes/LibraryPane";

type RailTab = "sources" | "library";

interface Props {
  ws: string;
  /** Open a stacked builder tab on a picked catalog entry. */
  onOpenSource: (sel: SourceSelection, label: string) => void;
  /** Open a stacked builder tab on a library panel's spec. */
  onOpenPanel: (panelId: string, title: string, cell: ReturnType<typeof specToCell>) => void;
  /** Bumped when a builder tab saves a panel — the Library tab re-lists. */
  libraryTick: number;
  /** Minimize the rail (the host renders the symmetric expand control). */
  onCollapse: () => void;
}

export function StudioRail({ ws, onOpenSource, onOpenPanel, libraryTick, onCollapse }: Props) {
  const [tab, setTab] = useState<RailTab>("sources");

  const tabButton = (value: RailTab, label: string) => (
    <button
      type="button"
      role="tab"
      aria-selected={tab === value}
      aria-label={`${value} tab`}
      className={cn(
        "flex-1 rounded-[calc(var(--radius)-2px)] px-2 py-1 text-xs font-medium transition-colors",
        tab === value ? "bg-panel text-fg shadow-[var(--shadow-1)]" : "text-muted hover:text-fg",
      )}
      onClick={() => setTab(value)}
    >
      {label}
    </button>
  );

  return (
    <AppRail
      label="studio rail"
      header={
        <>
          <div role="tablist" aria-label="studio rail tabs" className="flex h-8 min-w-0 flex-1 items-center gap-0.5 rounded-md bg-bg/60 p-0.5">
            {tabButton("sources", "Sources")}
            {tabButton("library", "Library")}
          </div>
          <Button
            aria-label="minimize studio rail"
            variant="ghost"
            size="icon"
            className="h-8 w-8 shrink-0"
            title="Minimize"
            onClick={onCollapse}
          >
            <PanelLeftClose size={14} />
          </Button>
        </>
      }
    >
      {tab === "sources" ? (
        <SourcesPane ws={ws} onOpen={onOpenSource} />
      ) : (
        <LibraryPane refreshKey={libraryTick} onOpen={onOpenPanel} />
      )}
    </AppRail>
  );
}
