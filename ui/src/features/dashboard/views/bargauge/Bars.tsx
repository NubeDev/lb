// The bargauge bar markup (viz chart-types scope, the `bargauge` `displayMode`/`orientation`/
// `showUnfilled` options). One filled track per value; the fill width/height is the value's fraction of
// canonical min/max, the fill color is the resolved threshold color (passed in — this file never
// resolves a color or formats a number; the panel does, through the bridges). `gradient` fades the fill,
// `lcd` segments it, `basic` is a solid fill. One responsibility: draw the bars from pre-resolved data.

import type { CSSProperties } from "react";

interface Bar {
  /** 0..1 fill fraction of the canonical min/max range. */
  fraction: number;
  /** The resolved threshold/fixed color (from `field.ts#valueColor`). */
  color: string;
  /** The value, already formatted through the one user-prefs bridge. */
  text: string;
}

/** Build the CSS fill for a bar by display mode — `gradient` fades to the color, `lcd` is a segmented
 *  repeating track, `basic` is a flat fill. Uses the resolved color directly (theme-correct already). */
function fillStyle(color: string, mode: "basic" | "lcd" | "gradient"): CSSProperties {
  if (mode === "gradient") return { background: `linear-gradient(90deg, ${color}33, ${color})` };
  if (mode === "lcd")
    return {
      background: `repeating-linear-gradient(90deg, ${color} 0 6px, transparent 6px 8px)`,
    };
  return { background: color };
}

export function BarGaugeBars({
  bars,
  horizontal,
  displayMode,
  showUnfilled,
  showValue,
}: {
  bars: Bar[];
  horizontal: boolean;
  displayMode: "basic" | "lcd" | "gradient";
  showUnfilled: boolean;
  showValue: boolean;
}) {
  return (
    <div
      className={`flex min-h-0 flex-1 gap-2 p-1 ${horizontal ? "flex-col justify-center" : "flex-row items-end justify-center"}`}
      aria-label="bargauge bars"
      data-display-mode={displayMode}
    >
      {bars.map((bar, i) => (
        <div
          key={i}
          className={horizontal ? "flex items-center gap-2" : "flex h-full flex-col items-center justify-end gap-1"}
        >
          <div
            className={`overflow-hidden rounded ${horizontal ? "h-3.5 flex-1" : "h-full w-3.5"} ${showUnfilled ? "bg-border/40" : ""}`}
            role="img"
            aria-label={`bargauge bar ${i}`}
          >
            <div
              className="rounded transition-[width,height]"
              style={{
                ...fillStyle(bar.color, displayMode),
                ...(horizontal
                  ? { width: `${bar.fraction * 100}%`, height: "100%" }
                  : { height: `${bar.fraction * 100}%`, width: "100%" }),
              }}
            />
          </div>
          {showValue && (
            <span className="text-xs font-medium tabular-nums" style={{ color: bar.color }} aria-label={`bargauge value ${i}`}>
              {bar.text}
            </span>
          )}
        </div>
      ))}
    </div>
  );
}
