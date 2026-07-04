// The `color-picker` primitive — a labelled single-color control whose WHOLE ROW is the trigger and
// whose editor is a hand-authored, in-DOM popover (H/S/L range inputs + a hex field), NOT a native
// `<input type="color">`. Why: the native input made only its 24×32px swatch a hit target (the row was
// dead), and WebKitGTK — the Tauri Linux webview — ships NO native color input, so the desktop click
// was a silent no-op (the shipped bug). This in-DOM popover works identically on every engine, is
// keyboard-operable, and adds no dependency. The stored value is the project's HSL triplet ("H S% L%");
// H/S/L edits and the hex field both round-trip through the theme layer's triplet helpers.
// One primitive per file (FILE-LAYOUT).

import * as React from "react";

import { cn } from "@/lib/utils";
import { colorToHslTriplet } from "@/lib/theme";
import { formatTriplet, hslToHex, parseTriplet, type Hsl } from "@/lib/theme/hsl-triplet";

interface ColorPickerProps {
  label: string;
  /** The current value as an HSL triplet "H S% L%". */
  value: string;
  /** Emits a new HSL triplet. */
  onChange: (triplet: string) => void;
  className?: string;
}

const CHANNELS: ReadonlyArray<{ key: keyof Hsl; label: string; max: number; unit: string }> = [
  { key: "h", label: "Hue", max: 360, unit: "°" },
  { key: "s", label: "Saturation", max: 100, unit: "%" },
  { key: "l", label: "Lightness", max: 100, unit: "%" },
];

export function ColorPicker({ label, value, onChange, className }: ColorPickerProps) {
  const [open, setOpen] = React.useState(false);
  const rootRef = React.useRef<HTMLDivElement>(null);
  const hsl = parseTriplet(value) ?? { h: 0, s: 0, l: 0 };
  const hex = hslToHex(hsl);

  // Close on outside click / Escape — the popover is in-DOM, so we manage dismissal ourselves.
  React.useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const setChannel = (key: keyof Hsl, n: number) => onChange(formatTriplet({ ...hsl, [key]: n }));

  const onHex = (raw: string) => {
    const triplet = colorToHslTriplet(raw.trim());
    if (triplet) onChange(triplet);
  };

  return (
    <div ref={rootRef} className={cn("relative", className)}>
      {/* Whole-row trigger — the fix for "only the swatch was clickable". */}
      <button
        type="button"
        aria-label={`${label}: ${value || "unset"}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center justify-between gap-2 rounded-md px-1 py-1 text-left transition-colors hover:bg-panel/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30"
      >
        <span className="text-xs text-fg">{label}</span>
        <span className="flex items-center gap-1.5">
          <span className="font-mono text-[11px] text-muted">{value || "—"}</span>
          <span
            aria-hidden
            className="h-4 w-6 rounded-sm border border-border"
            style={{ backgroundColor: hex }}
          />
        </span>
      </button>

      {open && (
        <div
          role="dialog"
          aria-label={`${label} color`}
          className="absolute right-0 z-50 mt-1 w-56 space-y-2.5 rounded-md border border-border bg-panel p-3 shadow-lg"
        >
          {CHANNELS.map((ch) => (
            <label key={ch.key} className="block space-y-1">
              <span className="flex items-center justify-between text-[11px] text-muted">
                <span>{ch.label}</span>
                <span className="font-mono text-fg">
                  {Math.round(hsl[ch.key])}
                  {ch.unit}
                </span>
              </span>
              <input
                type="range"
                aria-label={ch.label}
                min={0}
                max={ch.max}
                value={Math.round(hsl[ch.key])}
                onChange={(e) => setChannel(ch.key, Number(e.target.value))}
                className="h-1.5 w-full cursor-pointer accent-[hsl(var(--accent))]"
              />
            </label>
          ))}

          <label className="flex items-center gap-2">
            <span className="text-[11px] text-muted">Hex</span>
            <input
              type="text"
              aria-label={`${label} hex`}
              defaultValue={hex}
              key={hex}
              onBlur={(e) => onHex(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") onHex((e.target as HTMLInputElement).value);
              }}
              spellCheck={false}
              className="min-w-0 flex-1 rounded-sm border border-border bg-bg px-2 py-1 font-mono text-xs text-fg focus-visible:border-accent focus-visible:outline-none"
            />
          </label>
        </div>
      )}
    </div>
  );
}
