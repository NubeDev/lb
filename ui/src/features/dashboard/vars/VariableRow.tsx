// One variable's editor row (advanced-variables scope) — a sectioned form, NOT a flat wall of controls.
// A row is either COLLAPSED (a scannable summary line: `$site · Query · multi`, with edit/reorder/remove)
// or EXPANDED into labelled sections: General (name/label/type), Values/Source (type-specific), Selection
// (multi/all/required), and a disclosure-gated Advanced (regex/sort/refresh/allValue/hide). One variable
// is expanded at a time — the rest stay collapsed so the panel scans. FILE-LAYOUT: the row is its own file
// (the editor was over budget); the sections are small local views, one responsibility each.

import { useState } from "react";
import {
  Trash2,
  ArrowUp,
  ArrowDown,
  ChevronDown,
  ChevronRight,
  Tag,
  Database,
  MousePointerClick,
  SlidersHorizontal,
  X,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { Icon, IconPicker } from "@/lib/icons";
import type { Variable, VariableType } from "@/lib/vars";
import { PickerGroup, READ_SOURCE_GROUPS, type SourceEntry } from "../builder/sourcePicker";
import { VariableAdvancedFields } from "./VariableAdvancedFields";
import { variableTypeMeta, VARIABLE_TYPES } from "./variableTypeMeta";
import { variableSummary } from "./variableSummary";

const FIELD =
  "h-8 w-full rounded-md border border-border bg-bg px-2.5 text-xs text-fg transition-colors focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

/** The patch when an existing variable's type changes — a `datasource` keeps its fixed resolver tool. */
function typeChangePatch(type: VariableType): Partial<Variable> {
  if (type === "datasource") return { type, query: { tool: "datasource.list" } };
  return { type };
}

export function VariableRow({
  variable,
  entries,
  expanded,
  onToggle,
  onChange,
  onRemove,
  onMoveUp,
  onMoveDown,
}: {
  variable: Variable;
  entries: SourceEntry[];
  expanded: boolean;
  onToggle: () => void;
  onChange: (patch: Partial<Variable>) => void;
  onRemove: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}) {
  const meta = variableTypeMeta(variable.type);
  const TypeIcon = meta.icon;

  return (
    <div
      className="overflow-hidden rounded-lg border border-border bg-panel/40"
      aria-label={`variable row ${variable.name}`}
    >
      {/* Header — always visible; click to expand/collapse. */}
      <div className="flex items-center gap-2 px-2.5 py-2">
        {/* eslint-disable-next-line no-restricted-syntax -- a disclosure header is a custom clickable
            surface (chevron + type icon + summary), not a shadcn Button use case */}
        <button
          type="button"
          aria-label={expanded ? `collapse ${variable.name}` : `expand ${variable.name}`}
          onClick={onToggle}
          className="flex min-w-0 flex-1 items-center gap-2 rounded-md text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/25"
        >
          {expanded ? (
            <ChevronDown size={14} className="shrink-0 text-muted" />
          ) : (
            <ChevronRight size={14} className="shrink-0 text-muted" />
          )}
          <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-border bg-bg text-muted">
            <TypeIcon size={13} strokeWidth={1.75} />
          </span>
          <span className="min-w-0 flex-1">
            <span className="truncate text-[13px] font-medium text-fg">
              ${variable.name || "unnamed"}
            </span>
            {!expanded && <span className="ml-2 truncate text-[11px] text-muted">{variableSummary(variable)}</span>}
          </span>
        </button>
        <div className="flex shrink-0 items-center gap-0.5">
          <Button
            aria-label="move variable up"
            size="icon"
            variant="ghost"
            className="h-7 w-7 text-muted"
            onClick={onMoveUp}
          >
            <ArrowUp size={13} />
          </Button>
          <Button
            aria-label="move variable down"
            size="icon"
            variant="ghost"
            className="h-7 w-7 text-muted"
            onClick={onMoveDown}
          >
            <ArrowDown size={13} />
          </Button>
          <Button
            aria-label="remove variable"
            size="icon"
            variant="ghost"
            className="h-7 w-7 text-muted hover:bg-red-500/10 hover:text-red-500"
            onClick={onRemove}
          >
            <Trash2 size={13} />
          </Button>
        </div>
      </div>

      {expanded && (
        <div className="flex flex-col gap-4 border-t border-border/70 bg-bg/30 px-3 pb-3 pt-3">
          <GeneralSection variable={variable} onChange={onChange} />
          <ValuesSection variable={variable} entries={entries} onChange={onChange} />
          <SelectionSection variable={variable} onChange={onChange} />
          <AdvancedSection variable={variable} onChange={onChange} />
        </div>
      )}
    </div>
  );
}

/** A labelled section: an icon + title header, then its controls. The one layout primitive the row uses. */
function Section({
  icon: Icon,
  title,
  children,
}: {
  icon: typeof Tag;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted">
        <Icon size={12} strokeWidth={2} />
        {title}
      </div>
      {children}
    </section>
  );
}

/** A field with a small label above the control (the form's one field primitive). */
function Field({ label, htmlFor, children }: { label: string; htmlFor?: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-1 flex-col gap-1" htmlFor={htmlFor}>
      <span className="text-[11px] text-muted">{label}</span>
      {children}
    </label>
  );
}

/** The variable's bar icon: a compact square trigger showing the current icon (or a placeholder), which
 *  toggles an INLINE icon picker (in flow, so a scrolling/overflow-hidden ancestor can't clip it). Stores
 *  an opaque icon-lib name; a "clear" removes it. Returns a fragment so the trigger sits on the fields row
 *  and the revealed picker spans the full width below. */
function IconField({ icon, onChange }: { icon?: string; onChange: (icon: string | undefined) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <div className="flex flex-col gap-1">
        <span className="text-[11px] text-muted">Icon</span>
        <div className="flex items-center gap-1">
          <Button
            type="button"
            aria-label="variable icon"
            size="icon"
            variant="outline"
            className={cn("h-8 w-8", open && "border-accent text-accent")}
            onClick={() => setOpen((o) => !o)}
          >
            <Icon name={icon} fallback="image" className="size-4" aria-hidden />
          </Button>
          {icon && (
            <Button
              type="button"
              aria-label="clear variable icon"
              size="icon"
              variant="ghost"
              className="h-8 w-8 text-muted"
              onClick={() => onChange(undefined)}
            >
              <X className="size-3.5" />
            </Button>
          )}
        </div>
      </div>
      {open && (
        <div
          className="w-full rounded-lg border border-border bg-panel/60 p-2.5"
          aria-label="variable icon picker"
        >
          <IconPicker
            value={icon}
            autoFocus
            columns={8}
            pageSize={24}
            onSelect={(name) => {
              onChange(name);
              setOpen(false);
            }}
          />
        </div>
      )}
    </>
  );
}

function GeneralSection({
  variable,
  onChange,
}: {
  variable: Variable;
  onChange: (patch: Partial<Variable>) => void;
}) {
  return (
    <Section icon={Tag} title="General">
      <div className="flex flex-wrap items-end gap-2">
        <IconField icon={variable.icon} onChange={(icon) => onChange({ icon })} />
        <Field label="Name">
          {/* eslint-disable-next-line no-restricted-syntax -- token-bound native input; no shadcn Input variant */}
          <input
            aria-label="variable name"
            className={FIELD}
            placeholder="name"
            value={variable.name}
            onChange={(e) => onChange({ name: e.target.value.replace(/[^\w.]/g, "") })}
          />
        </Field>
        <Field label="Label (optional)">
          {/* eslint-disable-next-line no-restricted-syntax -- token-bound native input */}
          <input
            aria-label="variable label"
            className={FIELD}
            placeholder="shown on the bar"
            value={variable.label ?? ""}
            onChange={(e) => onChange({ label: e.target.value })}
          />
        </Field>
      </div>
      <Field label="Type">
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive; token-bound */}
        <select
          aria-label="variable type"
          className={FIELD}
          value={variable.type}
          onChange={(e) => onChange(typeChangePatch(e.target.value as VariableType))}
        >
          {VARIABLE_TYPES.map((t) => (
            <option key={t.type} value={t.type}>
              {t.title}
            </option>
          ))}
        </select>
      </Field>
      <p className="text-[11px] leading-relaxed text-muted">{variableTypeMeta(variable.type).description}</p>
    </Section>
  );
}

// A comma-list input that keeps the raw typed text locally (so a mid-typed comma isn't eaten by a live
// split/re-join), emitting the parsed list on every change.
function ListField({
  label,
  ariaLabel,
  placeholder,
  values,
  onChange,
}: {
  label: string;
  ariaLabel: string;
  placeholder: string;
  values: string[];
  onChange: (values: string[]) => void;
}) {
  const [text, setText] = useState(values.join(", "));
  return (
    <Field label={label}>
      {/* eslint-disable-next-line no-restricted-syntax -- token-bound native input */}
      <input
        aria-label={ariaLabel}
        className={FIELD}
        placeholder={placeholder}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          onChange(e.target.value.split(",").map((s) => s.trim()).filter(Boolean));
        }}
      />
    </Field>
  );
}

function ValuesSection({
  variable,
  entries,
  onChange,
}: {
  variable: Variable;
  entries: SourceEntry[];
  onChange: (patch: Partial<Variable>) => void;
}) {
  const isQuery = variable.type === "query" || variable.type === "source";
  const queryEntries = entries.filter((e) => e.source && !e.writes);
  const selectedEntryId =
    entries.find(
      (e) =>
        e.source?.tool === variable.query?.tool &&
        (variable.type === "source" ? e.group === "extension" || e.group === "series" : true),
    )?.id ?? "";

  // `datasource` resolves against a fixed tool (no picker); `const`/`text` have their own inputs below.
  const title =
    variable.type === "datasource" ? "Data source" : isQuery ? "Source" : "Values";

  return (
    <Section icon={Database} title={title}>
      {isQuery && (
        <Field label="Read source">
          {/* eslint-disable-next-line no-restricted-syntax -- token-bound native select; the friendly source picker */}
          <select
            aria-label="variable query source"
            className={FIELD}
            value={selectedEntryId}
            onChange={(e) => {
              const entry = entries.find((x) => x.id === e.target.value);
              if (entry?.source) onChange({ query: { tool: entry.source.tool, args: entry.source.args } });
            }}
          >
            <option value="">— pick a source —</option>
            {READ_SOURCE_GROUPS.map(({ group, label }) => (
              <PickerGroup key={group} entries={queryEntries} group={group} label={label} />
            ))}
          </select>
        </Field>
      )}
      {variable.type === "datasource" && (
        <p className="text-[11px] text-muted">Options are the workspace's registered datasources.</p>
      )}
      {variable.type === "custom" && (
        <ListField
          label="Values (comma-separated)"
          ariaLabel="variable custom values"
          placeholder="prod, staging, dev  —  or  West : WST"
          values={variable.custom ?? []}
          onChange={(custom) => onChange({ custom })}
        />
      )}
      {variable.type === "interval" && (
        <ListField
          label="Intervals (comma-separated)"
          ariaLabel="variable interval values"
          placeholder="1m, 5m, 1h"
          values={variable.interval ?? []}
          onChange={(interval) => onChange({ interval })}
        />
      )}
      {variable.type === "text" && (
        <Field label="Default text">
          {/* eslint-disable-next-line no-restricted-syntax -- token-bound native input */}
          <input
            aria-label="variable text default"
            className={FIELD}
            placeholder="default text"
            value={variable.text ?? ""}
            onChange={(e) => onChange({ text: e.target.value })}
          />
        </Field>
      )}
      {variable.type === "const" && (
        <Field label="Fixed value (hidden)">
          {/* eslint-disable-next-line no-restricted-syntax -- token-bound native input */}
          <input
            aria-label="variable const value"
            className={FIELD}
            placeholder="fixed value"
            value={variable.const ?? ""}
            onChange={(e) => onChange({ const: e.target.value })}
          />
        </Field>
      )}
    </Section>
  );
}

/** A labelled checkbox chip. */
function Check({
  ariaLabel,
  label,
  checked,
  onChange,
  title,
}: {
  ariaLabel: string;
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  title?: string;
}) {
  return (
    <label className="flex items-center gap-1.5 text-[11px] text-fg" title={title}>
      {/* eslint-disable-next-line no-restricted-syntax -- token-bound native checkbox */}
      <input
        aria-label={ariaLabel}
        type="checkbox"
        className="h-3.5 w-3.5 rounded border-border accent-accent"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      {label}
    </label>
  );
}

function SelectionSection({
  variable,
  onChange,
}: {
  variable: Variable;
  onChange: (patch: Partial<Variable>) => void;
}) {
  const hasOptions = variable.type === "query" || variable.type === "source" || variable.type === "datasource";
  return (
    <Section icon={MousePointerClick} title="Selection">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        {hasOptions && (
          <>
            <Check
              ariaLabel="variable multi"
              label="Multi-value"
              checked={!!variable.multi}
              onChange={(v) => onChange({ multi: v })}
            />
            <Check
              ariaLabel="variable include all"
              label="Include “All”"
              checked={!!variable.includeAll}
              onChange={(v) => onChange({ includeAll: v })}
            />
          </>
        )}
        <Check
          ariaLabel="variable required page parameter"
          label="Required (page parameter)"
          title="A required variable must be picked before the page loads — this makes it a template."
          checked={!!variable.required}
          onChange={(v) => onChange({ required: v })}
        />
      </div>
    </Section>
  );
}

/** The Advanced disclosure — collapsed by default; opens to the regex/sort/refresh/allValue/hide panel. */
function AdvancedSection({
  variable,
  onChange,
}: {
  variable: Variable;
  onChange: (patch: Partial<Variable>) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <section className="flex flex-col gap-2">
      <Button
        aria-label="toggle advanced options"
        size="sm"
        variant="ghost"
        className="-ml-1.5 h-7 w-fit gap-1 px-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted"
        onClick={() => setOpen((o) => !o)}
      >
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <SlidersHorizontal size={12} strokeWidth={2} />
        Advanced
      </Button>
      {open && (
        <div className="rounded-md border border-border/70 bg-panel/40 p-2.5">
          <VariableAdvancedFields variable={variable} onChange={onChange} />
        </div>
      )}
    </section>
  );
}
