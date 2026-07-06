// The LOCAL-store schema loader for the panel-builder Query tab (query-builder-common scope). One
// responsibility: load `store.schema` once when the editor needs it + project a stable empty
// `Schema` while loading or on deny. The editor (`SqlQueryEditor`) is agnostic to WHERE the schema
// comes from — it just consumes `Schema`; this hook is the surreal/host side of that contract.
// Mirrors the (now-removed) one-shot `readSchema()` effect that used to live inside `SqlQueryEditor`,
// lifted here so the editor stays transport-agnostic (the host owns the load).
//
// Honesty contract (rule 9 / system-catalog scope): a deny or load failure collapses to an EMPTY
// `tables: []` — never a fabricated roster. The Code half of the editor still works without schema
// (the author types raw SurrealQL); only the dropdowns are empty.

import { useEffect, useState } from "react";

import { readSchema, type Schema } from "@/lib/schema";

const EMPTY: Schema = { tables: [] };

/** Load the workspace's local-store schema once on mount. Re-fetches when `enabled` flips true
 *  (the Query tab is lazy: a restored EMPTY builder tab does not fire explorer verbs on mount). */
export function useLocalSchema(enabled: boolean): Schema {
  const [schema, setSchema] = useState<Schema>(EMPTY);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    readSchema()
      .then((s) => {
        if (!cancelled) setSchema(s);
      })
      .catch(() => {
        if (!cancelled) setSchema(EMPTY);
      });
    return () => {
      cancelled = true;
    };
  }, [enabled]);

  return schema;
}
