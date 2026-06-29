// The Field tab (viz panel-editor scope; field-config scope owns the semantics) — authors
// `fieldConfig.defaults`: the per-FIELD option set (displayName, unit, decimals, min/max, noValue,
// thresholds, color) + the timeseries draw-style `custom` bag. Editing here writes the typed
// `FieldConfig` onto the editor state; the render bridge formats values through user-prefs, never a
// local format. The unit dropdown is the single `units.ts` table (no free-typed unit). One
// responsibility: edit the default field options. Thresholds get their own small sub-component to keep
// this file focused.

import type { EditorState } from "../cellEditorState";
import type { FieldConfig, FieldOptions } from "@/lib/dashboard";
import { unitOptions } from "../../fieldconfig/units";
import { defaultTimeseriesCustom, readTimeseriesCustom, type GraphDrawStyle } from "../../views/timeseries/custom";
import { ThresholdsEditor } from "./ThresholdsEditor";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

export function FieldTab({ state, patch }: Props) {
  const fc: FieldConfig = state.fieldConfig ?? { defaults: {}, overrides: [] };
  const defaults = fc.defaults;
  const custom = readTimeseriesCustom(defaults.custom);

  const setDefaults = (next: Partial<FieldOptions>) =>
    patch({ fieldConfig: { ...fc, defaults: { ...defaults, ...next } } });
  const setCustom = (next: Partial<typeof custom>) =>
    setDefaults({ custom: { ...defaultTimeseriesCustom(), ...defaults.custom, ...next } });

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="field tab">
      <label className="grid gap-1 text-muted">
        Display name
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Input variant; styled native */}
        <input
          aria-label="field displayName"
          className={FIELD}
          value={defaults.displayName ?? ""}
          onChange={(e) => setDefaults({ displayName: e.target.value || undefined })}
        />
      </label>

      <label className="grid gap-1 text-muted">
        Unit
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (dashboard.md follow-up) */}
        <select
          aria-label="field unit"
          className={FIELD}
          value={defaults.unit ?? ""}
          onChange={(e) => setDefaults({ unit: e.target.value || undefined })}
        >
          <option value="">none</option>
          {unitOptions().map((u) => (
            <option key={u.id} value={u.id}>
              {u.id} {u.label ? `(${u.label})` : ""}
            </option>
          ))}
        </select>
      </label>

      <div className="grid grid-cols-3 gap-2">
        <label className="grid gap-1 text-muted">
          Decimals
          {/* eslint-disable-next-line no-restricted-syntax -- styled native number input */}
          <input
            aria-label="field decimals"
            type="number"
            className={FIELD}
            value={defaults.decimals ?? ""}
            onChange={(e) => setDefaults({ decimals: e.target.value === "" ? undefined : Number(e.target.value) })}
          />
        </label>
        <label className="grid gap-1 text-muted">
          Min
          {/* eslint-disable-next-line no-restricted-syntax -- styled native number input */}
          <input
            aria-label="field min"
            type="number"
            className={FIELD}
            value={defaults.min ?? ""}
            onChange={(e) => setDefaults({ min: e.target.value === "" ? undefined : Number(e.target.value) })}
          />
        </label>
        <label className="grid gap-1 text-muted">
          Max
          {/* eslint-disable-next-line no-restricted-syntax -- styled native number input */}
          <input
            aria-label="field max"
            type="number"
            className={FIELD}
            value={defaults.max ?? ""}
            onChange={(e) => setDefaults({ max: e.target.value === "" ? undefined : Number(e.target.value) })}
          />
        </label>
      </div>

      <label className="grid gap-1 text-muted">
        No value
        {/* eslint-disable-next-line no-restricted-syntax -- styled native input */}
        <input
          aria-label="field noValue"
          className={FIELD}
          value={defaults.noValue ?? ""}
          placeholder="text when null/empty"
          onChange={(e) => setDefaults({ noValue: e.target.value || undefined })}
        />
      </label>

      {state.view === "timeseries" && (
        <label className="grid gap-1 text-muted" data-options-group="drawstyle">
          Draw style
          {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (dashboard.md follow-up) */}
          <select
            aria-label="field drawStyle"
            className={FIELD}
            value={custom.drawStyle}
            onChange={(e) => setCustom({ drawStyle: e.target.value as GraphDrawStyle })}
          >
            <option value="line">line</option>
            <option value="bars">bars</option>
            <option value="points">points</option>
          </select>
        </label>
      )}

      <div className="mt-1 grid gap-1" data-options-group="thresholds">
        <div className="font-medium text-muted">Thresholds</div>
        <ThresholdsEditor
          value={defaults.thresholds}
          onChange={(thresholds) => setDefaults({ thresholds })}
        />
      </div>
    </div>
  );
}
