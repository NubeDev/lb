// The Grafana-style Builder⇄Code SQL editor (widget-builder Slice C) — ported from Grafana's
// `QueryEditor.tsx`. It switches between the visual `VisualEditor` (Builder) and the raw `RawEditor`
// (Code) by `editorMode`, keeping the two in sync:
//   - Builder edits the typed `SqlBuilderQuery`; every change regenerates the raw SurrealQL via
//     `toSurrealQL` (Builder is the source of truth in Builder mode).
//   - Code edits the raw string directly (the source of truth in Code mode).
//   - Builder→Code: free (regenerate the string).
//   - Code→Builder: CONFIRM first — hand-edited SQL may not round-trip, so switching back can clobber
//     it (Grafana's behaviour). On confirm we keep the existing builder query (the last visual state).
//
// The Table/Column dropdowns come from `store.schema` (Slice A), read once at authoring time in the
// trusted shell. The cell stores BOTH the raw string (what `store.query` runs) AND the builder query
// (when in Builder mode), so reopening returns to the builder. Builder mode can only ever generate a
// SELECT; Code mode is still parse-allowlisted to SELECT by `store.query`.

import { useEffect, useState } from "react";

import { readSchema, type Schema } from "@/lib/schema";
import { emptyQuery, emptySqlSource, type SqlSourceState } from "@/lib/panel-kit/sql/query";
import { RawEditor } from "./RawEditor";
import { SqlQueryHeader } from "./SqlQueryHeader";
import { toSurrealQL } from "@/lib/panel-kit/sql/toSurrealQL";
import { VisualEditor } from "./VisualEditor";

interface Props {
  /** The current SQL source state (mode + raw SQL + builder query + format). */
  value: SqlSourceState;
  /** Called with the next state on any edit. */
  onChange: (state: SqlSourceState) => void;
}

/** The Builder⇄Code SQL editor. */
export function SqlQueryEditor({ value, onChange }: Props) {
  const [schema, setSchema] = useState<Schema>({ tables: [] });

  // Load the workspace schema once (the visual builder's dropdowns). Tolerates a deny/empty — the
  // Code half still works without it (the author types raw SurrealQL).
  useEffect(() => {
    let cancelled = false;
    readSchema()
      .then((s) => {
        if (!cancelled) setSchema(s);
      })
      .catch(() => {
        if (!cancelled) setSchema({ tables: [] });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const switchMode = (mode: typeof value.mode) => {
    if (mode === value.mode) return;
    if (mode === "builder") {
      // Code→Builder: confirm, because the typed builder may not represent the hand-edited SQL and we
      // would regenerate from the (possibly stale) builder query — clobbering the raw string.
      const ok =
        typeof window === "undefined" ||
        window.confirm(
          "Switch to Builder? Hand-edited SQL may be replaced by the visual query.",
        );
      if (!ok) return;
      const builder = value.builder ?? emptyQuery();
      onChange({ ...value, mode: "builder", builder, rawSql: toSurrealQL(builder) });
    } else {
      // Builder→Code: free — the raw string is already in sync (Builder regenerates it on every edit).
      onChange({ ...value, mode: "code" });
    }
  };

  return (
    <div className="mt-2 grid gap-2" aria-label="sql query editor">
      <SqlQueryHeader
        mode={value.mode}
        format={value.format}
        onModeChange={switchMode}
        onFormatChange={(format) => onChange({ ...value, format })}
      />

      {value.mode === "builder" ? (
        <VisualEditor
          schema={schema}
          query={value.builder ?? emptyQuery()}
          onChange={(builder) =>
            // Builder is the source of truth — regenerate the raw SurrealQL on every change.
            onChange({ ...value, builder, rawSql: toSurrealQL(builder) })
          }
        />
      ) : (
        <RawEditor rawSql={value.rawSql} onChange={(rawSql) => onChange({ ...value, rawSql })} />
      )}
    </div>
  );
}

export { emptySqlSource };
