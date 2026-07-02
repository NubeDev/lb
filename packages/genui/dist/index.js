import { i as H, I as U } from "./validate-wbJctN0L.js";
import { e as Me, m as Ae, v as Se, w as Ie } from "./validate-wbJctN0L.js";
import { jsx as c, jsxs as b } from "react/jsx-runtime";
import { createElement as v, createContext as Y, useContext as G, Fragment as z } from "react";
function V(e, t) {
  if (t === "") return e;
  if (!t.startsWith("/")) return;
  const i = t.slice(1).split("/").map((n) => n.replace(/~1/g, "/").replace(/~0/g, "~"));
  let r = e;
  for (const n of i) {
    if (r == null) return;
    if (Array.isArray(r)) {
      const s = Number(n);
      if (!Number.isInteger(s) || s < 0 || s >= r.length) return;
      r = r[s];
    } else if (typeof r == "object")
      r = r[n];
    else
      return;
  }
  return r;
}
function j(e, t) {
  if (H(e)) return V(t, e.$bind);
  if (Array.isArray(e)) return e.map((i) => j(i, t));
  if (e !== null && typeof e == "object") {
    const i = {};
    for (const [r, n] of Object.entries(e)) i[r] = j(n, t);
    return i;
  }
  return e;
}
function X(e, t) {
  const i = {};
  for (const [r, n] of Object.entries(e ?? {})) i[r] = j(n, t);
  return i;
}
function fe(e = "cell") {
  return { v: U, surface: { surfaceId: e, root: "" }, components: {} };
}
function D(e, t, i) {
  if (t === "" || !t.startsWith("/")) return e;
  const r = t.slice(1).split("/").map((o) => o.replace(/~1/g, "/").replace(/~0/g, "~")), n = { ...e };
  let s = n;
  for (let o = 0; o < r.length - 1; o++) {
    const a = r[o], l = s[a];
    s[a] = l && typeof l == "object" && !Array.isArray(l) ? { ...l } : {}, s = s[a];
  }
  return s[r[r.length - 1]] = i, n;
}
function ge(e, t) {
  switch (t.type) {
    case "createSurface":
      return {
        v: e.v,
        surface: t.surface,
        components: { ...t.components },
        dataModel: t.dataModel
      };
    case "updateComponents": {
      const i = { ...e.components };
      for (const r of t.components) i[r.id] = r;
      return { ...e, components: i };
    }
    case "updateDataModel":
      return { ...e, dataModel: D(e.dataModel ?? {}, t.pointer, t.value) };
    case "deleteSurface":
      return { v: e.v, surface: { surfaceId: t.surfaceId, root: "" }, components: {} };
    default:
      return t;
  }
}
function J(e) {
  const t = /* @__PURE__ */ new Map();
  for (const r of e) {
    t.set(r.name, r);
    for (const n of r.deprecatedAliases ?? []) t.set(n, r);
  }
  return {
    entries: e,
    resolve: (r) => t.get(r),
    names: () => e.map((r) => r.name),
    has: (r) => t.has(r)
  };
}
function ye(e, t) {
  const i = [...e.entries].sort((r, n) => r.name.localeCompare(n.name)).map((r) => {
    var o, a;
    const n = {};
    for (const l of Object.keys(r.props).sort()) {
      const p = r.props[l];
      n[l] = { type: p.type }, p.description && (n[l].description = p.description), p.required && (n[l].required = !0), p.values && (n[l].values = p.values);
    }
    const s = { name: r.name, description: r.description, props: n };
    return (o = r.actions) != null && o.length && (s.actions = [...r.actions]), (a = r.deprecatedAliases) != null && a.length && (s.deprecatedAliases = [...r.deprecatedAliases]), s;
  });
  return { v: t, components: i };
}
function be(e) {
  return [...e.names()].sort();
}
function K(e, t) {
  const i = t.required ? "" : "?";
  let r = t.type;
  return t.type === "enum" && t.values && (r = t.values.map((n) => JSON.stringify(n)).join(" | ")), t.type === "binding" && (r = "binding"), `${e}${i}: ${r}`;
}
function Z(e) {
  var r;
  const t = Object.keys(e.props).map((n) => K(n, e.props[n])).join(", "), i = (r = e.actions) != null && r.length ? `  actions: ${e.actions.join(", ")}
` : "";
  return `- ${e.name}(${t})
    ${e.description}
${i}`;
}
function ve(e) {
  return ["Components you may use (and ONLY these):", "", ...[...e.entries].sort((i, r) => i.name.localeCompare(r.name)).map(Z)].join(`
`).trimEnd() + `
`;
}
const Q = /^(https?:|mailto:)/i;
function ee(e) {
  const t = e.trim();
  return Q.test(t) ? t : null;
}
const q = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*]+\*)|(\[[^\]]*\]\([^)]*\))/g;
function O(e, t) {
  const i = [];
  let r = 0, n, s = 0;
  for (q.lastIndex = 0; (n = q.exec(e)) !== null; ) {
    n.index > r && i.push(e.slice(r, n.index));
    const o = n[0], a = `${t}-${s++}`;
    if (o.startsWith("`"))
      i.push(v("code", { key: a }, o.slice(1, -1)));
    else if (o.startsWith("**"))
      i.push(v("strong", { key: a }, o.slice(2, -2)));
    else if (o.startsWith("*"))
      i.push(v("em", { key: a }, o.slice(1, -1)));
    else {
      const l = o.indexOf("]("), p = o.slice(1, l), u = o.slice(l + 2, -1), d = ee(u);
      d ? i.push(v("a", { key: a, href: d, rel: "noopener noreferrer", target: "_blank" }, p)) : i.push(p);
    }
    r = n.index + o.length;
  }
  return r < e.length && i.push(e.slice(r)), i;
}
function te(e, t) {
  const i = [];
  return e.forEach((r, n) => {
    n > 0 && i.push(v("br", { key: `${t}-br-${n}` })), i.push(...O(r, `${t}-l${n}`));
  }), i;
}
const E = /^(#{1,6})\s+(.*)$/, F = /^\s*[-*+]\s+(.*)$/, C = /^\s*\d+[.)]\s+(.*)$/;
function ne(e) {
  const i = (typeof e == "string" ? e : "").replace(/\r\n?/g, `
`).split(`
`), r = [];
  let n = 0, s = 0;
  for (; n < i.length; ) {
    const o = i[n];
    if (o.trim() === "") {
      n++;
      continue;
    }
    const a = E.exec(o);
    if (a) {
      const d = a[1].length;
      r.push(v(`h${d}`, { key: s++ }, O(a[2], `h${s}`))), n++;
      continue;
    }
    const l = F.test(o), p = !l && C.test(o);
    if (l || p) {
      const d = l ? F : C, h = [];
      let f = 0;
      for (; n < i.length; ) {
        const y = d.exec(i[n]);
        if (!y) break;
        h.push(v("li", { key: f }, O(y[1], `li${s}-${f}`))), f++, n++;
      }
      r.push(v(l ? "ul" : "ol", { key: s++ }, h));
      continue;
    }
    const u = [];
    for (; n < i.length && i[n].trim() !== "" && !E.test(i[n]) && !F.test(i[n]) && !C.test(i[n]); )
      u.push(i[n]), n++;
    r.push(v("p", { key: s++ }, te(u, `p${s}`)));
  }
  return r;
}
function $(e, t = NaN) {
  const i = typeof e == "number" ? e : Number(e);
  return Number.isFinite(i) ? i : t;
}
function R(e, t) {
  return $(t && e && typeof e == "object" ? e[t] : e);
}
function M(e = "no data") {
  return /* @__PURE__ */ c("div", { className: "gu-placeholder", children: e });
}
function re(e) {
  return e === "bars" || e === "points" ? e : "line";
}
function ie(e) {
  const t = Array.isArray(e.rows) ? e.rows : [];
  if (t.length === 0) return M();
  const i = typeof e.yField == "string" ? e.yField : void 0, r = typeof e.xField == "string" ? e.xField : void 0, n = re(e.drawStyle), s = t.map((m) => R(m, i)), o = r ? t.map((m) => R(m, r)) : t.map((m, w) => w), a = s.filter((m) => Number.isFinite(m));
  if (a.length === 0) return M();
  const l = 300, p = 100, u = 4, d = Math.min(...a), f = Math.max(...a) - d || 1, y = o.map((m) => Number.isFinite(m) ? m : 0), N = Math.min(...y), S = Math.max(...y) - N || 1, x = (m) => u + (m - N) / S * (l - 2 * u), P = (m) => p - u - (m - d) / f * (p - 2 * u), I = s.map((m, w) => ({ x: x(y[w]), y: Number.isFinite(m) ? P(m) : P(d) }));
  return /* @__PURE__ */ c("div", { className: "gu-chart", children: /* @__PURE__ */ b("svg", { viewBox: `0 0 ${l} ${p}`, role: "img", "aria-label": "timeseries", children: [
    n === "line" && /* @__PURE__ */ c(
      "polyline",
      {
        points: I.map((m) => `${m.x},${m.y}`).join(" "),
        fill: "none",
        stroke: "var(--gu-accent)",
        strokeWidth: 1.5
      }
    ),
    n === "points" && I.map((m, w) => /* @__PURE__ */ c("circle", { cx: m.x, cy: m.y, r: 2, fill: "var(--gu-accent)" }, w)),
    n === "bars" && I.map((m, w) => /* @__PURE__ */ c(
      "rect",
      {
        x: m.x - 1.5,
        y: m.y,
        width: 3,
        height: Math.max(0, p - u - m.y),
        fill: "var(--gu-accent)"
      },
      w
    ))
  ] }) });
}
function B(e) {
  return Array.isArray(e) ? e.map((t) => {
    if (!t || typeof t != "object") return null;
    const i = t;
    return { name: String(i.name ?? ""), value: $(i.value) };
  }).filter((t) => t !== null && Number.isFinite(t.value)) : [];
}
function oe(e) {
  const t = B(e.data);
  if (t.length === 0) return M();
  const i = e.horizontal === !0, r = Math.max(...t.map((l) => l.value), 0) || 1, n = 300, s = Math.max(60, t.length * (i ? 22 : 0) || 100), o = 4;
  if (i) {
    const l = (s - o) / t.length;
    return /* @__PURE__ */ c("div", { className: "gu-chart", children: /* @__PURE__ */ c("svg", { viewBox: `0 0 ${n} ${s}`, role: "img", "aria-label": "barchart", children: t.map((p, u) => {
      const d = p.value / r * (n - 2);
      return /* @__PURE__ */ c("rect", { x: 1, y: u * l + o / 2, width: Math.max(0, d), height: l - o, fill: "var(--gu-accent)" }, u);
    }) }) });
  }
  const a = (n - o) / t.length;
  return /* @__PURE__ */ c("div", { className: "gu-chart", children: /* @__PURE__ */ c("svg", { viewBox: `0 0 ${n} 100`, role: "img", "aria-label": "barchart", children: t.map((l, p) => {
    const u = l.value / r * 98;
    return /* @__PURE__ */ c("rect", { x: p * a + o / 2, y: 100 - u, width: a - o, height: Math.max(0, u), fill: "var(--gu-accent)" }, p);
  }) }) });
}
function se(e) {
  return e === "donut" ? "donut" : "pie";
}
const T = ["var(--gu-accent)", "var(--gu-warn)", "var(--gu-bad)", "var(--gu-muted)"];
function ae(e) {
  const t = B(e.data).filter((u) => u.value >= 0), i = t.reduce((u, d) => u + d.value, 0);
  if (t.length === 0 || i <= 0) return M();
  const r = se(e.pieType), n = 120, s = n / 2, o = n / 2, a = n / 2 - 2;
  let l = 0;
  const p = (u, d) => {
    const h = d * 2 * Math.PI - Math.PI / 2, f = (d + u) * 2 * Math.PI - Math.PI / 2, y = u > 0.5 ? 1 : 0, N = s + a * Math.cos(h), A = o + a * Math.sin(h), S = s + a * Math.cos(f), x = o + a * Math.sin(f);
    return `M ${s} ${o} L ${N} ${A} A ${a} ${a} 0 ${y} 1 ${S} ${x} Z`;
  };
  return /* @__PURE__ */ c("div", { className: "gu-chart", children: /* @__PURE__ */ b("svg", { viewBox: `0 0 ${n} ${n}`, role: "img", "aria-label": "piechart", children: [
    t.map((u, d) => {
      const h = u.value / i, f = p(h, l);
      return l += h, /* @__PURE__ */ c("path", { d: f, fill: T[d % T.length] }, d);
    }),
    r === "donut" && /* @__PURE__ */ c("circle", { cx: s, cy: o, r: a * 0.55, fill: "var(--gu-bg)" })
  ] }) });
}
function ce(e) {
  const t = $(e.value);
  if (!Number.isFinite(t)) return M();
  const i = $(e.min, 0), n = $(e.max, 100) - i || 1, s = Math.max(0, Math.min(1, (t - i) / n)), o = Array.isArray(e.thresholds) ? e.thresholds.map((x) => $(x)).filter((x) => Number.isFinite(x)) : [];
  let a = "var(--gu-ok)";
  o.length >= 1 && t >= o[o.length - 1] ? a = "var(--gu-bad)" : o.length >= 2 && t >= o[0] && (a = "var(--gu-warn)");
  const l = 120, p = l / 2, u = l / 2, d = l / 2 - 8, h = Math.PI * (1 - s), f = p - d, y = u, N = p + d * Math.cos(h), A = u - d * Math.sin(h);
  return /* @__PURE__ */ c("div", { className: "gu-chart", children: /* @__PURE__ */ b("svg", { viewBox: `0 0 ${l} ${u + 6}`, role: "img", "aria-label": "gauge", children: [
    /* @__PURE__ */ c("path", { d: `M ${f} ${y} A ${d} ${d} 0 0 1 ${p + d} ${u}`, fill: "none", stroke: "var(--gu-border)", strokeWidth: 8 }),
    /* @__PURE__ */ c("path", { d: `M ${f} ${y} A ${d} ${d} 0 0 1 ${N} ${A}`, fill: "none", stroke: a, strokeWidth: 8, strokeLinecap: "round" })
  ] }) });
}
function g(e, t = "") {
  return typeof e == "string" ? e : e == null ? t : String(e);
}
function k(e, t = NaN) {
  const i = typeof e == "number" ? e : Number(e);
  return Number.isFinite(i) ? i : t;
}
function W(e) {
  return e === "ok" || e === "warn" || e === "bad" ? ` gu-tone-${e}` : "";
}
const xe = J([
  // ---- normalize target ----
  {
    // The component `normalize` rewrites an unknown name to (labelled placeholder + warning). Kept in
    // the catalog so a normalized spec renders the intended inert placeholder rather than the surface's
    // last-resort `gu-unknown` fallback. Not something the agent should emit directly.
    name: "placeholder",
    description: "Inert labelled placeholder (normalize target for an unknown component).",
    props: { label: { type: "string", description: "What was replaced." } },
    render: (e) => /* @__PURE__ */ c("div", { className: "gu-placeholder", role: "status", children: g(e.props.label, "placeholder") })
  },
  // ---- layout ----
  {
    name: "stack",
    description: 'Vertical (default) or horizontal flex stack of child components. `children` is an array of component refs, e.g. Stack("vertical", [a, b]).',
    props: {
      direction: { type: "enum", values: ["vertical", "horizontal"], description: "Stack axis.", default: "vertical" },
      children: { type: "array", description: "Array of child component refs." }
    },
    render: (e) => {
      const t = e.props.direction === "horizontal";
      return /* @__PURE__ */ c("div", { className: t ? "gu-stack gu-horizontal" : "gu-stack", children: e.children });
    }
  },
  {
    name: "grid",
    description: "Responsive grid; `columns` (1..6) per row. `children` is an array of component refs, e.g. Grid(2, [a, b, c]).",
    props: {
      columns: { type: "number", description: "Columns per row, clamped 1..6.", default: 2 },
      children: { type: "array", description: "Array of child component refs." }
    },
    render: (e) => {
      const t = Math.max(1, Math.min(6, Math.round(k(e.props.columns, 2))));
      return /* @__PURE__ */ c("div", { className: "gu-grid", style: { "--gu-cols": String(t) }, children: e.children });
    }
  },
  {
    name: "card",
    description: 'A bordered card with an optional title, wrapping child components. `children` is an array of component refs, e.g. Card("Room", [a, b]).',
    props: {
      title: { type: "string", description: "Optional card heading." },
      children: { type: "array", description: "Array of child component refs." }
    },
    render: (e) => {
      const t = g(e.props.title);
      return /* @__PURE__ */ b("div", { className: "gu-card", children: [
        t && /* @__PURE__ */ c("div", { className: "gu-card-title", children: t }),
        e.children
      ] });
    }
  },
  // ---- text ----
  {
    name: "text",
    description: "A single paragraph of plain text; `muted` dims it.",
    props: {
      value: { type: "string", required: !0, description: "The text to display." },
      muted: { type: "boolean", description: "Render dimmed/secondary." }
    },
    render: (e) => {
      const t = e.props.muted === !0;
      return /* @__PURE__ */ c("p", { className: "gu-text", style: t ? { color: "var(--gu-muted)" } : void 0, children: g(e.props.value) });
    }
  },
  {
    name: "markdown",
    description: "Safe minimal markdown (headings, bold, italic, code, links, lists) — no raw HTML.",
    props: {
      value: { type: "string", required: !0, description: "Markdown source text." }
    },
    render: (e) => /* @__PURE__ */ c("div", { className: "gu-markdown", children: ne(g(e.props.value)) })
  },
  // ---- stat / gauge ----
  {
    name: "stat",
    description: "A big single-value KPI tile with optional label, unit and tone.",
    props: {
      value: { type: "binding", required: !0, description: "The KPI value (number or string)." },
      label: { type: "string", description: "Caption above the value." },
      unit: { type: "string", description: "Unit suffix appended to the value." },
      tone: { type: "enum", values: ["ok", "warn", "bad"], description: "Colour tone of the value." }
    },
    render: (e) => {
      const t = g(e.props.label), i = g(e.props.unit), r = e.props.value, n = typeof r == "number" ? String(r) : g(r, "—");
      return /* @__PURE__ */ b("div", { className: `gu-stat${W(e.props.tone)}`, children: [
        t && /* @__PURE__ */ c("div", { className: "gu-stat-label", children: t }),
        /* @__PURE__ */ b("div", { className: "gu-stat-value", children: [
          n,
          i ? ` ${i}` : ""
        ] })
      ] });
    }
  },
  {
    name: "gauge",
    description: "A radial arc gauge showing `value` between `min`/`max` with optional thresholds.",
    props: {
      value: { type: "binding", required: !0, description: "Current value." },
      min: { type: "number", description: "Range minimum.", default: 0 },
      max: { type: "number", description: "Range maximum.", default: 100 },
      thresholds: { type: "array", description: "Ascending numeric warn/bad thresholds." }
    },
    render: (e) => ce(e.props)
  },
  // ---- table ----
  {
    name: "table",
    description: "A data table over `rows`; `columns` selects/orders fields (else inferred).",
    props: {
      rows: { type: "binding", required: !0, description: "Array of row objects." },
      columns: { type: "array", description: "Column keys to show, in order." }
    },
    render: (e) => le(e.props)
  },
  // ---- charts ----
  {
    name: "timeseries",
    description: "A line/bars/points chart over `rows`; `yField` (+`xField`) select the numeric series.",
    props: {
      rows: { type: "binding", required: !0, description: "Array of data rows." },
      xField: { type: "string", description: "Row field for the x axis (else index)." },
      yField: { type: "string", description: "Row field for the y value." },
      color: { type: "string", description: "Reserved; series colour follows the theme." },
      drawStyle: { type: "enum", values: ["line", "bars", "points"], description: "How to draw the series." }
    },
    render: (e) => ie(e.props)
  },
  {
    name: "barchart",
    description: "A bar chart over `data` ({name,value}[]); `horizontal` flips the orientation.",
    props: {
      data: { type: "binding", required: !0, description: "Array of {name, value} items." },
      horizontal: { type: "boolean", description: "Draw horizontal bars." }
    },
    render: (e) => oe(e.props)
  },
  {
    name: "piechart",
    description: "A pie/donut chart over `data` ({name,value}[]).",
    props: {
      data: { type: "binding", required: !0, description: "Array of {name, value} items." },
      pieType: { type: "enum", values: ["pie", "donut"], description: "Solid pie or donut." }
    },
    render: (e) => ae(e.props)
  },
  // ---- tag / badge ----
  {
    name: "tag",
    description: "A small pill label with an optional tone. (`badge` is a deprecated alias.)",
    deprecatedAliases: ["badge"],
    props: {
      text: { type: "string", required: !0, description: "The label text." },
      tone: { type: "enum", values: ["ok", "warn", "bad"], description: "Colour tone." }
    },
    render: (e) => /* @__PURE__ */ c("span", { className: `gu-tag${W(e.props.tone)}`, children: g(e.props.text) })
  },
  // ---- controls (emit ONLY via rp.emit) ----
  {
    name: "button",
    description: "A button; emits `press` with its value when clicked.",
    actions: ["press"],
    props: {
      label: { type: "string", required: !0, description: "Button caption." },
      value: { type: "binding", description: "Value carried in the emitted action." }
    },
    render: (e) => /* @__PURE__ */ c("button", { type: "button", className: "gu-btn", onClick: () => e.emit("press", { value: e.props.value ?? null }), children: g(e.props.label, "Button") })
  },
  {
    name: "slider",
    description: "A range slider; emits `change` with the committed value on release.",
    actions: ["change"],
    props: {
      value: { type: "binding", description: "Initial/display value.", default: 0 },
      min: { type: "number", description: "Minimum.", default: 0 },
      max: { type: "number", description: "Maximum.", default: 100 },
      step: { type: "number", description: "Step size.", default: 1 },
      label: { type: "string", description: "Optional caption." }
    },
    render: (e) => ue(e)
  },
  {
    name: "switch",
    description: "An on/off toggle; emits `toggle` with the next boolean state.",
    actions: ["toggle"],
    props: {
      value: { type: "binding", description: "Current on/off state.", default: !1 },
      label: { type: "string", description: "Optional caption." }
    },
    render: (e) => {
      const t = e.props.value === !0, i = g(e.props.label);
      return /* @__PURE__ */ b("label", { className: "gu-switch", children: [
        /* @__PURE__ */ c("input", { type: "checkbox", checked: t, onChange: () => e.emit("toggle", { value: !t }) }),
        i && /* @__PURE__ */ c("span", { children: i })
      ] });
    }
  }
]);
function le(e) {
  const t = Array.isArray(e.rows) ? e.rows : [];
  if (t.length === 0) return /* @__PURE__ */ c("div", { className: "gu-placeholder", children: "no data" });
  const i = Array.isArray(e.columns) ? e.columns.map((n) => g(n)).filter(Boolean) : [], r = i.length > 0 ? i : Array.from(
    t.reduce((n, s) => {
      if (s && typeof s == "object") for (const o of Object.keys(s)) n.add(o);
      return n;
    }, /* @__PURE__ */ new Set())
  );
  return r.length === 0 ? /* @__PURE__ */ c("div", { className: "gu-placeholder", children: "no data" }) : /* @__PURE__ */ b("table", { className: "gu-table", children: [
    /* @__PURE__ */ c("thead", { children: /* @__PURE__ */ c("tr", { children: r.map((n) => /* @__PURE__ */ c("th", { children: n }, n)) }) }),
    /* @__PURE__ */ c("tbody", { children: t.map((n, s) => /* @__PURE__ */ c("tr", { children: r.map((o) => {
      const a = n && typeof n == "object" ? n[o] : void 0;
      return /* @__PURE__ */ c("td", { children: a == null ? "" : typeof a == "object" ? JSON.stringify(a) : String(a) }, o);
    }) }, s)) })
  ] });
}
function ue(e) {
  const t = k(e.props.min, 0), i = k(e.props.max, 100), r = k(e.props.step, 1), n = k(e.props.value, t), s = g(e.props.label), o = (a) => {
    const l = k(a.currentTarget.value, n);
    e.emit("change", { value: l });
  };
  return /* @__PURE__ */ b("label", { className: "gu-switch", children: [
    s && /* @__PURE__ */ c("span", { children: s }),
    /* @__PURE__ */ c(
      "input",
      {
        type: "range",
        className: "gu-range",
        min: Number.isFinite(t) ? t : 0,
        max: Number.isFinite(i) ? i : 100,
        step: Number.isFinite(r) && r > 0 ? r : 1,
        defaultValue: Number.isFinite(n) ? n : t,
        onMouseUp: o,
        onKeyUp: o
      }
    )
  ] });
}
const L = Y({}), de = L.Provider;
function we() {
  return G(L).bridge;
}
function _(e, t, i, r, n, s) {
  if (s.has(e)) return null;
  const o = t.components[e];
  if (!o)
    return /* @__PURE__ */ b("div", { className: "gu-missing", role: "status", children: [
      "missing component: ",
      e
    ] }, e);
  const a = r.resolve(o.component), l = new Set(s).add(e), p = (o.children ?? []).map((d) => /* @__PURE__ */ c(z, { children: _(d, t, i, r, n, l) }, d));
  if (!a)
    return /* @__PURE__ */ b("div", { className: "gu-unknown", role: "status", "data-component": o.component, children: [
      "unknown component: ",
      o.component
    ] }, e);
  const u = X(o.props, i);
  return /* @__PURE__ */ c(z, { children: a.render({ props: u, children: p, emit: (d, h) => n(e, d, h) }) }, e);
}
function $e({ spec: e, data: t, catalog: i, bridge: r, onAction: n }) {
  const s = t ?? e.dataModel ?? {}, o = (l, p, u) => {
    n == null || n({ surfaceId: e.surface.surfaceId, componentId: l, name: p, tool: p, context: u });
  }, a = e.surface.root;
  return /* @__PURE__ */ c(de, { value: { bridge: r }, children: /* @__PURE__ */ c("div", { className: "gu-root gu-surface", children: a ? _(a, e, s, i, o, /* @__PURE__ */ new Set()) : /* @__PURE__ */ c("div", { className: "gu-empty", role: "status", children: "empty widget" }) }) });
}
export {
  de as GenUiProvider,
  $e as GenUiSurface,
  U as IR_VERSION,
  ge as applyPatch,
  be as catalogNames,
  ve as catalogPrompt,
  J as defineCatalog,
  fe as emptySpec,
  Me as errors,
  H as isBinding,
  Ae as migrate,
  xe as nubeCatalog,
  X as resolveBindings,
  V as resolvePointer,
  j as resolveValue,
  ye as toCatalogJson,
  we as useGenUiBridge,
  Se as validate,
  Ie as warnings
};
