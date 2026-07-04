// The Customizer's Layout tab — sidebar Variant / Collapsible Mode / Position, ported from the
// shadcn-store template. Each control drives `theme.layout`, which `NavRail` spreads onto the shipped
// shadcn `<Sidebar>` (variant/collapsible/side), so the shell chrome re-lays-out live and the choice
// persists through the same `ui_theme` prefs blob as the Theme tab. DATA is the closed axis lists from
// the theme layer; no per-option branch beyond the diagram. One component per file (FILE-LAYOUT).

import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import {
  SIDEBAR_COLLAPSIBLES,
  SIDEBAR_SIDES,
  SIDEBAR_VARIANTS,
  useTheme,
  type SidebarCollapsible,
  type SidebarSide,
  type SidebarVariant,
} from "@/lib/theme";

import { OptionCard } from "./layout/OptionCard";
import { CollapsibleDiagram, SideDiagram, VariantDiagram } from "./layout/SidebarMiniDiagram";

const VARIANT_LABEL: Record<SidebarVariant, string> = { sidebar: "Default", floating: "Floating", inset: "Inset" };
const VARIANT_HINT: Record<SidebarVariant, string> = {
  sidebar: "Standard sidebar layout",
  floating: "Floating sidebar with border",
  inset: "Inset with rounded-md corners",
};
const COLLAPSIBLE_LABEL: Record<SidebarCollapsible, string> = { offcanvas: "Off Canvas", icon: "Icon", none: "None" };
const COLLAPSIBLE_HINT: Record<SidebarCollapsible, string> = {
  offcanvas: "Slides out of view",
  icon: "Collapses to icon only",
  none: "Always visible",
};
const SIDE_LABEL: Record<SidebarSide, string> = { left: "Left", right: "Right" };
const SIDE_HINT: Record<SidebarSide, string> = { left: "Sidebar on the left", right: "Sidebar on the right" };

export function LayoutTab() {
  const { theme, setLayout } = useTheme();
  const { variant, collapsible, side } = theme.layout;

  return (
    <div className="space-y-5 p-4">
      {/* Sidebar Variant */}
      <div className="space-y-2">
        <div>
          <Label>Sidebar variant</Label>
          <p className="mt-1 text-xs text-muted">{VARIANT_HINT[variant]}</p>
        </div>
        <div className="grid grid-cols-3 gap-2">
          {SIDEBAR_VARIANTS.map((v) => (
            <OptionCard
              key={v}
              name={VARIANT_LABEL[v]}
              aria-label={`Sidebar variant ${VARIANT_LABEL[v]}`}
              selected={variant === v}
              onSelect={() => setLayout({ variant: v })}
            >
              <VariantDiagram variant={v} />
            </OptionCard>
          ))}
        </div>
      </div>

      <Separator />

      {/* Sidebar Collapsible Mode */}
      <div className="space-y-2">
        <div>
          <Label>Collapsible mode</Label>
          <p className="mt-1 text-xs text-muted">{COLLAPSIBLE_HINT[collapsible]}</p>
        </div>
        <div className="grid grid-cols-3 gap-2">
          {SIDEBAR_COLLAPSIBLES.map((c) => (
            <OptionCard
              key={c}
              name={COLLAPSIBLE_LABEL[c]}
              aria-label={`Collapsible mode ${COLLAPSIBLE_LABEL[c]}`}
              selected={collapsible === c}
              onSelect={() => setLayout({ collapsible: c })}
            >
              <CollapsibleDiagram collapsible={c} />
            </OptionCard>
          ))}
        </div>
      </div>

      <Separator />

      {/* Sidebar Position */}
      <div className="space-y-2">
        <div>
          <Label>Position</Label>
          <p className="mt-1 text-xs text-muted">{SIDE_HINT[side]}</p>
        </div>
        <div className="grid grid-cols-2 gap-2">
          {SIDEBAR_SIDES.map((s) => (
            <OptionCard
              key={s}
              name={SIDE_LABEL[s]}
              aria-label={`Sidebar position ${SIDE_LABEL[s]}`}
              selected={side === s}
              onSelect={() => setLayout({ side: s })}
            >
              <SideDiagram side={s} />
            </OptionCard>
          ))}
        </div>
      </div>
    </div>
  );
}
