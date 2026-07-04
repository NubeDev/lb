// The glass-intensity picker — a segmented control writing the OPTIONAL `glass` axis (undefined =
// inherit the look). Only meaningful under a glass surface, so it renders ONLY when the resolved
// surface is `glass` (a member on a flat/elevated surface has nothing to tune). DATA is `GLASS_OPTIONS`;
// each option writes `data-glass` via the resolver → theme-dom, which scales the glass tokens in
// `globals.css`. No per-surface branch. One component per file (FILE-LAYOUT).

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { GLASS_OPTIONS, resolveAppearance, useTheme } from "@/lib/theme";

export function GlassPicker() {
  const { theme, setGlass } = useTheme();
  const resolved = resolveAppearance(theme);
  // Nothing to tune unless the surface is glass — hide the control entirely rather than show a dead knob.
  if (resolved.surface !== "glass") return null;
  return (
    <div className="space-y-2">
      <Label>Glass intensity</Label>
      <div className="grid grid-cols-3 gap-2" role="group" aria-label="Glass intensity">
        {GLASS_OPTIONS.map((o) => {
          const selected = resolved.glass === o.value;
          return (
            <Button
              key={o.value}
              type="button"
              size="sm"
              variant={selected ? "default" : "outline"}
              aria-pressed={selected}
              aria-label={o.label}
              title={o.hint}
              onClick={() => setGlass(o.value)}
              className={selected ? "px-2 text-xs" : "px-2 text-xs text-muted"}
            >
              {o.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
}
