// A tiny token-bound tab strip for the panel editor's options rail (viz panel-editor scope: the
// Query/Transform/Panel options/Field/Overrides tabs). In-house (button-driven, not Radix) so Phase 1
// ships the full tab structure without pulling a new primitive; generating a shadcn/Radix Tabs is the
// documented follow-up (dashboard.md, "Generate shadcn primitives"). One responsibility: the tab strip.

import { Button } from "@/components/ui/button";

interface Props {
  tabs: { id: string; label: string; badge?: number }[];
  active: string;
  onSelect: (id: string) => void;
}

export function EditorTabs({ tabs, active, onSelect }: Props) {
  return (
    <div className="flex flex-wrap gap-1 border-b border-border" role="tablist" aria-label="panel editor tabs">
      {tabs.map((t) => {
        const selected = t.id === active;
        return (
          <Button
            key={t.id}
            variant="ghost"
            role="tab"
            aria-selected={selected}
            aria-label={`tab ${t.label}`}
            className={`relative -mb-px h-auto gap-1.5 rounded-b-none rounded-t-md border-b-2 px-3 py-1.5 text-xs font-medium ${
              selected ? "border-accent text-fg" : "border-transparent text-muted hover:text-fg"
            }`}
            onClick={() => onSelect(t.id)}
          >
            {t.label}
            {t.badge ? (
              <span className="rounded-full bg-accent/15 px-1.5 text-[10px] text-accent">{t.badge}</span>
            ) : null}
          </Button>
        );
      })}
    </div>
  );
}
