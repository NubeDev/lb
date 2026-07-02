import { useState as v, useRef as x, useEffect as E } from "react";
import { jsx as u, jsxs as S } from "react/jsx-runtime";
function P(e) {
  return e.label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
function y(e) {
  return /\.(publish|write|enqueue|command|set|send|record|create|delete|resolve|derive|simulate)$/.test(
    e
  );
}
function q(e, s) {
  const i = s.startsWith(`${e}.`) ? s.slice(e.length + 1) : s;
  return `${e} · ${i}`;
}
function _(e) {
  return e.map((s) => ({
    id: `series:${s}`,
    group: "series",
    label: s,
    source: { tool: "series.read", args: { series: s } },
    writes: !1
  }));
}
function D(e) {
  return e.map((s) => ({
    id: `live:${s}`,
    group: "live",
    label: `${s} (live)`,
    source: { tool: "series.watch", args: { series: s } },
    writes: !1
  }));
}
function F(e) {
  var i, n, t;
  const s = [];
  for (const o of e) {
    if (!o.enabled) continue;
    const c = /* @__PURE__ */ new Set();
    (n = (i = o.ui) == null ? void 0 : i.scope) == null || n.forEach((r) => c.add(r)), (t = o.widgets) == null || t.forEach((r) => {
      var l;
      return (l = r.scope) == null ? void 0 : l.forEach((a) => c.add(a));
    });
    for (const r of c) {
      const l = y(r);
      s.push({
        id: `ext:${o.ext}:${r}`,
        group: l ? "action" : "extension",
        label: q(o.ext, r),
        source: l ? void 0 : { tool: r, args: {} },
        action: l ? { tool: r, argsTemplate: {} } : void 0,
        writes: l
      });
    }
  }
  return s;
}
function L(e) {
  const s = [];
  for (const i of e)
    if (i.enabled)
      for (const n of i.widgets ?? []) {
        const t = P(n);
        s.push({
          id: `widget:${i.ext}/${t}`,
          group: "widget",
          label: `${i.ext} · ${n.label}`,
          icon: n.icon,
          viewKey: `ext:${i.ext}/${t}`,
          writes: !1
        });
      }
  return s;
}
function T(e, s) {
  const i = new Map(s.map((t) => [t.type, t])), n = [];
  for (const t of e)
    for (const o of t.nodes ?? []) {
      const c = i.get(o.type);
      if (c) {
        for (const r of c.inputs ?? [])
          n.push({
            id: `flows:in:${t.id}:${o.id}:${r}`,
            group: "flows",
            label: `${t.name || t.id} › ${o.id} › ${r} (input)`,
            action: {
              tool: "flows.inject",
              argsTemplate: { id: t.id, node: o.id, port: r, value: "{{value}}" }
            },
            writes: !0
          });
        for (const r of c.outputs ?? [])
          n.push({
            id: `flows:out:${t.id}:${o.id}:${r}`,
            group: "flows",
            label: `${t.name || t.id} › ${o.id} › ${r} (output)`,
            source: {
              tool: "flows.node_state",
              args: { id: t.id, __flowNode: o.id, __flowPort: r }
            },
            writes: !1
          });
      }
    }
  return n;
}
const k = "sql:query";
function I() {
  return {
    id: k,
    group: "sql",
    label: "SQL query (direct SurrealDB)",
    source: { tool: "store.query", args: { sql: "" } },
    writes: !1
  };
}
function O(e) {
  return [
    ..._(e.series ?? []),
    ...D(e.series ?? []),
    ...F(e.extensions ?? []),
    ...L(e.extensions ?? []),
    ...T(e.flows ?? [], e.descriptors ?? []),
    I()
  ];
}
function R(e) {
  return { id: e.id, source: e.source, action: e.action, viewKey: e.viewKey };
}
function W(e, s) {
  const [i, n] = v({
    entries: [],
    installed: [],
    loading: !0
  }), t = x(e);
  return t.current = e, E(() => {
    const o = t.current;
    let c = !1;
    return n((r) => ({ ...r, loading: !0 })), (async () => {
      var w, g, $, m, b;
      const [r, l, a, f, K] = await Promise.all([
        ((w = o.listSeries) == null ? void 0 : w.call(o).catch(() => [])) ?? Promise.resolve([]),
        ((g = o.listExtensions) == null ? void 0 : g.call(o).catch(() => [])) ?? Promise.resolve([]),
        (($ = o.listFlows) == null ? void 0 : $.call(o).catch(() => [])) ?? Promise.resolve([]),
        ((m = o.listFlowNodes) == null ? void 0 : m.call(o).catch(() => [])) ?? Promise.resolve([]),
        ((b = o.listDatasources) == null ? void 0 : b.call(o).catch(() => [])) ?? Promise.resolve([])
      ]), p = o.getFlow, h = p ? (await Promise.all(
        a.map((d) => p(d.id).catch(() => null))
      )).filter((d) => d != null) : [];
      c || n({
        entries: O({ series: r, extensions: l, flows: h, descriptors: f }),
        installed: l,
        loading: !1
      });
    })(), () => {
      c = !0;
    };
  }, [s]), i;
}
const j = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "widget", label: "Extension widgets" },
  { group: "flows", label: "Flows" }
];
function B({
  entries: e,
  value: s = "",
  onSelect: i,
  loading: n = !1,
  groups: t = j,
  "aria-label": o = "source",
  className: c
}) {
  const r = (l) => {
    const a = e.find((f) => f.id === l) ?? null;
    i(a ? R(a) : null);
  };
  return /* @__PURE__ */ u("label", { className: `sp-root${c ? ` ${c}` : ""}`, children: /* @__PURE__ */ S(
    "select",
    {
      className: "sp-select",
      "aria-label": o,
      value: s,
      onChange: (l) => r(l.target.value),
      children: [
        /* @__PURE__ */ u("option", { value: "", children: n ? "loading sources…" : "— pick a source —" }),
        t.map(({ group: l, label: a }) => /* @__PURE__ */ u(C, { entries: e, group: l, label: a }, l))
      ]
    }
  ) });
}
function C({
  entries: e,
  group: s,
  label: i
}) {
  const n = e.filter((t) => t.group === s);
  return n.length === 0 ? null : /* @__PURE__ */ u("optgroup", { label: i, children: n.map((t) => /* @__PURE__ */ u("option", { value: t.id, children: t.label }, t.id)) });
}
export {
  k as SQL_SOURCE_ID,
  B as SourcePicker,
  O as buildSourceEntries,
  L as extWidgetEntries,
  F as extensionEntries,
  T as flowsEntries,
  D as liveEntries,
  R as selectionOf,
  _ as seriesEntries,
  I as sqlSourceEntry,
  W as useSourcePicker,
  P as widgetIdOf
};
