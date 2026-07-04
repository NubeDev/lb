// The one subsystem → visual identity map (icon + categorical hue) shared by the System surfaces, so
// a subsystem looks the same on the status grid, the topology graph, and the detail sheet. The hue is
// an *identity* color from the categorical ramp (`--chart-N` in globals.css) — never a state color;
// health keeps the semantic vocabulary in `health.ts`. Ids here are CORE subsystems (gateway, bus,
// store, …), not extensions — no rule-10 concern. One responsibility per file: the identity lookup.

import {
  Archive,
  Boxes,
  Cable,
  Database,
  Globe,
  Inbox,
  ListChecks,
  Activity,
  Radio,
  Send,
  Wrench,
  Box,
  type LucideIcon,
} from "lucide-react";

export interface SubsystemIdentity {
  icon: LucideIcon;
  /** The `--chart-N` token index carrying this subsystem's identity hue. */
  hue: number;
}

const IDENTITIES: Record<string, SubsystemIdentity> = {
  gateway: { icon: Globe, hue: 4 }, // cyan — ingress
  bus: { icon: Radio, hue: 2 }, // teal — motion
  mcp: { icon: Wrench, hue: 1 }, // violet — the tool contract
  extensions: { icon: Boxes, hue: 3 }, // orange — the runtime
  registry: { icon: Archive, hue: 7 }, // gold — signed artifacts
  store: { icon: Database, hue: 8 }, // blue — state
  ingest: { icon: Activity, hue: 6 }, // green — flow-in
  inbox: { icon: Inbox, hue: 5 }, // rose — awaiting a human
  outbox: { icon: Send, hue: 2 }, // teal — must-deliver motion
  jobs: { icon: ListChecks, hue: 1 }, // violet — durable sessions
  acp: { icon: Cable, hue: 4 }, // cyan — the stdio adapter
};

const FALLBACK: SubsystemIdentity = { icon: Box, hue: 1 };

/** The icon + hue for `subsystemId` (a stable fallback for ids this build doesn't know). */
export function subsystemIdentity(subsystemId: string): SubsystemIdentity {
  return IDENTITIES[subsystemId] ?? FALLBACK;
}

/** CSS color strings for an identity hue — full strength, tile fill, and tile ring. */
export function hueColors(hue: number): { fg: string; bg: string; border: string } {
  return {
    fg: `hsl(var(--chart-${hue}))`,
    bg: `hsl(var(--chart-${hue}) / 0.13)`,
    border: `hsl(var(--chart-${hue}) / 0.28)`,
  };
}
