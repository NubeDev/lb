// The shell's display-only cap gate. This mirrors the old App.tsx allowed[] checks exactly; the
// gateway remains the security boundary and re-checks every verb server-side.
//
// nav-reach scope: after the cap gate, surfaces are ALSO filtered by the caller's NAV-DERIVED reach
// set — the `reach:<surface>:view` caps folded into the token at login. A curated one-page nav yields
// only that surface's reach cap, so `allowedSurfaces` drops every other page from the rail AND the
// `CoreGate` route guard redirects a deep link to it. A fallback nav yields the wildcard `reach:*:view`
// and reaches everything (a default member/admin is unaffected). This is UX + defense-in-depth; the
// server `GET /surface/{s}` reach route is the boundary (a curled read still 403s there, not here).

import { CAP, hasCap, isAdmin } from "@/lib/session";
import type { CoreSurface } from "@/features/shell";

// The surfaces that are ALWAYS reachable (never nav-reach-gated) — a curated nav never withholds your
// own channels/inbox/outbox/settings. Mirrors the server's always-visible seed (`surface_gate_cap`
// returns None for these) and the fact that no `/surface/{s}` gate is called for them.
const ALWAYS_REACHABLE: readonly CoreSurface[] = ["channels", "inbox", "outbox", "settings"];

/** The fallback wildcard reach cap — a subject with no curated nav holds this and reaches every page. */
const REACH_ALL = "reach:*:view";

/** The `reach:` cap prefix — a token carrying ANY reach cap has a nav-derived reach set to honor. */
const REACH_PREFIX = "reach:";

/**
 * May the caller reach `surface`, per their nav-derived reach caps? True iff the surface is always
 * reachable, OR the token carries the wildcard `reach:*:view` (fallback), OR the concrete
 * `reach:<surface>:view`. Mirrors the server's `reach_check`; the wildcard is expanded here because
 * `hasCap` does an exact match and never expands `*`.
 *
 * **Degrade-open when no reach caps are present.** A token with NO `reach:` cap at all (a legacy token
 * minted before this feature, or any session whose reach fold was skipped) is treated as reach-all —
 * we never lock the rail down on the mere ABSENCE of reach data. This mirrors the server login's
 * degrade-open posture (a nav-resolve error folds `reach:*:view`, not an empty set). The server
 * `/surface/{s}` route stays the boundary regardless; a real curated token DOES carry concrete reach
 * caps, so this only affects the "reach unknown" case, never the "reach says no" case.
 */
export function mayReachSurface(caps: string[] | undefined, surface: CoreSurface): boolean {
  if (ALWAYS_REACHABLE.includes(surface)) return true;
  if (hasCap(caps, REACH_ALL)) return true;
  if (hasCap(caps, `reach:${surface}:view`)) return true;
  // No reach caps at all → reach unknown → degrade open (never lock down on absence).
  return !caps?.some((c) => c.startsWith(REACH_PREFIX));
}

export function allowedSurfaces(caps: string[] | undefined): CoreSurface[] {
  // Settings is always shown — every member can edit their OWN preferences (prefs.set is member-level).
  // The workspace-default + agent controls inside it are cap-gated per-control (admin), server-enforced.
  const allowed: CoreSurface[] = ["channels", "inbox", "outbox", "settings"];
  if (hasCap(caps, CAP.dashboardList)) allowed.push("dashboards");
  if (hasCap(caps, CAP.reportList)) allowed.push("reports");
  if (hasCap(caps, CAP.rulesRun)) allowed.push("rules");
  if (hasCap(caps, CAP.flowsList)) allowed.push("flows");
  if (hasCap(caps, CAP.datasourceList)) allowed.push("datasources");
  if (hasCap(caps, CAP.reminderList)) allowed.push("reminders");
  // insights (insights umbrella scope): member-level nav gate. The page shows for any session that
  // may list insights; the gateway re-checks `mcp:insight.<verb>:call` per verb server-side.
  if (hasCap(caps, CAP.insightList)) allowed.push("insights");
  if (hasCap(caps, CAP.seriesList)) allowed.push("ingest");
  // webhooks (webhooks scope): the admin-verb gate. The page shows for a session holding
  // `webhook.manage`; the gateway re-checks every verb server-side regardless. The public inbound
  // `POST /hooks/{ws}/{id}` route takes NO session token — this gates the management surface only.
  if (hasCap(caps, CAP.webhookManage)) allowed.push("webhooks");
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
  // nav-reach: narrow the cap-allowed set to the surfaces the caller's curated nav actually reaches
  // (fallback reaches all). The server `/surface/{s}` gate is the boundary; this keeps the rail + the
  // route guard honest so a non-nav page never renders and is never shown.
  return allowed.filter((surface) => mayReachSurface(caps, surface));
}
