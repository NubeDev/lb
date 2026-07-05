// Icon colors — the per-sidebar-icon color overrides, in a collapsible accordion. One click enables
// colorization by AUTO-ASSIGNING every rail surface a color from the 100-color palette (evenly
// hue-spread); each row then opens the swatch picker to hand-edit one icon, or paste a custom hex.
// The whole feature rides the existing `ui_theme` blob (the `iconColors` axis): presence = ON, absence
// = OFF, so "Clear all" fully reverts to default-fg icons. One component per file (FILE-LAYOUT).

import { Wand2 } from "lucide-react";

import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { autoAssignIconColors, useTheme } from "@/lib/theme";
import { RAIL_SURFACES } from "@/features/shell";

import { IconColorSwatch } from "./IconColorSwatch";

export function IconColors() {
  const { theme, setIconColors, setIconColor } = useTheme();
  const enabled = !!theme.iconColors;

  const enable = () => {
    // First enable: auto-assign every rail surface a palette color (evenly hue-spread, deterministic).
    setIconColors(autoAssignIconColors(RAIL_SURFACES.map((s) => s.key)));
  };

  return (
    <Accordion type="single" collapsible className="rounded-md border border-border">
      <AccordionItem value="icon-colors">
        <AccordionTrigger className="px-3 py-2.5 hover:bg-panel/40">
          <Label className="cursor-pointer">Icon colors{enabled ? ` (${Object.keys(theme.iconColors!).length})` : ""}</Label>
        </AccordionTrigger>
        <AccordionContent className="space-y-3 border-t border-border bg-panel/20 px-3 py-3">
          {!enabled ? (
            <div className="space-y-2">
              <p className="text-xs text-muted">
                Colorize each sidebar icon. A palette of 100 colors is auto-assigned; edit any icon after.
              </p>
              <Button type="button" variant="outline" size="sm" className="w-full" onClick={enable}>
                <Wand2 className="mr-1.5 h-3.5 w-3.5" />
                Auto-assign colors
              </Button>
            </div>
          ) : (
            <>
              <div className="flex items-center gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="flex-1"
                  onClick={() => setIconColors(autoAssignIconColors(RAIL_SURFACES.map((s) => s.key)))}
                >
                  <Wand2 className="mr-1.5 h-3.5 w-3.5" />
                  Re-run auto-assign
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="flex-1"
                  onClick={() => setIconColors(undefined)}
                >
                  Clear all
                </Button>
              </div>
              <p className="text-[11px] text-muted">
                Click a swatch to edit. Icons render in the default color when cleared.
              </p>
              <div className="space-y-0.5">
                {RAIL_SURFACES.map((s) => (
                  <IconColorSwatch
                    key={s.key}
                    surface={s.key}
                    label={s.label}
                    value={theme.iconColors![s.key]}
                    onClear={() => setIconColor(s.key, undefined)}
                  />
                ))}
              </div>
            </>
          )}
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  );
}
