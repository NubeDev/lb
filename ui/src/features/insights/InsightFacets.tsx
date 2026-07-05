// The insights facets rail — the AND-filter the list reads (insights umbrella scope). Axes:
// status (open/acked/resolved), severity floor (info/warning/critical), origin_ref (free text),
// and tag facets (a `{ k: v }` picker — TODO: drive from `tags.find` so the picker lists real
// facets, not free text). One component per file (FILE-LAYOUT §4 frontend).
//
// STUB: the facets render + emit `onChange`; the tag-facet picker's `tags.find`-driven dropdown
// is TODO (today a free-text `key=value` input). The `range` (time-window) facet is deferred.
//
// Voice match with the rest of the page: shadcn `Button` (toggle pills) + `Input` + `Badge`
// (selected tag chips) — no bespoke pills/inputs. A selected pill takes the accent tone
// (`variant="default"`); an unselected one is `variant="outline"`.

import { X } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { ListQuery, Severity, Status } from "@/lib/insights/insights.types";

interface Props {
  filter: ListQuery;
  onChange: (next: ListQuery) => void;
}

/** The facets rail. Emits a new filter on every change; the page's hook re-fetches. */
export function InsightFacets({ filter, onChange }: Props): JSX.Element {
  function setStatus(status?: Status) {
    onChange({ ...filter, status });
  }
  function setSeverity(severity?: Severity) {
    onChange({ ...filter, severity });
  }

  return (
    <div className="space-y-5 text-sm">
      <fieldset>
        <legend className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-muted">
          Status
        </legend>
        <div className="flex flex-wrap gap-1.5">
          {(["open", "acked", "resolved"] as Status[]).map((s) => {
            const active = filter.status === s;
            return (
              <Button
                key={s}
                type="button"
                size="sm"
                variant={active ? "default" : "outline"}
                aria-pressed={active}
                onClick={() => setStatus(active ? undefined : s)}
                className="h-7 px-2.5 text-xs capitalize"
              >
                {s}
              </Button>
            );
          })}
        </div>
      </fieldset>

      <fieldset>
        <legend className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-muted">
          Severity ≥
        </legend>
        <div className="flex flex-wrap gap-1.5">
          {(["info", "warning", "critical"] as Severity[]).map((s) => {
            const active = filter.severity === s;
            return (
              <Button
                key={s}
                type="button"
                size="sm"
                variant={active ? "default" : "outline"}
                aria-pressed={active}
                onClick={() => setSeverity(active ? undefined : s)}
                className="h-7 px-2.5 text-xs capitalize"
              >
                {s}
              </Button>
            );
          })}
        </div>
      </fieldset>

      <fieldset>
        <legend className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-muted">
          Producer ref
        </legend>
        <Input
          type="text"
          value={filter.origin_ref ?? ""}
          onChange={(e) =>
            onChange({ ...filter, origin_ref: e.target.value || undefined })
          }
          placeholder="rule:… / flow:…"
          className="h-8 text-xs"
        />
      </fieldset>

      <fieldset>
        <legend className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-muted">
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

/** A minimal `key=value` tag facet editor: add rows the list AND-filters on, remove to widen.
 *  Selected rows render as dismissible shadcn `Badge`s (the same chip shape the rest of the
 *  product uses for selected facets). */
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
    <div className="space-y-2">
      {Object.keys(tags).length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {Object.entries(tags).map(([k, v]) => (
            <Badge key={k} variant="default" className="gap-1 font-mono text-[10px]">
              {k}={v}
              <button
                type="button"
                onClick={() => remove(k)}
                aria-label={`remove facet ${k}`}
                className="inline-flex items-center rounded-sm opacity-70 hover:opacity-100"
              >
                <X size={10} />
              </button>
            </Badge>
          ))}
        </div>
      )}
      <Input
        type="text"
        placeholder="key=value ↵"
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            const el = e.currentTarget;
            add(el.value);
            el.value = "";
          }
        }}
        className="h-8 text-xs"
      />
    </div>
  );
}
