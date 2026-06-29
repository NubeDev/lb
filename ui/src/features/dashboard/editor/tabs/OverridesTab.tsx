// The Overrides tab (viz panel-editor scope: ship the tab from day one; field-config scope: Phase 1
// matchers are `byName`/`byType`). Authors `fieldConfig.overrides[]` — a matcher + the properties it
// sets — bounded by the shared cap (mirroring the host). Phase 1 keeps the property editor minimal (a
// dotted Grafana id + a value), accepted verbatim so import stays a copy; the deeper per-property UI is
// a Phase-2 follow-up. The point of shipping it now is that overrides round-trip through the ONE
// (de)serializer, so add≠edit can never reappear when the property UI grows. One responsibility: edit
// the override list.

import { Button } from "@/components/ui/button";
import type { EditorState } from "../cellEditorState";
import type { FieldConfig, FieldOverride, Matcher } from "@/lib/dashboard";
import { MAX_OVERRIDES } from "../../fieldconfig/caps";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

export function OverridesTab({ state, patch }: Props) {
  const fc: FieldConfig = state.fieldConfig ?? { defaults: {}, overrides: [] };
  const overrides = fc.overrides ?? [];

  const setOverrides = (next: FieldOverride[]) => patch({ fieldConfig: { ...fc, overrides: next } });
  const addOverride = () => {
    if (overrides.length >= MAX_OVERRIDES) return;
    setOverrides([...overrides, { matcher: { id: "byName", options: "" }, properties: [] }]);
  };
  const setMatcher = (idx: number, matcher: Matcher) =>
    setOverrides(overrides.map((o, i) => (i === idx ? { ...o, matcher } : o)));
  const setProp = (idx: number, id: string, value: string) =>
    setOverrides(
      overrides.map((o, i) => (i === idx ? { ...o, properties: id ? [{ id, value }] : [] } : o)),
    );
  const remove = (idx: number) => setOverrides(overrides.filter((_, i) => i !== idx));

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="overrides tab">
      {overrides.length === 0 && <p className="text-muted">No field overrides. Add one to style a specific field.</p>}
      {overrides.map((over, idx) => (
        <div key={idx} className="grid gap-1.5 rounded-md border border-border bg-bg p-2" aria-label={`override ${idx}`}>
          <div className="flex items-center gap-1.5">
            {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet */}
            <select
              aria-label={`override ${idx} matcher`}
              className={`${FIELD} w-28`}
              value={over.matcher.id}
              onChange={(e) => setMatcher(idx, { id: e.target.value as Matcher["id"], options: over.matcher.options })}
            >
              <option value="byName">by name</option>
              <option value="byType">by type</option>
            </select>
            {/* eslint-disable-next-line no-restricted-syntax -- styled native input */}
            <input
              aria-label={`override ${idx} match`}
              className={`${FIELD} flex-1`}
              placeholder={over.matcher.id === "byType" ? "number | string | time" : "field name"}
              value={String(over.matcher.options ?? "")}
              onChange={(e) => setMatcher(idx, { id: over.matcher.id, options: e.target.value })}
            />
            <Button
              variant="ghost"
              aria-label={`remove override ${idx}`}
              className="h-auto px-1.5 text-muted hover:text-red-500"
              onClick={() => remove(idx)}
            >
              ×
            </Button>
          </div>
          <div className="flex items-center gap-1.5">
            {/* eslint-disable-next-line no-restricted-syntax -- styled native input; dotted Grafana id verbatim */}
            <input
              aria-label={`override ${idx} prop id`}
              className={`${FIELD} w-32`}
              placeholder="unit | decimals | custom.lineWidth"
              value={over.properties[0]?.id ?? ""}
              onChange={(e) => setProp(idx, e.target.value, String(over.properties[0]?.value ?? ""))}
            />
            {/* eslint-disable-next-line no-restricted-syntax -- styled native input */}
            <input
              aria-label={`override ${idx} prop value`}
              className={`${FIELD} flex-1`}
              placeholder="value"
              value={String(over.properties[0]?.value ?? "")}
              onChange={(e) => setProp(idx, over.properties[0]?.id ?? "", e.target.value)}
            />
          </div>
        </div>
      ))}
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
