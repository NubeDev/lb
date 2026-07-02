import { i as k, I as h, m as j, v as S, e as A, w as E } from "./validate-wbJctN0L.js";
import { createParser as O, createStreamingParser as L } from "@openuidev/lang-core";
const N = "placeholder";
function T(e, n) {
  var t;
  if (k(e)) return e;
  switch (n.type) {
    case "number": {
      const r = typeof e == "number" ? e : Number(e);
      return Number.isFinite(r) ? r : n.default;
    }
    case "string":
      return typeof e == "string" ? e : e == null ? n.default : String(e);
    case "boolean":
      return typeof e == "boolean" ? e : !!e;
    case "enum":
      return typeof e == "string" && ((t = n.values) != null && t.includes(e)) ? e : n.default;
    case "array":
      return Array.isArray(e) ? e : n.default;
    case "object":
      return e && typeof e == "object" && !Array.isArray(e) ? e : n.default;
    case "binding":
      return e;
    default:
      return e;
  }
}
function R(e, n) {
  const t = [], r = new Set(Object.keys(e.components)), o = {};
  for (const [s, c] of Object.entries(e.components)) {
    const p = n.resolve(c.component);
    let f = c.component, i = { ...c.props ?? {} };
    if (!p)
      t.push({
        level: "warning",
        code: "unknown-component",
        message: `unknown component "${c.component}" → placeholder`,
        componentId: s
      }), f = N, i = { label: `unknown: ${c.component}` };
    else
      for (const [d, u] of Object.entries(p.props))
        if (d in i) {
          const l = T(i[d], u);
          l === void 0 ? (t.push({
            level: "warning",
            code: "bad-prop",
            message: `prop "${d}" of "${c.component}" had wrong type; dropped`,
            componentId: s
          }), delete i[d]) : l !== i[d] && (t.push({
            level: "warning",
            code: "coerced-prop",
            message: `prop "${d}" of "${c.component}" coerced to ${u.type}`,
            componentId: s
          }), i[d] = l);
        } else u.required && u.default !== void 0 && (i[d] = u.default, t.push({
          level: "warning",
          code: "defaulted-prop",
          message: `required prop "${d}" of "${c.component}" missing → default`,
          componentId: s
        }));
    let a = c.children;
    if (a) {
      const d = a.filter((u) => r.has(u));
      if (d.length !== a.length)
        for (const u of a.filter((l) => !r.has(l)))
          t.push({
            level: "warning",
            code: "dangling-child",
            message: `dropped dangling child "${u}" of "${s}"`,
            componentId: s
          });
      a = d;
    }
    o[s] = { id: s, component: f, props: i, ...a ? { children: a } : {} };
  }
  return { spec: { ...e, components: o }, findings: t };
}
function b() {
  return "root";
}
const g = {
  barchart: "BarChart",
  piechart: "PieChart",
  timeseries: "TimeSeries"
};
function B(e) {
  return g[e] ? g[e] : e.length === 0 ? e : e[0].toUpperCase() + e.slice(1);
}
function C(e) {
  for (const [n, t] of Object.entries(g))
    if (t === e) return n;
  return e.length === 0 ? e : e[0].toLowerCase() + e.slice(1);
}
function x(e) {
  const n = {};
  switch (e.type) {
    case "enum":
      n.type = "string", e.values && (n.enum = e.values);
      break;
    case "binding":
      break;
    case "string":
    case "number":
    case "boolean":
    case "array":
    case "object":
      n.type = e.type;
      break;
  }
  return e.description && (n.description = e.description), e.default !== void 0 && (n.default = e.default), n;
}
function P(e) {
  const n = {}, t = [];
  for (const [r, o] of Object.entries(e.props))
    n[r] = x(o), o.required && t.push(r);
  return { properties: n, required: t };
}
function w(e) {
  const n = {};
  for (const t of e.entries)
    n[B(t.name)] = P(t);
  return { $defs: n };
}
function y(e) {
  return typeof e == "object" && e !== null && e.type === "element";
}
function _(e, n, t) {
  return y(e) ? { childIds: [m(e, n, t)] } : Array.isArray(e) && e.length > 0 && e.every(y) ? { childIds: e.map((r) => m(r, n, t)) } : { value: e };
}
function m(e, n, t) {
  const r = t(e), o = {}, s = [];
  for (const [c, p] of Object.entries(e.props ?? {})) {
    const { value: f, childIds: i } = _(p, n, t);
    i ? s.push(...i) : f !== void 0 && (o[c] = f);
  }
  return n[r] = {
    id: r,
    component: C(e.typeName),
    ...Object.keys(o).length ? { props: o } : {},
    ...s.length ? { children: s } : {}
  }, r;
}
function I(e, n = "cell") {
  const t = {};
  if (!e)
    return { v: h, surface: { surfaceId: n, root: "" }, components: t };
  let r = 0;
  const o = /* @__PURE__ */ new Map(), s = /* @__PURE__ */ new Set(), p = m(e, t, (f) => {
    const i = o.get(f);
    if (i) return i;
    let a = f.statementId && !s.has(f.statementId) ? f.statementId : `c${r++}`;
    for (; s.has(a); ) a = `c${r++}`;
    return s.add(a), o.set(f, a), a;
  });
  return { v: h, surface: { surfaceId: n, root: p }, components: t };
}
function q(e) {
  const n = [];
  e.incomplete && n.push({ level: "warning", code: "incomplete-emission", message: "emission looks truncated" });
  for (const t of e.unresolved ?? [])
    n.push({ level: "warning", code: "unresolved-ref", message: `reference "${t}" was never defined (dropped)` });
  for (const t of e.orphaned ?? [])
    n.push({ level: "warning", code: "orphaned-statement", message: `statement "${t}" is unreachable from the root` });
  return n;
}
function z(e, n, t = "cell") {
  const o = O(w(n), b()).parse(e);
  return { ir: I(o.root, t), findings: q(o.meta) };
}
function U(e, n = "cell") {
  const t = L(w(e), b()), r = () => I(t.getResult().root, n);
  return {
    push: (o) => (t.push(o), r()),
    set: (o) => (t.set(o), r()),
    current: r
  };
}
const D = 8 * 1024;
function F(e) {
  return new TextEncoder().encode(JSON.stringify(e)).length;
}
function V(e, n) {
  const t = n.surfaceId ?? "cell", r = z(e, n.catalog, t);
  return $(r.ir, r.findings, n);
}
function H(e, n) {
  return $(j(e), [], n);
}
function $(e, n, t) {
  const r = t.maxBytes ?? D, { spec: o, findings: s } = R(e, t.catalog), c = S(o, { catalog: t.catalog }), p = [...n, ...s, ...c], f = A(c);
  if (f.length)
    return {
      ok: !1,
      findings: p,
      error: `widget spec is invalid: ${f.map((a) => a.message).join("; ")}`
    };
  const i = F(o);
  return i > r ? {
    ok: !1,
    findings: p,
    error: `widget spec is too large (${i} bytes > ${r}). Simplify the widget — one widget, one job.`
  } : { ok: !0, ir: o, findings: [...n, ...s, ...E(c)] };
}
export {
  D as GENUI_MAX_BYTES,
  N as PLACEHOLDER,
  H as acceptIr,
  V as acceptLang,
  w as buildLangLibrary,
  B as catalogToLangName,
  U as createLangStream,
  I as elementToIr,
  C as langNameToCatalog,
  b as langRootName,
  R as normalize,
  z as parseLang,
  F as specByteSize
};
