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

// "" is the sentinel for "None" — no explicit color scheme (stored as an absent `color`, which the
// resolver treats as the neutral accent). Every other entry is a real `FieldColorModeId`.
const NONE = "";
const MODES: Array<{ value: FieldColorModeId | typeof NONE; label: string }> = [
  { value: NONE, label: "None" },
  { value: "thresholds", label: "From thresholds" },
  { value: "fixed", label: "Single color" },
  { value: "palette-classic", label: "Classic palette" },
  { value: "palette-classic-by-name", label: "Classic (by name)" },
  { value: "continuous-GrYlRd", label: "Green-Yellow-Red" },
  { value: "continuous-RdYlGr", label: "Red-Yellow-Green" },
  { value: "continuous-viridis", label: "Viridis" },
];

export function ColorSchemeEditor({ value, onChange }: Props) {
  // An absent `color` reads as "None" (no scheme) — not a silently-defaulted "From thresholds". The
  // author picks thresholds explicitly when they want it.
  const mode = value?.mode ?? NONE;
  return (
    <div className="grid gap-1.5" aria-label="color scheme editor">
      <Select
        aria-label="color mode"
        className="h-8"
        value={mode}
        onChange={(e) => {
          const next = e.target.value as FieldColorModeId | typeof NONE;
          // "None" clears the scheme back to absent (the neutral default); every other mode materializes
          // its `FieldColor` (fixed also seeds a swatch so the reveal isn't empty).
          if (next === NONE) onChange(undefined);
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
