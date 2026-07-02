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
import { FlowsQuerySection } from "./FlowsQuerySection";
import { useSceneDocs } from "./useSceneDocs";

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
function dsKindOf(target: Target | undefined): "surreal" | "series" | "federation" | "flows" {
  const t = target?.datasource?.type;
  if (t === "federation" || target?.tool === "federation.query") return "federation";
  if (t === "series") return "series";
  if (t === "flows" || target?.tool === "flows.node_state") return "flows";
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
  // A flow INPUT control has no read target — it carries a `flows.inject` action; recognise the Flows
  // datasource from EITHER the target (output read) or the carried action (input control).
  const isFlowAction = state.carry.action?.tool === "flows.inject";
  const dsKind = isFlowAction ? "flows" : dsKindOf(primary);

  // A packaged extension `[[widget]]` cell carries NO target — it's identified by its `ext:<id>/<widget>`
  // view key (round-tripped verbatim through `state.view`). Selecting one sets the view + clears targets;
  // it lives in the source picker's "Extension widgets" group (finding 7: the group the viz-editor rework
  // dropped, restored here so a packaged tile is pickable in the live builder again).
  const isWidgetView = (state.view ?? "").startsWith("ext:");

  // Match the current selection back to a picker entry id: a widget cell → its `viewKey` entry; else the
  // series/sql target path (federation uses raw SQL, so it has no picker entry).
  const entryId = useMemo(() => {
    if (isWidgetView) return entries.find((e) => e.viewKey === state.view)?.id ?? "";
    return seedEntryId(
      primary ? ({ source: { tool: primary.tool, args: primary.args } } as never) : undefined,
      entries,
    );
  }, [isWidgetView, state.view, primary, entries]);
  const entry = entries.find((e) => e.id === entryId) ?? null;
  const isSql =
    !isWidgetView && dsKind === "surreal" && (entry?.id === SQL_SOURCE_ID || primary?.tool === "store.query");
  const isFederation = !isWidgetView && dsKind === "federation";
  const isFlows = !isWidgetView && dsKind === "flows";
  // The datasource dropdown's selected value (flows takes precedence over the target-derived value when
  // an inject action is carried — an input control has no read target to derive from).
  const dsValue = isFlows ? "flows" : dsValueOf(primary);
  const fedSource = (primary?.args?.source as string | undefined) ?? "";
  const fedSql = (primary?.args?.sql as string | undefined) ?? "";

  // --- selecting a DATASOURCE rewrites the primary target's shape (built-in vs federation). ---
  // Every branch also clears any `ext:` widget view: a datasource query and a packaged tile are mutually
  // exclusive cell shapes (the tile has no target), so switching datasource leaves the widget behind.
  const selectDatasource = (value: string) => {
    if (value.startsWith("federation:")) {
      const name = value.slice("federation:".length);
      const opt = dsOptions.find((o) => o.type === "federation" && o.name === name);
      const ds = opt ? refForOption(opt, ws) : { type: "federation" };
      patch({
        view: isWidgetView ? "" : state.view,
        sql: undefined,
        targets: [{ refId: primary?.refId || "A", tool: "federation.query", args: { source: name, sql: fedSql }, datasource: ds }],
      });
      return;
    }
    if (value === "series") {
      patch({ view: isWidgetView ? "" : state.view, sql: undefined, targets: [{ refId: primary?.refId || "A", tool: "", args: {}, datasource: { type: "series" } }], carry: { ...state.carry, action: undefined } });
      return;
    }
    if (value === "flows") {
      // Flows: an empty `flows`-typed target; the FlowsQuerySection picks the node port (input → an
      // inject action + control view; output → a node_state source + read view).
      patch({ view: isWidgetView ? "" : state.view, sql: undefined, targets: [{ refId: primary?.refId || "A", tool: "", args: {}, datasource: { type: "flows" } }], carry: { ...state.carry, action: undefined } });
      return;
    }
    // surreal (native) — reset to an empty native target; the source picker / SQL editor takes over.
    patch({ view: isWidgetView ? "" : state.view, sql: undefined, targets: [{ refId: primary?.refId || "A", tool: "", args: {}, datasource: { type: "surreal" } }], carry: { ...state.carry, action: undefined } });
  };

  const selectEntry = (id: string) => {
    const next = entries.find((e) => e.id === id) ?? null;
    // A packaged tile: the cell becomes `{view:"ext:<id>/<widget>"}` with no target/source (the tile owns
    // its data via `scope ∩ grant`, re-checked at the host). Clear any prior query target + SQL. The tile
    // keeps whatever `options` the cell already carried (e.g. a set `sceneId`), edited via the field below.
    if (next?.group === "widget" && next.viewKey) {
      patch({ view: next.viewKey as never, sql: undefined, targets: [], carry: { ...state.carry, action: undefined } });
      return;
    }
    // Leaving a widget view for a real source: drop the `ext:` view so the chosen source's view takes over.
    if (isWidgetView) {
      patch({ view: "" });
    }
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
          value={dsValue}
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

      {/* Flows — the flow→node→port picker (input port → control + inject action; output → read view). */}
      {isFlows && <FlowsQuerySection state={state} patch={patch} entries={entries} loading={loading} />}

      {/* Native surreal + series share the friendly source picker (labels → `{tool,args}`). */}
      {!isFederation && !isFlows && (
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
            {/* The finished packaged tiles (`[[widget]]`) — restored here (finding 7). Selecting one makes
                a `view:"ext:<id>/<widget>"` cell that owns its own render + data (no view chooser). */}
            <PickerGroup entries={entries} group="widget" label="Extension widgets" />
          </select>
        </label>
      )}

      {/* A packaged tile whose config is a scene doc: let the author pick the `sceneId` from the
          workspace's `scene:` docs (via `assets.list_docs`) instead of hand-seeding the cell — the
          generic half of finding 8. The chosen id lands in `cell.options.sceneId`, forwarded to the tile
          as `ctx.options.sceneId` by `ExtWidget`. Shown only for a Scene tile (its viewKey ends `/scene`). */}
      {isWidgetView && (state.view ?? "").endsWith("/scene") && (
        <SceneOptionsField ws={ws} state={state} patch={patch} />
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

/** The scene picker for a Scene `[[widget]]` tile — sets `cell.options.sceneId` from the workspace's
 *  `scene:` docs. Kept in this file (one small view over the Query tab's options), reading the shipped
 *  `assets.list_docs`; a denied/empty list is an honest "no scenes" (never a fabricated roster). */
function SceneOptionsField({
  ws,
  state,
  patch,
}: {
  ws: string;
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}) {
  const { scenes, loading } = useSceneDocs(ws);
  const sceneId = (state.options?.sceneId as string | undefined) ?? "";
  const setScene = (id: string) =>
    patch({ options: { ...state.options, sceneId: id || undefined } });
  return (
    <label className="grid gap-1 text-xs text-muted">
      Scene
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (see QueryTab note) */}
      <select
        aria-label="scene doc"
        className={`${FIELD} w-full`}
        value={sceneId}
        onChange={(e) => setScene(e.target.value)}
      >
        <option value="">{loading ? "loading scenes…" : "— pick a scene —"}</option>
        {scenes.map((s) => (
          <option key={s.id} value={s.id}>
            {s.title}
          </option>
        ))}
      </select>
    </label>
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
