// The custom `valueEditor` for HAVING (aggregate) rules in the react-querybuilder slice. A HAVING
// rule filters on an aggregate EXPRESSION (`avg(value) > 10`), so the editor renders TWO controls:
// the aggregation-function pick (count / count_distinct / sum / avg / min / max) and the comparison
// value input. The aggregation is stored on the rule's `meta.aggregation` (preserved by RQB core,
// never interpreted) — updated via `schema.dispatchQuery(withRuleMeta(…))` since RQB has no direct
// `handleOnMetaChange`. The comparison value goes through the normal `handleOnChange`.
//
// One responsibility per file (FILE-LAYOUT): this is only the HAVING value editor. The WHERE builder
// uses RQB's default value editor (no aggregation pick needed).

import type { ValueEditorProps } from "react-querybuilder";

import { Input } from "@/components/ui/input";
import type { SqlAggregation } from "@/lib/panel-kit/sql/query";
import { ruleAtPath, withRuleMeta } from "./filterRules";

/** The shared field styling — matches the retired VisualRows `FIELD` flavour (FILE-LAYOUT). */
const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2 text-[11px] text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

/** The aggregation functions exposed on a HAVING rule. Mirrors the SELECT-side aggregation list. */
const AGGREGATIONS: SqlAggregation[] = ["count", "count_distinct", "sum", "avg", "min", "max"];

/** The RQB operator names that carry no value (`null` / `notNull`) — hide the value input for them. */
const VALUELESS = new Set(["null", "notNull"]);

/** The HAVING value editor: aggregation pick + comparison value. Reads the aggregation from the
 *  rule's `meta` (defaulting to `count`); updates it by dispatching a `withRuleMeta` query patch. */
export function HavingValueEditor({
  value,
  operator,
  handleOnChange,
  path,
  schema,
}: ValueEditorProps) {
  const group = schema.getQuery() as Parameters<typeof ruleAtPath>[0];
  const rule = ruleAtPath(group, path);
  const aggregation = (rule?.meta?.aggregation as SqlAggregation | undefined) ?? "count";
  const showValue = !VALUELESS.has(operator);

  const setAggregation = (agg: SqlAggregation) => {
    schema.dispatchQuery(withRuleMeta(group, path, { aggregation: agg }));
  };

  return (
    <div className="flex items-center gap-1">
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
      <select
        aria-label="sql having aggregation"
        className={`${FIELD} w-32`}
        value={aggregation}
        onChange={(e) => setAggregation(e.target.value as SqlAggregation)}
      >
        {AGGREGATIONS.map((a) => (
          <option key={a} value={a}>
            {a}
          </option>
        ))}
      </select>
      {showValue && (
        <Input
          aria-label="sql filter value"
          className={`${FIELD} w-28`}
          value={String(value ?? "")}
          onChange={(e) => handleOnChange(e.target.value)}
        />
      )}
    </div>
  );
}
