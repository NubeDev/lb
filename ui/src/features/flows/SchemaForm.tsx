// The schema-driven node-config form (flows-canvas scope, Decision 3 — "no hardcoded UI"). One
// generic `SchemaForm` renders EVERY node's settings from its descriptor's inline JSON-Schema 2020-12
// and validates with `ajv` before the canvas calls `flows.save`. A new extension node gets a config
// form for free; the editor learns nothing about it. styled with shadcn primitives (Input/Select) —
// NO per-node hand-coded form, NO MUI/AntD/RJSF default theme.
//
// Coverage is the load-bearing risk (scope "Risks"): the renderer covers the JSON-Schema subset
// descriptors actually use (object/string/number/integer/boolean/enum + nested object + array of
// scalars + required). A descriptor that exceeds it fails LOUD ("unsupported schema") — never a
// silently-dropped field. `ajv` compiles the schema once per render and reports the verbatim error
// path so the author fixes the right field.

import { useMemo } from "react";
import Ajv2020 from "ajv/dist/2020";
import addFormats from "ajv-formats";
import { CodeEditor, codeLanguageExtension } from "@/components/codeeditor";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { RhaiExampleLibrary } from "./examples/RhaiExampleLibrary";

// A code `format` may register a helper that renders BELOW its editor (keyed on the opaque schema
// hint, not a node type). Rhai gets its examples library; add an entry here to give another language
// its own helper. `onUse(body)` replaces the field's source with the chosen snippet.
const CODE_FIELD_HELPERS: Record<string, (p: { onUse: (body: string) => void }) => JSX.Element> = {
  rhai: RhaiExampleLibrary,
};

/** A JSON-Schema 2020-12 document (the descriptor's `config`). */
export type JsonSchema = Record<string, unknown>;

/** The result of validating `value` against `schema` with ajv. */
export interface ValidationResult {
  ok: boolean;
  /** `dataPath`-ish field keys → the verbatim error message (for inline field errors). */
  errors: Record<string, string>;
}

/** Compile + validate a value against a JSON-Schema 2020-12 doc. Returns the per-field error map.
 *  An empty/`{}` schema accepts anything (a node with no config). */
export function validateConfig(schema: JsonSchema | undefined, value: unknown): ValidationResult {
  if (!schema || Object.keys(schema).length === 0 || schema.type === undefined) {
    return { ok: true, errors: {} };
  }
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  addFormats(ajv);
  let validate;
  try {
    validate = ajv.compile(schema);
  } catch {
    // A descriptor schema ajv cannot compile is a host-side reject; surface it loudly here too.
    return { ok: false, errors: { "": "unsupported schema" } };
  }
  if (validate(value)) return { ok: true, errors: {} };
  const errors: Record<string, string> = {};
  for (const err of validate.errors ?? []) {
    const path = (err.instancePath || "").replace(/^\//, "").replace(/\//g, ".");
    const key = path || (err.params && "missingProperty" in err.params ? String(err.params.missingProperty) : "");
    errors[key] = err.message ?? "invalid";
  }
  return { ok: false, errors };
}

interface SchemaFormProps {
  schema: JsonSchema;
  value: Record<string, unknown>;
  onChange: (next: Record<string, unknown>) => void;
  /** Read-only render (an executed node during an active run — Decision 1). */
  disabled?: boolean;
  /** Per-field error map (from `validateConfig`) — renders inline. */
  errors?: Record<string, string>;
}

/** Render a JSON-Schema object form. The top-level schema MUST be an object (the descriptor
 *  contract: a node's config is a table of named fields). Nested objects render recursively. */
export function SchemaForm({ schema, value, onChange, disabled, errors = {} }: SchemaFormProps) {
  const props = (schema.properties ?? {}) as Record<string, JsonSchema>;
  const required = new Set((schema.required ?? []) as string[]);
  if (schema.type !== undefined && schema.type !== "object") {
    return <div className="text-xs text-destructive">unsupported schema (top-level must be object)</div>;
  }
  return (
    <div className="flex flex-col gap-3" aria-label="node config form">
      {Object.entries(props).map(([key, sub]) => (
        <Field
          key={key}
          name={key}
          label={key}
          schema={sub}
          required={required.has(key)}
          value={value[key]}
          disabled={disabled}
          error={errors[key]}
          onChange={(v) => onChange({ ...value, [key]: v })}
        />
      ))}
      {Object.keys(props).length === 0 ? (
        <div className="rounded-md border border-dashed border-border px-2 py-3 text-center text-xs text-muted">
          This node has no configuration.
        </div>
      ) : null}
    </div>
  );
}

interface FieldProps {
  name: string;
  label: string;
  schema: JsonSchema;
  required: boolean;
  value: unknown;
  disabled?: boolean;
  error?: string;
  onChange: (v: unknown) => void;
}

/** Render one field by its schema `type`. Strings/numbers/booleans/enums get a primitive input;
 *  nested objects recurse; arrays of scalars get a comma-joined input (the common descriptor shape).
 *  An unsupported type fails loud. */
function Field({ name, label, schema, required, value, disabled, error, onChange }: FieldProps) {
  const enumOpts = schema.enum as unknown[] | undefined;
  const type = schema.type as string | undefined;

  if (enumOpts) {
    return (
      <Labeled name={name} label={label} required={required} error={error} schema={schema}>
        <Select
          aria-label={name}
          disabled={disabled}
          value={String(value ?? "")}
          onChange={(e) => onChange(parseEnum(e.target.value, enumOpts))}
        >
          <option value="">{disabled ? "(set)" : "—"}</option>
          {enumOpts.map((o) => (
            <option key={String(o)} value={String(o)}>
              {String(o)}
            </option>
          ))}
        </Select>
      </Labeled>
    );
  }

  if (type === "boolean") {
    // Booleans render as one label+control ROW (the checkbox sits beside its label, not under it).
    return (
      <div className="flex flex-col gap-1">
        <label className="flex items-center gap-2 text-xs font-medium text-fg">
          {/* eslint-disable-next-line no-restricted-syntax -- no shadcn checkbox primitive; a native checkbox */}
          <input
            type="checkbox"
            aria-label={name}
            disabled={disabled}
            checked={Boolean(value)}
            onChange={(e) => onChange(e.target.checked)}
          />
          {label}
          {required ? <span className="text-destructive"> *</span> : null}
        </label>
        <Help schema={schema} />
        {error ? <span className="text-xs text-destructive">{error}</span> : null}
      </div>
    );
  }

  if (type === "integer" || type === "number") {
    return (
      <Labeled name={name} label={label} required={required} error={error} schema={schema}>
        <Input
          type="number"
          aria-label={name}
          disabled={disabled}
          value={value === undefined || value === null ? "" : String(value)}
          onChange={(e) => onChange(parseNum(e.target.value, type === "integer"))}
        />
      </Labeled>
    );
  }

  if (type === "string") {
    const codeLang = codeLanguageExtension(schema.format);
    if (codeLang) {
      const Helper = CODE_FIELD_HELPERS[schema.format as string] as
        | ((p: { onUse: (body: string) => void }) => JSX.Element)
        | undefined;
      return (
        <Labeled name={name} label={label} required={required} error={error} schema={schema}>
          <div className="overflow-hidden rounded-md border border-border">
            <CodeEditor
              ariaLabel={name}
              value={String(value ?? "")}
              onChange={(v) => onChange(v)}
              extensions={[codeLang]}
              height="12rem"
            />
          </div>
          {/* A format may register a below-editor helper (e.g. rhai → an examples library). Data-driven
              off the opaque `format` hint, so no field logic branches on a node type. */}
          {Helper && !disabled ? <Helper onUse={(body) => onChange(body)} /> : null}
        </Labeled>
      );
    }
    return (
      <Labeled name={name} label={label} required={required} error={error} schema={schema}>
        <Input
          type="text"
          aria-label={name}
          disabled={disabled}
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value)}
        />
      </Labeled>
    );
  }

  if (type === "object") {
    const subProps = (schema.properties ?? {}) as Record<string, JsonSchema>;
    const subReq = new Set((schema.required ?? []) as string[]);
    const sub = (value ?? {}) as Record<string, unknown>;
    return (
      <div className="flex flex-col gap-2 rounded-md border border-border p-2">
        <span className="text-xs font-semibold text-fg">{label}</span>
        {Object.entries(subProps).map(([k, s]) => (
          <Field
            key={k}
            name={`${name}.${k}`}
            label={k}
            schema={s}
            required={subReq.has(k)}
            value={sub[k]}
            disabled={disabled}
            error={error}
            onChange={(v) => onChange({ ...sub, [k]: v })}
          />
        ))}
      </div>
    );
  }

  if (type === "array") {
    return (
      <Labeled name={name} label={label} required={required} error={error} schema={schema}>
        <Input
          type="text"
          aria-label={name}
          disabled={disabled}
          placeholder="comma-separated"
          value={Array.isArray(value) ? (value as unknown[]).map((v) => String(v)).join(",") : ""}
          onChange={(e) => onChange(parseArray(e.target.value))}
        />
      </Labeled>
    );
  }

  // A descriptor that exceeds the covered subset — fail LOUD (Decision 3 guardrail).
  return (
    <div className="text-xs text-destructive">
      unsupported schema for `{name}` (type `{type ?? "?"}`)
    </div>
  );
}

function Labeled({
  name,
  label,
  required,
  error,
  schema,
  children,
}: {
  name: string;
  label: string;
  required: boolean;
  error?: string;
  schema?: JsonSchema;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={name} className="text-xs font-medium text-fg">
        {label}
        {required ? <span className="text-destructive"> *</span> : null}
      </label>
      {children}
      <Help schema={schema} />
      {error ? <span className="text-xs text-destructive">{error}</span> : null}
    </div>
  );
}

/** The field's help line — the schema's own `description`, rendered under the control (flow-ui-polish:
 *  the descriptor already carries the words; the form just stopped dropping them). */
function Help({ schema }: { schema?: JsonSchema }) {
  const text = schema?.description;
  if (typeof text !== "string" || text.length === 0) return null;
  return <span className="text-[11px] leading-snug text-muted">{text}</span>;
}

function parseNum(s: string, integer: boolean): number | undefined {
  if (s === "") return undefined;
  const n = integer ? parseInt(s, 10) : Number(s);
  return Number.isFinite(n) ? n : undefined;
}

function parseEnum(s: string, opts: unknown[]): unknown {
  return opts.find((o) => String(o) === s);
}

function parseArray(s: string): unknown[] {
  return s.split(",").map((p) => p.trim()).filter((p) => p.length > 0);
}

/** A hook form-flavoured helper: validate `value` against `schema` on each change, returning the
 *  per-field error map + an `ok` flag the Save button gates on (no fake accept). */
export function useSchemaValidity(schema: JsonSchema, value: Record<string, unknown>): ValidationResult {
  return useMemo(() => validateConfig(schema, value), [schema, value]);
}
