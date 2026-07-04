// The preset picker — the built-in accent presets + the curated library, as a labelled select with a
// swatch preview, plus a Random button. Selecting a preset drives `setPreset` (which clears any custom/
// imported override). DATA comes from the theme layer (`THEME_PRESETS` + `BUILTIN_PRESETS`); this
// component has no per-preset branch. One component per file (FILE-LAYOUT).

import { Dices } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { BUILTIN_PRESETS, THEME_PRESETS, useTheme } from "@/lib/theme";

const BUILTIN_LABELS: Record<string, string> = { amber: "Amber", teal: "Teal", blue: "Blue" };

/** All selectable presets: built-in accents first, then the curated library. */
const OPTIONS: ReadonlyArray<{ value: string; name: string }> = [
  ...BUILTIN_PRESETS.map((v) => ({ value: v, name: BUILTIN_LABELS[v] ?? v })),
  ...THEME_PRESETS.map((p) => ({ value: p.value, name: p.name })),
];

export function PresetPicker() {
  const { theme, setPreset } = useTheme();

  const randomize = () => {
    const pick = OPTIONS[Math.floor(Math.random() * OPTIONS.length)];
    setPreset(pick.value);
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <Label>Theme preset</Label>
        <Button type="button" variant="ghost" size="sm" className="h-7 px-2 text-xs" onClick={randomize}>
          <Dices className="mr-1 h-3.5 w-3.5" />
          Random
        </Button>
      </div>
      <Select
        aria-label="Theme preset"
        value={theme.preset}
        onChange={(e) => setPreset(e.target.value)}
      >
        {OPTIONS.map((o) => (
          <option key={o.value} value={o.value}>
            {o.name}
          </option>
        ))}
      </Select>
    </div>
  );
}
