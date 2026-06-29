// The `@`-entity picker widget (channels-command-palette scope). An `x-lb-entity` arg auto-opens
// this: a keyboard-driven listbox of the entities the caller can list (from `useMentions`, backed
// by the existing list verb). Picking one emits the value as a chip in the parent. Empty list shows
// the lister's REASON, never a spinner or a blank box (the "no dead-ends" criterion). RENDER ONLY —
// the data is the `useMentions` set passed in; this file is markup + keyboard handling.

import type { Candidate } from "../parsePalette";

interface Props {
  /** The arg name being filled (the picker's heading). */
  arg: string;
  /** Ranked candidates (already filtered by the current `@` query upstream). */
  candidates: Candidate[];
  /** The pre-selected index (keyboard highlight). */
  selected: number;
  /** Set when `candidates` is empty — the human reason to show instead of a blank box. */
  reason: string | null;
  loading: boolean;
  /** Called when a candidate is chosen (Enter / click). */
  onPick: (value: string) => void;
  /** Move the highlight (↑ ↓). */
  onMove: (index: number) => void;
}

export function EntityPicker({ arg, candidates, selected, reason, loading, onPick, onMove }: Props) {
  return (
    <div className="border-t border-border bg-panel" aria-label={`pick ${arg}`}>
      <div className="px-3 py-1.5 text-xs font-medium text-muted">Pick a {arg}</div>
      {loading ? (
        <div className="px-3 py-2 text-xs text-muted">Loading…</div>
      ) : candidates.length === 0 ? (
        <div role="note" className="px-3 py-2 text-xs text-muted">
          {reason ?? `No ${arg} available`}
        </div>
      ) : (
        <ul role="listbox" aria-label={`${arg} options`} className="max-h-48 overflow-y-auto py-1">
          {candidates.map((c, i) => (
            <li
              key={c.value}
              role="option"
              aria-selected={i === selected}
              onMouseEnter={() => onMove(i)}
              onMouseDown={(e) => {
                e.preventDefault();
                onPick(c.value);
              }}
              className={`flex cursor-pointer items-baseline gap-2 px-3 py-1.5 text-sm ${
                i === selected ? "bg-accent/15 text-fg" : "text-fg"
              }`}
            >
              <span className="font-medium">{c.label}</span>
              {c.hint && <span className="text-xs text-muted">{c.hint}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
