// BrandSwatch — a tiny four-colour chip previewing a brand profile's palette (reports scope). Shown
// beside each option in the BrandPicker so the choice is visual, not just a name. One responsibility:
// render the primary/accent/text/background quartet as a row of colour squares.

import type { BrandColors } from "@/lib/brand";

export function BrandSwatch({ colors }: { colors: BrandColors }) {
  const swatches: Array<[string, string]> = [
    ["primary", colors.primary],
    ["accent", colors.accent],
    ["text", colors.text],
    ["background", colors.background],
  ];
  return (
    <span className="inline-flex items-center gap-0.5" aria-hidden>
      {swatches.map(([role, c]) => (
        <span
          key={role}
          title={`${role}: ${c}`}
          className="h-3 w-3 rounded-[3px] ring-1 ring-black/10"
          style={{ background: c || "transparent" }}
        />
      ))}
    </span>
  );
}
