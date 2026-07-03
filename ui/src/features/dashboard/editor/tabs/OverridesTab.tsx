// The Overrides tab (viz panel-editor scope; field-config scope) — authors `fieldConfig.overrides[]`:
// a matcher + the properties it sets per matching field. Since step 4 (editor-parity) this is done
// RIGHT, on the option registry: the matcher value is a real-field picker (byName) / type dropdown
// (byType) / pattern input (byRegex) / refId picker (byFrameRefID = "by query"); "add property" is a
// searchable picker over the SAME registry the Field tab uses, and each property renders its NORMAL
// typed control inline (via `Control`). Multiple properties per override. Bounded by the shared cap.
// One responsibility: edit the override list.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Combobox } from "@/components/ui/combobox";
import type { EditorState } from "../cellEditorState";
import type { FieldConfig, FieldOverride, Matcher, View } from "@/lib/dashboard";
import { canonicalView } from "@/lib/dashboard";
import { MAX_OVERRIDES } from "../../fieldconfig/caps";
import { FieldNamePicker } from "../fields/FieldNamePicker";
import { Control } from "../options/Control";
import { optionById, optionsForView } from "../options/registry";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

const MATCHER_LABELS: Array<{ id: Matcher["id"]; label: string }> = [
  { id: "byName", label: "Fields with name" },
  { id: "byType", label: "Fields with type" },
  { id: "byRegexp", label: "Fields matching regex" },
  { id: "byFrameRefID", label: "Fields returned by query" },
];

export function OverridesTab({ state, patch }: Props) {
  const fc: FieldConfig = state.fieldConfig ?? { defaults: {}, overrides: [] };
  const overrides = fc.overrides ?? [];
  const view = canonicalView((state.view || "timeseries") as View);
  // The properties an override can set for THIS viz — the same registry options the Field/per-viz tabs
  // render (universal + view-scoped). This is what makes "add property" cheap and consistent.
  const propOptions = optionsForView(view);
  // The refIds the panel's targets expose (for the byFrameRefID matcher).
  const refIds = state.targets.map((t) => t.refId).filter(Boolean);

  const setOverrides = (next: FieldOverride[]) => patch({ fieldConfig: { ...fc, overrides: next } });
  const addOverride = () => {
    if (overrides.length >= MAX_OVERRIDES) return;
    setOverrides([...overrides, { matcher: { id: "byName", options: "" }, properties: [] }]);
  };
  const setMatcher = (idx: number, matcher: Matcher) =>
    setOverrides(overrides.map((o, i) => (i === idx ? { ...o, matcher } : o)));
  const setProps = (idx: number, properties: Array<{ id: string; value: unknown }>) =>
    setOverrides(overrides.map((o, i) => (i === idx ? { ...o, properties } : o)));
  const removeOverride = (idx: number) => setOverrides(overrides.filter((_, i) => i !== idx));

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="overrides tab">
      {overrides.length === 0 && <p className="text-muted">No field overrides. Add one to style a specific field.</p>}
      {overrides.map((over, idx) => {
        const usedIds = new Set(over.properties.map((p) => p.id));
        const addable = propOptions.filter((d) => !usedIds.has(d.id));
        const addProp = (id: string) => {
          const def = optionById(id);
          setProps(idx, [...over.properties, { id, value: def?.default }]);
        };
        const setPropValue = (pid: string, value: unknown) =>
          setProps(idx, over.properties.map((p) => (p.id === pid ? { ...p, value } : p)));
        const removeProp = (pid: string) => setProps(idx, over.properties.filter((p) => p.id !== pid));

        return (
          <div key={idx} className="grid gap-2 rounded-md border border-border bg-bg p-2" aria-label={`override ${idx}`}>
            {/* Matcher row: kind + its typed value control. */}
            <div className="flex items-center gap-1.5">
              <Select
                aria-label={`override ${idx} matcher`}
                className="h-8 w-40"
                value={over.matcher.id}
                onChange={(e) => setMatcher(idx, { id: e.target.value as Matcher["id"], options: "" })}
              >
                {MATCHER_LABELS.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.label}
                  </option>
                ))}
              </Select>
              <MatcherValue matcher={over.matcher} idx={idx} refIds={refIds} onChange={(options) => setMatcher(idx, { ...over.matcher, options })} />
              <Button variant="ghost" aria-label={`remove override ${idx}`} className="ml-auto h-auto px-1.5 text-muted hover:text-red-500" onClick={() => removeOverride(idx)}>
                ×
              </Button>
            </div>

            {/* Properties: each renders its NORMAL typed control (from the registry). */}
            <div className="grid gap-1.5 pl-1">
              {over.properties.map((p) => {
                const def = optionById(p.id);
                return (
                  <div key={p.id} className="grid gap-1" aria-label={`override ${idx} property ${p.id}`}>
                    <div className="flex items-center gap-1.5">
                      <span className="w-28 shrink-0 truncate text-muted">{def?.label ?? p.id}</span>
                      {def ? (
                        <div className="flex-1">
                          <Control control={def.control} label={`${p.id} value`} value={p.value} onChange={(v) => setPropValue(p.id, v)} />
                        </div>
                      ) : (
                        <Input aria-label={`${p.id} value`} className="h-8 flex-1 text-xs" value={String(p.value ?? "")} onChange={(e) => setPropValue(p.id, e.target.value)} />
                      )}
                      <Button variant="ghost" aria-label={`remove property ${p.id}`} className="h-auto px-1.5 text-muted hover:text-red-500" onClick={() => removeProp(p.id)}>
                        ×
                      </Button>
                    </div>
                  </div>
                );
              })}
              <Combobox
                aria-label={`override ${idx} add property`}
                className="w-56"
                options={addable.map((d) => ({ value: d.id, label: d.label, group: d.group }))}
                value=""
                placeholder="+ Add override property"
                onChange={addProp}
              />
            </div>
          </div>
        );
      })}
      <Button
        variant="outline"
        size="sm"
        aria-label="add override"
        className="h-auto justify-self-start px-2 py-1 text-[11px] text-muted hover:text-fg"
        onClick={addOverride}
      >
        + Add override
      </Button>
    </div>
  );
}

/** The matcher's value control, typed per matcher kind: byName -> a real-field picker; byType -> a type
 *  dropdown; byRegex -> a pattern input; byFrameRefID -> a refId dropdown. */
function MatcherValue({
  matcher,
  idx,
  refIds,
  onChange,
}: {
  matcher: Matcher;
  idx: number;
  refIds: string[];
  onChange: (options: unknown) => void;
}) {
  const value = String(matcher.options ?? "");
  if (matcher.id === "byName") {
    return <FieldNamePicker aria-label={`override ${idx} match`} className="flex-1" value={value} onChange={onChange} />;
  }
  if (matcher.id === "byType") {
    return (
      <Select aria-label={`override ${idx} match`} className="h-8 flex-1" value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">— pick a type —</option>
        {["number", "string", "time", "boolean"].map((t) => (
          <option key={t} value={t}>
            {t}
          </option>
        ))}
      </Select>
    );
  }
  if (matcher.id === "byFrameRefID") {
    return (
      <Select aria-label={`override ${idx} match`} className="h-8 flex-1" value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">— pick a query —</option>
        {refIds.map((r) => (
          <option key={r} value={r}>
            Query {r}
          </option>
        ))}
      </Select>
    );
  }
  return <Input aria-label={`override ${idx} match`} className="h-8 flex-1 text-xs" placeholder="/pattern/" value={value} onChange={(e) => onChange(e.target.value)} />;
}
