// RuleRail — the left rail of saved rules (rules-workbench scope): list via `rules.list`, open via
// `rules.get` (the parent hook), delete via `rules.delete`. Styled to match the dashboard roster:
// a card-backed aside with a header control row and selectable list rows. "New rule" reveals an
// inline NAME-first create form right here in the rail (where rules live) — the user names the rule,
// we derive the id, save, and open it. No far-away "Save as…". One component per file (FILE-LAYOUT).

import { useState, type FormEvent } from "react";
import { FileCode2, Plus, Trash2 } from "lucide-react";

import { AppRail } from "@/components/app/rail";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { SavedRule } from "@/lib/rules";

interface RuleRailProps {
  roster: SavedRule[];
  selectedId: string | null;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  /** Name-first create: derive an id from `name`, save the current buffer, open the new rule. */
  onCreate: (name: string) => Promise<string | null>;
}

export function RuleRail({ roster, selectedId, onOpen, onDelete, onCreate }: RuleRailProps) {
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed || busy) return;
    setBusy(true);
    const id = await onCreate(trimmed);
    setBusy(false);
    if (id) {
      setName("");
      setCreating(false);
    }
  }

  return (
    <AppRail
      label="rule rail"
      header={
        creating ? (
          <form className="w-full space-y-2" onSubmit={submit}>
            <Input
              aria-label="new rule name"
              autoFocus
              className="h-8"
              placeholder="Rule name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setName("");
                  setCreating(false);
                }
              }}
            />
            <div className="flex gap-2">
              <Button
                aria-label="create rule"
                className="h-8 flex-1"
                size="sm"
                type="submit"
                disabled={busy || !name.trim()}
              >
                {busy ? "Creating…" : "Create rule"}
              </Button>
              <Button
                aria-label="cancel new rule"
                className="h-8"
                size="sm"
                variant="ghost"
                type="button"
                onClick={() => {
                  setName("");
                  setCreating(false);
                }}
              >
                Cancel
              </Button>
            </div>
          </form>
        ) : (
          <Button
            aria-label="new rule"
            className="w-full justify-center"
            size="sm"
            variant="outline"
            onClick={() => setCreating(true)}
          >
            <Plus size={14} /> New rule
          </Button>
        )
      }
    >
      <ul className="space-y-1">
        {roster.length === 0 ? (
          <li className="rounded-md border border-dashed border-border bg-bg/60 px-3 py-3 text-xs text-muted">
            No saved rules yet.
          </li>
        ) : (
          roster.map((r) => {
            const active = r.id === selectedId;
            return (
              <li key={r.id}>
                <div
                  className={`group flex items-center gap-1 rounded-md border px-2.5 py-2 text-left text-sm transition-colors ${
                    active
                      ? "border-accent/25 bg-accent/15 text-accent shadow-sm shadow-black/5"
                      : "border-transparent text-fg hover:border-border hover:bg-bg"
                  }`}
                >
                  <Button
                    aria-label={`open rule ${r.id}`}
                    className="h-auto flex-1 justify-start gap-2 truncate px-0 font-normal"
                    size="sm"
                    variant="ghost"
                    onClick={() => onOpen(r.id)}
                  >
                    <FileCode2 size={14} className="shrink-0" />
                    <span className="truncate">{r.name || r.id}</span>
                  </Button>
                  <Button
                    aria-label={`delete rule ${r.id}`}
                    className="h-7 w-7 shrink-0 text-muted hover:text-destructive"
                    size="icon"
                    variant="ghost"
                    onClick={() => onDelete(r.id)}
                  >
                    <Trash2 size={13} />
                  </Button>
                </div>
              </li>
            );
          })
        )}
      </ul>
    </AppRail>
  );
}
