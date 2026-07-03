import { useState as w, useRef as b, useEffect as $ } from "react";
import { jsx as a, jsxs as m } from "react/jsx-runtime";
function h(e) {
  return e.label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
function v(e) {
  return /\.(publish|write|enqueue|command|set|send|record|create|delete|resolve|derive|simulate)$/.test(
    e
  );
}
function x(e, o) {
  const s = o.startsWith(`${e}.`) ? o.slice(e.length + 1) : o;
  return `${e} · ${s}`;
}
function S(e) {
  return e.map((o) => ({
    id: `series:${o}`,
    group: "series",
    label: o,
    source: { tool: "series.read", args: { series: o } },
    writes: !1
  }));
}
function E(e) {
  return e.map((o) => ({
    id: `live:${o}`,
    group: "live",
    label: `${o} (live)`,
    source: { tool: "series.watch", args: { series: o } },
    writes: !1
  }));
}
function P(e) {
  var s, r, t;
  const o = [];
  for (const l of e) {
    if (!l.enabled) continue;
    const c = /* @__PURE__ */ new Set();
    (r = (s = l.ui) == null ? void 0 : s.scope) == null || r.forEach((i) => c.add(i)), (t = l.widgets) == null || t.forEach((i) => {
      var n;
      return (n = i.scope) == null ? void 0 : n.forEach((u) => c.add(u));
    });
    for (const i of c) {
      const n = v(i);
      o.push({
        id: `ext:${l.ext}:${i}`,
        group: n ? "action" : "extension",
        label: x(l.ext, i),
        source: n ? void 0 : { tool: i, args: {} },
        action: n ? { tool: i, argsTemplate: {} } : void 0,
        writes: n
      });
    }
  }
  return o;
}
function y(e) {
  const o = [];
  for (const s of e)
    if (s.enabled)
      for (const r of s.widgets ?? []) {
        const t = h(r);
        o.push({
          id: `widget:${s.ext}/${t}`,
          group: "widget",
          label: `${s.ext} · ${r.label}`,
          icon: r.icon,
          viewKey: `ext:${s.ext}/${t}`,
          writes: !1
        });
      }
  return o;
}
function _(e, o) {
  const s = new Map(o.map((t) => [t.type, t])), r = [];
  for (const t of e)
    for (const l of t.nodes ?? []) {
      const c = s.get(l.type);
      if (c) {
        for (const i of c.inputs ?? [])
          r.push({
            id: `flows:in:${t.id}:${l.id}:${i}`,
            group: "flows",
            label: `${t.name || t.id} › ${l.id} › ${i} (input)`,
            action: {
              tool: "flows.inject",
              argsTemplate: { id: t.id, node: l.id, port: i, value: "{{value}}" }
            },
            writes: !0
          });
        for (const i of c.outputs ?? [])
          r.push({
            id: `flows:out:${t.id}:${l.id}:${i}`,
            group: "flows",
            label: `${t.name || t.id} › ${l.id} › ${i} (output)`,
            source: {
              tool: "flows.node_state",
              args: { id: t.id, __flowNode: l.id, __flowPort: i }
            },
            writes: !1
          });
      }
    }
  return r;
}
const q = "sql:query";
function D() {
  return {
    id: q,
    group: "sql",
    label: "SQL query (direct SurrealDB)",
    source: { tool: "store.query", args: { sql: "" } },
    writes: !1
  };
}
function R(e) {
  return [
    ...S(e.series ?? []),
    ...E(e.series ?? []),
    ...P(e.extensions ?? []),
    ...y(e.extensions ?? []),
    ..._(e.flows ?? [], e.descriptors ?? []),
    D()
  ];
}
function L(e) {
  return { id: e.id, source: e.source, action: e.action, viewKey: e.viewKey };
}
async function O(e) {
  var n, u, f, p, g;
  const [o, s, r, t, l] = await Promise.all([
    ((n = e.listSeries) == null ? void 0 : n.call(e).catch(() => [])) ?? Promise.resolve([]),
    ((u = e.listExtensions) == null ? void 0 : u.call(e).catch(() => [])) ?? Promise.resolve([]),
    ((f = e.listFlows) == null ? void 0 : f.call(e).catch(() => [])) ?? Promise.resolve([]),
    ((p = e.listFlowNodes) == null ? void 0 : p.call(e).catch(() => [])) ?? Promise.resolve([]),
    ((g = e.listDatasources) == null ? void 0 : g.call(e).catch(() => [])) ?? Promise.resolve([])
  ]), c = e.getFlow, i = c ? (await Promise.all(r.map((d) => c(d.id).catch(() => null)))).filter((d) => d != null) : [];
  return {
    entries: R({ series: o, extensions: s, flows: i, descriptors: t }),
    installed: s
  };
}
function F(e, o) {
  const [s, r] = w({
    entries: [],
    installed: [],
    loading: !0
  }), t = b(e);
  return t.current = e, $(() => {
    const l = t.current;
    let c = !1;
    return r((i) => ({ ...i, loading: !0 })), (async () => {
      const { entries: i, installed: n } = await O(l);
      c || r({ entries: i, installed: n, loading: !1 });
    })(), () => {
      c = !0;
    };
  }, [o]), s;
}
const I = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "widget", label: "Extension widgets" },
  { group: "flows", label: "Flows" }
], B = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "action", label: "Action (control)" },
  { group: "widget", label: "Extension widgets" }
];
function T({
  entries: e,
  value: o = "",
  onSelect: s,
  loading: r = !1,
  groups: t = I,
  "aria-label": l = "source",
  className: c
}) {
  const i = (n) => {
    const u = e.find((f) => f.id === n) ?? null;
    s(u ? L(u) : null);
  };
  return /* @__PURE__ */ a("label", { className: `sp-root${c ? ` ${c}` : ""}`, children: /* @__PURE__ */ m(
    "select",
    {
      className: "sp-select",
      "aria-label": l,
      value: o,
      onChange: (n) => i(n.target.value),
      children: [
        /* @__PURE__ */ a("option", { value: "", children: r ? "loading sources…" : "— pick a source —" }),
        t.map(({ group: n, label: u }) => /* @__PURE__ */ a(U, { entries: e, group: n, label: u }, n))
      ]
    }
  ) });
}
function U({
  entries: e,
  group: o,
  label: s
}) {
  const r = e.filter((t) => t.group === o);
  return r.length === 0 ? null : /* @__PURE__ */ a("optgroup", { label: s, children: r.map((t) => /* @__PURE__ */ a("option", { value: t.id, children: t.label }, t.id)) });
}
export {
  B as BUILDER_SOURCE_GROUPS,
  U as PickerGroup,
  I as READ_SOURCE_GROUPS,
  q as SQL_SOURCE_ID,
  T as SourcePicker,
  R as buildSourceEntries,
  y as extWidgetEntries,
  P as extensionEntries,
  _ as flowsEntries,
  E as liveEntries,
  O as loadSourcePicker,
  L as selectionOf,
  S as seriesEntries,
  D as sqlSourceEntry,
  F as useSourcePicker,
  h as widgetIdOf
};
