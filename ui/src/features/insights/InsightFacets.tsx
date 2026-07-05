// The insights facets sidebar — the AND-filter the list reads (insights umbrella scope). Axes:
// status (open/acked/resolved), severity floor (info/warning/critical), origin_ref (free text),
// and tag facets (a `{ k: v }` picker — TODO: drive from `tags.find` so the picker lists real
// facets, not free text). One component per file (FILE-LAYOUT §4 frontend).
//
// STUB: the facets render + emit `onChange`; the tag-facet picker's `tags.find`-driven dropdown
// is TODO (today a free-text `key=value` input). The `range` (time-window) facet is deferred.

import type { ListQuery, Severity, Status } from "@/lib/insights/insights.types";

interface Props {
  filter: ListQuery;
  onChange: (next: ListQuery) => void;
}

/** The facets sidebar. Emits a new filter on every change; the page's hook re-fetches. */
export function InsightFacets({ filter, onChange }: Props): JSX.Element {
  function setStatus(status?: Status) {
    onChange({ ...filter, status });
  }
  function setSeverity(severity?: Severity) {
    onChange({ ...filter, severity });
  }

  return (
    <div className="space-y-4 text-sm">
      <fieldset>
        <legend className="mb-1 text-xs font-semibold uppercase text-muted-foreground">
          Status
        </legend>
        <div className="flex flex-wrap gap-1">
          {(["open", "acked", "resolved"] as Status[]).map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => setStatus(filter.status === s ? undefined : s)}
              className={`rounded-full border px-2 py-0.5 text-xs ${
                filter.status === s ? "border-primary bg-primary text-primary-foreground" : "border-border"
              }`}
            >
              {s}
            </button>
          ))}
        </div>
      </fieldset>

      <fieldset>
        <legend className="mb-1 text-xs font-semibold uppercase text-muted-foreground">
          Severity ≥
        </legend>
        <div className="flex flex-wrap gap-1">
          {(["info", "warning", "critical"] as Severity[]).map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => setSeverity(filter.severity === s ? undefined : s)}
              className={`rounded-full border px-2 py-0.5 text-xs ${
                filter.severity === s ? "border-primary bg-primary text-primary-foreground" : "border-border"
              }`}
            >
              {s}
            </button>
          ))}
        </div>
      </fieldset>

      <fieldset>
        <legend className="mb-1 text-xs font-semibold uppercase text-muted-foreground">
          Producer ref
        </legend>
        <input
          type="text"
          value={filter.origin_ref ?? ""}
          onChange={(e) =>
            onChange({ ...filter, origin_ref: e.target.value || undefined })
          }
          placeholder="rule:… / flow:…"
          className="w-full rounded-md border border-border px-2 py-1 text-xs"
        />
      </fieldset>

      <fieldset>
        <legend className="mb-1 text-xs font-semibold uppercase text-muted-foreground">
          Tag facets
        </legend>
        {/* A working `key=value` tag facet editor (the filter carries `tags`; the host resolves the
            subset via the tag graph). A `tags.find`-driven autocomplete of real facet values is the
            named follow-up — the dashboard variable Query-source precedent. */}
        <TagFacetEditor
          tags={filter.tags ?? {}}
          onChange={(tags) =>
            onChange({ ...filter, tags: Object.keys(tags).length ? tags : undefined })
          }
        />
      </fieldset>
    </div>
  );
}

/** A minimal `key=value` tag facet editor: add rows the list AND-filters on, remove to widen. */
function TagFacetEditor({
  tags,
  onChange,
}: {
  tags: Record<string, string>;
  onChange: (tags: Record<string, string>) => void;
}): JSX.Element {
  function add(entry: string) {
    const [k, ...rest] = entry.split("=");
    const key = k.trim();
    const value = rest.join("=").trim();
    if (!key || !value) return;
    onChange({ ...tags, [key]: value });
  }
  function remove(key: string) {
    const next = { ...tags };
    delete next[key];
    onChange(next);
  }
  return (
    <div className="space-y-1">
      <div className="flex flex-wrap gap-1">
        {Object.entries(tags).map(([k, v]) => (
          <button
            key={k}
            type="button"
            onClick={() => remove(k)}
            title="Remove facet"
            className="rounded-full border border-primary bg-primary px-2 py-0.5 text-xs text-primary-foreground"
          >
            {k}={v} ✕
          </button>
        ))}
      </div>
      <input
        type="text"
        placeholder="key=value ↵"
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            add((e.target as HTMLInputElement).value);
            (e.target as HTMLInputElement).value = "";
          }
        }}
        className="w-full rounded-md border border-border px-2 py-1 text-xs"
      />
    </div>
  );
}
