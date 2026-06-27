// The generated typed write form (data-console scope) — renders inputs FROM a schema so writing a
// sample is a typed, structured form instead of raw JSON. Recursive: a scalar field is one input
// (number/text/checkbox/timestamp), an `object` nests a labeled group, an `array` is a repeatable
// list of its element shape (add/remove rows). The assembled value is a real JSON `payload` for
// `ingest.write` — the schema only shapes the form, never the backend.

import { Plus, X } from "lucide-react";

import {
  type Field,
  emptyValue,
  isContainer,
} from "@/lib/ingest/schema.types";

/** Render the fields of an object value as a labeled group of typed inputs. */
export function SchemaFields({
  fields,
  value,
  onChange,
  depth = 0,
}: {
  fields: Field[];
  value: Record<string, unknown>;
  onChange: (next: Record<string, unknown>) => void;
  depth?: number;
}) {
  const set = (name: string, v: unknown) => onChange({ ...value, [name]: v });

  return (
    <div className={depth > 0 ? "ml-3 flex flex-col gap-2 border-l border-border/60 pl-3" : "flex flex-col gap-2.5"}>
      {fields
        .filter((f) => f.name)
        .map((f) => (
          <FieldInput
            key={f.name}
            field={f}
            value={value[f.name]}
            onChange={(v) => set(f.name, v)}
            depth={depth}
          />
        ))}
    </div>
  );
}

function FieldInput({
  field,
  value,
  onChange,
  depth,
}: {
  field: Field;
  value: unknown;
  onChange: (v: unknown) => void;
  depth: number;
}) {
  const label = (
    <span className="text-xs font-medium text-muted">
      {field.name}
      {isContainer(field.type) && (
        <span className="ml-1.5 text-[10px] uppercase tracking-wide text-muted/50">
          {field.type}
        </span>
      )}
    </span>
  );

  if (field.type === "object") {
    const obj = (value && typeof value === "object" ? value : {}) as Record<string, unknown>;
    return (
      <div>
        <div className="mb-1.5">{label}</div>
        <SchemaFields
          fields={field.fields ?? []}
          value={obj}
          onChange={onChange as (v: Record<string, unknown>) => void}
          depth={depth + 1}
        />
      </div>
    );
  }

  if (field.type === "array") {
    const items = Array.isArray(value) ? value : [];
    const elementFields = field.fields ?? [];
    return (
      <div>
        <div className="mb-1.5 flex items-center gap-2">
          {label}
          <span className="text-[11px] text-muted/60">· {items.length}</span>
        </div>
        <div className="ml-3 flex flex-col gap-2 border-l border-border/60 pl-3">
          {items.map((item, i) => (
            <div key={i} className="group relative rounded border border-border/60 bg-bg/40 p-2">
              <button
                type="button"
                aria-label={`remove ${field.name} item ${i + 1}`}
                onClick={() => onChange(items.filter((_, j) => j !== i))}
                className="absolute right-1 top-1 rounded p-0.5 text-muted/50 opacity-0 transition group-hover:opacity-100 hover:text-red-400 focus-visible:opacity-100"
              >
                <X size={13} />
              </button>
              <SchemaFields
                fields={elementFields}
                value={(item && typeof item === "object" ? item : {}) as Record<string, unknown>}
                onChange={(v) => onChange(items.map((it, j) => (j === i ? v : it)))}
                depth={depth + 1}
              />
            </div>
          ))}
          <button
            type="button"
            onClick={() =>
              onChange([
                ...items,
                Object.fromEntries(
                  elementFields.filter((f) => f.name).map((f) => [f.name, emptyValue(f)]),
                ),
              ])
            }
            className="inline-flex items-center gap-1 self-start rounded px-1.5 py-1 text-xs text-muted transition-colors hover:text-accent focus-visible:text-accent focus-visible:outline-none"
          >
            <Plus size={13} /> Add item
          </button>
        </div>
      </div>
    );
  }

  // Scalars — a labeled typed input.
  return (
    <label className="flex flex-col gap-1">
      {label}
      <ScalarInput field={field} value={value} onChange={onChange} />
    </label>
  );
}

function ScalarInput({
  field,
  value,
  onChange,
}: {
  field: Field;
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  const cls =
    "rounded border border-border bg-bg px-2 py-1 text-sm placeholder:text-muted/50 focus-visible:border-accent focus-visible:outline-none";

  if (field.type === "boolean") {
    return (
      <button
        type="button"
        role="switch"
        aria-checked={!!value}
        aria-label={field.name}
        onClick={() => onChange(!value)}
        className={`relative h-5 w-9 rounded-full border transition-colors ${
          value ? "border-accent bg-accent/30" : "border-border bg-bg"
        }`}
      >
        <span
          className={`absolute top-0.5 h-3.5 w-3.5 rounded-full transition-transform ${
            value ? "left-0.5 translate-x-4 bg-accent" : "left-0.5 bg-muted"
          }`}
        />
      </button>
    );
  }

  if (field.type === "number" || field.type === "timestamp") {
    return (
      <input
        type="number"
        aria-label={field.name}
        placeholder={field.type === "timestamp" ? "unix seconds" : "0"}
        value={value === undefined || value === null ? "" : String(value)}
        onChange={(e) => onChange(e.target.value === "" ? null : Number(e.target.value))}
        className={cls}
      />
    );
  }

  return (
    <input
      type="text"
      aria-label={field.name}
      value={value === undefined || value === null ? "" : String(value)}
      onChange={(e) => onChange(e.target.value)}
      className={cls}
    />
  );
}
