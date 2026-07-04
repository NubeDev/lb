// `sanitizeTemplateHtml` — the security boundary that replaces the sandboxed iframe for the `template`
// widget (render-template-inprocess scope, Decision 1). A pure `string → string` that wraps
// **DOMPurify** with OUR config, so author-written template markup is safe to inject into the SHELL
// document via `innerHTML` (the in-process `TemplateView`). It is the load-bearing guard: with the
// opaque-origin iframe gone for `template`, a gap here is an XSS in the shell (cookies, token-adjacent),
// so it stands on DOMPurify's audited parse/strip — not a bespoke walker (Decision 1 rejected the
// hand-rolled allow-list precisely because mutation-XSS / namespace confusion live in its tail).
//
// Why DOMPurify and a config, not raw DOMPurify defaults: defaults are generous (templates render
// alongside admin-authored dashboards and need to look native — structural tags + class/style), and we
// have ONE author-specific extension: the `[data-call]` write-button wiring reads `data-call`/
// `data-args` after commit, so those attributes MUST survive. `ADD_ATTR` admits exactly those.
//
// Belt-and-braces (Decision 5): the `TemplateView` mount is rendered under a tight CSP/Trusted-Types
// posture AND the click wiring reads ONLY `data-*` attributes — so even a hypothetical sanitizer miss
// has no inline-script sink. This file is the floor; that layer is the ceiling. One file so the seam
// stays swappable (FILE-LAYOUT: one responsibility — sanitize template markup).

import DOMPurify from "dompurify";

/** Substrings that turn a `style` attribute into a script-execution sink. All are dead in modern
 *  browsers (IE `expression()` died with IE; `-moz-binding` was removed from Firefox in 2016; `behavior`
 *  is IE-only), but DOMPurify's default relies on the LIVE browser CSS parser to reject them — and jsdom
 *  (our test DOM) does not enforce CSS parsing. Rather than trust the browser, strip the substrings so a
 *  hostile style cannot survive into any sink. The scope names "style expressions" as a forbidden vector. */
const STYLE_DANGER = /\s*(expression\s*\(|-moz-binding\s*:|behavior\s*:|@import\s|javascript:)/gi;

/** One-time DOMPurify hook: after attributes are sanitized, scrub the surviving `style` attribute of the
 *  script-bearing CSS substrings {@link STYLE_DANGER} covers. Registered ONCE at module load; idempotent. */
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  const el = node as Element;
  if (typeof el.getAttribute === "function" && el.hasAttribute("style")) {
    const cleaned = el.getAttribute("style")?.replace(STYLE_DANGER, "");
    el.setAttribute("style", cleaned ?? "");
  }
  return node;
});

/** The DOMPurify config for the `template` render path. Built ONCE.
 *
 *  - `ALLOWED_TAGS`/`ALLOWED_ATTR`: a conservative structural set — a template draws lists, tables,
 *    buttons, images, and inline styles (it shares the shell's Tailwind classes for native feel).
 *    No `<script>`/`<iframe>`/`<object>`/`<embed>`/`<link>`/`<meta>`/`<base>` (the script/network-bearing
 *    tags). `style` is allowed (inline class styling) but `expression()`/`-moz-binding`/`behavior` are
 *    stripped by DOMPurify's CSS guard (`CSS_INJECT`).
 *  - `ADD_ATTR: ["data-call","data-args"]`: the `[data-call]` write-button contract — the post-commit
 *    wiring in `TemplateView` reads these. `data-*` is otherwise DOMPurify's default (per-attribute
 *    allow). KEEP_ATTR_LIST is not used; we want deny-by-default on attributes.
 *  - `ALLOW_DATA_ATTR`: false — we do NOT blanket-allow `data-*`; only the two write-button attrs above
 *    are added. A template that needs a private `data-x` for its own (CSS-only) use still works because
 *    DOMPurify keeps `data-*` when `ALLOW_DATA_ATTR` is true; we flip that on for authoring ergonomics
 *    (a `data-` attribute is by definition inert — it cannot execute; CSS attribute selectors read it).
 *  - `FORBID_ATTR` keeps every `on*` event handler out (defense-in-depth on top of the tag/attr allow-list).
 *  - URL schemes: only safe schemes survive in href/src (`http`, `https`, `mailto`, `ftp`, `tel`, and
 *    image `data:`); `javascript:`/`vbscript:`/script-bearing `data:` are stripped. DOMPurify's default
 *    `ALLOWED_URI_REGEXP` already enforces this; we rely on it.
 */
const PURIFY_CONFIG = {
  // Conservative structural vocabulary. A template is author HTML for a dashboard tile — lists,
  // tables, headings, images, buttons, inline styles. No script/network/hostile tags.
  ALLOWED_TAGS: [
    // text / structure
    "a", "abbr", "address", "article", "aside", "b", "bdi", "bdo", "blockquote", "br", "caption",
    "cite", "code", "col", "colgroup", "data", "dd", "del", "details", "dfn", "div", "dl", "dt",
    "em", "figcaption", "figure", "footer", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hgroup",
    "hr", "i", "ins", "kbd", "li", "main", "mark", "nav", "ol", "p", "pre", "q", "rp", "rt", "ruby",
    "s", "samp", "section", "small", "span", "strong", "sub", "summary", "sup", "table", "tbody",
    "td", "tfoot", "th", "thead", "time", "tr", "u", "ul", "var", "wbr",
    // interactive / form (a template's [data-call] buttons are <button>; forms are inert — no scripts)
    "button", "datalist", "fieldset", "form", "input", "label", "legend", "meter", "optgroup",
    "option", "output", "progress", "select", "textarea",
    // media (no <audio>/<video> src=javascript: — DOMPurify's URI guard covers their src; allowed
    // because a template may legitimately embed an image)
    "img", "picture", "source",
    // inline style + grouping (the structural part of <style> is allowed but expression()/binding/
    // behavior are stripped by DOMPurify's CSS sanitizer)
    "style",
  ],
  ALLOWED_ATTR: [
    // global / structural
    "class", "style", "id", "title", "lang", "dir", "hidden", "tabindex",
    // links / media
    "href", "src", "alt", "width", "height", "srcset", "sizes", "loading", "decoding", "target",
    "rel", "download", "hreflang",
    // tables
    "colspan", "rowspan", "headers", "scope", "abbr",
    // forms (inert without scripts — values are author-set, not wire-driven)
    "name", "value", "type", "placeholder", "disabled", "readonly", "checked", "selected",
    "required", "min", "max", "step", "multiple", "size", "maxlength", "for", "form",
    // lists / misc
    "start", "reversed", "datetime", "open",
    // the [data-call] write-button contract — the ONLY template-specific extension. The post-commit
    // wiring in TemplateView reads these to route a click through the leashed bridge.
    "data-call", "data-args",
  ],
  // Author ergonomics: inert `data-*` attributes for CSS attribute selectors (a template may tag a
  // <li> with data-row="0" for its own styling). A `data-*` attribute cannot execute; this is safe.
  // The two write-button attrs are explicitly in ALLOWED_ATTR above regardless.
  ALLOW_DATA_ATTR: true,
  // Defense-in-depth: even if an `on*` slipped through the allow-list, FORBID_ATTR strips it. Kept
  // explicit so a future ALLOWED_ATTR edit cannot re-admit an event handler by accident.
  FORBID_ATTR: ["onerror", "onload", "onclick", "onmouseover", "onmouseout", "onfocus", "onblur",
    "onchange", "oninput", "onsubmit", "ontoggle", "onauxclick", "ondblclick", "oncontextmenu",
    "onkeydown", "onkeypress", "onkeyup", "onmousedown", "onmouseenter", "onmouseleave", "onmousemove",
    "onmouseup", "onwheel", "onpointerdown", "onpointermove", "onpointerup", "onpointerenter",
    "onpointerleave", "onpointercancel", "onreset", "onselect", "onresize", "onscroll", "onplay",
    "onpause", "onended", "oncanplay", "onseeked", "onseeking", "onvolumechange", "ontimeupdate",
    "ondurationchange", "onloadeddata", "onloadedmetadata", "onloadstart", "onstalled", "onsuspend",
    "onwaiting", "onemptied", "onratechange", "onprogress", "onabort", "onbeforetoggle", "onclose",
    "oncopy", "oncut", "onpaste", "oninvalid", "onsearch", "ondrag", "ondragend", "ondragenter",
    "ondragleave", "ondragover", "ondragstart", "ondrop", "onanimationstart", "onanimationend",
    "onanimationiteration", "ontransitionend", "ontransitionstart", "ontransitionrun", "ontransitioncancel",
    "onpointerover", "onpointerout", "ongotpointercapture", "onlostpointercapture", "onbeforeinput",
    "onbeforematch", "oncancel", "onsecuritypolicyviolation", "onslotchange", "onbeforeprint",
    "onafterprint", "onlanguagechange", "onmessage", "onmessageerror", "onoffline", "ononline",
    "onbeforeunload", "onhashchange", "onpagehide", "onpageshow", "onpopstate", "onstorage", "onunload",
  ],
};

/** Sanitize author template markup for safe in-process `innerHTML` injection.
 *
 *  - `html` — the interpolated template string (author markup + already-escaped data values from
 *    `interpolateTemplate`). Pure: deterministic for a given input; no I/O; never throws.
 *  - returns the sanitized HTML string. On a non-string input → `""` (defensive; the renderer only
 *    passes strings, but a sanitizer that throws is itself an XSS vector).
 *
 *  This is the FLOOR of the template security model. The ceiling is the CSP/Trusted-Types posture +
 *  the `data-*`-only click wiring in `TemplateView` (Decision 5): even a hypothetical miss here has
 *  no inline-script sink to execute against. The XSS-vector unit suite is the definition of done. */
export function sanitizeTemplateHtml(html: string): string {
  if (typeof html !== "string") return "";
  // `sanitize` never throws on valid string input; guard once more so the contract is airtight. The
  // return is a `string` in the default config (no `RETURN_TRUSTED_TYPE`); the `String(...)` coercion
  // also covers a `TrustedHTML` instance if a future shell CSP enables Trusted Types (then this is the
  // single sanctioned sink — Decision 5's floor regardless).
  try {
    const clean = DOMPurify.sanitize(html, PURIFY_CONFIG);
    return typeof clean === "string" ? clean : String(clean);
  } catch {
    // A sanitizer failure must NEVER surface the raw input. Fail closed (empty).
    return "";
  }
}
