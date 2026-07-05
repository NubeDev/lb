// The app sidebar — shadcn/ui Sidebar wired to Lazybones surfaces. It uses the same global
// Lazybones tokens as the rest of the shell, with cap-gated entries supplied by App.tsx.

import {
  Activity,
  Boxes,
  CalendarClock,
  Database,
  Hash,
  Lightbulb,
  Network,
  Inbox,
  LayoutDashboard,
  LogOut,
  Plug,
  Puzzle,
  ScrollText,
  Telescope,
  Webhook as WebhookIcon,
  Workflow,
  FlaskConical,
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
import { useTheme } from "@/lib/theme";

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
}

interface SurfaceDef {
  key: CoreSurface;
  icon: typeof Hash;
  label: string;
}

/** The built-in fallback rail, bucketed into labelled categories so it reads as sections rather than
 *  one long flat list (sidebar-16 shape). This ONLY shapes the fallback: when a server-authored nav
 *  applies (`resolvedItems`), that owns grouping instead (nav scope). `settings` lives in the footer,
 *  not a group. A group whose members are all cap-stripped renders nothing (no empty label). */
const SURFACE_GROUPS: { label: string; items: SurfaceDef[] }[] = [
  {
    label: "Workspace",
    items: [
      { key: "channels", icon: Hash, label: "Channels" },
      { key: "dashboards", icon: LayoutDashboard, label: "Dashboards" },
      { key: "inbox", icon: Inbox, label: "Inbox" },
      { key: "outbox", icon: Send, label: "Outbox" },
      // insights (insights umbrella scope): the durable data-finding record. Workspace-level
      // attention surface — open/acked/resolved findings with severity + dedup, faceted through the
      // tag graph. Cap-gated on `insight.list` (allowed.ts); the gateway re-checks every verb.
      { key: "insights", icon: Lightbulb, label: "Insights" },
    ],
  },
  {
    label: "Automation",
    items: [
      { key: "rules", icon: ScrollText, label: "Rules" },
      { key: "flows", icon: Workflow, label: "Flows" },
      { key: "reminders", icon: CalendarClock, label: "Reminders" },
    ],
  },
  {
    label: "Data",
    items: [
      { key: "datasources", icon: Plug, label: "Datasources" },
      { key: "ingest", icon: Activity, label: "Ingest" },
      // webhooks (webhooks scope): a first-class inbound-HTTP surface beside the other data inlets.
      // The page is cap-gated on `webhook.manage` (allowed.ts); the gateway re-checks every verb
      // server-side. Sits in the Data group (the wizard reads like a data-surface); the component
      // itself lives in `features/admin/` because it mirrors the ApiKeysAdmin pattern.
      { key: "webhooks", icon: WebhookIcon, label: "Webhooks" },
      { key: "data", icon: Database, label: "Data" },
    ],
  },
  {
    label: "Build",
    items: [
      // Extensions + Studio are one merged, tabbed page — a single rail entry. `extensions` is the
      // rail key (its tab lands first); the merged page shows whichever tabs the session's caps allow.
      { key: "extensions", icon: Boxes, label: "Studio" },
      { key: "data-studio", icon: FlaskConical, label: "Data Studio" },
    ],
  },
  {
    label: "System",
    items: [
      { key: "system", icon: Network, label: "System" },
      { key: "telemetry", icon: Telescope, label: "Telemetry" },
      { key: "admin", icon: Shield, label: "Admin" },
    ],
  },
];

/** The `settings` surface — rendered in the footer, not a category group. */
const SETTINGS_SURFACE: SurfaceDef = { key: "settings", icon: Settings, label: "Settings" };

/** The brand mark — a two-hue (accent → secondary accent) tile, the same signature gradient the page
 *  headers carry. `--accent-foreground` keeps the glyph legible on the accent in every preset/mode. */
function BrandMark() {
  return (
    <div
      className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-[11px] font-bold shadow-sm"
      style={{
        background: "linear-gradient(135deg, hsl(var(--accent)), hsl(var(--accent-2)))",
        color: "hsl(var(--accent-foreground))",
      }}
    >
      lb
    </div>
  );
}

/** Every surface, flattened — used to build the icon lookup so the resolved rail and the fallback
 *  stay in lockstep. */
const SURFACES: SurfaceDef[] = [
  ...SURFACE_GROUPS.flatMap((g) => g.items),
  SETTINGS_SURFACE,
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
  onSelectDashboard,
}: Props) {
  // The sidebar variant/collapsible/side come from the member's theme (Customizer → Layout tab), so
  // the shell chrome re-lays-out live and the choice persists/roams through the theme prefs blob.
  const { theme } = useTheme();
  const { variant, collapsible, side } = theme.layout;

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
      // Deep-board links land on the specific board, applying any pinned/template binding as `?var-`
      // (reusable-pages) — falling back to the plain Dashboards surface when no deep-link handler.
      const varKey = it.vars ? `:${JSON.stringify(it.vars)}` : "";
      return (
        <SidebarMenuItem key={`dash:${keyHint}:${it.dashboard ?? it.label}${varKey}`}>
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
    <Sidebar collapsible={collapsible} variant={variant} side={side}>
      <SidebarHeader>
        <div className="hidden h-8 w-full items-center justify-center group-data-[collapsible=icon]:flex">
          <BrandMark />
        </div>
        <SidebarMenu className="group-data-[collapsible=icon]:hidden">
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" tooltip="Lazybones" aria-label="Lazybones">
              <BrandMark />
              <div className="grid flex-1 text-left text-sm leading-tight">
                <span className="truncate font-semibold tracking-tight">Lazybones</span>
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
          // Fallback: the built-in `SURFACE_GROUPS`, cap-gated by `allowed` (never a blank rail). Each
          // category is a labelled section; a group whose members are all cap-stripped renders nothing.
          <>
            {SURFACE_GROUPS.map((grp) => {
              // The merged "Studio" entry (keyed `extensions`) shows when EITHER of its tabs' caps is
              // allowed — `studio` (Build) counts too. Clicking it lands on the bare `/studio` redirect,
              // which forwards to the first tab the session can reach (a build-only user gets Build).
              const canSee = (s: SurfaceDef) =>
                allowed.includes(s.key) || (s.key === "extensions" && allowed.includes("studio"));
              const visible = grp.items.filter(canSee);
              if (visible.length === 0) return null;
              return (
                <SidebarGroup key={grp.label}>
                  <SidebarGroupLabel>{grp.label}</SidebarGroupLabel>
                  <SidebarGroupContent>
                    <SidebarMenu>
                      {visible.map(({ key, icon, label }) => item(key, label, icon))}
                    </SidebarMenu>
                  </SidebarGroupContent>
                </SidebarGroup>
              );
            })}

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
