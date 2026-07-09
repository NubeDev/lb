// OptionsStep (panel-wizard scope, step 5 — redesigned per the scope's resolved decision #3). A compact
// grouped FORM of option rows (Standard → Graph styles → … per view), with NO chart of its own: the ONE
// pinned `OptionFocusPreview` beside the wizard is the only render surface. Hovering or focusing a row
// reports the option upward (`onFocusOption`); the host points the pinned preview's `optionFocus` at it,
// so the preview highlights the region that option affects. Dead options still surface themselves with
// the honest "renderer pending" note in their row.
//
// No row-local state — every row reads/writes through `patch` → `writeOption` against the wizard's
// `EditorState` (the load-bearing no-drift invariant). A presentation-option toggle reshapes cached
// frames (the shipped fetch/shape split — no second `viz.query`); only data steps (TransformStep) re-fetch.
//
// One responsibility: render the per-view option groups as a compact form + report the focused option.

import { useState } from "react";
import {
  ArrowLeftRight,
  ChevronDown,
  ChevronRight,
  Hash,
  LayoutGrid,
  LineChart,
  Link,
  List,
  MessageSquare,
  Monitor,
  PanelBottom,
  Ruler,
  Settings2,
  SlidersHorizontal,
  Table,
  TriangleAlert,
  type LucideIcon,
} from "lucide-react";

import type { View } from "@/lib/dashboard";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { canonicalView } from "@/lib/dashboard";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { groupOptions, optionsForView } from "@/features/panel-builder/options/registry";
import { OptionSectionCard } from "@/features/panel-builder/options/OptionSectionCard";
import { WizardTour } from "./WizardTour";

/** One icon per registry group name — a visual anchor for the collapsed section headers. A group name
 *  not listed here (a future def file) falls back to the generic settings glyph. */
const GROUP_ICONS: Record<string, LucideIcon> = {
  "Standard options": SlidersHorizontal,
  Thresholds: TriangleAlert,
  "Value mappings": ArrowLeftRight,
  "Data links": Link,
  "Graph styles": LineChart,
  Axis: Ruler,
  Legend: List,
  Tooltip: MessageSquare,
  "Table options": Table,
  Cell: LayoutGrid,
  "Value options": Hash,
  Display: Monitor,
  Footer: PanelBottom,
};

interface Props {
  state: EditorState;
  /** Apply a state patch (presentation options only re-shape; this never bumps the refresh tick). */
  patch: (next: Partial<EditorState>) => void;
  /** Report the option being edited (hover/focus) — the host points the pinned preview at it. */
  onFocusOption?: (optionId: string) => void;
  /** The option the host's preview is currently focusing (drives the row highlight). */
  focusedOption?: string;
  /** The viewer's principal — keys the tour's per-user dismissal (absent ⇒ tour never fires). */
  userId?: string;
}

export function OptionsStep({ state, patch, onFocusOption, focusedOption, userId }: Props) {
  const view = canonicalView(state.view || "timeseries") as View;
  const defs = optionsForView(view);
  const groups = groupOptions(defs);
  // Which groups are expanded — presentation-only disclosure state (never authoring state). The first
  // group (Standard) opens by default; the rest start collapsed so the step reads as a short list, not
  // a wall. Keyed by group name so a view switch re-derives cleanly.
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const isOpen = (group: string, index: number) => open[group] ?? index === 0;

  if (defs.length === 0) {
    return (
      <div className="rounded-md border border-dashed border-border p-4 text-xs text-muted">
        No options registered for {view}.
      </div>
    );
  }

  return (
    <div className="grid gap-4" aria-label="wizard options step">
      <WizardTour userId={userId} />
      <div className="grid gap-1">
        <h2 className="text-sm font-medium text-fg">Advanced options</h2>
        <p className="text-xs text-muted">
          The basics live on the Chart type step — this is the full per-view option set. The preview on
          the right highlights the part each option affects as you edit it.
        </p>
      </div>
      {groups.map(({ group, options }, i) => {
        const Icon = GROUP_ICONS[group] ?? Settings2;
        return (
        <Collapsible
          key={group}
          open={isOpen(group, i)}
          onOpenChange={(next) => setOpen((o) => ({ ...o, [group]: next }))}
        >
          <section className="grid gap-1.5" aria-label={`option group ${group}`}>
            <CollapsibleTrigger
              className="flex w-full items-center gap-1.5 text-left text-[11px] font-medium uppercase tracking-wide text-muted hover:text-fg"
              aria-label={`toggle group ${group}`}
            >
              {isOpen(group, i) ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
              <Icon size={13} className="text-accent/80" aria-hidden />
              {group}
              <span className="font-normal normal-case tracking-normal text-muted/70">
                {options.length} {options.length === 1 ? "option" : "options"}
              </span>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <div className="divide-y divide-border/60 overflow-hidden rounded-md border border-border bg-bg">
                {options.map((def) => (
                  <OptionSectionCard
                    key={def.id}
                    def={def}
                    view={view}
                    state={state}
                    patch={patch}
                    onFocus={onFocusOption}
                    focused={focusedOption === def.id}
                  />
                ))}
              </div>
            </CollapsibleContent>
          </section>
        </Collapsible>
        );
      })}
    </div>
  );
}
