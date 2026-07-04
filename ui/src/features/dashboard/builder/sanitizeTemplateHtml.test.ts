// The XSS-vector suite for `sanitizeTemplateHtml` (render-template-inprocess scope, Decision 1).
// This test IS the security boundary: with the sandboxed iframe gone for `template`, a gap in the
// sanitizer is an XSS in the shell (cookies, token-adjacent). So the suite is EXHAUSTIVE — every
// classic XSS vector, mutation tricks, namespace confusion, URL schemes, and the resilience cases
// (idempotence, never-throws, malformed input). It is run as a plain unit test (no gateway, no DOM
// render) — `sanitizeTemplateHtml` is a pure `string → string`, the cheapest place to prove the wall.
//
// What MUST survive: the `data-call`/`data-args` attributes (the `[data-call]` write-button wiring
// reads them after commit) and a conservative structural tag/attribute set (a template is author
// HTML — <ul>, <li>, <button>, class, style, etc.). What MUST NOT survive: anything that executes
// script, loads remote resources, or breaks out of the markup contract. See the scope's Testing plan.

import { describe, it, expect } from "vitest";

import { sanitizeTemplateHtml } from "./sanitizeTemplateHtml";

/** Helper: assert the sanitized output does NOT contain a script-execution substring anywhere,
 *  case-insensitively, across the `onerror`/`onload`/`on*` family and the obvious sinks. */
function assertNoScriptSink(out: string, label = ""): void {
  const lower = out.toLowerCase();
  // No <script> block (open or close).
  expect(lower, `${label}: no <script>`).not.toContain("</script");
  expect(lower, `${label}: no <script`).not.toContain("<script");
  // No inline event handler attributes (onload, onerror, onclick, onanything).
  expect(lower, `${label}: no on* handler`).not.toMatch(/\son\w+\s*=/);
  // No javascript: URL scheme (in any attribute value, post-sanitize).
  expect(lower, `${label}: no javascript: url`).not.toContain("javascript:");
  // No script-data: URL (an <a href="data:text/html…"> or <iframe src=data:...>). Image data: is allowed.
  // We assert no `data:text/html` or `data:application/` (the script-bearing data: media types).
  expect(lower, `${label}: no script data: url`).not.toContain("data:text/html");
  expect(lower, `${label}: no script data: url`).not.toContain("data:application/");
}

describe("sanitizeTemplateHtml — XSS vectors (the security wall)", () => {
  it("keeps allowed structural markup and classes unchanged", () => {
    const out = sanitizeTemplateHtml(
      `<div class="p-2"><ul><li>one</li><li>two</li></ul><p style="color:red">x</p></div>`,
    );
    expect(out).toContain("<div");
    expect(out).toContain("<ul>");
    expect(out).toContain("<li>one</li>");
    expect(out).toContain('class="p-2"');
    expect(out).toContain('style="color:red"');
  });

  it("preserves the data-call and data-args attributes (the write-button contract)", () => {
    const out = sanitizeTemplateHtml(
      `<button data-call="rules.run" data-args='{"rule_id":"r1"}'>Recompute</button>`,
    );
    expect(out).toContain('data-call="rules.run"');
    expect(out).toContain("data-args=");
    expect(out).toContain("&quot;rule_id&quot;"); // DOMPurify re-serializes attribute quotes; value survives
    expect(out).toContain("Recompute");
  });

  it("strips a bare <script> block", () => {
    const out = sanitizeTemplateHtml(`<div>x</div><script>alert(1)</script><div>y</div>`);
    assertNoScriptSink(out);
    expect(out).toContain("<div>x</div>");
    expect(out).toContain("<div>y</div>");
  });

  it("strips <script src=...> (remote script load)", () => {
    const out = sanitizeTemplateHtml(`<script src="https://evil.example/x.js"></script><p>hi</p>`);
    assertNoScriptSink(out);
    expect(out).not.toContain("evil.example");
    expect(out).toContain("<p>hi</p>");
  });

  it("strips all on* event-handler attributes (onerror, onload, onclick, onmouseover, …)", () => {
    const vectors = [
      `<img src=x onerror="alert(1)">`,
      `<img src=x onerror="fetch('/steal')">`,
      `<body onload="alert(1)">x</body>`,
      `<div onclick="alert(1)">click</div>`,
      `<svg onload="alert(1)">`,
      `<input onfocus=alert(1) autofocus>`,
      `<a href="#" onmouseover="alert(1)">x</a>`,
    ];
    for (const v of vectors) {
      const out = sanitizeTemplateHtml(v);
      assertNoScriptSink(out, v);
    }
  });

  it("strips javascript: URLs from href/src/action/formaction (anchors, images, forms)", () => {
    const vectors = [
      `<a href="javascript:alert(1)">x</a>`,
      `<a href="JaVaScRiPt:alert(1)">x</a>`,
      `<a href=" javascript:alert(1)">x</a>`,
      `<iframe src="javascript:alert(1)"></iframe>`,
      `<form action="javascript:alert(1)"><button>x</button></form>`,
      `<button formaction="javascript:alert(1)">x</button>`,
    ];
    for (const v of vectors) {
      const out = sanitizeTemplateHtml(v);
      assertNoScriptSink(out, v);
    }
  });

  it("strips <iframe>, <object>, <embed>, <link>, <meta>, <base>", () => {
    const vectors = [
      `<iframe src="https://evil.example"></iframe>`,
      `<object data="https://evil.example/swf"></object>`,
      `<embed src="https://evil.example/x">`,
      `<link rel="stylesheet" href="https://evil.example/x.css">`,
      `<meta http-equiv="refresh" content="0;url=https://evil.example">`,
      `<base href="https://evil.example/">`,
    ];
    for (const v of vectors) {
      const out = sanitizeTemplateHtml(v);
      expect(out.toLowerCase(), v).not.toContain("<iframe");
      expect(out.toLowerCase(), v).not.toContain("<object");
      expect(out.toLowerCase(), v).not.toContain("<embed");
      expect(out.toLowerCase(), v).not.toContain("<link");
      expect(out.toLowerCase(), v).not.toContain("<meta");
      expect(out.toLowerCase(), v).not.toContain("<base");
      expect(out.toLowerCase(), v).not.toContain("evil.example");
    }
  });

  it("allows safe image data: URLs (img src=data:image/...) but strips script-bearing data: URLs", () => {
    const safe = sanitizeTemplateHtml(`<img src="data:image/png;base64,iVBORw0KGgo=" alt="x">`);
    expect(safe).toContain("data:image/png;base64,");
    expect(safe).toContain('alt="x"');

    const hostile = sanitizeTemplateHtml(`<img src="data:text/html,<script>alert(1)</script>">`);
    assertNoScriptSink(hostile);
  });

  it("strips style expressions and script-bearing styles (IE expression(), -moz-binding, behavior)", () => {
    const vectors = [
      `<div style="width: expression(alert(1))">x</div>`,
      `<div style="-moz-binding: url('https://evil.example/x.xml')">x</div>`,
      `<style>x { behavior: url(#default#time2); }</style>`,
    ];
    for (const v of vectors) {
      const out = sanitizeTemplateHtml(v);
      const lower = out.toLowerCase();
      expect(lower, v).not.toContain("expression(");
      expect(lower, v).not.toContain("-moz-binding");
      expect(lower, v).not.toContain("behavior:");
    }
  });

  it("defeats mutation-XSS: a <style> or comment that breaks out into a script context cannot re-emerge", () => {
    // Classic mutation trick: an unclosed comment / a <style> that swallows the next element, then a
    // closing sequence that re-opens parsing in a script-favoring context. DOMPurify re-parses through
    // a real DOM, so it does not naively concatenate; assert no <script>/on* survives the round-trip.
    const mutants = [
      `<style><style /><img src=x onerror=alert(1)>`,
      `<!--><img src=x onerror=alert(1)>-->`,
      `<noscript><p title="</noscript><img src=x onerror=alert(1)>">`,
      `<svg><style><img src=x onerror=alert(1)></style></svg>`,
      `<xmp><img src=x onerror=alert(1)></xmp>`,
      `<<script>alert(1)//<</script>`,
      `<svg></p><style><a id="</style><img src=1 onerror=alert(1)>">`,
      `<math><mtext><table><mglyph><style><img src=x onerror=alert(1)>`,
    ];
    for (const v of mutants) {
      const out = sanitizeTemplateHtml(v);
      assertNoScriptSink(out, v);
    }
  });

  it("defeats svg / math namespace tricks (foreignObject, <svg><script>, <use href=...>)", () => {
    const vectors = [
      `<svg><script>alert(1)</script></svg>`,
      `<svg><foreignobject><body><script>alert(1)</script></body></foreignobject></svg>`,
      `<svg><use href="data:image/svg+xml,<svg><script>alert(1)</script></svg>" /></svg>`,
      `<math><maction actiontype="statusline#" xlink:href="javascript:alert(1)">x</maction></math>`,
      `<svg><animate href="javascript:alert(1)" attributeName="href" /></svg>`,
    ];
    for (const v of vectors) {
      const out = sanitizeTemplateHtml(v);
      assertNoScriptSink(out, v);
    }
  });

  it("strips a tag that is ONLY an event handler (no real content) and the handler does not survive", () => {
    const out = sanitizeTemplateHtml(`<details open ontoggle=alert(1)>x</details>`);
    assertNoScriptSink(out);
    expect(out).toContain("x");
  });

  it("never throws on malformed / truncated / hostile input (resilience)", () => {
    const malformed = [
      ``,
      `<`,
      `>`,
      `<<<`,
      `<div`,
      `<div class=`,
      `<div class="x`,
      `</div>`,
      `<script`,
      `<<img src=x onerror=alert(1)>`,
      `<a href="javascript:`,
      `\x00\x01\x02<div>\x03</div>`,
      `<img src=x onerror=alert(1) `.repeat(1000),
      `<${"a".repeat(10000)}>`,
      `{"this":"is not html"}`,
      `plain text with no tags at all`,
    ];
    for (const v of malformed) {
      expect(() => sanitizeTemplateHtml(v), v.slice(0, 40)).not.toThrow();
      assertNoScriptSink(sanitizeTemplateHtml(v), v.slice(0, 40));
    }
  });

  it("is idempotent: sanitize(sanitize(x)) === sanitize(x)", () => {
    const inputs = [
      `<div class="p-2"><ul><li>a</li></ul></div>`,
      `<button data-call="x.y" data-args='{"n":1}'>Go</button>`,
      `<img src=x onerror=alert(1)>`,
      `<script>alert(1)</script><p>ok</p>`,
      `<a href="javascript:alert(1)">x</a>`,
      `<svg><script>x</script></svg>`,
      `<div style="color:red" onclick="alert(1)">hi</div>`,
    ];
    for (const v of inputs) {
      const once = sanitizeTemplateHtml(v);
      const twice = sanitizeTemplateHtml(once);
      expect(twice, v).toBe(once);
    }
  });

  it("does not strip data-* attributes broadly beyond the structural need (data-call survives in nested markup)", () => {
    const out = sanitizeTemplateHtml(
      `<div class="card"><button class="btn" data-call="store.query" data-args='{"sql":"SELECT 1"}'>Run</button></div>`,
    );
    expect(out).toContain('data-call="store.query"');
    expect(out).toContain("data-args=");
    expect(out).toContain('class="card"');
    expect(out).toContain('class="btn"');
  });

  it("treats a non-string input defensively (returns an empty string, never throws)", () => {
    // Defense-in-depth: the renderer always passes a string, but a sanitizer that throws on a weird
    // input is itself an XSS vector (a try/catch in the renderer would then render the unsanitized
    // string). Assert the contract: bad input → empty string, never an exception.
    expect(sanitizeTemplateHtml(null as unknown as string)).toBe("");
    expect(sanitizeTemplateHtml(undefined as unknown as string)).toBe("");
    expect(sanitizeTemplateHtml(123 as unknown as string)).toBe("");
  });
});
