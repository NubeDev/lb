// The preview lens (setup scope) — an HONEST, client-side approximation of what a given user will see
// in their sidebar, computed from their EFFECTIVE caps (resolved server-side via `authz.resolve`) and
// the NAV that applies to them. It mirrors the two real rendering paths in `NavRail`:
//
//   • A nav applies  → the rail renders ONLY that nav's items, each cap-stripped (`resolvedMenu`).
//                       So the preview strips the nav's items the same way and renders THOSE.
//   • No nav (built-in sidebar) → the rail renders `allowedSurfaces(caps)` (the fallback).
//
// This is why building a 1-item nav must preview as 1 item, not the whole cap-allowed set: when a nav
// applies it REPLACES the surface list. This module reuses the shell's own gate (`allowedSurfaces`,
// `SURFACE_DEF`) so the preview and the real rail agree; the gateway re-checks every verb server-side
// regardless (rule 5). A PREVIEW, never a boundary.

import { allowedSurfaces } from "@/features/routing/allowed";
import { SURFACE_DEF } from "@/features/shell/surfaceDefs";
import type { CoreSurface } from "@/features/shell";
import { CAP, hasCap } from "@/lib/session";
import type { SourcedCap } from "@/lib/admin/grants.api";
import type { NavItem } from "@/lib/nav";

/** One row the previewed user would see in their rail, with a display label + an icon key. A
 *  `dashboard`/`ext` row reuses the Dashboards/Studio surface icon (as the real rail does). */
export interface PreviewRow {
  label: string;
  /** The surface whose icon to show (dashboards for a board, extensions for an ext page). */
  icon: CoreSurface;
}

/** The provenance chip for a cap the user holds (why they have it). */
export type CapWhy = "direct" | { role: string } | { team: string };

/** Flatten resolved sourced-caps to the plain cap-string list the gates expect. */
export function flattenCaps(sourced: SourcedCap[]): string[] {
  return sourced.map((s) => s.cap);
}

/** The distinct provenance chips for a cap (direct / via role / via team) — for the "why" summary. */
export function capWhy(sourced: SourcedCap): CapWhy[] {
  return sourced.source.map((s) =>
    s.kind === "direct" ? "direct" : s.kind === "role" ? { role: s.name } : { team: s.name },
  );
}

/** Does `caps` allow reaching this core surface? Reuses the shell's own display gate so the preview
 *  and the real rail agree exactly. (Reach caps aren't folded into a resolve, so this is the cap gate
 *  — the honest "can this surface render for them" test.) */
function surfaceAllowed(caps: string[], surface: string): boolean {
  return allowedSurfaces(caps).includes(surface as CoreSurface);
}

/**
 * The rail rows for a user with `caps` when the given NAV applies — the nav's items, cap-stripped and
 * flattened one level (a `group`'s children are inlined into the preview), mirroring `resolvedMenu`.
 * An item the user can't reach is dropped (the lens). Empty nav or all-stripped → empty (the real rail
 * would then fall back, but an APPLIED nav that resolves empty is itself the honest answer here).
 */
export function previewNavRows(items: NavItem[], caps: string[]): PreviewRow[] {
  const rows: PreviewRow[] = [];
  const push = (list: NavItem[]) => {
    for (const it of list) {
      if (it.kind === "surface" && it.surface) {
        if (surfaceAllowed(caps, it.surface)) {
          rows.push({ label: it.label || SURFACE_DEF[it.surface as CoreSurface]?.label || it.surface, icon: it.surface as CoreSurface });
        }
      } else if (it.kind === "dashboard" || it.kind === "tag-group" || it.kind === "template-group") {
        // Dashboard-backed rows survive if the user can read dashboards (list/get is the proxy the
        // real resolver uses per board; a tag/template group expands to boards under the same gate).
        if (hasCap(caps, CAP.dashboardList) || hasCap(caps, CAP.dashboardGet)) {
          rows.push({ label: it.label || it.dashboard || "Dashboard", icon: "dashboards" });
        }
      } else if (it.kind === "ext" && it.ext) {
        if (hasCap(caps, CAP.extList)) rows.push({ label: it.label || it.ext, icon: "extensions" });
      } else if (it.kind === "group" && it.items) {
        push(it.items); // one nesting level, inlined for the preview
      }
    }
  };
  push(items);
  return rows;
}

/** The rail rows for a user with `caps` when NO nav applies — the built-in fallback = `allowedSurfaces`
 *  (the exact fallback the real rail renders), each with its canonical label + icon. */
export function previewFallbackRows(caps: string[]): PreviewRow[] {
  return allowedSurfaces(caps).map((key) => ({
    label: SURFACE_DEF[key]?.label ?? key,
    icon: key,
  }));
}
