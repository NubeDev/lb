// The insights filter toolbar — sits ABOVE the list (a normal table/search register, not a left
// rail). Three groups, all on one wrapping row (insights umbrella scope):
//   1. A command-style search (filters `origin_ref`) — mono font, search glyph, ⌘K hint.
//   2. A Status segmented control (All / Open / Acked / Resolved) — single-select.
//   3. A Severity toggle group with colored dots — single-select floor (critical → critical-only,
//      warning → warning+critical, info → all).
// Plus the pinned tag facets as dismissible chips with a Clear-all control.
//
// Built on shadcn primitives (ui-standards-scope): `Button` (segmented pills + toggle dots),
// `Input` (search), `Badge` (tag chips). Tokens only — destructive/warning/accent-2 carry the
// severity hues, no raw `red-…`/`amber-…` literals. One component per file (FILE-LAYOUT).

import { Plus, Search, X } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { ListQuery, Severity, Status } from "@/lib/insights/insights.types";

interface Props {
  filter: ListQuery;
  onChange: (next: ListQuery) => void;
}

const STATUSES: (Status | "all")[] = ["all", "open", "acked", "resolved"];
const SEVERITIES: Severity[] = ["critical", "warning", "info"];

/** The severity → dot tone + label tone (matches the row dots in `InsightsList`). */
function severityTone(s: Severity): { dot: string; text: string } {
  if (s === "critical") return { dot: "bg-destructive", text: "text-destructive" };
  if (s === "warning") return { dot: "bg-warning", text: "text-warning" };
  return { dot: "bg-accent-2", text: "text-accent-2" };
}

/** The filters toolbar. Emits a new filter on every change; the page's hook re-fetches. */
export function InsightFacets({ filter, onChange }: Props): JSX.Element {
  const activeStatus = filter.status ?? "all";
  const activeSeverity = filter.severity ?? null;

  function setStatus(s: Status | "all") {
    onChange({ ...filter, status: s === "all" ? undefined : s });
  }
  function setSeverity(s: Severity | null) {
    onChange({ ...filter, severity: s ?? undefined });
  }
  function setOriginRef(v: string) {
    onChange({ ...filter, origin_ref: v || undefined });
  }
  function removeTag(k: string) {
    if (!filter.tags) return;
    const next = { ...filter.tags };
    delete next[k];
    onChange({ ...filter, tags: Object.keys(next).length ? next : undefined });
  }
  function addTag(entry: string) {
    const [k, ...rest] = entry.split("=");
    const key = k.trim();
    const value = rest.join("=").trim();
    if (!key || !value) return;
    onChange({ ...filter, tags: { ...(filter.tags ?? {}), [key]: value } });
  }
  function clearAll() {
    onChange({ limit: filter.limit, cursor: filter.cursor });
  }

  const tagEntries = filter.tags ? Object.entries(filter.tags) : [];
  const hasAny =
    filter.status !== undefined ||
    filter.severity !== undefined ||
    filter.origin_ref !== undefined ||
    tagEntries.length > 0;

  return (
    <div className="space-y-3">
      {/* Search + grouped toggle row. */}
      <div className="flex flex-col gap-3 lg:flex-row lg:items-end">
        {/* Command-style search → origin_ref. */}
        <div className="flex-1">
          <label
            htmlFor="insight-search"
            className="mb-1 block text-[10px] font-medium uppercase tracking-tight text-muted"
          >
            Search producer / ref
          </label>
          <div className="relative">
            <Search
              size={15}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted"
            />
            <Input
              id="insight-search"
              type="text"
              value={filter.origin_ref ?? ""}
              onChange={(e) => setOriginRef(e.target.value)}
              placeholder="rule:… / flow:… / producer:edge-01"
              className="h-9 pl-9 font-mono text-xs"
              aria-label="search producer ref"
            />
          </div>
        </div>

        {/* Status segmented control. */}
        <fieldset className="lg:w-auto">
          <legend className="mb-1 block text-[10px] font-medium uppercase tracking-tight text-muted lg:sr-only">
            Status
          </legend>
          <div
            role="group"
            aria-label="filter by status"
            className="inline-flex items-center gap-0.5 rounded-md border border-border bg-panel p-0.5"
          >
            {STATUSES.map((s) => {
              const active = activeStatus === s;
              return (
                <Button
                  key={s}
                  type="button"
                  size="sm"
                  variant="ghost"
                  aria-pressed={active}
                  onClick={() => setStatus(s)}
                  className={cn(
                    "h-7 px-2.5 text-xs capitalize",
                    active
                      ? "bg-accent/15 font-semibold text-accent"
                      : "text-muted hover:text-fg",
                  )}
                >
                  {s}
                </Button>
              );
            })}
          </div>
        </fieldset>

        {/* Severity toggle dots. */}
        <fieldset className="lg:w-auto">
          <legend className="mb-1 block text-[10px] font-medium uppercase tracking-tight text-muted lg:sr-only">
            Severity ≥
          </legend>
          <div
            role="group"
            aria-label="filter by severity floor"
            className="inline-flex items-center gap-1 rounded-md border border-border bg-panel p-0.5"
          >
            {SEVERITIES.map((s, i) => {
              const active = activeSeverity === s;
              const tone = severityTone(s);
              return (
                <span key={s} className="flex items-center">
                  {i > 0 && <span className="mx-0.5 h-3 w-px bg-border" aria-hidden />}
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    aria-pressed={active}
                    onClick={() => setSeverity(active ? null : s)}
                    className={cn(
                      "h-7 gap-1.5 px-2.5 text-xs",
                      active ? "font-semibold text-fg" : "text-muted hover:text-fg",
                    )}
                  >
                    <span
                      className={cn(
                        "h-2 w-2 rounded-full",
                        tone.dot,
                        active ? "opacity-100" : "opacity-40",
                      )}
                      aria-hidden
                    />
                    <span className="capitalize">{s}</span>
                  </Button>
                </span>
              );
            })}
          </div>
        </fieldset>
      </div>

      {/* Pinned facets (tag chips) + add-facet input + clear-all. */}
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="text-[10px] font-medium uppercase tracking-tight text-muted">
          Pinned
        </span>
        {filter.origin_ref && (
          <Badge variant="secondary" className="gap-1 font-mono text-[10px]">
            ref:{filter.origin_ref}
            <Button
              type="button"
              variant="ghost"
              aria-label="clear producer ref filter"
              onClick={() => setOriginRef("")}
              className="h-3 w-3 p-0 hover:bg-transparent"
            >
              <X size={10} />
            </Button>
          </Badge>
        )}
        {tagEntries.map(([k, v]) => (
          <Badge key={k} variant="secondary" className="gap-1 font-mono text-[10px]">
            {k}:{v}
            <Button
              type="button"
              variant="ghost"
              aria-label={`remove facet ${k}`}
              onClick={() => removeTag(k)}
              className="h-3 w-3 p-0 hover:bg-transparent"
            >
              <X size={10} />
            </Button>
          </Badge>
        ))}
        {/* Inline add-facet input — `key=value` + Enter pins a tag the list AND-filters on. */}
        <Input
          type="text"
          aria-label="add tag facet"
          placeholder="key=value ↵"
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              const el = e.currentTarget;
              addTag(el.value);
              el.value = "";
            }
          }}
          className="h-6 w-40 font-mono text-[11px]"
        />
        {hasAny && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={clearAll}
            className="h-6 px-1.5 text-[11px] font-semibold text-accent"
          >
            <Plus size={12} /> Clear all
          </Button>
        )}
      </div>
    </div>
  );
}
