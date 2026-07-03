// The shell's display-only cap gate. This mirrors the old App.tsx allowed[] checks exactly; the
// gateway remains the security boundary and re-checks every verb server-side.

import { CAP, hasCap, isAdmin } from "@/lib/session";
import type { CoreSurface } from "@/features/shell";

export function allowedSurfaces(caps: string[] | undefined): CoreSurface[] {
  // Settings is always shown — every member can edit their OWN preferences (prefs.set is member-level).
  // The workspace-default + agent controls inside it are cap-gated per-control (admin), server-enforced.
  const allowed: CoreSurface[] = ["channels", "inbox", "outbox", "settings"];
  if (hasCap(caps, CAP.dashboardList)) allowed.push("dashboards");
  if (hasCap(caps, CAP.rulesRun)) allowed.push("rules");
  if (hasCap(caps, CAP.flowsList)) allowed.push("flows");
  if (hasCap(caps, CAP.datasourceList)) allowed.push("datasources");
  if (hasCap(caps, CAP.reminderList)) allowed.push("reminders");
  if (hasCap(caps, CAP.seriesList)) allowed.push("ingest");
  if (hasCap(caps, CAP.storeScan)) allowed.push("data");
  if (hasCap(caps, CAP.systemOverview)) allowed.push("system");
  // The MCP / ACP service pages are drilled from the System page; each shows behind its own cap.
  if (hasCap(caps, CAP.systemTools)) allowed.push("system-mcp");
  if (hasCap(caps, CAP.systemAcp)) allowed.push("system-acp");
  if (hasCap(caps, CAP.telemetryRead)) allowed.push("telemetry");
  if (isAdmin(caps)) allowed.push("admin");
  if (hasCap(caps, CAP.extList)) allowed.push("extensions");
  if (hasCap(caps, CAP.devkitTemplates)) allowed.push("studio");
  // Data Studio (data-studio scope): the explore + panel-build workbench. Member-level — shown for any
  // session that can read data (list series is the "can explore" proxy); the build/save-as-library
  // action is separately gated on `panel.save`, and every explore call re-checks its source's own cap.
  if (hasCap(caps, CAP.seriesList)) allowed.push("data-studio");
  return allowed;
}
