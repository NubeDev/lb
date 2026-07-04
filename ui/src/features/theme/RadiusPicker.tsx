// The radius picker — a row of stops writing the single global `--radius`. DATA is `THEME_RADIUS_OPTIONS`
// from the theme layer. One component per file (FILE-LAYOUT).

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { THEME_RADIUS_OPTIONS, useTheme } from "@/lib/theme";

export function RadiusPicker() {
  const { theme, setRadius } = useTheme();
  return (
    <div className="space-y-2">
      <Label>Radius</Label>
      <div className="grid grid-cols-5 gap-2" role="group" aria-label="Corner radius">
        {THEME_RADIUS_OPTIONS.map((o) => {
          const selected = theme.radius === o.value;
          return (
            <Button
              key={o.value}
              type="button"
              size="sm"
              variant={selected ? "default" : "outline"}
              aria-label={`Radius ${o.label}`}
              aria-pressed={selected}
              onClick={() => setRadius(o.value)}
              className={selected ? "px-2" : "px-2 text-muted"}
            >
              {o.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
}
