// The motion picker — a segmented control writing the OPTIONAL motion axis (undefined = inherit the
// look). DATA is `MOTION_OPTIONS`; each writes `data-motion` via the resolver → theme-dom (and the
// `useMotionPref` JS seam reads the same value). A note explains the reduced-motion override. No
// per-motion branch. One component per file (FILE-LAYOUT).

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { MOTION_OPTIONS, resolveAppearance, useTheme } from "@/lib/theme";

export function MotionPicker() {
  const { theme, setMotion } = useTheme();
  const resolved = resolveAppearance(theme);
  return (
    <div className="space-y-2">
      <Label>Motion</Label>
      <div className="grid grid-cols-3 gap-2" role="group" aria-label="Motion profile">
        {MOTION_OPTIONS.map((o) => {
          const selected = resolved.motion === o.value;
          return (
            <Button
              key={o.value}
              type="button"
              size="sm"
              variant={selected ? "default" : "outline"}
              aria-pressed={selected}
              aria-label={o.label}
              title={o.hint}
              onClick={() => setMotion(o.value)}
              className={selected ? "px-2 text-xs" : "px-2 text-xs text-muted"}
            >
              {o.label}
            </Button>
          );
        })}
      </div>
      <p className="text-[11px] text-muted">
        Your system’s reduced-motion setting forces motion off unless you explicitly choose Full.
      </p>
    </div>
  );
}
