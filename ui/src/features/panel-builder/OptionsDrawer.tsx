// The collapsed options drawer (data-studio-10x scope, phase 3 stage 3) — the query-first flow's
// "refine on demand": the full option surface (`OptionsSections`, searchable) folds behind one
// collapsed bar. Power depth intact, default cost zero. One responsibility: the disclosure chrome;
// the sections are the children.

import { useState } from "react";
import { ChevronDown, ChevronRight, SlidersHorizontal } from "lucide-react";

import { cn } from "@/lib/utils";

interface Props {
  children: React.ReactNode;
}

export function OptionsDrawer({ children }: Props) {
  // OPEN by default — editing the chart must never be hidden (the collapse is for reclaiming preview
  // space, not the resting state). Collapsing is a per-tab, in-session choice.
  const [open, setOpen] = useState(true);
  return (
    <div className={cn("flex min-h-0 shrink-0 flex-col", open && "min-h-[14rem] flex-1")}>
      <button
        type="button"
        aria-label="options drawer"
        aria-expanded={open}
        className="flex shrink-0 items-center gap-1.5 border-t border-border py-1.5 text-xs font-medium text-muted hover:text-fg"
        onClick={() => setOpen((o) => !o)}
      >
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <SlidersHorizontal size={12} />
        Options
        <span className="font-normal">— query, plot, transform, field, overrides</span>
      </button>
      {open && <div className="min-h-0 flex-1">{children}</div>}
    </div>
  );
}
