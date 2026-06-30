// The Query tab (viz panel-editor scope) — authors the panel's target(s). Phase 3 adds a DATASOURCE
// dropdown ABOVE the source picker (viz datasource-binding scope): the two built-ins (native SurrealDB +
// Series) and each registered FEDERATION source (`datasource.list`, ws-walled). The chosen datasource
// sets `target.datasource` ({type, uid?}) and steers the rest of the tab:
//   - surreal → the SQL Builder⇄Code editor over `store.query` (unchanged Phase-1 path);
//   - series  → the friendly source picker over `series.*`;
//   - federation → `federation.query` with `{ source, sql }` — the RAW SQL editor (the federation
//     schema-dropdown verb is DEFERRED this phase, so a federation source authors raw SQL honestly).
// ADD == EDIT: the dropdown reflects the SAVED `target.datasource`. One responsibility: pick/edit the query.

import { useMemo } from "react";
import { Play } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { Target } from "@/lib/dashboard";
import type { EditorState } from "../cellEditorState";
import { useSourcePicker } from "../../builder/useSourcePicker";
import { seedEntryId } from "../../builder/WidgetBuilder";
import { SQL_SOURCE_ID, type SourceEntry } from "../../builder/sourcePicker";
import { SqlQueryEditor, emptySqlSource } from "../../builder/sql/SqlQueryEditor";
import { useDatasourceList, refForOption, type DatasourceOption } from "./useDatasourceList";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

interface Props {
  ws: string;
  state: EditorState;
  /** Apply a partial update to the editor state (the editor owns the merge + re-render). */
  patch: (next: Partial<EditorState>) => void;
  /** Force the live preview to re-run the current query (the "Run" button) even if the spec is unchanged. */
  onRun?: () => void;
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

/** Which built-in/federation datasource the saved target binds — drives the dropdown's selected value. */
function dsKindOf(target: Target | undefined): "surreal" | "series" | "federation" {
  const t = target?.datasource?.type;
  if (t === "federation" || target?.tool === "federation.query") return "federation";
  if (t === "series") return "series";
  return "surreal";
}

/** The dropdown value for a target: built-ins keyed by type; a federation source keyed by its name. */
function dsValueOf(target: Target | undefined): string {
  const kind = dsKindOf(target);
  if (kind === "federation") return `federation:${(target?.args?.source as string) ?? (target?.datasource?.uid?.split(":").pop() ?? "")}`;
  return kind;
}

export function QueryTab({ ws, state, patch, onRun }: Props) {
  const { entries, loading } = useSourcePicker(ws);
  const { options: dsOptions, loading: dsLoading } = useDatasourceList(ws);
  const primary = state.targets[0];
  const dsKind = dsKindOf(primary);

  // Match the current target back to a picker entry id (series/sql path only — federation uses raw SQL).
  const entryId = useMemo(
    () => seedEntryId(primary ? ({ source: { tool: primary.tool, args: primary.args } } as never) : undefined, entries),
    [primary, entries],
  );
  const entry = entries.find((e) => e.id === entryId) ?? null;
  const isSql = dsKind === "surreal" && (entry?.id === SQL_SOURCE_ID || primary?.tool === "store.query");
  const isFederation = dsKind === "federation";
  const fedSource = (primary?.args?.source as string | undefined) ?? "";
  const fedSql = (primary?.args?.sql as string | undefined) ?? "";

  // --- selecting a DATASOURCE rewrites the primary target's shape (built-in vs federation). ---
  const selectDatasource = (value: string) => {
    if (value.startsWith("federation:")) {
      const name = value.slice("federation:".length);
      const opt = dsOptions.find((o) => o.type === "federation" && o.name === name);
      const ds = opt ? refForOption(opt, ws) : { type: "federation" };
      patch({
        sql: undefined,
        targets: [{ refId: primary?.refId || "A", tool: "federation.query", args: { source: name, sql: fedSql }, datasource: ds }],
      });
      return;
    }
    if (value === "series") {
      patch({ sql: undefined, targets: [{ refId: primary?.refId || "A", tool: "", args: {}, datasource: { type: "series" } }] });
      return;
    }
    // surreal (native) — reset to an empty native target; the source picker / SQL editor takes over.
    patch({ sql: undefined, targets: [{ refId: primary?.refId || "A", tool: "", args: {}, datasource: { type: "surreal" } }] });
  };

  const selectEntry = (id: string) => {
    const next = entries.find((e) => e.id === id) ?? null;
    if (next?.id === SQL_SOURCE_ID) {
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
        Datasource
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (see dashboard.md follow-up) */}
        <select
          aria-label="panel datasource"
          className={`${FIELD} w-full`}
          value={dsValueOf(primary)}
          onChange={(e) => selectDatasource(e.target.value)}
        >
          {dsLoading && <option value="">loading datasources…</option>}
          {dsOptions.map((o) => (
            <option key={optionValue(o)} value={optionValue(o)}>
              {o.label}
            </option>
          ))}
        </select>
      </label>

      {/* Native surreal + series share the friendly source picker (labels → `{tool,args}`). */}
      {!isFederation && (
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
      )}

      {/* Native SQL — the Builder⇄Code editor, rehydrated from `state.sql` so EDIT reopens the builder. */}
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

      {/* Federation — RAW SQL against the chosen source (`federation.query {source, sql}`). The schema
          dropdown verb is deferred this phase, so the author writes SQL directly (honest). */}
      {isFederation && (
        <div className="grid gap-2">
          <label className="grid gap-1 text-xs text-muted">
            SQL ({fedSource || "no source"})
            {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Textarea primitive in this repo (see WidgetBuilder.tsx) */}
            <textarea
              aria-label="federation sql"
              className={`${FIELD} h-24 w-full resize-y py-1.5 font-mono`}
              value={fedSql}
              placeholder="SELECT …"
              // Cmd/Ctrl+Enter runs the query — the editor convention alongside the Run button.
              onKeyDown={(e) => {
                if ((e.metaKey || e.ctrlKey) && e.key === "Enter") onRun?.();
              }}
              onChange={(e) =>
                patch({
                  targets: [{ refId: primary?.refId || "A", tool: "federation.query", args: { source: fedSource, sql: e.target.value }, datasource: primary?.datasource ?? { type: "federation" } }],
                })
              }
            />
          </label>
          <div className="flex justify-end">
            <Button
              aria-label="run query"
              size="sm"
              variant="solid"
              disabled={!fedSource || !fedSql.trim()}
              onClick={() => onRun?.()}
            >
              <Play size={12} /> Run
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

/** The dropdown <option> value for a datasource option (built-in by type, federation by name). */
function optionValue(o: DatasourceOption): string {
  return o.type === "federation" ? `federation:${o.name}` : o.type;
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
