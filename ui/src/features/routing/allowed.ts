// The shell's display-only cap gate. This mirrors the old App.tsx allowed[] checks exactly; the
// gateway remains the security boundary and re-checks every verb server-side.

import { CAP, hasCap, isAdmin } from "@/lib/session";
import type { CoreSurface } from "@/features/shell";

export function allowedSurfaces(caps: string[] | undefined): CoreSurface[] {
  const allowed: CoreSurface[] = ["channels", "inbox", "outbox"];
  if (hasCap(caps, CAP.dashboardList)) allowed.push("dashboards");
  if (hasCap(caps, CAP.rulesRun)) allowed.push("rules");
  if (hasCap(caps, CAP.chainsGet)) allowed.push("chains");
  if (hasCap(caps, CAP.flowsList)) allowed.push("flows");
  if (hasCap(caps, CAP.datasourceList)) allowed.push("datasources");
  if (hasCap(caps, CAP.reminderList)) allowed.push("reminders");
  if (hasCap(caps, CAP.seriesList)) allowed.push("ingest");
  if (hasCap(caps, CAP.storeScan)) allowed.push("data");
  if (hasCap(caps, CAP.systemOverview)) allowed.push("system");
  // The MCP / ACP service pages are drilled from the System page; each shows behind its own cap.
  if (hasCap(caps, CAP.systemTools)) allowed.push("system-mcp");
  if (hasCap(caps, CAP.systemAcp)) allowed.push("system-acp");
  if (isAdmin(caps)) allowed.push("admin");
  if (hasCap(caps, CAP.extList)) allowed.push("extensions");
  if (hasCap(caps, CAP.devkitTemplates)) allowed.push("studio");
  return allowed;
}
