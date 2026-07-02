// Unit tests for the eval-free `template` interpolator (widget-builder scope). A pure function → a
// direct test (no gateway/mocks — it exercises the REAL interpreter the frame runs via `.toString()`).

import { describe, it, expect } from "vitest";

import { interpolateTemplate, type TemplateData } from "./templateInterpolate";

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
});
