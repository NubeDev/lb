// The look picker — a grid of one-click look cards. Picking a look RESETS the axes it defines (the
// provider's `setLook` → `applyLook`), so it lands like its thumbnail; a one-line hint says so. DATA is
// `THEME_LOOKS`; no per-look branch (the id is opaque). One component per file (FILE-LAYOUT).

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { THEME_LOOKS, useTheme } from "@/lib/theme";

export function LookPicker() {
  const { theme, setLook } = useTheme();
  return (
    <div className="space-y-2">
      <Label>Look</Label>
      <div className="grid grid-cols-2 gap-2" role="group" aria-label="Look pack">
        {THEME_LOOKS.map((look) => {
          const selected = theme.look === look.id;
          return (
            <Button
              key={look.id}
              type="button"
              variant={selected ? "default" : "outline"}
              aria-pressed={selected}
              aria-label={look.label}
              onClick={() => setLook(look.id)}
              className={`h-auto flex-col items-start whitespace-normal p-2.5 text-left ${
                selected ? "" : "hover:bg-panel/50"
              }`}
            >
              <span className="block text-xs font-medium">{look.label}</span>
              <span className={`mt-0.5 block text-[11px] leading-tight ${selected ? "opacity-90" : "text-muted"}`}>
                {look.blurb}
              </span>
            </Button>
          );
        })}
      </div>
      <p className="text-[11px] text-muted">Picking a look resets its colors, fonts, radius, surface, and motion.</p>
    </div>
  );
}
