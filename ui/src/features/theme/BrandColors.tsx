// Brand Colors — per-token hand-tweak of the base palette, inside a collapsible accordion. Editing a
// token promotes the current theme to a `custom` theme (seeded from the live computed palette for BOTH
// modes so the un-edited mode is preserved), overriding just the edited token in the ACTIVE mode. This
// is why charts/panels re-theme from a brand tweak: we write base tokens, not shadcn tokens.
// One component per file (FILE-LAYOUT).

import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { ColorPicker } from "@/components/ui/color-picker";
import { Label } from "@/components/ui/label";
import { BASE_TOKENS, readComputedBasePalette, useTheme, type BasePalette, type CustomTheme } from "@/lib/theme";

export function BrandColors() {
  const { theme, setCustom } = useTheme();

  // The palette currently on screen (custom override, or the resolved preset/static block).
  const active: BasePalette = theme.custom
    ? theme.custom[theme.mode]
    : readComputedBasePalette();

  const editToken = (key: keyof BasePalette, triplet: string) => {
    // Seed both modes from the current custom theme, or from the live computed palette for the active
    // mode (leaving the other mode as whatever the custom theme / a fresh read gives). This keeps the
    // opposite mode's colors intact when tweaking one mode.
    const base: CustomTheme = theme.custom ?? {
      light: readComputedBasePaletteFor("light", theme.mode, active),
      dark: readComputedBasePaletteFor("dark", theme.mode, active),
    };
    setCustom({
      ...base,
      [theme.mode]: { ...base[theme.mode], [key]: triplet },
    });
  };

  return (
    <Accordion type="single" collapsible className="rounded-md border border-border">
      <AccordionItem value="brand-colors">
        <AccordionTrigger className="px-3 py-2.5 hover:bg-panel/40">
          <Label className="cursor-pointer">Brand colors ({theme.mode})</Label>
        </AccordionTrigger>
        <AccordionContent className="space-y-2 border-t border-border bg-panel/20 px-3 py-3">
          {BASE_TOKENS.map((t) => (
            <ColorPicker
              key={t.key}
              label={t.label}
              value={active[t.key]}
              onChange={(triplet) => editToken(t.key, triplet)}
            />
          ))}
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  );
}

/** The seed palette for `mode`: if it's the active mode, use the on-screen values; otherwise read the
 *  computed root (which reflects the active mode) as a reasonable starting point for the other mode too.
 *  The other mode is refined the moment the user switches to it and edits. */
function readComputedBasePaletteFor(mode: "light" | "dark", activeMode: string, active: BasePalette): BasePalette {
  return mode === activeMode ? active : readComputedBasePalette();
}
