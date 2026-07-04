import { useState as E, useRef as q, useEffect as k, useMemo as N } from "react";
import { jsx as d, jsxs as y } from "react/jsx-runtime";
function U(e) {
  return e.label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
function F(e) {
  return /\.(publish|write|enqueue|command|set|send|record|create|delete|resolve|derive|simulate)$/.test(
    e
  );
}
function I(e, t) {
  const l = t.startsWith(`${e}.`) ? t.slice(e.length + 1) : t;
  return `${e} · ${l}`;
}
function M(e) {
  return e.map((t) => ({
    id: `series:${t}`,
    group: "series",
    label: t,
    source: { tool: "series.read", args: { series: t } },
    writes: !1
  }));
}
function A(e) {
  return e.map((t) => ({
    id: `live:${t}`,
    group: "live",
    label: `${t} (live)`,
    source: { tool: "series.watch", args: { series: t } },
    writes: !1
  }));
}
function B(e) {
  var l, i, o;
  const t = [];
  for (const n of e) {
    if (!n.enabled) continue;
    const a = /* @__PURE__ */ new Set();
    (i = (l = n.ui) == null ? void 0 : l.scope) == null || i.forEach((s) => a.add(s)), (o = n.widgets) == null || o.forEach((s) => {
      var c;
      return (c = s.scope) == null ? void 0 : c.forEach((u) => a.add(u));
    });
    for (const s of a) {
      const c = F(s);
      t.push({
        id: `ext:${n.ext}:${s}`,
        group: c ? "action" : "extension",
        label: I(n.ext, s),
        source: c ? void 0 : { tool: s, args: {} },
        action: c ? { tool: s, argsTemplate: {} } : void 0,
        writes: c
      });
    }
  }
  return t;
}
function G(e) {
  const t = [];
  for (const l of e)
    if (l.enabled)
      for (const i of l.widgets ?? []) {
        const o = U(i);
        t.push({
          id: `widget:${l.ext}/${o}`,
          group: "widget",
          label: `${l.ext} · ${i.label}`,
          icon: i.icon,
          viewKey: `ext:${l.ext}/${o}`,
          data: i.data === !0,
          writes: !1
        });
      }
  return t;
}
function T(e, t) {
  const l = new Map(t.map((o) => [o.type, o])), i = [];
  for (const o of e)
    for (const n of o.nodes ?? []) {
      const a = l.get(n.type);
      if (a) {
        for (const s of a.inputs ?? [])
          i.push({
            id: `flows:in:${o.id}:${n.id}:${s}`,
            group: "flows",
            label: `${o.name || o.id} › ${n.id} › ${s} (input)`,
            action: {
              tool: "flows.inject",
              argsTemplate: { id: o.id, node: n.id, port: s, value: "{{value}}" }
            },
            writes: !0
          });
        for (const s of a.outputs ?? [])
          i.push({
            id: `flows:out:${o.id}:${n.id}:${s}`,
            group: "flows",
            label: `${o.name || o.id} › ${n.id} › ${s} (output)`,
            source: {
              tool: "flows.node_state",
              args: { id: o.id, __flowNode: n.id, __flowPort: s }
            },
            writes: !1
          });
      }
    }
  return i;
}
function K(e) {
  return e.map((t) => ({
    id: `rule:${t.id}`,
    group: "rules",
    label: t.name || t.id,
    source: { tool: "rules.run", args: { rule_id: t.id } },
    writes: !1,
    params: t.params ?? []
  }));
}
const j = "sql:query";
function Q() {
  return {
    id: j,
    group: "sql",
    label: "SQL query (direct SurrealDB)",
    source: { tool: "store.query", args: { sql: "" } },
    writes: !1
  };
}
function W(e) {
  return [
    ...M(e.series ?? []),
    ...A(e.series ?? []),
    ...B(e.extensions ?? []),
    ...G(e.extensions ?? []),
    ...T(e.flows ?? [], e.descriptors ?? []),
    ...K(e.rules ?? []),
    Q()
  ];
}
function R(e) {
  return { id: e.id, source: e.source, action: e.action, viewKey: e.viewKey };
}
async function Z(e) {
  var u, g, h, b, p, w;
  const [t, l, i, o, n, a] = await Promise.all([
    ((u = e.listSeries) == null ? void 0 : u.call(e).catch(() => [])) ?? Promise.resolve([]),
    ((g = e.listExtensions) == null ? void 0 : g.call(e).catch(() => [])) ?? Promise.resolve([]),
    ((h = e.listFlows) == null ? void 0 : h.call(e).catch(() => [])) ?? Promise.resolve([]),
    ((b = e.listFlowNodes) == null ? void 0 : b.call(e).catch(() => [])) ?? Promise.resolve([]),
    ((p = e.listDatasources) == null ? void 0 : p.call(e).catch(() => [])) ?? Promise.resolve([]),
    ((w = e.listRules) == null ? void 0 : w.call(e).catch(() => [])) ?? Promise.resolve([])
  ]), s = e.getFlow, c = s ? (await Promise.all(i.map((m) => s(m.id).catch(() => null)))).filter((m) => m != null) : [];
  return {
    entries: W({
      series: t,
      extensions: l,
      flows: c,
      descriptors: o,
      rules: a
    }),
    installed: l
  };
}
function X(e, t) {
  const [l, i] = E({
    entries: [],
    installed: [],
    loading: !0
  }), o = q(e);
  return o.current = e, k(() => {
    const n = o.current;
    let a = !1;
    return i((s) => ({ ...s, loading: !0 })), (async () => {
      const { entries: s, installed: c } = await Z(n);
      a || i({ entries: s, installed: c, loading: !1 });
    })(), () => {
      a = !0;
    };
  }, [t]), l;
}
const _ = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "widget", label: "Extension widgets" },
  { group: "flows", label: "Flows" },
  { group: "rules", label: "Rules" }
], Y = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "action", label: "Action (control)" },
  { group: "widget", label: "Extension widgets" }
];
function ee({
  entries: e,
  value: t = "",
  onSelect: l,
  loading: i = !1,
  groups: o = _,
  "aria-label": n = "source",
  className: a
}) {
  const s = (c) => {
    const u = e.find((g) => g.id === c) ?? null;
    l(u ? R(u) : null);
  };
  return /* @__PURE__ */ d("label", { className: `sp-root${a ? ` ${a}` : ""}`, children: /* @__PURE__ */ y(
    "select",
    {
      className: "sp-select",
      "aria-label": n,
      value: t,
      onChange: (c) => s(c.target.value),
      children: [
        /* @__PURE__ */ d("option", { value: "", children: i ? "loading sources…" : "— pick a source —" }),
        o.map(({ group: c, label: u }) => /* @__PURE__ */ d(z, { entries: e, group: c, label: u }, c))
      ]
    }
  ) });
}
function z({
  entries: e,
  group: t,
  label: l
}) {
  const i = e.filter((o) => o.group === t);
  return i.length === 0 ? null : /* @__PURE__ */ d("optgroup", { label: l, children: i.map((o) => /* @__PURE__ */ d("option", { value: o.id, children: o.label }, o.id)) });
}
function te({
  entries: e,
  value: t = "",
  onSelect: l,
  onSelectEntry: i,
  loading: o = !1,
  groups: n = _,
  "aria-label": a = "source",
  className: s,
  placeholder: c = "Search sources…",
  autoFocus: u = !1
}) {
  const [g, h] = E(""), [b, p] = E(!1), [w, m] = E(0), L = q(null), v = e.find((r) => r.id === t) ?? null, $ = N(() => {
    const r = g.trim().toLowerCase(), f = [];
    for (const { group: S, label: P } of n)
      e.filter(
        (x) => x.group === S && (r === "" || x.label.toLowerCase().includes(r) || P.toLowerCase().includes(r))
      ).forEach((x, O) => f.push({ entry: x, groupLabel: P, firstOfGroup: O === 0 }));
    return f;
  }, [e, n, g]), D = (r) => {
    l(r ? R(r) : null), i == null || i(r), p(!1), h("");
  }, C = (r) => {
    r.key === "ArrowDown" ? (r.preventDefault(), p(!0), m((f) => Math.min(f + 1, $.length - 1))) : r.key === "ArrowUp" ? (r.preventDefault(), m((f) => Math.max(f - 1, 0))) : r.key === "Enter" ? (r.preventDefault(), b && $[w] && D($[w].entry)) : r.key === "Escape" && p(!1);
  };
  return /* @__PURE__ */ y("div", { className: `sp-root sp-combo${s ? ` ${s}` : ""}`, children: [
    /* @__PURE__ */ d(
      "input",
      {
        className: "sp-combo-input",
        role: "combobox",
        "aria-expanded": b,
        "aria-label": a,
        "aria-autocomplete": "list",
        autoFocus: u,
        value: b ? g : (v == null ? void 0 : v.label) ?? "",
        placeholder: o ? "loading sources…" : v ? v.label : c,
        onFocus: () => p(!0),
        onBlur: () => setTimeout(() => p(!1), 120),
        onChange: (r) => {
          h(r.target.value), p(!0), m(0);
        },
        onKeyDown: C
      }
    ),
    b && /* @__PURE__ */ y("ul", { className: "sp-combo-list", role: "listbox", "aria-label": a, ref: L, children: [
      $.length === 0 && /* @__PURE__ */ d("li", { className: "sp-combo-empty", children: "No matching sources" }),
      $.map((r, f) => /* @__PURE__ */ y("li", { role: "presentation", children: [
        r.firstOfGroup && /* @__PURE__ */ d("div", { className: "sp-combo-group", children: r.groupLabel }),
        /* @__PURE__ */ d(
          "button",
          {
            type: "button",
            role: "option",
            "aria-selected": f === w,
            className: `sp-combo-option${f === w ? " is-active" : ""}${r.entry.id === t ? " is-selected" : ""}`,
            onMouseDown: (S) => {
              S.preventDefault(), D(r.entry);
            },
            onMouseEnter: () => m(f),
            children: r.entry.label
          }
        )
      ] }, r.entry.id))
    ] })
  ] });
}
export {
  Y as BUILDER_SOURCE_GROUPS,
  z as PickerGroup,
  _ as READ_SOURCE_GROUPS,
  j as SQL_SOURCE_ID,
  te as SourceCombobox,
  ee as SourcePicker,
  W as buildSourceEntries,
  G as extWidgetEntries,
  B as extensionEntries,
  T as flowsEntries,
  A as liveEntries,
  Z as loadSourcePicker,
  K as rulesEntries,
  R as selectionOf,
  M as seriesEntries,
  Q as sqlSourceEntry,
  X as useSourcePicker,
  U as widgetIdOf
};
