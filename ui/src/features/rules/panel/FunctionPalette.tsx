// The function palette — a searchable, categorized list of the engine's registered Rhai verbs, each
// click-to-insert (rules-editor-ux scope). The catalog is the static `CATALOG` (mirrors the crate); the
// search box filters by verb name or signature substring, collapsing empty groups. Discovery without
// reading docs: a newcomer types "roll", finds `rollup`, clicks it in. One component per file.

import { useMemo, useState } from "react";
import { Search } from "lucide-react";

import { Input } from "@/components/ui/input";
import { CATALOG } from "../catalog";
import { FunctionEntry } from "./FunctionEntry";

interface FunctionPaletteProps {
  /** Insert a snippet at the editor cursor. */
  onInsert: (snippet: string) => void;
}

/** The categorized, searchable function palette. */
export function FunctionPalette({ onInsert }: FunctionPaletteProps) {
  const [q, setQ] = useState("");

  const groups = useMemo(() => {
    const needle = q.trim().toLowerCase();
    if (!needle) return CATALOG;
    return CATALOG.map((g) => ({
      ...g,
      entries: g.entries.filter(
        (e) =>
          e.name.toLowerCase().includes(needle) ||
          e.signature.toLowerCase().includes(needle),
      ),
    })).filter((g) => g.entries.length > 0);
  }, [q]);

  return (
    <div aria-label="function palette" className="flex h-full flex-col">
      <div className="relative px-2 pb-2 pt-2">
        <Search size={13} className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-muted" />
        <Input
          aria-label="search functions"
          placeholder="Search functions…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          className="h-8 pl-7 text-xs"
        />
      </div>

      <div className="flex-1 overflow-auto px-1 pb-2">
        {groups.length === 0 ? (
          <p aria-label="no functions" className="px-2 py-6 text-center text-xs text-muted">
            No functions match “{q}”.
          </p>
        ) : (
          groups.map((g) => (
            <section key={g.category} aria-label={`category ${g.label}`} className="mb-2">
              <header className="px-2 pb-1 pt-1.5">
                <h3 className="text-[11px] font-semibold uppercase tracking-wide text-muted">
                  {g.label}
                </h3>
              </header>
              <div className="grid gap-0.5">
                {g.entries.map((e) => (
                  <FunctionEntry key={e.name + e.signature} entry={e} onInsert={onInsert} />
                ))}
              </div>
            </section>
          ))
        )}
      </div>
    </div>
  );
}
