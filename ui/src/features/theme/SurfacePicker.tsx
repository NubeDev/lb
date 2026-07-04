// The surface picker — a segmented control writing the OPTIONAL surface axis (undefined = inherit the
// look). DATA is `SURFACE_OPTIONS`; each option writes `data-surface` via the resolver → theme-dom. No
// per-surface branch. One component per file (FILE-LAYOUT).

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { SURFACE_OPTIONS, resolveAppearance, useTheme } from "@/lib/theme";

export function SurfacePicker() {
  const { theme, setSurface } = useTheme();
  const resolved = resolveAppearance(theme);
  return (
    <div className="space-y-2">
      <Label>Surface</Label>
      <div className="grid grid-cols-3 gap-2" role="group" aria-label="Surface treatment">
        {SURFACE_OPTIONS.map((o) => {
          const selected = resolved.surface === o.value;
          return (
            <Button
              key={o.value}
              type="button"
              size="sm"
              variant={selected ? "default" : "outline"}
              aria-pressed={selected}
              aria-label={o.label}
              title={o.hint}
              onClick={() => setSurface(o.value)}
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
