// The props-driven source picker UI — a grouped <select> the author picks a source from by friendly
// label (dashboard widget-builder scope, "The source picker"), now reusable. It renders the entries
// `useSourcePicker` built, grouped by origin, and calls `onSelect` with the chosen entry's
// `SourceSelection`. Presentational + data-via-props (no I/O, no `@/`), self-themed via scoped
// `--sp-*` tokens a host can override — the `@nube/panel`/`@nube/nav-rail` discipline.
//
// It renders the read/source groups (series/live/sql/extension/widget/flows). A host that also wants
// the federation DATASOURCE dropdown or the flow node→port sub-picker composes those around this
// (they need host-specific target shaping); this component owns the one thing every consumer needs —
// "pick a source by label."

import { selectionOf, type SourceEntry } from "./sourcePicker";
import type { SourceSelection } from "./types";

/** The groups this picker renders, in display order, with their section labels. `action` is omitted by
 *  default (write controls are a separate authoring intent); a host that wants them passes `groups`. */
const DEFAULT_GROUPS: { group: SourceEntry["group"]; label: string }[] = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "widget", label: "Extension widgets" },
  { group: "flows", label: "Flows" },
];

export interface SourcePickerProps {
  /** The assembled entries (from `useSourcePicker`). */
  entries: SourceEntry[];
  /** The currently-selected entry id (controlled) — "" for none. */
  value?: string;
  /** Called with the chosen entry's selection (or null when cleared to "— pick —"). */
  onSelect: (selection: SourceSelection | null) => void;
  /** True while the entries load — shows a loading placeholder. */
  loading?: boolean;
  /** Override which groups show + their order/labels (default: the read groups above). */
  groups?: { group: SourceEntry["group"]; label: string }[];
  /** Accessible label for the select (default "source"). */
  "aria-label"?: string;
  /** Extra className on the root <label> (host layout). */
  className?: string;
}

export function SourcePicker({
  entries,
  value = "",
  onSelect,
  loading = false,
  groups = DEFAULT_GROUPS,
  "aria-label": ariaLabel = "source",
  className,
}: SourcePickerProps) {
  const choose = (id: string) => {
    const entry = entries.find((e) => e.id === id) ?? null;
    onSelect(entry ? selectionOf(entry) : null);
  };
  return (
    <label className={`sp-root${className ? ` ${className}` : ""}`}>
      <select
        className="sp-select"
        aria-label={ariaLabel}
        value={value}
        onChange={(e) => choose(e.target.value)}
      >
        <option value="">{loading ? "loading sources…" : "— pick a source —"}</option>
        {groups.map(({ group, label }) => (
          <PickerGroup key={group} entries={entries} group={group} label={label} />
        ))}
      </select>
    </label>
  );
}

/** One `<optgroup>` for a source group, empty-tolerant (no section when it has no entries). */
function PickerGroup({
  entries,
  group,
  label,
}: {
  entries: SourceEntry[];
  group: SourceEntry["group"];
  label: string;
}) {
  const items = entries.filter((e) => e.group === group);
  if (items.length === 0) return null;
  return (
    <optgroup label={label}>
      {items.map((e) => (
        <option key={e.id} value={e.id}>
          {e.label}
        </option>
      ))}
    </optgroup>
  );
}
