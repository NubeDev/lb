// RuleRail — the left rail of saved rules (rules-workbench scope): list via `rules.list`, open via
// `rules.get` (the parent hook), delete via `rules.delete`. One component per file (FILE-LAYOUT).

import { Button } from "@/components/ui/button";
import type { SavedRule } from "@/lib/rules";

interface RuleRailProps {
  roster: SavedRule[];
  selectedId: string | null;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  onNew: () => void;
}

export function RuleRail({ roster, selectedId, onOpen, onDelete, onNew }: RuleRailProps) {
  return (
    <nav aria-label="rule rail" className="flex w-56 flex-col border-r border-border">
      <Button
        aria-label="new rule"
        className="m-2"
        size="sm"
        variant="outline"
        onClick={onNew}
      >
        + New rule
      </Button>
      <ul className="flex-1 overflow-auto">
        {roster.length === 0 ? (
          <li className="px-3 py-2 text-sm text-muted">No saved rules yet.</li>
        ) : (
          roster.map((r) => (
            <li
              key={r.id}
              className={`flex items-center justify-between px-3 py-1.5 text-sm ${
                r.id === selectedId ? "bg-muted" : ""
              }`}
            >
              <Button
                aria-label={`open rule ${r.id}`}
                className="h-auto flex-1 justify-start truncate px-0 py-0 font-normal hover:underline"
                size="sm"
                variant="ghost"
                onClick={() => onOpen(r.id)}
              >
                {r.name || r.id}
              </Button>
              <Button
                aria-label={`delete rule ${r.id}`}
                className="ml-2 h-auto px-1 py-0 text-muted hover:text-red-600"
                size="sm"
                variant="ghost"
                onClick={() => onDelete(r.id)}
              >
                ✕
              </Button>
            </li>
          ))
        )}
      </ul>
    </nav>
  );
}
