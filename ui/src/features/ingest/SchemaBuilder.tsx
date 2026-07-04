// The recursive schema builder (data-console scope) — the nested, typed field editor at the heart of
// the Create-series wizard. Each row is a field: a name input + a type picker; `object`/`array` fields
// reveal a nested builder for their sub-fields, indented behind a connecting rail so depth reads at a
// glance. Add/remove per level. Pure controlled component — the parent owns the `Field[]` state.
//
// Design: depth is shown by a left rail (not nested cards — nested cards are always wrong). The type
// picker is a compact segmented control on hover/focus affordances the rest of the app already uses
// (amber accent, `bg-bg` inputs, small text scale).

import { ChevronRight, GripVertical, Plus, X } from "lucide-react";

import {
  type Field,
  type FieldType,
  TYPE_LABELS,
  isContainer,
  newField,
} from "@/lib/ingest/schema.types";

const TYPES: FieldType[] = ["number", "string", "boolean", "timestamp", "object", "array"];

interface Props {
  fields: Field[];
  onChange: (fields: Field[]) => void;
  /** Nesting depth (0 at the root) — drives the indent rail. */
  depth?: number;
}

export function SchemaBuilder({ fields, onChange, depth = 0 }: Props) {
  const update = (i: number, next: Field) =>
    onChange(fields.map((f, j) => (j === i ? next : f)));
  const remove = (i: number) => onChange(fields.filter((_, j) => j !== i));
  const add = () => onChange([...fields, newField("number")]);

  return (
    <div className={depth > 0 ? "ml-3 border-l border-border/60 pl-3" : ""}>
      <ul className="flex flex-col gap-1.5">
        {fields.map((field, i) => (
          <FieldRow
            key={i}
            field={field}
            depth={depth}
            onChange={(next) => update(i, next)}
            onRemove={() => remove(i)}
          />
        ))}
      </ul>

      <button
        type="button"
        onClick={add}
        className="mt-2 inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted transition-colors hover:text-accent focus-visible:text-accent focus-visible:outline-none"
      >
        <Plus size={13} />
        {depth === 0 ? "Add field" : "Add sub-field"}
      </button>
    </div>
  );
}

function FieldRow({
  field,
  depth,
  onChange,
  onRemove,
}: {
  field: Field;
  depth: number;
  onChange: (f: Field) => void;
  onRemove: () => void;
}) {
  const container = isContainer(field.type);

  const setType = (type: FieldType) => {
    // Keep nested fields when switching between the two container types; drop them for a scalar.
    onChange({
      name: field.name,
      type,
      fields: isContainer(type) ? (field.fields ?? []) : undefined,
    });
  };

  return (
    <li>
      <div className="group flex items-center gap-1.5">
        {/* Depth caret — a quiet structural cue, brightens for containers. */}
        <ChevronRight
          size={13}
          className={`shrink-0 transition-transform ${
            container ? "rotate-90 text-accent/70" : "text-border"
          }`}
          aria-hidden
        />

        <input
          aria-label={`field name${depth > 0 ? ` (depth ${depth})` : ""}`}
          placeholder={field.type === "array" ? "list name" : "field name"}
          value={field.name}
          onChange={(e) => onChange({ ...field, name: e.target.value })}
          className="min-w-0 flex-1 rounded-md border border-border bg-bg px-2 py-1 text-sm placeholder:text-muted/60 focus-visible:border-accent focus-visible:outline-none"
        />

        <TypePicker value={field.type} onChange={setType} />

        <button
          type="button"
          aria-label={`remove ${field.name || "field"}`}
          onClick={onRemove}
          className="shrink-0 rounded-md p-1 text-muted/50 opacity-0 transition group-hover:opacity-100 focus-visible:opacity-100 focus-visible:outline-none hover:text-red-400"
        >
          <X size={14} />
        </button>
      </div>

      {/* A container reveals its nested builder; a short hint when empty. */}
      {container && (
        <div className="mt-1.5">
          {field.type === "array" && (
            <div className="ml-3 mb-1 pl-3 text-[11px] text-muted/70">
              each item has these fields
            </div>
          )}
          <SchemaBuilder
            fields={field.fields ?? []}
            depth={depth + 1}
            onChange={(fields) => onChange({ ...field, fields })}
          />
        </div>
      )}
    </li>
  );
}

/** A compact type picker — a native select styled to the app's input language (escapes overflow
 *  clipping that a custom popover would hit inside the scrolling wizard body). */
function TypePicker({
  value,
  onChange,
}: {
  value: FieldType;
  onChange: (t: FieldType) => void;
}) {
  return (
    <div className="relative shrink-0">
      <select
        aria-label="field type"
        value={value}
        onChange={(e) => onChange(e.target.value as FieldType)}
        className={`appearance-none rounded-md border border-border bg-panel py-1 pl-2 pr-6 text-xs font-medium transition-colors focus-visible:border-accent focus-visible:outline-none ${
          isContainer(value) ? "text-accent" : "text-fg"
        }`}
      >
        {TYPES.map((t) => (
          <option key={t} value={t} className="bg-panel text-fg">
            {TYPE_LABELS[t]}
          </option>
        ))}
      </select>
      <GripVertical
        size={11}
        className="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 rotate-90 text-muted/50"
        aria-hidden
      />
    </div>
  );
}
