// The font picker — two selects (sans + mono) writing the OPTIONAL font axes (undefined = inherit the
// look). The resolved family is shown so the member sees what the look defaults to. Selecting a family
// lazy-loads its woff2 (the provider's apply effect calls `loadFont`). DATA is `SANS_FONTS`/`MONO_FONTS`;
// no per-family branch. One component per file (FILE-LAYOUT).

import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { MONO_FONTS, SANS_FONTS, resolveAppearance, useTheme } from "@/lib/theme";

export function FontPicker() {
  const { theme, setFontSans, setFontMono } = useTheme();
  const resolved = resolveAppearance(theme);

  return (
    <div className="space-y-2">
      <Label>Fonts</Label>
      <div className="grid grid-cols-2 gap-2">
        <label className="space-y-1">
          <span className="text-[11px] text-muted">Sans</span>
          <Select
            aria-label="Sans font"
            value={resolved.fontSans}
            onChange={(e) => setFontSans(e.target.value)}
          >
            {SANS_FONTS.map((f) => (
              <option key={f.id} value={f.id}>
                {f.label}
              </option>
            ))}
          </Select>
        </label>
        <label className="space-y-1">
          <span className="text-[11px] text-muted">Mono</span>
          <Select
            aria-label="Mono font"
            value={resolved.fontMono}
            onChange={(e) => setFontMono(e.target.value)}
          >
            {MONO_FONTS.map((f) => (
              <option key={f.id} value={f.id}>
                {f.label}
              </option>
            ))}
          </Select>
        </label>
      </div>
    </div>
  );
}
