// Surface/path mapping for the tenant-prefixed hash URL grammar. Every surface lives under
// `/t/<ws>/…` so a shared link is self-describing AND carries a guard: the `<ws>` segment is a
// display/deep-link hint only — the gateway still derives the real workspace from the verified
// token (§7). A pasted link whose `<ws>` differs from the recipient's token is redirected, never
// obeyed (see the `/t/$ws` layout guard in createAppRouter).

import type { CoreSurface, Surface } from "@/features/shell";

export const CORE_PATHS: Record<CoreSurface, string> = {
  channels: "/channels",
  dashboards: "/dashboards",
  rules: "/rules",
  flows: "/flows",
  datasources: "/datasources",
  reminders: "/reminders",
  ingest: "/ingest",
  data: "/data",
  system: "/system",
  "system-mcp": "/system/mcp",
  "system-acp": "/system/acp",
  telemetry: "/telemetry",
  inbox: "/inbox",
  outbox: "/outbox",
  admin: "/admin",
  // Extensions + Studio are two tabs of one merged Studio page (each its own cap-gated route).
  extensions: "/studio/extensions",
  studio: "/studio/build",
  "data-studio": "/data-studio",
  settings: "/settings",
};

/** Prefix a tenant-relative surface path with the workspace segment. */
export function tenantPath(ws: string, surfacePath: string): string {
  return `/t/${encodeURIComponent(ws)}${surfacePath}`;
}

/** The tenant-relative path for a surface (no `/t/<ws>` prefix). */
export function pathForSurface(surface: Surface): string {
  if (surface.startsWith("ext:")) return `/ext/${encodeURIComponent(surface.slice(4))}`;
  return CORE_PATHS[surface as CoreSurface];
}

/** The full, shareable path for a surface within a workspace. */
export function fullPathForSurface(ws: string, surface: Surface): string {
  return tenantPath(ws, pathForSurface(surface));
}

/** Strip the `/t/<ws>` prefix, returning the tenant-relative remainder (or the input unchanged). */
export function stripTenant(pathname: string): string {
  const m = pathname.match(/^\/t\/[^/]+(\/.*)?$/);
  return m ? (m[1] ?? "/") : pathname;
}

export function surfaceForPath(pathname: string): Surface {
  const rel = stripTenant(pathname);
  if (rel.startsWith("/ext/")) {
    return `ext:${decodeURIComponent(rel.slice("/ext/".length))}`;
  }
  // Match exact, else a surface path that this URL lives under (e.g. `/datasources/timescale` →
  // `datasources`, so the detail route keeps the same nav-active + capability gate as the list).
  const exact = (Object.entries(CORE_PATHS) as [CoreSurface, string][]).find(
    ([, path]) => path === rel,
  );
  if (exact) return exact[0];
  const prefix = (Object.entries(CORE_PATHS) as [CoreSurface, string][])
    .filter(([, path]) => path !== "/")
    .find(([, path]) => rel.startsWith(`${path}/`));
  return prefix?.[0] ?? "channels";
}
