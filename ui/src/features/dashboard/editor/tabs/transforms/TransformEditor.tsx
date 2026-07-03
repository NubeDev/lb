// The per-transform editor dispatcher (editor-parity scope, step 3) — maps a transform id to its typed
// editor. ALL 11 shipped ids get a real editor now (organize/filterByValue/groupBy/calculateField the
// deep ones; reduce/sortBy/limit/filterFieldsByName/joinByField the small forms; merge/seriesToRows a
// no-options note). The raw-JSON textarea remains ONLY as the escape hatch for an IMPORTED UNSUPPORTED
// id (labeled as such), never for a shipped one. One responsibility: id → its editor.

import type { Transformation } from "@/lib/dashboard";
import type { TransformId } from "../transformRegistry";
import { TRANSFORM_DEFS } from "../transformRegistry";
import { OrganizeEditor } from "./OrganizeEditor";
import { FilterByValueEditor } from "./FilterByValueEditor";
import { GroupByEditor } from "./GroupByEditor";
import { CalculateFieldEditor } from "./CalculateFieldEditor";
import {
  ReduceEditor,
  SortByEditor,
  LimitEditor,
  FilterFieldsByNameEditor,
  JoinByFieldEditor,
  NoOptionsEditor,
} from "./SmallEditors";
import { RawJsonOptions } from "./RawJsonOptions";

const SUPPORTED = new Set<string>(TRANSFORM_DEFS.map((d) => d.id));

interface Props {
  t: Transformation;
  onChange: (options: Record<string, unknown>) => void;
}

export function TransformEditor({ t, onChange }: Props) {
  const options = t.options ?? {};
  switch (t.id as TransformId) {
    case "organize":
      return <OrganizeEditor options={options} onChange={onChange} />;
    case "filterByValue":
      return <FilterByValueEditor options={options} onChange={onChange} />;
    case "groupBy":
      return <GroupByEditor options={options} onChange={onChange} />;
    case "calculateField":
      return <CalculateFieldEditor options={options} onChange={onChange} />;
    case "reduce":
      return <ReduceEditor options={options} onChange={onChange} />;
    case "sortBy":
      return <SortByEditor options={options} onChange={onChange} />;
    case "limit":
      return <LimitEditor options={options} onChange={onChange} />;
    case "filterFieldsByName":
      return <FilterFieldsByNameEditor options={options} onChange={onChange} />;
    case "joinByField":
      return <JoinByFieldEditor options={options} onChange={onChange} />;
    case "merge":
    case "seriesToRows":
      return <NoOptionsEditor id={t.id} />;
    default:
      // An imported transform id we don't type — the honest escape hatch, clearly labeled.
      return (
        <div className="grid gap-1">
          {!SUPPORTED.has(t.id) && (
            <p className="text-[11px] text-muted">
              Imported transform <code>{t.id}</code> — no typed editor; edit its raw options.
            </p>
          )}
          <RawJsonOptions opts={options} onChange={onChange} />
        </div>
      );
  }
}
