// A client-side **series schema** (data-console scope). The backend is schemaless — a `Sample` is
// `{series, payload:any-JSON, labels, ts, seq}` — so a "schema" is a UI definition that (a) drives a
// typed write form and (b) is persisted as a record so it reloads when the series is revisited. It is
// a recursive field tree: a field is a scalar, or an `object`/`array` carrying nested fields.

/** The value types a field can hold. `object`/`array` are containers carrying nested `fields`. */
export type FieldType = "number" | "string" | "boolean" | "timestamp" | "object" | "array";

/** One field in a schema. `fields` is present (and meaningful) only for `object` (its keys) and
 *  `array` (the shape of each element — modeled as the element's fields). */
export interface Field {
  /** The field key (an identifier within its parent object). */
  name: string;
  type: FieldType;
  /** Nested fields for `object`/`array`. Undefined/empty for scalars. */
  fields?: Field[];
}

/** A named series + its field tree. `description` is free text shown in the explorer. */
export interface SeriesSchema {
  series: string;
  description?: string;
  fields: Field[];
}

/** The container types that hold nested fields. */
export function isContainer(type: FieldType): boolean {
  return type === "object" || type === "array";
}

/** A fresh scalar field with a placeholder name. */
export function newField(type: FieldType = "number"): Field {
  return isContainer(type) ? { name: "", type, fields: [] } : { name: "", type };
}

/** A zero/empty value for a field type — the initial value the generated form starts a field at. */
export function emptyValue(field: Field): unknown {
  switch (field.type) {
    case "number":
    case "timestamp":
      return 0;
    case "string":
      return "";
    case "boolean":
      return false;
    case "object": {
      const obj: Record<string, unknown> = {};
      for (const f of field.fields ?? []) if (f.name) obj[f.name] = emptyValue(f);
      return obj;
    }
    case "array":
      return [];
  }
}

/** Build the initial payload object for a schema (every top-level field at its empty value). */
export function emptyPayload(schema: SeriesSchema): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const f of schema.fields) if (f.name) out[f.name] = emptyValue(f);
  return out;
}

/** A short, human label for a field's type (used in the builder + the explorer). */
export const TYPE_LABELS: Record<FieldType, string> = {
  number: "Number",
  string: "Text",
  boolean: "Boolean",
  timestamp: "Timestamp",
  object: "Object",
  array: "List",
};
