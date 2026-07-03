// The app sidebar — shadcn/ui Sidebar wired to Lazybones surfaces. It uses the same global
// Lazybones tokens as the rest of the shell, with cap-gated entries supplied by App.tsx.

import {
  Activity,
  Boxes,
  CalendarClock,
  Database,
  Hash,
  Network,
  Inbox,
  LayoutDashboard,
  LogOut,
  Plug,
  Puzzle,
  ScrollText,
  Telescope,
  Workflow,
  Wrench,
  Send,
  Settings,
  Shield,
} from "lucide-react";

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
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { ThemeSwitcher } from "@/features/theme";

/** The fixed core surfaces the shell ships. */
export type CoreSurface =
  | "channels"
  | "dashboards"
  | "rules"
  | "flows"
  | "datasources"
  | "reminders"
  | "ingest"
  | "data"
  | "system"
  | "system-mcp"
  | "system-acp"
  | "telemetry"
  | "inbox"
  | "outbox"
  | "admin"
  | "extensions"
  | "studio"
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
  kind: "surface" | "dashboard" | "ext" | "tag-group" | "group";
  label: string;
  surface?: string;
  dashboard?: string;
  ext?: string;
  items?: ResolvedNavItem[];
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
}

const SURFACES: { key: CoreSurface; icon: typeof Hash; label: string }[] = [
  { key: "channels", icon: Hash, label: "Channels" },
  { key: "dashboards", icon: LayoutDashboard, label: "Dashboards" },
  { key: "rules", icon: ScrollText, label: "Rules" },
  { key: "flows", icon: Workflow, label: "Flows" },
  { key: "datasources", icon: Plug, label: "Datasources" },
  { key: "reminders", icon: CalendarClock, label: "Reminders" },
  { key: "ingest", icon: Activity, label: "Ingest" },
  { key: "data", icon: Database, label: "Data" },
  { key: "system", icon: Network, label: "System" },
  { key: "telemetry", icon: Telescope, label: "Telemetry" },
  { key: "inbox", icon: Inbox, label: "Inbox" },
  { key: "outbox", icon: Send, label: "Outbox" },
  { key: "admin", icon: Shield, label: "Admin" },
  { key: "extensions", icon: Boxes, label: "Extensions" },
  { key: "studio", icon: Wrench, label: "Studio" },
  { key: "settings", icon: Settings, label: "Settings" },
];

/** The surface → icon lookup a resolved `surface` item renders with (its own icon when known; a
 *  generic one otherwise). Built from `SURFACES` so the fallback and the resolved rail stay in
 *  lockstep — the scope's "fallback correctness" guard. */
const SURFACE_ICON: Record<string, typeof Hash> = Object.fromEntries(
  SURFACES.map((s) => [s.key, s.icon]),
);

export function NavRail({
  active,
  onSelect,
  onSignOut,
  allowed,
  extSlots = [],
  resolvedItems = null,
}: Props) {
  const item = (key: Surface, label: string, Icon: typeof Hash, onClick?: () => void) => {
    const selected = active === key;
    return (
      <SidebarMenuItem key={`${key}:${label}`}>
        <SidebarMenuButton
          aria-label={label}
          aria-current={selected ? "page" : undefined}
          isActive={selected}
          tooltip={label}
          onClick={onClick ?? (() => onSelect(key))}
        >
          <Icon />
          <span>{label}</span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    );
  };

  // Render one resolved nav entry (nav scope). A `surface`/`ext` maps to its `onSelect` target; a
  // `dashboard`/`tag-group` dashboard navigates to the Dashboards surface (a specific-board deep link
  // is a named follow-up — the lens still SHOWS the entry). The item was already cap-stripped
  // server-side, so anything here is reachable. Route gates are untouched: this only hides, never
  // blocks.
  const resolvedItem = (it: ResolvedNavItem, keyHint: string) => {
    if (it.kind === "surface" && it.surface) {
      const key = it.surface as Surface;
      return item(key, it.label, SURFACE_ICON[it.surface] ?? Hash);
    }
    if (it.kind === "ext" && it.ext) {
      const key = `ext:${it.ext}` as Surface;
      return item(key, it.label, Puzzle);
    }
    if (it.kind === "dashboard") {
      // Deep-board links land on the Dashboards page (the board host); the label is the board's.
      return (
        <SidebarMenuItem key={`dash:${keyHint}:${it.dashboard ?? it.label}`}>
          <SidebarMenuButton
            aria-label={it.label}
            tooltip={it.label}
            onClick={() => onSelect("dashboards")}
          >
            <LayoutDashboard />
            <span>{it.label}</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
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
              .map((it, i) => resolvedItem(it, String(i)))}
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
                {(grp.items ?? []).map((it, i) => resolvedItem(it, `${gi}-${i}`))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}
    </>
  );

  // Use the resolved nav when one applies (non-empty); otherwise the built-in `SURFACES` fallback
  // (never a blank rail — nav scope).
  const useResolved = !!resolvedItems && resolvedItems.length > 0;

  return (
    <Sidebar collapsible="icon" variant="sidebar">
      <SidebarHeader>
        <div className="hidden h-8 w-full items-center justify-center group-data-[collapsible=icon]:flex">
          <div className="flex h-8 w-8 items-center justify-center rounded-md border border-border bg-bg text-[11px] font-semibold text-accent shadow-sm">
            lb
          </div>
        </div>
        <SidebarMenu className="group-data-[collapsible=icon]:hidden">
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" tooltip="Lazybones" aria-label="Lazybones">
              <div className="flex h-8 w-8 items-center justify-center rounded-md border border-border bg-bg text-[11px] font-semibold text-accent shadow-sm">
                lb
              </div>
              <div className="grid flex-1 text-left text-sm leading-tight">
                <span className="truncate font-semibold">Lazybones</span>
                <span className="truncate text-xs text-muted">workspace ops</span>
              </div>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        <div className="flex items-center justify-end px-1 group-data-[collapsible=icon]:justify-center">
          <SidebarTrigger aria-label="Toggle sidebar" title="Toggle sidebar" />
        </div>
      </SidebarHeader>

      <SidebarContent>
        {useResolved ? (
          // A user-/team-authored nav applies — render the resolved (cap-stripped) menu (nav scope).
          resolvedMenu(resolvedItems!)
        ) : (
          // Fallback: today's built-in `SURFACES`, cap-gated by `allowed` (never a blank rail).
          <>
            <SidebarGroup>
              <SidebarGroupLabel>Core</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {SURFACES.filter((s) => allowed.includes(s.key)).map(({ key, icon, label }) =>
                    item(key, label, icon),
                  )}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>

            {extSlots.length > 0 && (
              <SidebarGroup>
                <SidebarGroupLabel>Extensions</SidebarGroupLabel>
                <SidebarGroupContent>
                  <SidebarMenu>
                    {extSlots.map((s) => item(`ext:${s.ext}`, s.label, Puzzle))}
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            )}
          </>
        )}
      </SidebarContent>

      <SidebarFooter>
        <ThemeSwitcher />
        <SidebarMenu>
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
