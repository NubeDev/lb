// The color-scheme picker (editor-parity scope, step 2) — authors `fieldConfig.defaults.color`
// (a `FieldColor`: mode + optional fixedColor), the exact shape `fieldconfig/color.ts` resolves at
// render. Before this, the color scheme had no editor at all. A `fixed` mode reveals the swatch picker;
// the other modes (thresholds / palettes / continuous schemes) store just the mode. One responsibility:
// edit a field's color scheme.

import { Select } from "@/components/ui/select";
import { ColorSwatchPicker } from "@/components/ui/color-swatch";
import type { FieldColor, FieldColorModeId } from "@/lib/dashboard";
import { COLOR_SWATCHES } from "../palette";

interface Props {
  value: FieldColor | undefined;
  onChange: (next: FieldColor | undefined) => void;
}

const MODES: Array<{ value: FieldColorModeId; label: string }> = [
  { value: "thresholds", label: "From thresholds" },
  { value: "fixed", label: "Single color" },
  { value: "palette-classic", label: "Classic palette" },
  { value: "palette-classic-by-name", label: "Classic (by name)" },
  { value: "continuous-GrYlRd", label: "Green-Yellow-Red" },
  { value: "continuous-RdYlGr", label: "Red-Yellow-Green" },
  { value: "continuous-viridis", label: "Viridis" },
];

export function ColorSchemeEditor({ value, onChange }: Props) {
  const mode = value?.mode ?? "thresholds";
  return (
    <div className="grid gap-1.5" aria-label="color scheme editor">
      <Select
        aria-label="color mode"
        className="h-8"
        value={mode}
        onChange={(e) => {
          const next = e.target.value as FieldColorModeId;
          // "thresholds" is Grafana's default; storing it explicitly is redundant, so clear back to
          // absent for the default and only materialize a non-default mode (keeps the round-trip clean).
          if (next === "thresholds") onChange(undefined);
          else onChange({ mode: next, ...(next === "fixed" ? { fixedColor: value?.fixedColor ?? "green" } : {}) });
        }}
      >
        {MODES.map((m) => (
          <option key={m.value} value={m.value}>
            {m.label}
          </option>
        ))}
      </Select>
      {mode === "fixed" && (
        <ColorSwatchPicker
          aria-label="fixed color"
          palette={COLOR_SWATCHES}
          value={value?.fixedColor ?? "green"}
          onChange={(fixedColor) => onChange({ mode: "fixed", fixedColor })}
        />
      )}
    </div>
  );
}
