// TopMenuNav — the alternative workspace-nav mode (shell-chrome-layout scope). A sibling renderer
// to `NavRail`: fed the SAME resolved-nav data the rail consumes (resolvedItems, allowed, extSlots,
// pinned, handlers) and rendering it as a horizontal shadcn `Menubar` above the header. Each
// `SURFACE_GROUPS` bucket becomes a `MenubarMenu`; its surfaces become `MenubarItem`s. A resolved
// nav renders the same way — top-level `group`s become menus, flat entries fold into a leading
// "Menu" trigger. The Pinned favorites, the Extensions slots, and the no-lockout escape hatch +
// Sign out all appear (relocated into the menubar) so no rail affordance is lost. Extension ids
// stay opaque `ext:<id>` refs — no branch on identity (CLAUDE §10). One component per file.
//
// Visual: a FLOATING elevated menubar card (the horizontal peer of the floating-sidebar variant) —
// `--panel` surface, rounded, hairline border, soft shadow, inset from the page edges — so the nav
// reads as a self-contained control surface rather than an edge-to-edge strip. Triggers carry a
// leading icon + label (a real app menubar), the menu that OWNS the active surface lights with an
// accent tint, and the right cluster groups Settings + the account menu behind a divider so no glyph
// floats alone. The shadcn `Menubar` primitive is unchanged; only its className chrome is overridden.

import {
  Boxes,
  ChevronDown,
  Database,
  LayoutDashboard,
  LayoutGrid,
  LogOut,
  Menu as MenuIcon,
  Network,
  Pin,
  Puzzle,
  Settings,
  Undo2,
  Workflow,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarTrigger,
} from "@/components/ui/menubar";
import { cn } from "@/lib/utils";
import { useBranding } from "@/lib/branding";
import { useTheme } from "@/lib/theme";

import { BrandHeader } from "./BrandHeader";
import { itemRef } from "./nav-item-ref";
import { SURFACE_DEF } from "./surfaceDefs";
import { SURFACE_GROUPS, type CoreSurface, type ExtSlot, type ResolvedNavItem, type Surface } from "./NavRail";

interface Props {
  active: Surface;
  onSelect: (surface: Surface) => void;
  onSignOut: () => void;
  allowed: CoreSurface[];
  extSlots?: ExtSlot[];
  resolvedItems?: ResolvedNavItem[] | null;
  onSelectDashboard?: (dashboard: string, vars?: Record<string, string>) => void;
  hidden?: string[];
  pinned?: ResolvedNavItem[];
  onTogglePin?: (ref: string) => void;
  usingBuiltin?: boolean;
  onShowAllPages?: () => void;
  onUseMyMenu?: () => void;
}

/** A leading icon per built-in group bucket — a presentation concern owned by THIS top-menu renderer
 *  (the sidebar shows the bucket as a text label, so the icon lives here, not in the shared
 *  `SURFACE_GROUPS`). Keyed on the core group LABEL, which is fixed shell data — not an extension id,
 *  so rule 10 holds. An unknown bucket (a future group) falls back to a generic list glyph. */
const GROUP_ICON: Record<string, LucideIcon> = {
  Workspace: LayoutGrid,
  Automation: Workflow,
  Data: Database,
  Build: Boxes,
  System: Network,
};

/** The flat-menubar chrome overrides. The primitive ships a bordered, shadowed `--panel` pill; the
 *  FLOATING card already IS that surface, so the inner `Menubar` strips its own box (transparent, no
 *  border/shadow, auto height, no padding) and the triggers sit flush inside the card. */
const FLAT_MENUBAR = "h-auto gap-0.5 rounded-none border-0 bg-transparent p-0 shadow-none";

/** A menubar trigger styled as a desktop-menu tab: icon + label, muted by default, lighting to
 *  `--fg` + a soft accent wash on hover/open. When `active`, it holds the accent wash + `--fg` text
 *  (the "you are here" cue a menubar has instead of a filled rail pill). */
function triggerClass(active: boolean) {
  return cn(
    "h-9 gap-2 rounded-md px-3 text-[13px] font-medium text-muted transition-colors",
    "[&_svg]:size-4 [&_svg]:shrink-0 [&_svg]:opacity-80",
    "hover:bg-accent/10 hover:text-fg focus:bg-accent/10 focus:text-fg",
    "data-[state=open]:bg-accent/10 data-[state=open]:text-fg",
    active && "bg-accent/10 text-fg [&_svg]:text-accent [&_svg]:opacity-100",
  );
}

export function TopMenuNav({
  active,
  onSelect,
  onSignOut,
  allowed,
  extSlots = [],
  resolvedItems = null,
  onSelectDashboard,
  hidden = [],
  pinned = [],
  onTogglePin,
  usingBuiltin = false,
  onShowAllPages,
  onUseMyMenu,
}: Props) {
  const { brand } = useBranding();
  const { theme } = useTheme();
  const iconColorFor = (key: Surface): string | undefined =>
    theme.iconColors && typeof key === "string" ? theme.iconColors[key] : undefined;
  const pinnedRefs = new Set(pinned.map(itemRef));
  const useResolved = !!resolvedItems && resolvedItems.length > 0;
  const isHidden = (ref: string) => hidden.includes(ref);

  /** Does this menu own the active surface? Drives the active tint on the trigger. */
  const ownsActive = (items: (CoreSurface | ResolvedNavItem)[]) =>
    items.some((it) => {
      if (typeof it === "string") return active === it;
      if (it.kind === "surface") return active === it.surface;
      if (it.kind === "ext") return active === `ext:${it.ext}`;
      if (it.kind === "dashboard") return active === "dashboards";
      return false;
    });

  /** A dropdown row: icon + label, active highlight, optional pin glyph (read-only visibility in v1;
   *  the rail stays the primary pin surface). */
  const surfaceItem = (key: Surface, label: string, Icon: LucideIcon, pinRef?: string) => {
    const selected = active === key;
    const iconColor = iconColorFor(key);
    return (
      <MenubarItem
        key={`surf:${key}:${label}`}
        onSelect={(e) => {
          e.preventDefault();
          onSelect(key);
        }}
        data-active={selected || undefined}
        className="gap-2.5 py-1.5 data-[active]:bg-accent/10 data-[active]:font-medium data-[active]:text-accent"
      >
        <Icon
          className={cn("size-4", selected ? "text-accent" : "text-muted")}
          style={iconColor ? { color: iconColor } : undefined}
        />
        <span className="flex-1">{label}</span>
        {pinRef && pinnedRefs.has(pinRef) && <Pin className="size-3 fill-current text-accent" />}
      </MenubarItem>
    );
  };

  /** A resolved nav entry (nav scope) — surface/ext/dashboard → its handler. */
  const resolvedItem = (it: ResolvedNavItem) => {
    if (it.kind === "surface" && it.surface) {
      const def = SURFACE_DEF[it.surface as CoreSurface];
      return surfaceItem(it.surface as Surface, it.label, def?.icon ?? LayoutGrid, itemRef(it));
    }
    if (it.kind === "ext" && it.ext) {
      return surfaceItem(`ext:${it.ext}` as Surface, it.label, Puzzle, itemRef(it));
    }
    if (it.kind === "dashboard") {
      const selected = active === "dashboards";
      return (
        <MenubarItem
          key={`dash:${it.dashboard ?? it.label}`}
          onSelect={(e) => {
            e.preventDefault();
            it.dashboard && onSelectDashboard ? onSelectDashboard(it.dashboard, it.vars) : onSelect("dashboards");
          }}
          className="gap-2.5 py-1.5"
        >
          <LayoutDashboard className="size-4 text-muted" />
          <span className="flex-1">{it.label}</span>
          {!it.vars && pinnedRefs.has(itemRef(it)) && <Pin className="size-3 fill-current text-accent" />}
          {selected && <span className="sr-only">(current)</span>}
        </MenubarItem>
      );
    }
    return null;
  };

  /** Fallback buckets: each SURFACE_GROUPS entry → a MenubarMenu, with its group icon. */
  const fallbackMenus = SURFACE_GROUPS.map((grp) => {
    const canSee = (s: CoreSurface) => allowed.includes(s) || (s === "extensions" && allowed.includes("studio"));
    const visible = grp.items.filter((s) => canSee(s) && !isHidden(s));
    if (visible.length === 0) return null;
    const Icon = GROUP_ICON[grp.label] ?? MenuIcon;
    return (
      <MenubarMenu key={`grp:${grp.label}`}>
        <MenubarTrigger className={triggerClass(ownsActive(visible))}>
          <Icon />
          {grp.label}
        </MenubarTrigger>
        <MenubarContent>
          {visible.map((key) => {
            const def = SURFACE_DEF[key];
            return surfaceItem(key, def.label, def.icon, key);
          })}
        </MenubarContent>
      </MenubarMenu>
    );
  });

  /** Resolved menu: flat entries fold into a leading "Menu" trigger; `group`s become their own menus. */
  const flatItems = (resolvedItems ?? []).filter((it) => it.kind !== "group");
  const groups = (resolvedItems ?? []).filter((it) => it.kind === "group");
  const resolvedMenus = (
    <>
      {flatItems.length > 0 && (
        <MenubarMenu>
          <MenubarTrigger className={triggerClass(ownsActive(flatItems))}>
            <MenuIcon />
            Menu
          </MenubarTrigger>
          <MenubarContent>{flatItems.map(resolvedItem)}</MenubarContent>
        </MenubarMenu>
      )}
      {groups.map((grp, gi) => {
        const Icon = GROUP_ICON[grp.label] ?? MenuIcon;
        return (
          <MenubarMenu key={`grp:${gi}:${grp.label}`}>
            <MenubarTrigger className={triggerClass(ownsActive(grp.items ?? []))}>
              <Icon />
              {grp.label}
            </MenubarTrigger>
            <MenubarContent>{(grp.items ?? []).map(resolvedItem)}</MenubarContent>
          </MenubarMenu>
        );
      })}
    </>
  );

  const extItems = extSlots.filter((s) => !isHidden(`ext:${s.ext}`));
  const extOwnsActive = extItems.some((s) => active === `ext:${s.ext}`);
  const settingsActive = active === "settings";

  return (
    <div className="bg-bg px-2 pt-2">
      {/* The floating menubar card — the horizontal peer of the floating-sidebar variant. */}
      <div className="flex h-12 items-center gap-1.5 rounded-lg border border-border bg-panel px-2 shadow-sm">
        {/* Brand as the leading, non-menu element (scope OQ4). Static: no sidebar to collapse here. */}
        <div className="flex shrink-0 items-center">
          <BrandHeader brand={brand} canToggle={false} onToggle={() => {}} toggleLabel="" />
        </div>
        <span className="mx-1 h-6 w-px shrink-0 bg-border" aria-hidden />

        <Menubar
          className={cn(FLAT_MENUBAR, "flex-1", theme.layout.menuAlign === "center" ? "justify-center" : "justify-start")}
        >
          {useResolved ? resolvedMenus : fallbackMenus}

          {pinned.length > 0 && (
            <MenubarMenu>
              <MenubarTrigger className={triggerClass(false)}>
                <Pin />
                Pinned
              </MenubarTrigger>
              <MenubarContent>{pinned.map(resolvedItem)}</MenubarContent>
            </MenubarMenu>
          )}

          {extItems.length > 0 && (
            <MenubarMenu>
              <MenubarTrigger className={triggerClass(extOwnsActive)}>
                <Puzzle />
                Extensions
              </MenubarTrigger>
              <MenubarContent>
                {extItems.map((s) =>
                  surfaceItem(`ext:${s.ext}` as Surface, s.label, Puzzle, `ext:${s.ext}`),
                )}
              </MenubarContent>
            </MenubarMenu>
          )}
        </Menubar>

        {/* Right cluster: Settings + the account menu, grouped behind a divider so no glyph floats. */}
        <span className="mx-1 h-6 w-px shrink-0 bg-border" aria-hidden />
        <button
          type="button"
          aria-label="Settings"
          title="Settings"
          onClick={() => onSelect("settings")}
          className={cn(
            "flex size-8 shrink-0 items-center justify-center rounded-md text-muted transition-colors",
            "hover:bg-accent/10 hover:text-fg focus-visible:bg-accent/10 focus-visible:text-fg focus-visible:outline-none",
            settingsActive && "bg-accent/10 text-accent",
          )}
        >
          <Settings className="size-4" />
        </button>

        <Menubar className={cn(FLAT_MENUBAR, "shrink-0")}>
          <MenubarMenu>
            <MenubarTrigger
              aria-label="Account"
              className={cn(
                "h-8 gap-1.5 rounded-md border border-border/70 bg-card/50 px-1.5 pr-2 text-muted transition-colors",
                "hover:bg-accent/10 hover:text-fg focus:bg-accent/10 focus:text-fg",
                "data-[state=open]:bg-accent/10 data-[state=open]:text-fg",
              )}
            >
              <span
                aria-hidden
                className="flex size-5 items-center justify-center rounded-[5px] text-[11px] font-semibold text-accent-foreground"
                style={{ background: "linear-gradient(135deg, hsl(var(--accent)), hsl(var(--accent-2)))" }}
              >
                {brand.siteAbbr.slice(0, 1).toUpperCase()}
              </span>
              <ChevronDown className="size-3.5" />
            </MenubarTrigger>
            <MenubarContent align="end">
              {onTogglePin && active !== "settings" && (
                <MenubarItem
                  onSelect={(e) => {
                    e.preventDefault();
                    onTogglePin(String(active));
                  }}
                  className="gap-2.5 py-1.5"
                >
                  <Pin
                    className={`size-4 ${pinnedRefs.has(String(active)) ? "fill-current text-accent" : "text-muted"}`}
                  />
                  <span>{pinnedRefs.has(String(active)) ? "Unpin current page" : "Pin current page"}</span>
                </MenubarItem>
              )}
              {usingBuiltin && onUseMyMenu ? (
                <MenubarItem
                  onSelect={(e) => {
                    e.preventDefault();
                    onUseMyMenu();
                  }}
                  className="gap-2.5 py-1.5"
                >
                  <Undo2 className="size-4 text-muted" />
                  <span>Use my menu</span>
                </MenubarItem>
              ) : useResolved && onShowAllPages ? (
                <MenubarItem
                  onSelect={(e) => {
                    e.preventDefault();
                    onShowAllPages();
                  }}
                  className="gap-2.5 py-1.5"
                >
                  <LayoutGrid className="size-4 text-muted" />
                  <span>Show all pages</span>
                </MenubarItem>
              ) : null}
              {((usingBuiltin && onUseMyMenu) || (useResolved && onShowAllPages)) && <MenubarSeparator />}
              {onTogglePin && active !== "settings" && <MenubarSeparator />}
              <MenubarItem
                variant="destructive"
                onSelect={(e) => {
                  e.preventDefault();
                  onSignOut();
                }}
                className="gap-2.5 py-1.5"
              >
                <LogOut className="size-4" />
                <span>Sign out</span>
              </MenubarItem>
            </MenubarContent>
          </MenubarMenu>
        </Menubar>
      </div>
    </div>
  );
}
