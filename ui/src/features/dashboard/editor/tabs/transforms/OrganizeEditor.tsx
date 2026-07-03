// The Organize-fields transform editor (editor-parity scope, step 3 HEADLINE — the user's screenshot:
// "Organize fields is a raw JSON textarea"). Grafana's row list over the ACTUAL result fields: reorder
// (up/down — the shipped `indexByName` order), hide (the eye — `excludeByName`), inline rename
// (`renameByName`). The rows come from `useResultFields` (the live preview's viz.query frames), so the
// author works over real field names, never types them. A field with no preview yet degrades to the
// names already referenced in the options (so editing a saved config still works offline). One
// responsibility: author an `organize` transform's options.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Eye, EyeOff } from "lucide-react";
import { useResultFields } from "../../fields/FieldsContext";

interface Props {
  options: Record<string, unknown>;
  onChange: (options: Record<string, unknown>) => void;
}

type NumMap = Record<string, number>;
type BoolMap = Record<string, boolean>;
type StrMap = Record<string, string>;

function asBoolMap(v: unknown): BoolMap {
  return v && typeof v === "object" ? (v as BoolMap) : {};
}
function asNumMap(v: unknown): NumMap {
  return v && typeof v === "object" ? (v as NumMap) : {};
}
function asStrMap(v: unknown): StrMap {
  return v && typeof v === "object" ? (v as StrMap) : {};
}

/** The ordered field list: the real result fields, ordered by `indexByName` where present (Grafana's
 *  rule), with any fields referenced only in the options (a saved config over data not yet previewed)
 *  appended so nothing an author configured disappears. */
function orderedFields(resultFields: string[], index: NumMap, exclude: BoolMap, rename: StrMap): string[] {
  const known = new Set<string>(resultFields);
  for (const k of Object.keys({ ...index, ...exclude, ...rename })) known.add(k);
  const all = [...known];
  all.sort((a, b) => {
    const ia = a in index ? index[a] : Number.MAX_SAFE_INTEGER;
    const ib = b in index ? index[b] : Number.MAX_SAFE_INTEGER;
    if (ia !== ib) return ia - ib;
    return resultFields.indexOf(a) - resultFields.indexOf(b);
  });
  return all;
}

export function OrganizeEditor({ options, onChange }: Props) {
  const resultFields = useResultFields();
  const exclude = asBoolMap(options.excludeByName);
  const index = asNumMap(options.indexByName);
  const rename = asStrMap(options.renameByName);
  const fields = orderedFields(resultFields, index, exclude, rename);

  /** Rewrite `indexByName` so the given order is explicit (0..n-1). */
  const writeOrder = (order: string[]) => {
    const nextIndex: NumMap = {};
    order.forEach((name, i) => (nextIndex[name] = i));
    onChange({ ...options, indexByName: nextIndex });
  };
  const move = (name: string, dir: -1 | 1) => {
    const order = [...fields];
    const i = order.indexOf(name);
    const j = i + dir;
    if (j < 0 || j >= order.length) return;
    [order[i], order[j]] = [order[j], order[i]];
    writeOrder(order);
  };
  const toggleHide = (name: string) => {
    const next = { ...exclude };
    if (next[name]) delete next[name];
    else next[name] = true;
    onChange({ ...options, excludeByName: next });
  };
  const setRename = (name: string, label: string) => {
    const next = { ...rename };
    if (label) next[name] = label;
    else delete next[name];
    onChange({ ...options, renameByName: next });
  };

  if (fields.length === 0) {
    return (
      <p className="text-xs text-muted">
        No fields to organize yet — run the query (the preview) to list the result fields here.
      </p>
    );
  }

  return (
    <ul className="grid gap-1" aria-label="organize fields">
      {fields.map((name, idx) => {
        const hidden = !!exclude[name];
        return (
          <li key={name} className="flex items-center gap-1.5" aria-label={`organize field ${name}`}>
            <div className="flex flex-col">
              <Button type="button" size="sm" variant="ghost" aria-label={`move up ${name}`} disabled={idx === 0} className="h-3 px-1 text-[9px] leading-none" onClick={() => move(name, -1)}>
                ▲
              </Button>
              <Button type="button" size="sm" variant="ghost" aria-label={`move down ${name}`} disabled={idx === fields.length - 1} className="h-3 px-1 text-[9px] leading-none" onClick={() => move(name, 1)}>
                ▼
              </Button>
            </div>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              aria-label={`${hidden ? "show" : "hide"} ${name}`}
              aria-pressed={hidden}
              className="h-6 px-1 text-muted"
              onClick={() => toggleHide(name)}
            >
              {hidden ? <EyeOff size={13} /> : <Eye size={13} />}
            </Button>
            <span className={`w-28 shrink-0 truncate font-mono text-[11px] ${hidden ? "text-muted line-through" : "text-fg"}`}>{name}</span>
            <Input
              aria-label={`rename ${name}`}
              className="h-7 flex-1 text-xs"
              placeholder={`rename “${name}”`}
              value={rename[name] ?? ""}
              onChange={(e) => setRename(name, e.target.value)}
            />
          </li>
        );
      })}
    </ul>
  );
}
