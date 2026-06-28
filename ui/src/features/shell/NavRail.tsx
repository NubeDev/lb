// The app sidebar — shadcn/ui Sidebar wired to Lazybones surfaces. It uses the same global
// Lazybones tokens as the rest of the shell, with cap-gated entries supplied by App.tsx.

import {
  Activity,
  Boxes,
  Database,
  Hash,
  Network,
  Inbox,
  LayoutDashboard,
  LogOut,
  Puzzle,
  Wrench,
  Send,
  Shield,
  Users,
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
  | "members"
  | "dashboards"
  | "ingest"
  | "data"
  | "system"
  | "inbox"
  | "outbox"
  | "admin"
  | "extensions"
  | "studio";

/** A selected surface: a core one, or an **extension page** keyed `ext:<id>` (ui-federation scope). */
export type Surface = CoreSurface | `ext:${string}`;

/** An extension-contributed sidebar page slot (ui-federation scope). */
export interface ExtSlot {
  ext: string;
  label: string;
}

interface Props {
  active: Surface;
  onSelect: (surface: Surface) => void;
  onSignOut: () => void;
  /** Core surfaces the session is allowed to SEE (cap-gated by the caller). Admin/extensions appear
   *  only for an admin session; the gateway re-checks every verb regardless (admin-console scope). */
  allowed: CoreSurface[];
  /** Installed extension pages contributed to the sidebar (ui-federation scope). */
  extSlots?: ExtSlot[];
}

const SURFACES: { key: CoreSurface; icon: typeof Hash; label: string }[] = [
  { key: "channels", icon: Hash, label: "Channels" },
  { key: "members", icon: Users, label: "Members" },
  { key: "dashboards", icon: LayoutDashboard, label: "Dashboards" },
  { key: "ingest", icon: Activity, label: "Ingest" },
  { key: "data", icon: Database, label: "Data" },
  { key: "system", icon: Network, label: "System" },
  { key: "inbox", icon: Inbox, label: "Inbox" },
  { key: "outbox", icon: Send, label: "Outbox" },
  { key: "admin", icon: Shield, label: "Admin" },
  { key: "extensions", icon: Boxes, label: "Extensions" },
  { key: "studio", icon: Wrench, label: "Studio" },
];

export function NavRail({ active, onSelect, onSignOut, allowed, extSlots = [] }: Props) {
  const item = (key: Surface, label: string, Icon: typeof Hash) => {
    const selected = active === key;
    return (
      <SidebarMenuItem key={key}>
        <SidebarMenuButton
          aria-label={label}
          aria-current={selected ? "page" : undefined}
          isActive={selected}
          tooltip={label}
          onClick={() => onSelect(key)}
        >
          <Icon />
          <span>{label}</span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    );
  };

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
