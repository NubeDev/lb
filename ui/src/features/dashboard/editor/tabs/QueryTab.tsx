// The Query tab (viz panel-editor scope) — authors the panel's target(s) by REUSING the shipped source
// picker (friendly labels → `{tool,args}`, never a raw tool name) and the SQL Builder⇄Code editor. It
// edits the editor state's PRIMARY target + the rehydrated SQL Builder state, so reopening a SQL panel
// returns to the builder (the precise bug this slice fixes). Phase 1 authors one target; the `sources[]`
// shape is preserved so multi-target (datasource-binding, Phase 3) is additive. One responsibility:
// pick/edit the query.

import { useMemo } from "react";

import type { Target } from "@/lib/dashboard";
import type { EditorState } from "../cellEditorState";
import { useSourcePicker } from "../../builder/useSourcePicker";
import { seedEntryId } from "../../builder/WidgetBuilder";
import { SQL_SOURCE_ID, type SourceEntry } from "../../builder/sourcePicker";
import { SqlQueryEditor, emptySqlSource } from "../../builder/sql/SqlQueryEditor";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

interface Props {
  ws: string;
  state: EditorState;
  /** Apply a partial update to the editor state (the editor owns the merge + re-render). */
  patch: (next: Partial<EditorState>) => void;
}

/** Build the primary target from a chosen picker entry (a read source → its `{tool,args}`). */
function targetFromEntry(entry: SourceEntry | null, prev: Target | undefined): Target {
  const refId = prev?.refId || "A";
  if (!entry || !entry.source) return { refId, tool: "", args: {}, datasource: { type: "surreal" } };
  return {
    refId,
    tool: entry.source.tool,
    args: (entry.source.args as Record<string, unknown>) ?? {},
    datasource: { type: entry.source.tool === "store.query" ? "surreal" : "series" },
  };
}

export function QueryTab({ ws, state, patch }: Props) {
  const { entries, loading } = useSourcePicker(ws);
  const primary = state.targets[0];

  // Match the current target back to a picker entry id (so the dropdown reflects the saved source —
  // including the SQL source by its `store.query` tool). Reuses the shipped `seedEntryId`.
  const entryId = useMemo(
    () => seedEntryId(primary ? ({ source: { tool: primary.tool, args: primary.args } } as never) : undefined, entries),
    [primary, entries],
  );
  const entry = entries.find((e) => e.id === entryId) ?? null;
  const isSql = entry?.id === SQL_SOURCE_ID || primary?.tool === "store.query";

  const selectEntry = (id: string) => {
    const next = entries.find((e) => e.id === id) ?? null;
    if (next?.id === SQL_SOURCE_ID) {
      // Switch to the SQL source — seed an empty builder state if none, and set the target tool.
      const sql = state.sql ?? emptySqlSource();
      patch({
        sql,
        targets: [{ ...targetFromEntry(next, primary), tool: "store.query", args: { sql: sql.rawSql } }],
      });
    } else {
      patch({ sql: undefined, targets: [targetFromEntry(next, primary)] });
    }
  };

  return (
    <div className="grid gap-3 py-3" aria-label="query tab">
      <label className="grid gap-1 text-xs text-muted">
        Source
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (see dashboard.md follow-up) */}
        <select
          aria-label="panel source"
          className={`${FIELD} w-full`}
          value={entryId}
          onChange={(e) => selectEntry(e.target.value)}
        >
          <option value="">{loading ? "loading sources…" : "— pick a source —"}</option>
          <PickerGroup entries={entries} group="series" label="Series" />
          <PickerGroup entries={entries} group="live" label="Live (Zenoh)" />
          <PickerGroup entries={entries} group="sql" label="Direct SurrealDB" />
          <PickerGroup entries={entries} group="extension" label="Installed extension" />
        </select>
      </label>

      {/* The SQL Builder⇄Code editor — its state rehydrates from `state.sql` so EDIT reopens the
          builder, not Code-only. Every change writes BOTH the rawSql (what `store.query` runs) and the
          builder query back through the editor state + the target args. */}
      {isSql && (
        <SqlQueryEditor
          value={state.sql ?? emptySqlSource()}
          onChange={(sql) =>
            patch({
              sql,
              targets: [{ ...(primary ?? targetFromEntry(entry, undefined)), tool: "store.query", args: { sql: sql.rawSql } }],
            })
          }
        />
      )}
    </div>
  );
}

function PickerGroup({ entries, group, label }: { entries: SourceEntry[]; group: SourceEntry["group"]; label: string }) {
  const items = entries.filter((e) => e.group === group);
  if (items.length === 0) return null;
  return (
    <optgroup label={label}>
      {items.map((e) => (
        <option key={e.id} value={e.id}>
          {e.label}
        </option>
      ))}
    </optgroup>
  );
}
