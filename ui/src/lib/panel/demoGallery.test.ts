// The starter gallery renders cleanly through the REAL template engine (the same eval-free
// `interpolateTemplate` + `sanitizeTemplateHtml` the `TemplateView` runs). This guards the two ways a
// hand-written starter silently breaks: an unsupported `{{…}}` construct (leaves a literal token), or
// markup the sanitizer strips (a tag/attr not on the allow-list — e.g. someone adds an <svg>). If a
// binding didn't resolve, a `{{token}}` survives; if the sanitizer ate the structure, the row content
// vanishes. Both are asserted here so the examples can't rot into the ugly/empty state we fixed.
//
// MOVED from features/admin/setup/templateGallery.test.ts alongside the gallery (now demoGallery.ts).

import { describe, expect, it } from "vitest";

import { interpolateTemplate, type TemplateData } from "@/features/dashboard/builder/templateInterpolate";
import { sanitizeTemplateHtml } from "@/features/dashboard/builder/sanitizeTemplateHtml";
import { TEMPLATE_GALLERY } from "@/lib/panel";

const ROWS = [
  { site: "Airport Terminal T2", total_kwh: 1284.5, peak_kwh: 9.4, avg_kwh: 4.31, pct: 100, rnk: 1 },
  { site: "Warehouse North", total_kwh: 902.1, peak_kwh: 7.2, avg_kwh: 3.1, pct: 70, rnk: 2 },
  { site: "Office HQ", total_kwh: 544, peak_kwh: 5.1, avg_kwh: 2.05, pct: 42, rnk: 3 },
];
const DATA: TemplateData = { rows: ROWS, latest: ROWS[ROWS.length - 1] };

describe("template starter gallery", () => {
  for (const ex of TEMPLATE_GALLERY) {
    it(`${ex.id}: interpolates with no leftover {{tokens}} and survives the sanitizer`, () => {
      const interpolated = interpolateTemplate(ex.code, DATA);
      // No unresolved template token remains (an unsupported construct leaves a literal `{{…}}`).
      expect(interpolated).not.toMatch(/\{\{|\}\}/);

      const clean = sanitizeTemplateHtml(interpolated);
      // The real data landed and survived sanitizing (structure + values were not stripped) — every
      // example surfaces the top site's total, so a stripped/broken template loses this.
      expect(clean).toContain("Airport Terminal T2");
      expect(clean).toContain("1284.5");
      // A non-trivial widget, not a stripped-to-nothing husk.
      expect(clean.length).toBeGreaterThan(400);
      // The templates are INLINE-STYLED (the sanitizer strips <style> blocks) — assert the styling
      // actually survived rather than collapsing to unstyled text.
      expect(clean).toContain("style=");
      expect(clean).not.toContain("<style");
      // The bar examples size a fill by the pre-computed share — assert the literal width landed.
      if (ex.code.includes("width:{{pct}}%")) expect(clean).toContain("width:100%");
    });
  }

  it("the stats example keeps its native <details> toggle through the sanitizer (a JS-free expander)", () => {
    const stats = TEMPLATE_GALLERY.find((e) => e.id === "stats")!;
    const clean = sanitizeTemplateHtml(interpolateTemplate(stats.code, DATA));
    // The disclosure survives sanitizing — a real, no-JS "Show all sites" toggle.
    expect(clean).toContain("<details");
    expect(clean).toContain("<summary");
    expect(clean).toContain("Show all sites");
  });

  it("every example has a summary SQL and a distinct id/label", () => {
    const ids = new Set(TEMPLATE_GALLERY.map((e) => e.id));
    expect(ids.size).toBe(TEMPLATE_GALLERY.length);
    for (const ex of TEMPLATE_GALLERY) {
      expect(ex.sql).toMatch(/SELECT/i);
      expect(ex.label.length).toBeGreaterThan(0);
    }
  });
});
