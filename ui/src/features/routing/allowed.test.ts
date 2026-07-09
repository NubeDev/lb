// nav-reach scope: `allowedSurfaces` narrows the cap-allowed rail to the surfaces the caller's
// NAV-DERIVED reach caps (`reach:<surface>:view`, folded into the token at login) actually grant. This
// is UX + defense-in-depth — the server `GET /surface/{s}` route is the boundary (see
// `role/gateway/tests/nav_reach_test.rs`), but the rail + `CoreGate` route guard must agree with it so
// a non-nav page is never shown and a deep link to it redirects.

import { describe, expect, it } from "vitest";

import { allowedSurfaces, mayReachSurface } from "./allowed";
import { CAP } from "@/lib/session";

// A full author's cap set — enough that every cap-gated surface would pass the cap gate, so the reach
// filter is the ONLY thing that can drop a surface. (We don't need the admin caps; the point is the
// reach intersection, not the cap list.)
const MEMBER_CAPS = [
  CAP.dashboardList,
  CAP.rulesRun,
  CAP.flowsList,
  CAP.datasourceList,
  CAP.reminderList,
  CAP.seriesList,
  CAP.insightList,
];

describe("mayReachSurface", () => {
  it("always-reachable surfaces are never nav-reach-gated", () => {
    for (const s of ["channels", "inbox", "outbox", "settings"] as const) {
      // Even with NO reach cap at all, the always-visible surfaces stay reachable.
      expect(mayReachSurface([], s)).toBe(true);
    }
  });

  it("the wildcard `reach:*:view` (fallback) reaches every gated surface", () => {
    for (const s of ["dashboards", "rules", "flows", "ingest", "datasources"] as const) {
      expect(mayReachSurface(["reach:*:view"], s)).toBe(true);
    }
  });

  it("a concrete reach cap reaches ONLY that surface", () => {
    const caps = ["reach:dashboards:view"];
    expect(mayReachSurface(caps, "dashboards")).toBe(true);
    expect(mayReachSurface(caps, "rules")).toBe(false);
    expect(mayReachSurface(caps, "ingest")).toBe(false);
  });
});

describe("allowedSurfaces nav-reach intersection", () => {
  it("a curated one-page nav (reach:dashboards:view) shows ONLY dashboards + the always-visible seed", () => {
    const caps = [...MEMBER_CAPS, "reach:dashboards:view"];
    const allowed = allowedSurfaces(caps);
    // The one granted page is present…
    expect(allowed).toContain("dashboards");
    // …the always-visible seed stays…
    for (const s of ["channels", "inbox", "outbox", "settings"] as const) {
      expect(allowed).toContain(s);
    }
    // …and every OTHER cap-allowed page is dropped by the reach filter (the headline).
    for (const s of ["rules", "flows", "ingest", "datasources", "data-studio"] as const) {
      expect(allowed).not.toContain(s);
    }
  });

  it("a fallback nav (reach:*:view) shows every cap-allowed page (no lock-out)", () => {
    const caps = [...MEMBER_CAPS, "reach:*:view"];
    const allowed = allowedSurfaces(caps);
    for (const s of ["dashboards", "rules", "flows", "ingest", "datasources"] as const) {
      expect(allowed).toContain(s);
    }
  });
});
