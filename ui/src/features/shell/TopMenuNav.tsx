// TopMenuNav — the alternative workspace-nav mode (shell-chrome-layout scope). A sibling renderer
// to `NavRail`: fed the SAME resolved-nav data the rail consumes (resolvedItems, allowed, extSlots,
// pinned, handlers) and rendering it as a horizontal shadcn `Menubar` above the header. Each
// `SURFACE_GROUPS` bucket becomes a `MenubarMenu`; its surfaces become `MenubarItem`s. A resolved
// nav renders the same way — top-level `group`s become menus, flat entries fold into a leading
// "Menu" trigger. The Pinned favorites, the Extensions slots, and the no-lockout escape hatch +
// Sign out all appear (relocated into the menubar) so no rail affordance is lost. Extension ids
// stay opaque `ext:<id>` refs — no branch on identity (CLAUDE §10). One component per file.

import { ChevronDown, LayoutDashboard, LayoutGrid, LogOut, Pin, Puzzle, Undo2 } from "lucide-react";

import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarTrigger,
} from "@/components/ui/menubar";
import { useBranding } from "@/lib/branding";
import { useTheme } from "@/lib/theme";

import { BrandHeader } from "./BrandHeader";
import { NavActivePill, NavMenuMotionItem } from "./NavMenuMotion";
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

/** The per-icon colour override (Settings → Theme → Icon colors) — same inline-`color` trick NavRail
 *  uses, so the lucide `<svg>` inherits `currentColor`. */
function useIconColor() {
  const { theme } = useTheme();
  return (key: Surface): string | undefined => {
    if (!theme.iconColors) return undefined;
    return typeof key === "string" ? theme.iconColors[key] : undefined;
  };
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
  const iconColorFor = useIconColor();
  const pinnedRefs = new Set(pinned.map(itemRef));
  const useResolved = !!resolvedItems && resolvedItems.length > 0;
  const isHidden = (ref: string) => hidden.includes(ref);

  /** A menu item: icon + label, active highlight, optional pin toggle. The pin toggle is a nested
   *  submenu-less affordance — in a menubar we surface it as a trailing `Pin` glyph button inside the
   *  row (radix items don't nest buttons cleanly, so the pin is a non-interactive glyph here; the
   *  rail keeps the hover toggle. Pinning/unpinning from the top menu is via a context action in a
   *  follow-up — the rail remains the primary pin surface). v1: pin state is read-only visibility. */
  const surfaceItem = (key: Surface, label: string, Icon: typeof Pin, pinRef?: string) => {
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
        className="gap-2 data-[active]:text-accent data-[active]:font-medium"
      >
        <Icon className="size-4" style={iconColor ? { color: iconColor } : undefined} />
        <span className="flex-1">{label}</span>
        {pinRef && pinnedRefs.has(pinRef) && <Pin className="size-3 fill-current text-accent" />}
      </MenubarItem>
    );
  };

  /** A resolved nav entry (nav scope) — surface/ext/dashboard → its handler. */
  const resolvedItem = (it: ResolvedNavItem) => {
    if (it.kind === "surface" && it.surface) {
      const def = SURFACE_DEF[it.surface as CoreSurface];
      return surfaceItem(it.surface as Surface, it.label, def?.icon ?? ChevronDown, itemRef(it));
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
          className="gap-2"
        >
          <LayoutDashboard className="size-4" />
          <span className="flex-1">{it.label}</span>
          {!it.vars && pinnedRefs.has(itemRef(it)) && <Pin className="size-3 fill-current text-accent" />}
          {selected && <span className="sr-only">(current)</span>}
        </MenubarItem>
      );
    }
    return null;
  };

  /** Fallback buckets: each SURFACE_GROUPS entry → a MenubarMenu. */
  const fallbackMenus = SURFACE_GROUPS.map((grp) => {
    const canSee = (s: CoreSurface) => allowed.includes(s) || (s === "extensions" && allowed.includes("studio"));
    const visible = grp.items.filter((s) => canSee(s) && !isHidden(s));
    if (visible.length === 0) return null;
    return (
      <MenubarMenu key={`grp:${grp.label}`}>
        <MenubarTrigger>{grp.label}</MenubarTrigger>
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
          <MenubarTrigger>Menu</MenubarTrigger>
          <MenubarContent>{flatItems.map(resolvedItem)}</MenubarContent>
        </MenubarMenu>
      )}
      {groups.map((grp, gi) => (
        <MenubarMenu key={`grp:${gi}:${grp.label}`}>
          <MenubarTrigger>{grp.label}</MenubarTrigger>
          <MenubarContent>{(grp.items ?? []).map(resolvedItem)}</MenubarContent>
        </MenubarMenu>
      ))}
    </>
  );

  const extItems = extSlots.filter((s) => !isHidden(`ext:${s.ext}`));
  return (
    <div className="flex items-center gap-2 border-b border-border bg-card/60 px-3 py-1.5">
      {/* The brand as the leading, non-menu element of the menubar (scope OQ4 lean). Static: there is
          no sidebar to collapse in top-menu mode, so canToggle=false. */}
      <div className="flex items-center">
        <BrandHeader brand={brand} canToggle={false} onToggle={() => {}} toggleLabel="" />
      </div>

      <NavMenuMotionItem index={0} className="relative flex flex-1 items-center">
        {active !== "settings" && <NavActivePill />}
        <Menubar>
          {useResolved ? resolvedMenus : fallbackMenus}

          {pinned.length > 0 && (
            <MenubarMenu>
              <MenubarTrigger className="gap-1.5">
                <Pin className="size-3.5" />
                Pinned
              </MenubarTrigger>
              <MenubarContent>{pinned.map(resolvedItem)}</MenubarContent>
            </MenubarMenu>
          )}

          {extItems.length > 0 && (
            <MenubarMenu>
              <MenubarTrigger className="gap-1.5">
                <Puzzle className="size-3.5" />
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
      </NavMenuMotionItem>

      {/* Right-aligned overflow: the no-lockout escape hatch + Sign out (one account menu). */}
      <Menubar>
        <MenubarMenu>
          <MenubarTrigger aria-label="Account" className="gap-1.5">
            <LogOut className="size-3.5" />
          </MenubarTrigger>
          <MenubarContent align="end">
            {onTogglePin && active !== "settings" && (
              <MenubarItem
                onSelect={(e) => {
                  e.preventDefault();
                  onTogglePin(String(active));
                }}
                className="gap-2"
              >
                <Pin className={`size-4 ${pinnedRefs.has(String(active)) ? "fill-current text-accent" : ""}`} />
                <span>{pinnedRefs.has(String(active)) ? "Unpin current page" : "Pin current page"}</span>
              </MenubarItem>
            )}
            {usingBuiltin && onUseMyMenu ? (
              <MenubarItem
                onSelect={(e) => {
                  e.preventDefault();
                  onUseMyMenu();
                }}
                className="gap-2"
              >
                <Undo2 className="size-4" />
                <span>Use my menu</span>
              </MenubarItem>
            ) : useResolved && onShowAllPages ? (
              <MenubarItem
                onSelect={(e) => {
                  e.preventDefault();
                  onShowAllPages();
                }}
                className="gap-2"
              >
                <LayoutGrid className="size-4" />
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
              className="gap-2"
            >
              <LogOut className="size-4" />
              <span>Sign out</span>
            </MenubarItem>
          </MenubarContent>
        </MenubarMenu>
      </Menubar>
    </div>
  );
}
