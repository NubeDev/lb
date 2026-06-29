// A lightweight segmented tab control for the authoring panel (Functions | Examples | Data) — built
// from the shadcn Button (no shadcn Tabs primitive exists, and pulling in Radix Tabs for three buttons
// is more than this needs). Accent marks the active tab; the rest are quiet ghosts (product register:
// accent for selection only). Generic over a string tab id. One component per file (FILE-LAYOUT).

import { Button } from "@/components/ui/button";
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

/** A row of segmented tabs; the active one carries the accent. */
export function PanelTabs<T extends string>({ tabs, active, onChange }: PanelTabsProps<T>) {
  return (
    <div
      role="tablist"
      aria-label="authoring panel tabs"
      className="flex gap-1 border-b border-border bg-muted/40 p-1"
    >
      {tabs.map((t) => {
        const selected = t.id === active;
        return (
          <Button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={selected}
            aria-label={`tab ${t.label}`}
            variant="ghost"
            size="sm"
            onClick={() => onChange(t.id)}
            className={cn(
              "h-7 flex-1 text-xs",
              selected
                ? "bg-accent text-accent-foreground hover:bg-accent"
                : "text-muted hover:text-fg",
            )}
          >
            {t.label}
          </Button>
        );
      })}
    </div>
  );
}
