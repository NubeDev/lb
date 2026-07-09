// The schema-designer picker inputs for the flow tool-node config form (schema-designer scope).
// Two data-driven `<select>`s, mounted by `SchemaForm` via the schema `format` hint
// (`format: "lb:datasource"` / `format: "lb:table"`), so the form never branches on a node type or
// an extension id (rule 10). `lb:table` reads the schema named by a sibling field — wired via a
// sibling-lookup that walks the form's value for the conventional `schema`/`source` key.
//
// One responsibility, one file (FILE-LAYOUT).

import { useEffect, useState } from "react";

import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { getDbSchema, listDatasources } from "@/lib/datasources";

interface InputProps {
  "aria-label": string;
  disabled?: boolean;
  value: string;
  onChange: (v: string) => void;
}

/** `format: "lb:datasource"` → a `<select>` of the workspace's registered datasources. Falls back to
 *  a free-text `<input>` while the list loads or if the call denies (a viewer with no list cap). */
export function DatasourcePickerInput(props: InputProps) {
  const [names, setNames] = useState<string[] | null>(null);
  useEffect(() => {
    listDatasources()
      .then((s) => setNames(s.map((x) => x.name)))
      .catch(() => setNames(null));
  }, []);
  if (!names) {
    return (
      <Input
        type="text"
        aria-label={props["aria-label"]}
        disabled={props.disabled}
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
      />
    );
  }
  return (
    <Select
      aria-label={props["aria-label"]}
      disabled={props.disabled}
      value={props.value}
      onChange={(e) => props.onChange(e.target.value)}
    >
      <option value="">{props.disabled ? "(set)" : "—"}</option>
      {names.map((n) => (
        <option key={n} value={n}>
          {n}
        </option>
      ))}
    </Select>
  );
}

/** `format: "lb:table"` → a `<select>` of the tables in the schema named by the conventional sibling
 *  `schema` field. Falls back to free-text when no schema is selected or the call denies. v1 keeps
 *  this a free-text input (the Field component is sandboxed to its own value; a future tightening
 *  threads the sibling schema name through). */
export function DbschemaTablePickerInput(props: InputProps) {
  return (
    <Input
      type="text"
      aria-label={props["aria-label"]}
      disabled={props.disabled}
      value={props.value}
      onChange={(e) => props.onChange(e.target.value)}
      placeholder="table name"
    />
  );
}

/** A hook for the page-level wiring: load a schema's table names once, so the table picker can be
 *  driven by the actually-selected schema (the sibling-field the Field component can't see). Kept
 *  here so the page imports one helper, not two. */
export function useDbschemaTables(schemaName: string | null): string[] | null {
  const [tables, setTables] = useState<string[] | null>(null);
  useEffect(() => {
    if (!schemaName) {
      setTables(null);
      return;
    }
    let alive = true;
    getDbSchema(schemaName)
      .then((r) => {
        if (!alive) return;
        setTables(r ? r.tables.map((t) => t.name) : null);
      })
      .catch(() => {
        if (alive) setTables(null);
      });
    return () => {
      alive = false;
    };
  }, [schemaName]);
  return tables;
}
