// The single source of truth for a core surface's chrome — its icon + human label. Both the
// sidebar (`NavRail`) and the Data Studio workbench tabs/menu (`workbenchPanes`, `WorkbenchTab`)
// read from this map, so a surface's icon is set ONCE here and every downstream surface inherits
// the change. Keep this file flat data only — ordering/grouping is a presentation concern that
// lives in `NavRail` (the sidebar) and `workbenchPanes` (the studio's dock).
//
// Rule 10 holds: surface ids are opaque DATA. Nothing here branches on an extension id; the
// `ext:<id>` slots the rail also renders are dynamic and intentionally NOT in this map (they
// fall back to a generic icon in `NavRail`).

import {
  Activity,
  Boxes,
  CalendarClock,
  Database,
  FileText,
  FlaskConical,
  Hash,
  Inbox,
  LayoutDashboard,
  Lightbulb,
  Network,
  Plug,
  ScrollText,
  Send,
  Settings,
  Shield,
  Telescope,
  Webhook as WebhookIcon,
  Workflow,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import type { CoreSurface } from "./NavRail";

export interface SurfaceDef {
  key: CoreSurface;
  icon: LucideIcon;
  label: string;
}

/**
 * One entry per core surface. Add a new core surface here and EVERY consumer (sidebar rail,
 * Data Studio tab icon, "+ Open view" menu, studio workbench) picks it up — never hand-wire an
 * icon for a named surface elsewhere.
 *
 * The label is the canonical display name (e.g. the rail tooltip, the dock tab title); the
 * sidebar's group bucketing/wording is owned by `NavRail` and may surface its own override.
 */
export const SURFACE_DEF: Record<CoreSurface, SurfaceDef> = {
  channels: { key: "channels", icon: Hash, label: "Channels" },
  dashboards: { key: "dashboards", icon: LayoutDashboard, label: "Dashboards" },
  reports: { key: "reports", icon: FileText, label: "Reports" },
  rules: { key: "rules", icon: ScrollText, label: "Rules" },
  flows: { key: "flows", icon: Workflow, label: "Flows" },
  datasources: { key: "datasources", icon: Plug, label: "Datasources" },
  reminders: { key: "reminders", icon: CalendarClock, label: "Reminders" },
  ingest: { key: "ingest", icon: Activity, label: "Ingest" },
  webhooks: { key: "webhooks", icon: WebhookIcon, label: "Webhooks" },
  data: { key: "data", icon: Database, label: "Data" },
  system: { key: "system", icon: Network, label: "System" },
  "system-mcp": { key: "system-mcp", icon: Network, label: "MCP" },
  "system-acp": { key: "system-acp", icon: Network, label: "Capabilities" },
  telemetry: { key: "telemetry", icon: Telescope, label: "Telemetry" },
  inbox: { key: "inbox", icon: Inbox, label: "Inbox" },
  outbox: { key: "outbox", icon: Send, label: "Outbox" },
  insights: { key: "insights", icon: Lightbulb, label: "Insights" },
  admin: { key: "admin", icon: Shield, label: "Admin" },
  // The merged Studio tabbed page — sidebar shows ONE entry keyed `extensions` labelled "Studio"
  // (its click lands on `/studio`, which forwards to the first tab the caps allow).
  extensions: { key: "extensions", icon: Boxes, label: "Studio" },
  studio: { key: "studio", icon: Boxes, label: "Studio" },
  "data-studio": { key: "data-studio", icon: FlaskConical, label: "Data Studio" },
  settings: { key: "settings", icon: Settings, label: "Settings" },
};

/** Look up the def for a surface; `undefined` for an unknown surface (the caller's fallback path). */
export function surfaceDef(key: CoreSurface): SurfaceDef | undefined {
  return SURFACE_DEF[key];
}

/** The icon for a surface — `undefined` for an unknown surface (the caller picks a fallback). */
export function surfaceIcon(key: CoreSurface): LucideIcon | undefined {
  return SURFACE_DEF[key]?.icon;
}

/** The flat list of all surfaces — used to build derived structures (rail list, icon-colorizer). */
export const SURFACES: readonly SurfaceDef[] = Object.values(SURFACE_DEF);
