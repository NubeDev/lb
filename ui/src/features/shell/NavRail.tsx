// The app sidebar — shadcn/ui Sidebar wired to Lazybones surfaces. It uses the same global
// Lazybones tokens as the rest of the shell, with cap-gated entries supplied by App.tsx.

import { Hash, LayoutDashboard, LogOut, Pin, Puzzle } from "lucide-react";

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  useSidebar,
} from "@/components/ui/sidebar";
import { useBranding } from "@/lib/branding";
import { useTheme } from "@/lib/theme";

import { BrandHeader } from "./BrandHeader";
import { NavActivePill, NavMenuMotionItem } from "./NavMenuMotion";
import { SURFACE_DEF, SURFACES } from "./surfaceDefs";

/** The fixed core surfaces the shell ships. */
export type CoreSurface =
  | "channels"
  | "dashboards"
  | "rules"
  | "flows"
  | "datasources"
  | "reminders"
  | "ingest"
  | "webhooks"
  | "data"
  | "system"
  | "system-mcp"
  | "system-acp"
  | "telemetry"
  | "inbox"
  | "outbox"
  | "insights"
  | "admin"
  | "extensions"
  | "studio"
  | "data-studio"
  | "settings";

/** A selected surface: a core one, or an **extension page** keyed `ext:<id>` (ui-federation scope). */
export type Surface = CoreSurface | `ext:${string}`;

/** An extension-contributed sidebar page slot (ui-federation scope). */
export interface ExtSlot {
  ext: string;
  label: string;
}

/** One entry in a resolved nav menu (nav scope) — the shape `nav.resolve` returns, already
 *  tag-expanded and cap-stripped. A `group` carries nested `items` (one level). The rail renders this
 *  directly when a nav applies, falling back to the built-in `SURFACES` otherwise. */
export interface ResolvedNavItem {
  kind: "surface" | "dashboard" | "ext" | "tag-group" | "template-group" | "group";
  label: string;
  surface?: string;
  dashboard?: string;
  ext?: string;
  items?: ResolvedNavItem[];
  /** reusable-pages: a resolved variable binding the rail folds into the board link as
   *  `?var-<name>=<value>` — a pinned `dashboard` instance, or a template-group child. */
  vars?: Record<string, string>;
}

interface Props {
  active: Surface;
  onSelect: (surface: Surface) => void;
  onSignOut: () => void;
  /** Core surfaces the session is allowed to SEE (cap-gated by the caller). Admin/extensions appear
   *  only for an admin session; the gateway re-checks every verb regardless (admin-console scope).
   *  Drives the built-in FALLBACK rail when no nav applies. */
  allowed: CoreSurface[];
  /** Installed extension pages contributed to the sidebar (ui-federation scope). */
  extSlots?: ExtSlot[];
  /** The caller's resolved nav menu (nav scope). When present with items, the rail renders THIS
   *  (already cap-stripped server-side) instead of the built-in `SURFACES` fallback. `null`/empty =
   *  fall back to `SURFACES.filter(allowed)` — never a blank rail. Route gates are untouched: the nav
   *  only *hides*, it does not *block* (a deep link to a permitted-but-unlisted page still works). */
  resolvedItems?: ResolvedNavItem[] | null;
  /** reusable-pages: navigate to a specific board (`dashboard:{id}`), optionally applying a pinned/
   *  template binding as `?var-<name>=<value>`. Falls back to the plain Dashboards surface when absent. */
  onSelectDashboard?: (dashboard: string, vars?: Record<string, string>) => void;
  /** hide-and-pins: the workspace hidden-set echo (`nav.resolve`) — refs subtracted from the
   *  built-in FALLBACK rail (the resolved menu arrives already stripped server-side). Declutter
   *  only: route gates are untouched; a permitted deep link still works. */
  hidden?: string[];
  /** hide-and-pins: the caller's pinned favorites, resolved server-side (cap-, ext-, and
   *  hidden-stripped), in the member's order. Rendered as a Pinned section above the menu. */
  pinned?: ResolvedNavItem[];
  /** hide-and-pins: flip one pin ref (bare surface key | `ext:<id>` | `dashboard:<id>`) in the
   *  member-owned `nav_pref`. When absent, the rail shows no pin affordance. */
  onTogglePin?: (ref: string) => void;
}

/** A rail entry's ref in the shared hide/pin grammar (mirrors the resolver's `item_ref`). */
function itemRef(it: ResolvedNavItem): string {
  if (it.kind === "ext" && it.ext) return `ext:${it.ext}`;
  if (it.kind === "dashboard" && it.dashboard) return it.dashboard;
  return it.surface ?? "";
}

/** The built-in fallback rail, bucketed into labelled categories so it reads as sections rather than
 *  one long flat list (sidebar-16 shape). This ONLY shapes the fallback: when a server-authored nav
 *  applies (`resolvedItems`), that owns grouping instead (nav scope). `settings` lives in the footer,
 *  not a group. A group whose members are all cap-stripped renders nothing (no empty label). The icon
 *  + label per key come from the shared `SURFACE_DEF` map (`surfaceDefs.ts`) — never re-defined here. */
export const SURFACE_GROUPS: { label: string; items: CoreSurface[] }[] = [
  {
    label: "Workspace",
    items: ["channels", "dashboards", "inbox", "outbox",
      // insights (insights umbrella scope): the durable data-finding record. Workspace-level
      // attention surface — open/acked/resolved findings with severity + dedup, faceted through the
      // tag graph. Cap-gated on `insight.list` (allowed.ts); the gateway re-checks every verb.
      "insights"],
  },
  {
    label: "Automation",
    items: ["rules", "flows", "reminders"],
  },
  {
    label: "Data",
    items: ["datasources", "ingest",
      // webhooks (webhooks scope): a first-class inbound-HTTP surface beside the other data inlets.
      // The page is cap-gated on `webhook.manage` (allowed.ts); the gateway re-checks every verb
      // server-side. Sits in the Data group (the wizard reads like a data-surface); the component
      // itself lives in `features/admin/` because it mirrors the ApiKeysAdmin pattern.
      "webhooks", "data"],
  },
  {
    label: "Build",
    items: [
      // Extensions + Studio are one merged, tabbed page — a single rail entry. `extensions` is the
      // rail key (its tab lands first); the merged page shows whichever tabs the session's caps allow.
      "extensions", "data-studio"],
  },
  {
    label: "System",
    items: ["system", "telemetry", "admin"],
  },
];

/** The flat list of sidebar surfaces the icon-colorizer (Settings → Theme) iterates: every rail entry
 *  (the body groups + the footer Settings entry) as `{ key, label }`. DATA, derived from the single
 *  source of truth in `surfaceDefs.ts` — never a second hand-maintained list. Extension slots
 *  (`ext:<id>`) are dynamic and intentionally not enumerated here; they fall back to default fg
 *  unless the member sets one through future per-ext UI. */
export const RAIL_SURFACES: readonly { key: CoreSurface; label: string }[] = SURFACES.map((s) => ({
  key: s.key,
  label: s.label,
}));

export function NavRail({
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
}: Props) {
  // The sidebar variant/collapsible/side come from the member's theme (Customizer → Layout tab), so
  // the shell chrome re-lays-out live and the choice persists/roams through the theme prefs blob.
  const { theme } = useTheme();
  const { variant, side } = theme.layout;
  // The logo doubles as the collapse toggle (reusing the provider's `toggleSidebar`). It is only
  // interactive when the collapsible mode allows collapsing — "none" means always-open, so the logo
  // stays a plain static mark with no pointer/hover/tooltip and no toggle behavior.
  const { toggleSidebar, collapsible, state } = useSidebar();
  const canToggle = collapsible !== "none";
  const expanded = state === "expanded";
  const toggleLabel = expanded ? "Collapse sidebar" : "Expand sidebar";
  // The workspace brand (workspace-branding scope). `brand` is always set: the provider seeds from
  // the localStorage boot cache (no flash on refresh) or the neutral default on a first-ever visit,
  // then the live `prefs.resolve` confirms it. The admin sets it in Settings → Branding; every
  // member of the workspace resolves the same brand.
  const { brand } = useBranding();
  // Per-icon color overrides (Settings → Theme → Icon colors). Applied as inline `color` so it wins
  // over the button's text-* classes without fighting specificity, and inherits into the lucide
  // `<svg>` (which uses `currentColor`). Surfaces not in the map render in the default fg.
  const iconColorFor = (key: Surface): string | undefined => {
    if (!theme.iconColors) return undefined;
    return typeof key === "string" ? theme.iconColors[key] : undefined;
  };

  // The refs currently pinned (for the toggle's pressed state). Derived from the RESOLVED pins —
  // a stripped pin isn't visible, so it can't be toggled here (the stored record keeps it; it
  // comes back when un-hidden/regranted).
  const pinnedRefs = new Set(pinned.map(itemRef));

  // The hover pin/unpin toggle (hide-and-pins scope) — rail-only affordance, member-owned write.
  // Hidden entirely in icon-collapsed mode (no room) and when the shell passed no handler.
  const pinToggle = (ref: string) => {
    if (!onTogglePin || !ref) return null;
    const isPinned = pinnedRefs.has(ref);
    return (
      <button
        type="button"
        aria-label={isPinned ? "Unpin" : "Pin"}
        aria-pressed={isPinned}
        title={isPinned ? "Unpin" : "Pin"}
        className="absolute right-1 top-1/2 -translate-y-1/2 rounded-sm p-1 text-muted opacity-0 transition-opacity hover:text-fg focus-visible:opacity-100 group-hover/navitem:opacity-100 group-data-[collapsible=icon]:hidden"
        onClick={(e) => {
          e.stopPropagation();
          onTogglePin(ref);
        }}
      >
        <Pin className={`h-3.5 w-3.5 ${isPinned ? "fill-current" : ""}`} />
      </button>
    );
  };

  const item = (
    key: Surface,
    label: string,
    Icon: typeof Hash,
    onClick?: () => void,
    pinRef?: string,
    index = 0,
  ) => {
    const selected = active === key;
    const iconColor = iconColorFor(key);
    return (
      <NavMenuMotionItem key={`${key}:${label}`} index={index} className="group/navitem relative">
        {/* The sliding shared-element pill provides the active fill; the button drops its own active
            background/ring (keeps only the accent text + weight) so the single pill glides between
            items on navigation instead of the fill snapping on/off. */}
        {selected && <NavActivePill />}
        <SidebarMenuButton
          aria-label={label}
          aria-current={selected ? "page" : undefined}
          isActive={selected}
          tooltip={label}
          onClick={onClick ?? (() => onSelect(key))}
          className="relative bg-transparent shadow-none data-[active=true]:bg-transparent data-[active=true]:shadow-none"
        >
          <Icon
            className="transition-transform duration-200 group-hover/navitem:scale-110"
            style={iconColor ? { color: iconColor } : undefined}
          />
          <span>{label}</span>
        </SidebarMenuButton>
        {pinRef !== undefined && pinToggle(pinRef)}
      </NavMenuMotionItem>
    );
  };

  // Render one resolved nav entry (nav scope). A `surface`/`ext` maps to its `onSelect` target; a
  // `dashboard`/`tag-group` dashboard navigates to the Dashboards surface (a specific-board deep link
  // is a named follow-up — the lens still SHOWS the entry). The item was already cap-stripped
  // server-side, so anything here is reachable. Route gates are untouched: this only hides, never
  // blocks.
  const resolvedItem = (it: ResolvedNavItem, keyHint: string, index = 0) => {
    if (it.kind === "surface" && it.surface) {
      const key = it.surface as Surface;
      return item(key, it.label, SURFACE_DEF[it.surface as CoreSurface]?.icon ?? Hash, undefined, itemRef(it), index);
    }
    if (it.kind === "ext" && it.ext) {
      const key = `ext:${it.ext}` as Surface;
      return item(key, it.label, Puzzle, undefined, itemRef(it), index);
    }
    if (it.kind === "dashboard") {
      // Deep-board links land on the specific board, applying any pinned/template binding as `?var-`
      // (reusable-pages) — falling back to the plain Dashboards surface when no deep-link handler.
      const varKey = it.vars ? `:${JSON.stringify(it.vars)}` : "";
      return (
        <NavMenuMotionItem
          key={`dash:${keyHint}:${it.dashboard ?? it.label}${varKey}`}
          index={index}
          className="group/navitem relative"
        >
          <SidebarMenuButton
            aria-label={it.label}
            tooltip={it.label}
            onClick={() =>
              it.dashboard && onSelectDashboard
                ? onSelectDashboard(it.dashboard, it.vars)
                : onSelect("dashboards")
            }
          >
            <LayoutDashboard />
            <span>{it.label}</span>
          </SidebarMenuButton>
          {/* A vars-bound entry is a nav-authored page instance — not pinnable by ref in v1. */}
          {!it.vars && pinToggle(itemRef(it))}
        </NavMenuMotionItem>
      );
    }
    return null;
  };

  // A resolved menu: flat entries + one level of `group` (a labeled subsection). A `tag-group`
  // resolves to a `group` server-side, so both render the same way here.
  const resolvedMenu = (items: ResolvedNavItem[]) => (
    <>
      <SidebarGroup>
        <SidebarGroupLabel>Menu</SidebarGroupLabel>
        <SidebarGroupContent>
          <SidebarMenu>
            {items
              .filter((it) => it.kind !== "group")
              .map((it, i) => resolvedItem(it, String(i), i))}
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>
      {items
        .filter((it) => it.kind === "group")
        .map((grp, gi) => (
          <SidebarGroup key={`grp:${gi}:${grp.label}`}>
            <SidebarGroupLabel>{grp.label}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {(grp.items ?? []).map((it, i) => resolvedItem(it, `${gi}-${i}`, i))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}
    </>
  );

  // Use the resolved nav when one applies (non-empty); otherwise the built-in `SURFACES` fallback
  // (never a blank rail — nav scope).
  const useResolved = !!resolvedItems && resolvedItems.length > 0;

  // hide-and-pins: the Pinned section — the member's favorites, resolved (already stripped)
  // server-side, above whichever menu applies. Renders nothing when the member has no live pins.
  const pinnedGroup = pinned.length > 0 && (
    <SidebarGroup>
      <SidebarGroupLabel>Pinned</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>{pinned.map((it, i) => resolvedItem(it, `pin-${i}`, i))}</SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );

  // hide-and-pins: the FALLBACK is the one tier the server can't strip (the menu lives here), so
  // subtract the workspace hidden-set client-side. Refs are opaque strings; an ext slot's key is
  // already its `ext:<id>` ref. Declutter only — routes stay reachable by deep link.
  const isHidden = (ref: string) => hidden.includes(ref);

  return (
    <Sidebar variant={variant} side={side}>
      <SidebarHeader>
        {/* Header row: the workspace brand IS the collapse toggle (no separate trigger button). When
            the collapsible mode allows it, the brand mark springs on hover/press and clicking toggles
            expand/collapse; in "none" mode it renders as a plain static brand. Motion is gated by the
            member's preference (BrandHeader → useMotionPref). Icon mode centers it. */}
        <div className="flex items-center group-data-[collapsible=icon]:justify-center">
          <SidebarMenu>
            <SidebarMenuItem>
              <BrandHeader
                brand={brand}
                canToggle={canToggle}
                onToggle={toggleSidebar}
                toggleLabel={toggleLabel}
              />
            </SidebarMenuItem>
          </SidebarMenu>
        </div>
      </SidebarHeader>

      <SidebarContent>
        {pinnedGroup}
        {useResolved ? (
          // A user-/team-authored nav applies — render the resolved (cap-stripped, hidden-stripped)
          // menu (nav scope).
          resolvedMenu(resolvedItems!)
        ) : (
          // Fallback: the built-in `SURFACE_GROUPS`, cap-gated by `allowed` and minus the workspace
          // hidden-set (never a blank rail). Each category is a labelled section; a group whose
          // members are all stripped renders nothing.
          <>
            {SURFACE_GROUPS.map((grp) => {
              // The merged "Studio" entry (keyed `extensions`) shows when EITHER of its tabs' caps is
              // allowed — `studio` (Build) counts too. Clicking it lands on the bare `/studio` redirect,
              // which forwards to the first tab the session can reach (a build-only user gets Build).
              const canSee = (s: CoreSurface) =>
                allowed.includes(s) || (s === "extensions" && allowed.includes("studio"));
              const visible = grp.items.filter((s) => canSee(s) && !isHidden(s));
              if (visible.length === 0) return null;
              return (
                <SidebarGroup key={grp.label}>
                  <SidebarGroupLabel>{grp.label}</SidebarGroupLabel>
                  <SidebarGroupContent>
                    <SidebarMenu>
                      {visible.map((key, i) => {
                        const def = SURFACE_DEF[key];
                        return item(key, def.label, def.icon, undefined, key, i);
                      })}
                    </SidebarMenu>
                  </SidebarGroupContent>
                </SidebarGroup>
              );
            })}

            {extSlots.filter((s) => !isHidden(`ext:${s.ext}`)).length > 0 && (
              <SidebarGroup>
                <SidebarGroupLabel>Extensions</SidebarGroupLabel>
                <SidebarGroupContent>
                  <SidebarMenu>
                    {extSlots
                      .filter((s) => !isHidden(`ext:${s.ext}`))
                      .map((s, i) => item(`ext:${s.ext}`, s.label, Puzzle, undefined, `ext:${s.ext}`, i))}
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            )}
          </>
        )}
      </SidebarContent>

      <SidebarFooter>
        <SidebarMenu>
          {/* Settings moved to the page-header top-right (the gear next to the workspace chip) — the
              rail footer keeps only Sign out. A server-authored nav can still place settings itself. */}
          <SidebarMenuItem>
            <SidebarMenuButton aria-label="Sign out" tooltip="Sign out" onClick={onSignOut}>
              <LogOut />
              <span>Sign out</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
