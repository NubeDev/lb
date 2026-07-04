// A SEARCHABLE grouped source picker (data-studio-ux scope, "the first 10 seconds"). The shipped
// `<select>` (SourcePicker) forces the author to scroll a long list of paragraph-length labels; this is
// the rich alternative — type to filter across every group, arrow-key to move, Enter to pick. Same model
// (`SourceEntry` → `selectionOf`), same `--sp-*` token discipline, still zero I/O and zero `@/` imports
// so an extension reuses it exactly as it reuses the `<select>`. The `<select>` stays exported as the
// minimal renderer for embedders that don't want a popover.
//
// One responsibility: pick a source by typing. Grouping/labelling reuse the SAME group list the `<select>`
// renders, so the two stay in lock-step.

import { useMemo, useRef, useState } from "react";

import { selectionOf, type SourceEntry } from "./sourcePicker";
import { READ_SOURCE_GROUPS, type SourceGroup } from "./SourcePicker";
import type { SourceSelection } from "./types";

export interface SourceComboboxProps {
  /** The assembled entries (from `useSourcePicker`). */
  entries: SourceEntry[];
  /** The currently-selected entry id (controlled) — "" for none. */
  value?: string;
  /** Called with the chosen entry's selection (or null when cleared). */
  onSelect: (selection: SourceSelection | null) => void;
  /** Also called with the RAW entry (or null) — for a host that keys on `entry.id` (e.g. edit-mode
   *  seeding, or a tool shared across entries like `rules.run`) where the folded selection loses the id.
   *  Optional; `onSelect` fires regardless. */
  onSelectEntry?: (entry: SourceEntry | null) => void;
  /** True while the entries load. */
  loading?: boolean;
  /** Which groups show + their order/labels (default: the read groups). */
  groups?: SourceGroup[];
  /** Accessible label (default "source"). */
  "aria-label"?: string;
  /** Extra className on the root. */
  className?: string;
  /** Placeholder for the search input. */
  placeholder?: string;
  /** Autofocus the search box on mount (Data Studio focuses it so type-to-search is the first action). */
  autoFocus?: boolean;
}

/** A flat, ordered, filtered list of {entry, groupLabel} — grouped for display, flat for keyboard nav. */
interface Row {
  entry: SourceEntry;
  groupLabel: string;
  /** True when this row is the first of its group (renders the section header above it). */
  firstOfGroup: boolean;
}

export function SourceCombobox({
  entries,
  value = "",
  onSelect,
  onSelectEntry,
  loading = false,
  groups = READ_SOURCE_GROUPS,
  "aria-label": ariaLabel = "source",
  className,
  placeholder = "Search sources…",
  autoFocus = false,
}: SourceComboboxProps) {
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLUListElement>(null);

  const selected = entries.find((e) => e.id === value) ?? null;

  // Build the visible rows: for each configured group, its matching entries (label OR group label
  // contains the query, case-insensitive), in group order. A row is the first of its group so we render
  // one section header per group present.
  const rows = useMemo<Row[]>(() => {
    const q = query.trim().toLowerCase();
    const out: Row[] = [];
    for (const { group, label } of groups) {
      const items = entries.filter(
        (e) =>
          e.group === group &&
          (q === "" || e.label.toLowerCase().includes(q) || label.toLowerCase().includes(q)),
      );
      items.forEach((entry, i) => out.push({ entry, groupLabel: label, firstOfGroup: i === 0 }));
    }
    return out;
  }, [entries, groups, query]);

  const choose = (entry: SourceEntry | null) => {
    onSelect(entry ? selectionOf(entry) : null);
    onSelectEntry?.(entry);
    setOpen(false);
    setQuery("");
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setOpen(true);
      setActive((a) => Math.min(a + 1, rows.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (open && rows[active]) choose(rows[active].entry);
    } else if (e.key === "Escape") {
      setOpen(false);
    }
  };

  return (
    <div className={`sp-root sp-combo${className ? ` ${className}` : ""}`}>
      <input
        className="sp-combo-input"
        role="combobox"
        aria-expanded={open}
        aria-label={ariaLabel}
        aria-autocomplete="list"
        // eslint-disable-next-line jsx-a11y/no-autofocus -- deliberate: type-to-search is the first action
        autoFocus={autoFocus}
        value={open ? query : selected?.label ?? ""}
        placeholder={loading ? "loading sources…" : selected ? selected.label : placeholder}
        onFocus={() => setOpen(true)}
        onBlur={() => setTimeout(() => setOpen(false), 120)}
        onChange={(e) => {
          setQuery(e.target.value);
          setOpen(true);
          setActive(0);
        }}
        onKeyDown={onKeyDown}
      />
      {open && (
        <ul className="sp-combo-list" role="listbox" aria-label={ariaLabel} ref={listRef}>
          {rows.length === 0 && <li className="sp-combo-empty">No matching sources</li>}
          {rows.map((row, i) => (
            <li key={row.entry.id} role="presentation">
              {row.firstOfGroup && <div className="sp-combo-group">{row.groupLabel}</div>}
              <button
                type="button"
                role="option"
                aria-selected={i === active}
                className={`sp-combo-option${i === active ? " is-active" : ""}${row.entry.id === value ? " is-selected" : ""}`}
                // onMouseDown (not onClick) so it fires before the input's blur closes the list.
                onMouseDown={(e) => {
                  e.preventDefault();
                  choose(row.entry);
                }}
                onMouseEnter={() => setActive(i)}
              >
                {row.entry.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
