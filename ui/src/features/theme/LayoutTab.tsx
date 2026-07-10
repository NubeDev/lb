// The Customizer's Layout tab — sidebar Variant / Collapsible Mode / Position, ported from the
// shadcn-store template. Each control drives `theme.layout`, which `NavRail` spreads onto the shipped
// shadcn `<Sidebar>` (variant/collapsible/side), so the shell chrome re-lays-out live and the choice
// persists through the same `ui_theme` prefs blob as the Theme tab. DATA is the closed axis lists from
// the theme layer; no per-option branch beyond the diagram. One component per file (FILE-LAYOUT).

import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import {
  HEADER_STYLES,
  NAV_MODES,
  SIDEBAR_COLLAPSIBLES,
  SIDEBAR_SIDES,
  SIDEBAR_VARIANTS,
  useTheme,
  type HeaderStyle,
  type NavMode,
  type SidebarCollapsible,
  type SidebarSide,
  type SidebarVariant,
} from "@/lib/theme";

import { OptionCard } from "./layout/OptionCard";
import {
  CollapsibleDiagram,
  HeaderDiagram,
  NavDiagram,
  SideDiagram,
  VariantDiagram,
} from "./layout/SidebarMiniDiagram";

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
const HEADER_LABEL: Record<HeaderStyle, string> = { band: "Band", breadcrumbs: "Breadcrumbs" };
const HEADER_HINT: Record<HeaderStyle, string> = {
  band: "Today's icon-chip title strip",
  breadcrumbs: "A Workspace / Page crumb trail",
};
const NAV_LABEL: Record<NavMode, string> = { sidebar: "Sidebar", topmenu: "Top menu" };
const NAV_HINT: Record<NavMode, string> = {
  sidebar: "Left navigation rail (today)",
  topmenu: "Horizontal menu bar above content",
};

export function LayoutTab() {
  const { theme, setLayout } = useTheme();
  const { variant, collapsible, side, header, nav } = theme.layout;
  // When the nav mode is `topmenu`, the sidebar-specific axes (variant/collapsible/side) no longer
  // affect the layout — they're kept (never cleared) but visibly marked "sidebar only" so there's no
  // hidden state and no dead end. Switching back to `sidebar` restores them intact.
  const sidebarAxesInactive = nav === "topmenu";

  return (
    <div className="space-y-5 p-4">
      {/* Header style (shell-chrome-layout scope) — Band | Breadcrumbs */}
      <div className="space-y-2">
        <div>
          <Label>Header style</Label>
          <p className="mt-1 text-xs text-muted">{HEADER_HINT[header]}</p>
        </div>
        <div className="grid grid-cols-2 gap-2">
          {HEADER_STYLES.map((h) => (
            <OptionCard
              key={h}
              name={HEADER_LABEL[h]}
              aria-label={`Header style ${HEADER_LABEL[h]}`}
              selected={header === h}
              onSelect={() => setLayout({ header: h })}
            >
              <HeaderDiagram header={h} />
            </OptionCard>
          ))}
        </div>
      </div>

      <Separator />

      {/* Navigation mode (shell-chrome-layout scope) — Sidebar | Top menu */}
      <div className="space-y-2">
        <div>
          <Label>Navigation</Label>
          <p className="mt-1 text-xs text-muted">{NAV_HINT[nav]}</p>
        </div>
        <div className="grid grid-cols-2 gap-2">
          {NAV_MODES.map((m) => (
            <OptionCard
              key={m}
              name={NAV_LABEL[m]}
              aria-label={`Navigation ${NAV_LABEL[m]}`}
              selected={nav === m}
              onSelect={() => setLayout({ nav: m })}
            >
              <NavDiagram nav={m} />
            </OptionCard>
          ))}
        </div>
      </div>

      <Separator />

      {/* Sidebar Variant */}
      <div className="space-y-2">
        <div>
          <Label>Sidebar variant</Label>
          <p className="mt-1 text-xs text-muted">
            {VARIANT_HINT[variant]}
            {sidebarAxesInactive && <span className="ml-1 italic text-muted/70">(sidebar only)</span>}
          </p>
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
          <p className="mt-1 text-xs text-muted">
            {COLLAPSIBLE_HINT[collapsible]}
            {sidebarAxesInactive && <span className="ml-1 italic text-muted/70">(sidebar only)</span>}
          </p>
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
          <p className="mt-1 text-xs text-muted">
            {SIDE_HINT[side]}
            {sidebarAxesInactive && <span className="ml-1 italic text-muted/70">(sidebar only)</span>}
          </p>
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
