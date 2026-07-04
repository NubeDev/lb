// The light/dark mode toggle inside the Theme tab (Settings → Theme).
// On a custom/imported theme, flipping mode re-applies the correct light/dark variant automatically —
// `theme-dom` reads the active mode's palette. One component per file (FILE-LAYOUT).

import { Moon, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { THEME_MODE_OPTIONS, useTheme } from "@/lib/theme";

const ICON = { dark: Moon, light: Sun } as const;

export function ModeToggle() {
  const { theme, setMode } = useTheme();
  return (
    <div className="space-y-2">
      <Label>Mode</Label>
      <div className="grid grid-cols-2 gap-2" role="group" aria-label="Color mode">
        {THEME_MODE_OPTIONS.map((o) => {
          const Icon = ICON[o.value];
          const selected = theme.mode === o.value;
          return (
            <Button
              key={o.value}
              type="button"
              size="sm"
              variant={selected ? "default" : "outline"}
              aria-label={`Use ${o.label.toLowerCase()} mode`}
              aria-pressed={selected}
              onClick={() => setMode(o.value)}
              className={selected ? undefined : "text-muted"}
            >
              <Icon className="h-4 w-4" />
              {o.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
}
