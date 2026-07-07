// Unit tests for the eval-free `template` interpolator (widget-builder scope). A pure function → a
// direct test (no gateway/mocks — it exercises the REAL interpreter the frame runs via `.toString()`).

import { describe, it, expect } from "vitest";

import { interpolateTemplate, type TemplateData } from "./templateInterpolate";
import { DEFAULT_INLINE_CODE } from "./editors/TemplateSourceField";
import { sanitizeTemplateHtml } from "./sanitizeTemplateHtml";

const data = (rows: Array<Record<string, unknown>>, extra: Partial<TemplateData> = {}): TemplateData => ({
  rows,
  latest: rows.length ? rows[rows.length - 1] : null,
  ...extra,
});

describe("interpolateTemplate", () => {
  it("binds a scalar path", () => {
    expect(interpolateTemplate("<b>{{rows.length}}</b>", data([{ a: 1 }, { a: 2 }]))).toBe("<b>2</b>");
  });

  it("reads the latest row", () => {
    expect(interpolateTemplate("{{latest.a}}", data([{ a: 1 }, { a: 9 }]))).toBe("9");
  });

  it("iterates rows with {{#each}} using the item as context", () => {
    const out = interpolateTemplate("<ul>{{#each rows}}<li>{{seq}}</li>{{/each}}</ul>", data([{ seq: 1 }, { seq: 2 }]));
    expect(out).toBe("<ul><li>1</li><li>2</li></ul>");
  });

  it("renders nothing for {{#each}} over a non-array / empty", () => {
    expect(interpolateTemplate("[{{#each rows}}x{{/each}}]", data([]))).toBe("[]");
    expect(interpolateTemplate("[{{#each nope}}x{{/each}}]", data([{ a: 1 }]))).toBe("[]");
  });

  it("escapes interpolated DATA values (no markup injection from a row)", () => {
    const out = interpolateTemplate(
      "<ul>{{#each rows}}<li>{{name}}</li>{{/each}}</ul>",
      data([{ name: "<img src=x onerror=alert(1)>" }]),
    );
    expect(out).not.toContain("<img");
    expect(out).toBe("<ul><li>&lt;img src=x onerror=alert(1)&gt;</li></ul>");
  });

  it("escapes values inside each blocks and attributes", () => {
    const out = interpolateTemplate(
      `{{#each rows}}<span title="{{title}}">{{title}}</span>{{/each}}`,
      data([{ title: `a"b<c` }]),
    );
    expect(out).toBe(`<span title="a&quot;b&lt;c">a&quot;b&lt;c</span>`);
  });

  it("renders an unknown path as empty, never crashes", () => {
    expect(interpolateTemplate("[{{a.b.c.d}}]", data([]))).toBe("[]");
  });

  it("renders an object value as compact JSON, not [object Object]", () => {
    expect(interpolateTemplate("{{latest}}", data([{ a: 1 }]))).toBe(`{&quot;a&quot;:1}`);
  });

  it("leaves author markup untouched (only data is escaped)", () => {
    const tpl = `<div class="p-2"><button data-call="x.y" data-args='{"n":1}'>Go</button></div>`;
    expect(interpolateTemplate(tpl, data([]))).toBe(tpl);
  });

  it("the shipped default template renders point_reading rows (the last-N-per-meter query shape)", () => {
    // Guards the starter example against drifting from the engine: rows are what the demo query
    // returns — SELECT *, ROW_NUMBER() … AS rn FROM point_reading WHERE point_id IN (…) → rn <= 100.
    const rows = [
      { time: 1751500000, point_id: "meter-001-kwh", value: 4.51, rn: 1 },
      { time: 1751400000, point_id: "meter-002-kwh", value: 13.83, rn: 1 },
    ];
    const out = interpolateTemplate(DEFAULT_INLINE_CODE, { rows, latest: rows[rows.length - 1] });
    expect(out).toContain("2 readings"); // {{rows.length}}
    expect(out).toContain("13.83"); // {{latest.value}} in the hero
    expect(out).toContain("meter-001-kwh"); // {{point_id}} per feed row
    expect(out).toContain("4.51"); // {{value}} per feed row
    expect(out).toContain("#1"); // {{rn}} chip
    expect(out).not.toContain("{{"); // every binding resolved
    // The styled look must SURVIVE the real sanitizer (DOMPurify keeps plain style declarations) —
    // otherwise the shipped example renders as unstyled text.
    const safe = sanitizeTemplateHtml(out);
    expect(safe).toContain("linear-gradient");
    expect(safe).toContain("--accent");
  });
});
