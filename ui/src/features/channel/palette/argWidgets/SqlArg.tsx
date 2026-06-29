// The SQL arg widget (channels-command-palette scope). An `x-lb-widget:"sql"` arg renders this: a
// mini SQL editor with table/column autocomplete sourced from the discovery SELECTs (`useSqlSchema`,
// cached per source). It is a plain textarea (no heavy editor dep — keyboard-first, jsdom-testable)
// with a suggestion strip; `Ctrl/⌘+Enter` submits the whole command. RENDER + local input only;
// the schema data is the hook passed in (FILE-LAYOUT).

import { useMemo, useState } from "react";

import { useSqlSchema } from "./useSqlSchema";

interface Props {
  /** The chosen source name (its schema drives autocomplete); null disables suggestions. */
  source: string | null;
  /** The source kind (postgres/sqlite) — selects the discovery dialect. */
  kind: string;
  value: string;
  onChange: (sql: string) => void;
  /** Ctrl/⌘+Enter — submit the whole command. */
  onSubmit: () => void;
  /** Esc — cancel back out of the SQL arg. */
  onCancel: () => void;
}

/** The word currently under typing (the trailing `[A-Za-z0-9_]+`), for autocomplete matching. */
function trailingWord(sql: string): string {
  const m = /([A-Za-z0-9_]+)$/.exec(sql);
  return m ? m[1] : "";
}

export function SqlArg({ source, kind, value, onChange, onSubmit, onCancel }: Props) {
  const { tables, columns, ensureColumns } = useSqlSchema(source, kind);
  const [open, setOpen] = useState(true);

  // Candidates = every table + every already-discovered column, filtered by the trailing word.
  const suggestions = useMemo(() => {
    const word = trailingWord(value).toLowerCase();
    const allCols = Object.values(columns).flat();
    const pool = Array.from(new Set([...tables, ...allCols]));
    if (word === "") return tables.slice(0, 8);
    return pool.filter((n) => n.toLowerCase().includes(word)).slice(0, 8);
  }, [value, tables, columns]);

  function accept(name: string) {
    // Replace the trailing word with the picked identifier.
    const next = value.replace(/([A-Za-z0-9_]+)$/, "") + name + " ";
    onChange(next);
    if (tables.includes(name)) ensureColumns(name);
  }

  return (
    <div className="border-t border-border bg-panel p-2" aria-label="sql editor">
      <textarea
        aria-label="sql"
        value={value}
        autoFocus
        rows={3}
        spellCheck={false}
        onChange={(e) => {
          onChange(e.target.value);
          setOpen(true);
          const word = trailingWord(e.target.value);
          if (tables.includes(word)) ensureColumns(word);
        }}
        onKeyDown={(e) => {
          if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
            e.preventDefault();
            onSubmit();
          } else if (e.key === "Escape") {
            e.preventDefault();
            onCancel();
          }
        }}
        placeholder="SELECT … (⌘/Ctrl+Enter to run)"
        className="control-field min-h-[4.5rem] w-full resize-y font-mono text-sm"
      />
      {open && suggestions.length > 0 && (
        <ul
          role="listbox"
          aria-label="sql suggestions"
          className="mt-1 flex flex-wrap gap-1"
        >
          {suggestions.map((s) => (
            <li key={s} role="option" aria-selected={false}>
              <button
                type="button"
                onMouseDown={(e) => {
                  e.preventDefault();
                  accept(s);
                }}
                className="soft-button h-7 px-2 text-xs"
              >
                {s}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
