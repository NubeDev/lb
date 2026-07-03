// A lightweight segmented tab control for the authoring panel (Functions | Examples | Data) — built
// from the shadcn Button (no shadcn Tabs primitive exists, and pulling in Radix Tabs for three buttons
// is more than this needs). Accent marks the active tab; the rest are quiet ghosts (product register:
// accent for selection only). Generic over a string tab id. One component per file (FILE-LAYOUT).

import { cn } from "@/lib/utils";

export interface PanelTab<T extends string> {
  id: T;
  label: string;
}

interface PanelTabsProps<T extends string> {
  tabs: PanelTab<T>[];
  active: T;
  onChange: (id: T) => void;
}

/** A row of underline tabs; the active one carries an accent underline (quiet, IDE-like register). */
export function PanelTabs<T extends string>({ tabs, active, onChange }: PanelTabsProps<T>) {
  return (
    <div
      role="tablist"
      aria-label="authoring panel tabs"
      className="flex items-stretch gap-1 border-b border-border bg-card px-2"
    >
      {tabs.map((t) => {
        const selected = t.id === active;
        return (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={selected}
            aria-label={`tab ${t.label}`}
            onClick={() => onChange(t.id)}
            className={cn(
              "relative -mb-px border-b-2 px-2.5 py-2 text-xs font-medium transition-colors",
              "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent",
              selected
                ? "border-accent text-fg"
                : "border-transparent text-muted hover:text-fg",
            )}
          >
            {t.label}
          </button>
        );
      })}
    </div>
  );
}
