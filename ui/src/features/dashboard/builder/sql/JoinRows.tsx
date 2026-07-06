// The Joins section of the row-list (Rules) builder body — the rows counterpart of the canvas's
// drag-to-connect joins (visual-canvas-builder slice). One row per `SqlJoin`: type · table · ON
// (left table.column = right column). Gated by the HOST on `dialect === "standard"` (SurrealQL has
// no ANSI JOIN — the canvas gates the same way). Every edit is a typed `SqlBuilderQuery` change;
// `emitSql` renders the preview — no SQL strings are built here.
//
// A join whose ON columns aren't both picked yet is PENDING (`on: []`) — the emitter keeps it (and
// anything referencing its table) out of the SQL until it's wired, exactly like a canvas table
// that hasn't been connected. One responsibility per file (FILE-LAYOUT).

import { useState } from "react";
import { Plus, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { Schema } from "@/lib/schema";
import type { SqlBuilderQuery, SqlJoin, SqlJoinType } from "@/lib/panel-kit/sql/query";
import { removeTable } from "@/features/query-builder/canvas/canvasModel";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

const JOIN_TYPES: SqlJoinType[] = ["inner", "left", "right", "full", "cross"];

interface Props {
  schema: Schema;
  query: SqlBuilderQuery;
  onChange: (q: SqlBuilderQuery) => void;
}

/** A half-picked ON key (view state only — the model carries `on: []` until both columns are
 *  chosen, so a partial pick never emits and never persists). */
interface DraftOn {
  leftTable?: string;
  leftColumn: string;
  rightColumn: string;
}

/** The Joins rows — type · table · ON key pickers per join, plus "+ join". */
export function JoinRows({ schema, query, onChange }: Props) {
  const joins = query.joins ?? [];
  // Keyed by join index; cleared once the key commits to the model (or the join table changes).
  const [drafts, setDrafts] = useState<Record<number, DraftOn>>({});
  const columnsOf = (table: string) =>
    schema.tables.find((t) => t.name === table)?.columns.map((c) => c.name) ?? [];

  // Tables a NEW join can target: in the schema, not the FROM table, not already joined.
  const addable = schema.tables
    .map((t) => t.name)
    .filter((n) => n !== query.table && !joins.some((j) => j.table === n));

  const addJoin = () => {
    if (!addable.length) return;
    // ON starts empty (pending): the join enters the SQL only once both columns are picked.
    const join: SqlJoin = { table: addable[0], type: "inner", on: [] };
    onChange({ ...query, joins: [...joins, join] });
  };

  const setJoin = (i: number, join: SqlJoin) =>
    onChange({ ...query, joins: joins.map((j, idx) => (idx === i ? join : j)) });

  /** Swap a join's target table — clears its ON keys (model + draft) and every stale reference
   *  to the old table. */
  const setJoinTable = (i: number, table: string) => {
    const cleaned = removeTable(query, joins[i].table);
    const nextJoins = [...(cleaned.joins ?? [])];
    nextJoins.splice(i, 0, { table, type: joins[i].type, on: [] });
    setDrafts((d) => {
      const next = { ...d };
      delete next[i];
      return next;
    });
    onChange({ ...cleaned, joins: nextJoins });
  };

  return (
    <div className="grid gap-1" aria-label="sql joins">
      {joins.map((j, i) => {
        // The committed ON key wins; a half-picked one lives in the draft (never in the model).
        const on0 = j.on?.[0] ?? drafts[i];
        const leftTable = on0?.leftTable ?? query.table;
        // The left side of join i can be the FROM table or any EARLIER-joined table.
        const leftTables = [query.table, ...joins.slice(0, i).map((x) => x.table)];
        const setOn = (patch: Partial<DraftOn>) => {
          const next: DraftOn = {
            leftTable: "leftTable" in patch ? patch.leftTable : on0?.leftTable,
            leftColumn: patch.leftColumn ?? on0?.leftColumn ?? "",
            rightColumn: patch.rightColumn ?? on0?.rightColumn ?? "",
          };
          if (next.leftColumn && next.rightColumn) {
            // Both columns picked ⇒ commit the wired ON key to the model, drop the draft.
            setDrafts((d) => {
              const rest = { ...d };
              delete rest[i];
              return rest;
            });
            setJoin(i, {
              ...j,
              on: [{
                ...(next.leftTable && next.leftTable !== query.table
                  ? { leftTable: next.leftTable }
                  : {}),
                leftColumn: next.leftColumn,
                rightColumn: next.rightColumn,
              }],
            });
          } else {
            // Half-picked ⇒ keep it as view state; the model stays pending (out of the SQL).
            setDrafts((d) => ({ ...d, [i]: next }));
            if ((j.on ?? []).length > 0) setJoin(i, { ...j, on: [] });
          }
        };
        return (
          <div key={`${j.table}:${i}`} className="flex flex-wrap items-center gap-1">
            {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
            <select
              aria-label={`join type ${i}`}
              className={FIELD}
              value={j.type}
              onChange={(e) => setJoin(i, { ...j, type: e.target.value as SqlJoinType })}
            >
              {JOIN_TYPES.map((t) => (
                <option key={t} value={t}>
                  {t.toUpperCase()} JOIN
                </option>
              ))}
            </select>
            {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
            <select
              aria-label={`join table ${i}`}
              className={FIELD}
              value={j.table}
              onChange={(e) => setJoinTable(i, e.target.value)}
            >
              {[j.table, ...addable].map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
            {j.type !== "cross" && (
              <>
                <span className="text-[11px] text-muted">on</span>
                {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
                <select
                  aria-label={`join left table ${i}`}
                  className={FIELD}
                  value={leftTable}
                  onChange={(e) => setOn({ leftTable: e.target.value, leftColumn: "" })}
                >
                  {leftTables.map((t) => (
                    <option key={t} value={t}>
                      {t}
                    </option>
                  ))}
                </select>
                {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
                <select
                  aria-label={`join left column ${i}`}
                  className={FIELD}
                  value={on0?.leftColumn ?? ""}
                  onChange={(e) => setOn({ leftColumn: e.target.value })}
                >
                  <option value="">— column —</option>
                  {columnsOf(leftTable).map((c) => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                </select>
                <span className="text-[11px] text-muted">=</span>
                {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
                <select
                  aria-label={`join right column ${i}`}
                  className={FIELD}
                  value={on0?.rightColumn ?? ""}
                  onChange={(e) => setOn({ rightColumn: e.target.value })}
                >
                  <option value="">— column —</option>
                  {columnsOf(j.table).map((c) => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                </select>
              </>
            )}
            {j.type !== "cross" && (!on0?.leftColumn || !on0?.rightColumn) && (
              <span className="rounded-md bg-warning/10 px-1.5 py-0.5 text-[10px] text-warning">
                pick both columns — not in the SQL yet
              </span>
            )}
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={`remove join ${i}`}
              onClick={() => onChange(removeTable(query, j.table))}
              className="h-7 w-7 shrink-0 text-muted"
            >
              <X size={12} />
            </Button>
          </div>
        );
      })}
      <Button
        type="button"
        variant="ghost"
        size="sm"
        aria-label="add join"
        onClick={addJoin}
        disabled={!query.table || addable.length === 0}
        className="h-6 w-fit px-1.5 text-[11px] text-muted"
      >
        <Plus size={11} /> add join
      </Button>
    </div>
  );
}
