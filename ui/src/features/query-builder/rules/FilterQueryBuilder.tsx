// The react-querybuilder-backed filter editor (the react-querybuilder slice). Two stacked
// `<QueryBuilder>` instances in `independentCombinators` mode (detected from the `RuleGroupTypeIC`
// query shape): one for WHERE (plain filters) and one for HAVING (aggregate filters with an
// aggregation pick via `HavingValueEditor`). Each edit fires `onChange` with a new `SqlBuilderQuery`
// — the host keeps `rawSql` in sync via `emitSql`. The live preview stays in the host (VisualRows).
//
// MODEL-AS-TRUTH (rule 10 / the scope invariant): the projection is ONE-WAY for display
// (`SqlFilter[] → RuleGroupTypeIC` via `toRuleGroup`); on every `onQueryChange` we flatten BACK to
// `SqlFilter[]` via `fromRuleGroup` and write it into `query.filters`. We NEVER call react-querybuilder's
// `formatQuery()` — it would bypass the dialect seam and has an injection surface. `emitSql` stays the
// only SQL string owner. The emitters and goldens never change.
//
// One responsibility per file (FILE-LAYOUT): this file is only the WHERE+HAVING host. The pure
// projection lives in `filterRules.ts`; the HAVING aggregation editor in `HavingValueEditor.tsx`.

import { QueryBuilder } from "react-querybuilder";
import type { RuleGroupTypeIC } from "react-querybuilder";

import type { Schema } from "@/lib/schema";
import type { SqlBuilderQuery, SqlFilter } from "@/lib/panel-kit/sql/query";
import {
  fromRuleGroup,
  RQB_COMBINATORS,
  RQB_OPERATORS,
  schemaToFields,
  toRuleGroup,
} from "./filterRules";
import { HavingValueEditor } from "./HavingValueEditor";

/** Shared field styling mapped onto our tokens — the same flavour the retired VisualRows used so the
 *  rules editor sits on the panel like the rest of the builder. Applied via `controlClassnames`
 *  (ADDITIVE to RQB's marker classes; we do NOT import RQB's default stylesheet). */
const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-[11px] text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

/** The Tailwind-token classnames for every RQB internal element we render. RQB applies these
 *  additively to its standard marker classes; without the default stylesheet the markers are unstyled
 *  so these classes ARE the visual layer (the "Tailwind variant" the scope calls for, hand-rolled
 *  onto our tokens rather than importing a third-party stylesheet). */
const CONTROL_CLASSNAMES = {
  queryBuilder: "text-fg",
  ruleGroup: "grid gap-1.5 rounded-md border border-border/60 bg-panel/40 p-2",
  header: "mb-1 flex items-center gap-1",
  body: "grid gap-1.5",
  rule: "flex flex-wrap items-center gap-1",
  fields: FIELD,
  operators: `${FIELD} w-20`,
  value: `${FIELD} w-28`,
  combinators: `${FIELD} w-20`,
  addRule: "h-7 rounded-md px-2 text-[11px] text-accent hover:bg-accent/10",
  addGroup: "hidden",
  removeRule: "h-7 w-7 rounded-md text-muted hover:bg-accent/10 hover:text-fg",
  removeGroup: "hidden",
  cloneRule: "hidden",
  cloneGroup: "hidden",
} as const;

interface Props {
  schema: Schema;
  query: SqlBuilderQuery;
  onChange: (query: SqlBuilderQuery) => void;
}

/** The react-querybuilder-backed filter editor: WHERE + HAVING (aggregates). Each builder projects
 *  `query.filters` (split by `isAggregate`) into a `RuleGroupTypeIC`; edits flatten back into
 *  `SqlFilter[]` and the non-matching half is preserved untouched. */
export function FilterQueryBuilder({ schema, query, onChange }: Props) {
  const fields = schemaToFields(schema, query);
  const whereGroup = toRuleGroup(query.filters, query, false);
  const havingGroup = toRuleGroup(query.filters, query, true);

  const replaceClause = (next: SqlFilter[], aggregate: boolean): void => {
    const kept = query.filters.filter((f) => !!f.isAggregate !== aggregate);
    onChange({ ...query, filters: [...kept, ...next] });
  };

  const onWhereChange = (g: RuleGroupTypeIC) => replaceClause(fromRuleGroup(g, query, false), false);
  const onHavingChange = (g: RuleGroupTypeIC) => replaceClause(fromRuleGroup(g, query, true), true);

  // `showCombinatorsBetweenRules` renders the and/or dropdown INLINE between rules (the natural IC
  // shape). `enableMountQueryChange={false}` avoids a redundant first-paint write — the projection is
  // already the source of truth for `query.filters`.
  const sharedProps = {
    fields,
    operators: RQB_OPERATORS,
    combinators: RQB_COMBINATORS,
    controlClassnames: CONTROL_CLASSNAMES,
    showCombinatorsBetweenRules: true,
    enableMountQueryChange: false,
    enableDragAndDrop: false,
    translations: { addRule: { label: "+ filter" }, removeRule: { label: "" } },
  } as const;

  return (
    <div className="grid gap-2" aria-label="sql filter builder">
      <Clause label="Filter (WHERE)">
        <QueryBuilder
          {...sharedProps}
          query={whereGroup}
          onQueryChange={onWhereChange}
          controlElements={{ addGroupAction: () => null, cloneRuleAction: () => null, cloneGroupAction: () => null }}
        />
      </Clause>
      <Clause label="HAVING (aggregates)">
        <QueryBuilder
          {...sharedProps}
          {...{
            translations: { addRule: { label: "+ having" }, removeRule: { label: "" } },
          }}
          query={havingGroup}
          onQueryChange={onHavingChange}
          controlElements={{ addGroupAction: () => null, cloneRuleAction: () => null, cloneGroupAction: () => null, valueEditor: HavingValueEditor }}
        />
      </Clause>
    </div>
  );
}

/** A labelled clause wrapper — mirrors the retired VisualRows `Row` flavour (label + control). */
function Clause({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[80px_1fr] items-start gap-2">
      <span className="pt-1.5 text-[11px] font-medium text-muted">{label}</span>
      <div>{children}</div>
    </div>
  );
}
