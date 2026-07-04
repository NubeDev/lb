// The `color-picker` primitive — a labelled single-color control. A native `<input type="color">`
// (hex) plus a hex text field, bound to a token so the Customizer's Brand Colors can edit a base token.
// The stored value is the project's HSL-triplet format ("H S% L%"); this component converts hex↔triplet
// at its edges via the theme layer's `colorToHslTriplet`, so callers speak triplets and the OS picker
// speaks hex. Hand-authored, token-bound (no new dep). One primitive per file (FILE-LAYOUT).

import * as React from "react";

import { cn } from "@/lib/utils";
import { colorToHslTriplet } from "@/lib/theme";

interface ColorPickerProps {
  label: string;
  /** The current value as an HSL triplet "H S% L%". */
  value: string;
  /** Emits a new HSL triplet. */
  onChange: (triplet: string) => void;
  className?: string;
}

/** Convert an "H S% L%" triplet to a `#rrggbb` for the native picker. Returns "#000000" if unparseable. */
function tripletToHex(triplet: string): string {
  const m = triplet.trim().match(/^(-?\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)%\s+(-?\d+(?:\.\d+)?)%$/);
  if (!m) return "#000000";
  const h = parseFloat(m[1]);
  const s = parseFloat(m[2]) / 100;
  const l = parseFloat(m[3]) / 100;
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const mm = l - c / 2;
  let r = 0;
  let g = 0;
  let b = 0;
  if (h < 60) [r, g, b] = [c, x, 0];
  else if (h < 120) [r, g, b] = [x, c, 0];
  else if (h < 180) [r, g, b] = [0, c, x];
  else if (h < 240) [r, g, b] = [0, x, c];
  else if (h < 300) [r, g, b] = [x, 0, c];
  else [r, g, b] = [c, 0, x];
  const to = (n: number) =>
    Math.round((n + mm) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${to(r)}${to(g)}${to(b)}`;
}

export function ColorPicker({ label, value, onChange, className }: ColorPickerProps) {
  const hex = tripletToHex(value);
  const inputId = React.useId();

  return (
    <div className={cn("flex w-full items-center justify-between gap-2", className)}>
      <label htmlFor={inputId} className="text-xs text-fg">
        {label}
      </label>
      <span className="flex items-center gap-1.5">
        <input
          id={inputId}
          type="color"
          aria-label={label}
          value={hex}
          onChange={(e) => {
            const triplet = colorToHslTriplet(e.target.value);
            if (triplet) onChange(triplet);
          }}
          className="h-6 w-8 cursor-pointer rounded border border-border bg-bg p-0.5"
        />
        <span className="font-mono text-[11px] text-muted">{value || "—"}</span>
      </span>
    </div>
  );
}
