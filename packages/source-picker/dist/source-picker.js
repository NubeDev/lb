import { useState as b, useRef as C, useEffect as _, useMemo as M } from "react";
import { jsx as l, jsxs as d } from "react/jsx-runtime";
function U(e) {
  return e.label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
function B(e) {
  return /\.(publish|write|enqueue|command|set|send|record|create|delete|resolve|derive|simulate)$/.test(
    e
  );
}
function F(e, t) {
  const o = t.startsWith(`${e}.`) ? t.slice(e.length + 1) : t;
  return `${e} · ${o}`;
}
function G(e) {
  return e.map((t) => ({
    id: `series:${t}`,
    group: "series",
    label: t,
    source: { tool: "series.read", args: { series: t } },
    writes: !1
  }));
}
function j(e) {
  return e.map((t) => ({
    id: `live:${t}`,
    group: "live",
    label: `${t} (live)`,
    source: { tool: "series.watch", args: { series: t } },
    writes: !1
  }));
}
function Q(e) {
  var o, s, n;
  const t = [];
  for (const a of e) {
    if (!a.enabled) continue;
    const r = /* @__PURE__ */ new Set();
    (s = (o = a.ui) == null ? void 0 : o.scope) == null || s.forEach((i) => r.add(i)), (n = a.widgets) == null || n.forEach((i) => {
      var u;
      return (u = i.scope) == null ? void 0 : u.forEach((p) => r.add(p));
    });
    for (const i of r) {
      const u = B(i);
      t.push({
        id: `ext:${a.ext}:${i}`,
        group: u ? "action" : "extension",
        label: F(a.ext, i),
        source: u ? void 0 : { tool: i, args: {} },
        action: u ? { tool: i, argsTemplate: {} } : void 0,
        writes: u
      });
    }
  }
  return t;
}
function W(e) {
  const t = [];
  for (const o of e)
    if (o.enabled)
      for (const s of o.widgets ?? []) {
        const n = U(s);
        t.push({
          id: `widget:${o.ext}/${n}`,
          group: "widget",
          label: `${o.ext} · ${s.label}`,
          icon: s.icon,
          viewKey: `ext:${o.ext}/${n}`,
          data: s.data === !0,
          writes: !1
        });
      }
  return t;
}
function Z(e, t) {
  const o = new Map(t.map((n) => [n.type, n])), s = [];
  for (const n of e)
    for (const a of n.nodes ?? []) {
      const r = o.get(a.type);
      if (r) {
        for (const i of r.inputs ?? [])
          s.push({
            id: `flows:in:${n.id}:${a.id}:${i}`,
            group: "flows",
            label: `${n.name || n.id} › ${a.id} › ${i} (input)`,
            action: {
              tool: "flows.inject",
              argsTemplate: { id: n.id, node: a.id, port: i, value: "{{value}}" }
            },
            writes: !0
          });
        for (const i of r.outputs ?? [])
          s.push({
            id: `flows:out:${n.id}:${a.id}:${i}`,
            group: "flows",
            label: `${n.name || n.id} › ${a.id} › ${i} (output)`,
            source: {
              tool: "flows.node_state",
              args: { id: n.id, __flowNode: a.id, __flowPort: i }
            },
            writes: !1
          });
      }
    }
  return s;
}
function z(e) {
  return e.map((t) => ({
    id: `rule:${t.id}`,
    group: "rules",
    label: t.name || t.id,
    source: { tool: "rules.run", args: { rule_id: t.id } },
    writes: !1,
    params: t.params ?? []
  }));
}
const Y = "sql:query";
function H() {
  return {
    id: Y,
    group: "sql",
    label: "SQL query (direct SurrealDB)",
    source: { tool: "store.query", args: { sql: "" } },
    writes: !1
  };
}
function J(e) {
  return [
    ...G(e.series ?? []),
    ...j(e.series ?? []),
    ...Q(e.extensions ?? []),
    ...W(e.extensions ?? []),
    ...Z(e.flows ?? [], e.descriptors ?? []),
    ...z(e.rules ?? []),
    H()
  ];
}
function I(e) {
  return { id: e.id, source: e.source, action: e.action, viewKey: e.viewKey };
}
const L = {
  datasources: "listDatasources",
  schema: "readSchema",
  series: "listSeries",
  channels: "listChannels",
  insights: "listInsights",
  inbox: "listInbox",
  extensions: "listExtensions",
  rules: "listRules",
  flowSummaries: "listFlows",
  flowDescriptors: "listFlowNodes"
}, V = Object.keys(L);
function X(e) {
  return e instanceof Error ? e.message : String(e);
}
async function T(e, t) {
  const o = {}, s = (n, a) => {
    o[n] = a, t == null || t((r) => ({ ...r, [n]: a }));
  };
  return await Promise.all(
    V.map(async (n) => {
      const a = e[L[n]];
      if (a)
        try {
          const r = await a();
          s(n, { status: "ready", data: r });
        } catch (r) {
          s(n, { status: "denied", error: X(r) });
        }
    })
  ), o;
}
async function ee(e) {
  const t = await T(e), o = f(t.flowSummaries, []), s = f(t.flowDescriptors, []), n = e.getFlow, a = n ? (await Promise.all(o.map((p) => n(p.id).catch(() => null)))).filter((p) => p != null) : [], r = f(t.series, []), i = f(t.extensions, []);
  f(t.datasources, []);
  const u = f(t.rules, []);
  return {
    entries: J({
      series: r,
      extensions: i,
      flows: a,
      descriptors: s,
      rules: u
    }),
    installed: i
  };
}
function f(e, t) {
  return (e == null ? void 0 : e.status) === "ready" ? e.data : t;
}
function he(e, t) {
  const [o, s] = b({
    entries: [],
    installed: [],
    loading: !0
  }), n = C(e);
  return n.current = e, _(() => {
    const a = n.current;
    let r = !1;
    return s((i) => ({ ...i, loading: !0 })), (async () => {
      const { entries: i, installed: u } = await ee(a);
      r || s({ entries: i, installed: u, loading: !1 });
    })(), () => {
      r = !0;
    };
  }, [t]), o;
}
const te = [
  {
    kind: "datasources",
    label: "Datasources",
    hint: "Registered external sources — click to query by name."
  },
  {
    kind: "schema",
    label: "Local tables",
    hint: "Tables in this workspace's store — click to insert a name."
  },
  {
    kind: "series",
    label: "Series",
    hint: "Discoverable timeseries — click to read 24h of history."
  },
  {
    kind: "channels",
    label: "Channels",
    hint: "Registered channels in this workspace — click to reference one."
  },
  {
    kind: "insights",
    label: "Insights",
    hint: "Open data findings — click to reference one."
  },
  {
    kind: "inbox",
    label: "Inbox",
    hint: "Items in this channel's inbox — click to reference one."
  }
];
function fe(e) {
  return e.map((t) => ({
    kind: "datasource",
    id: `datasource:${t.name}`,
    name: t.name,
    rowKind: t.kind,
    endpoint: t.endpoint
  }));
}
function ge(e) {
  return e.tables.map((t) => ({
    kind: "table",
    id: `table:${t.name}`,
    table: t.name
  }));
}
function be(e) {
  const t = [];
  for (const o of e.tables)
    for (const s of o.columns)
      t.push({
        kind: "column",
        id: `column:${o.name}.${s.name}`,
        table: o.name,
        column: s.name
      });
  return t;
}
function we(e) {
  return e.map((t) => ({ kind: "series", id: `series:${t}`, name: t }));
}
function ne(e) {
  return e.map((t) => ({ kind: "channel", id: `channel:${t.id}`, name: t.id }));
}
function se(e) {
  return e.map((t) => ({
    kind: "insight",
    id: `insight:${t.id}`,
    title: t.title,
    severity: t.severity,
    status: t.status
  }));
}
function ae(e) {
  return e.map((t) => ({ kind: "inbox", id: `inbox:${t.id}`, channel: t.channel }));
}
const R = {};
function $e(e, t) {
  const [o, s] = b(R), n = C(e);
  return n.current = e, _(() => {
    const a = n.current;
    let r = !1;
    return s(R), T(a, (i) => {
      r || s((u) => i(u));
    }).catch(() => {
    }), () => {
      r = !0;
    };
  }, [t]), o;
}
const q = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "widget", label: "Extension widgets" },
  { group: "flows", label: "Flows" },
  { group: "rules", label: "Rules" }
], Ne = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "action", label: "Action (control)" },
  { group: "widget", label: "Extension widgets" }
];
function ke({
  entries: e,
  value: t = "",
  onSelect: o,
  loading: s = !1,
  groups: n = q,
  "aria-label": a = "source",
  className: r
}) {
  const i = (u) => {
    const p = e.find((w) => w.id === u) ?? null;
    o(p ? I(p) : null);
  };
  return /* @__PURE__ */ l("label", { className: `sp-root${r ? ` ${r}` : ""}`, children: /* @__PURE__ */ d(
    "select",
    {
      className: "sp-select",
      "aria-label": a,
      value: t,
      onChange: (u) => i(u.target.value),
      children: [
        /* @__PURE__ */ l("option", { value: "", children: s ? "loading sources…" : "— pick a source —" }),
        n.map(({ group: u, label: p }) => /* @__PURE__ */ l(oe, { entries: e, group: u, label: p }, u))
      ]
    }
  ) });
}
function oe({
  entries: e,
  group: t,
  label: o
}) {
  const s = e.filter((n) => n.group === t);
  return s.length === 0 ? null : /* @__PURE__ */ l("optgroup", { label: o, children: s.map((n) => /* @__PURE__ */ l("option", { value: n.id, children: n.label }, n.id)) });
}
function xe({
  entries: e,
  value: t = "",
  onSelect: o,
  onSelectEntry: s,
  loading: n = !1,
  groups: a = q,
  "aria-label": r = "source",
  className: i,
  placeholder: u = "Search sources…",
  autoFocus: p = !1
}) {
  const [w, S] = b(""), [k, h] = b(!1), [x, y] = b(0), P = C(null), $ = e.find((c) => c.id === t) ?? null, N = M(() => {
    const c = w.trim().toLowerCase(), m = [];
    for (const { group: E, label: O } of a)
      e.filter(
        (v) => v.group === E && (c === "" || v.label.toLowerCase().includes(c) || O.toLowerCase().includes(c))
      ).forEach((v, K) => m.push({ entry: v, groupLabel: O, firstOfGroup: K === 0 }));
    return m;
  }, [e, a, w]), D = (c) => {
    o(c ? I(c) : null), s == null || s(c), h(!1), S("");
  }, A = (c) => {
    c.key === "ArrowDown" ? (c.preventDefault(), h(!0), y((m) => Math.min(m + 1, N.length - 1))) : c.key === "ArrowUp" ? (c.preventDefault(), y((m) => Math.max(m - 1, 0))) : c.key === "Enter" ? (c.preventDefault(), k && N[x] && D(N[x].entry)) : c.key === "Escape" && h(!1);
  };
  return /* @__PURE__ */ d("div", { className: `sp-root sp-combo${i ? ` ${i}` : ""}`, children: [
    /* @__PURE__ */ l(
      "input",
      {
        className: "sp-combo-input",
        role: "combobox",
        "aria-expanded": k,
        "aria-label": r,
        "aria-autocomplete": "list",
        autoFocus: p,
        value: k ? w : ($ == null ? void 0 : $.label) ?? "",
        placeholder: n ? "loading sources…" : $ ? $.label : u,
        onFocus: () => h(!0),
        onBlur: () => setTimeout(() => h(!1), 120),
        onChange: (c) => {
          S(c.target.value), h(!0), y(0);
        },
        onKeyDown: A
      }
    ),
    k && /* @__PURE__ */ d("ul", { className: "sp-combo-list", role: "listbox", "aria-label": r, ref: P, children: [
      N.length === 0 && /* @__PURE__ */ l("li", { className: "sp-combo-empty", children: "No matching sources" }),
      N.map((c, m) => /* @__PURE__ */ d("li", { role: "presentation", children: [
        c.firstOfGroup && /* @__PURE__ */ l("div", { className: "sp-combo-group", children: c.groupLabel }),
        /* @__PURE__ */ l(
          "button",
          {
            type: "button",
            role: "option",
            "aria-selected": m === x,
            className: `sp-combo-option${m === x ? " is-active" : ""}${c.entry.id === t ? " is-selected" : ""}`,
            onMouseDown: (E) => {
              E.preventDefault(), D(c.entry);
            },
            onMouseEnter: () => y(m),
            children: c.entry.label
          }
        )
      ] }, c.entry.id))
    ] })
  ] });
}
function le({ spec: e, state: t, children: o }) {
  return /* @__PURE__ */ d("section", { className: "sp-catalog-section", "aria-label": `section ${e.label}`, children: [
    /* @__PURE__ */ d("header", { className: "sp-catalog-section-head", children: [
      /* @__PURE__ */ l("h3", { className: "sp-catalog-section-title", children: e.label }),
      /* @__PURE__ */ l("p", { className: "sp-catalog-section-hint", children: e.hint })
    ] }),
    ie(t, o)
  ] });
}
function ie(e, t) {
  return e.status === "loading" ? /* @__PURE__ */ l("div", { "aria-label": "loading", className: "sp-catalog-skeleton" }) : e.status === "denied" ? /* @__PURE__ */ l("p", { "aria-label": "denied", className: "sp-catalog-denied", children: "Not permitted." }) : t(e.data);
}
function g({ children: e }) {
  return /* @__PURE__ */ l("p", { className: "sp-catalog-empty", children: e });
}
function re({ schema: e, onSelect: t }) {
  return /* @__PURE__ */ l("ul", { "aria-label": "schema browser", className: "sp-catalog-tree", children: e.tables.map((o) => /* @__PURE__ */ l(ce, { name: o.name, columns: o.columns.map((s) => s.name), onSelect: t }, o.name)) });
}
function ce({
  name: e,
  columns: t,
  onSelect: o
}) {
  const [s, n] = b(!1);
  return /* @__PURE__ */ d("li", { children: [
    /* @__PURE__ */ d("div", { className: "sp-catalog-tree-row", children: [
      /* @__PURE__ */ l(
        "button",
        {
          type: "button",
          "aria-label": `toggle table ${e}`,
          "aria-expanded": s,
          className: "sp-catalog-toggle",
          onClick: () => n((a) => !a),
          children: s ? "▾" : "▸"
        }
      ),
      /* @__PURE__ */ d(
        "button",
        {
          type: "button",
          "aria-label": `insert table ${e}`,
          className: "sp-catalog-tree-table",
          onClick: () => o({ kind: "table", id: `table:${e}`, table: e }),
          children: [
            /* @__PURE__ */ l("span", { "aria-hidden": "true", className: "sp-catalog-icon", children: "▦" }),
            e
          ]
        }
      )
    ] }),
    s ? /* @__PURE__ */ l("ul", { className: "sp-catalog-tree-columns", children: t.length === 0 ? /* @__PURE__ */ l("li", { className: "sp-catalog-tree-no-columns", children: "no columns" }) : t.map((a) => /* @__PURE__ */ l("li", { children: /* @__PURE__ */ l(
      "button",
      {
        type: "button",
        "aria-label": `insert column ${e}.${a}`,
        className: "sp-catalog-tree-column",
        onClick: () => o({ kind: "column", id: `column:${e}.${a}`, table: e, column: a }),
        children: a
      }
    ) }, a)) }) : null
  ] });
}
function ye({
  sections: e,
  onSelect: t,
  sectionSpecs: o = te,
  className: s
}) {
  return /* @__PURE__ */ l("div", { "aria-label": "data explorer", className: `sp-root sp-catalog${s ? ` ${s}` : ""}`, children: o.map((n) => {
    const a = e[n.kind];
    return a ? /* @__PURE__ */ l(le, { spec: n, state: a, children: (r) => ue(n.kind, r, t) }, n.kind) : null;
  }) });
}
function ue(e, t, o) {
  switch (e) {
    case "datasources": {
      const s = t ?? [];
      return s.length === 0 ? /* @__PURE__ */ l(g, { children: "No external datasources registered." }) : /* @__PURE__ */ l("ul", { className: "sp-catalog-list", children: s.map((n) => /* @__PURE__ */ l("li", { children: /* @__PURE__ */ d(
        "button",
        {
          type: "button",
          "aria-label": `insert datasource ${n.name}`,
          className: "sp-catalog-row sp-catalog-row-datasource",
          onClick: () => o({
            kind: "datasource",
            id: `datasource:${n.name}`,
            name: n.name,
            rowKind: n.kind,
            endpoint: n.endpoint
          }),
          children: [
            /* @__PURE__ */ d("span", { className: "sp-catalog-row-label", children: [
              /* @__PURE__ */ l("span", { "aria-hidden": "true", className: "sp-catalog-icon", children: "◳" }),
              n.name
            ] }),
            /* @__PURE__ */ l("span", { className: "sp-catalog-row-sub", children: n.endpoint ? `${n.kind} · ${n.endpoint}` : n.kind })
          ]
        }
      ) }, n.name)) });
    }
    case "schema": {
      const s = t;
      return s.tables.length === 0 ? /* @__PURE__ */ l(g, { children: "No local tables yet." }) : /* @__PURE__ */ l(re, { schema: s, onSelect: o });
    }
    case "series": {
      const s = t ?? [];
      return s.length === 0 ? /* @__PURE__ */ l(g, { children: "No series in this workspace." }) : /* @__PURE__ */ l("ul", { className: "sp-catalog-list", children: s.map((n) => /* @__PURE__ */ l("li", { children: /* @__PURE__ */ d(
        "button",
        {
          type: "button",
          "aria-label": `insert series ${n}`,
          className: "sp-catalog-row sp-catalog-row-series",
          onClick: () => o({ kind: "series", id: `series:${n}`, name: n }),
          children: [
            /* @__PURE__ */ l("span", { "aria-hidden": "true", className: "sp-catalog-icon", children: "〜" }),
            n
          ]
        }
      ) }, n)) });
    }
    case "channels": {
      const s = t ?? [];
      return s.length === 0 ? /* @__PURE__ */ l(g, { children: "No channels registered." }) : /* @__PURE__ */ l("ul", { className: "sp-catalog-list", children: s.map((n) => {
        const a = ne([n])[0];
        return /* @__PURE__ */ l("li", { children: /* @__PURE__ */ d(
          "button",
          {
            type: "button",
            "aria-label": `insert channel ${n.id}`,
            className: "sp-catalog-row sp-catalog-row-channel",
            onClick: () => o(a),
            children: [
              /* @__PURE__ */ l("span", { "aria-hidden": "true", className: "sp-catalog-icon", children: "#" }),
              n.id
            ]
          }
        ) }, a.id);
      }) });
    }
    case "insights": {
      const s = t ?? [];
      return s.length === 0 ? /* @__PURE__ */ l(g, { children: "No insights in this workspace." }) : /* @__PURE__ */ l("ul", { className: "sp-catalog-list", children: s.map((n) => {
        const a = se([n])[0];
        return /* @__PURE__ */ l("li", { children: /* @__PURE__ */ d(
          "button",
          {
            type: "button",
            "aria-label": `insert insight ${n.title}`,
            className: "sp-catalog-row sp-catalog-row-insight",
            onClick: () => o(a),
            children: [
              /* @__PURE__ */ d("span", { className: "sp-catalog-row-label", children: [
                /* @__PURE__ */ l("span", { "aria-hidden": "true", className: "sp-catalog-icon", children: "◆" }),
                n.title
              ] }),
              (n.severity || n.status) && /* @__PURE__ */ l("span", { className: "sp-catalog-row-sub", children: [n.severity, n.status].filter(Boolean).join(" · ") })
            ]
          }
        ) }, a.id);
      }) });
    }
    case "inbox": {
      const s = t ?? [];
      return s.length === 0 ? /* @__PURE__ */ l(g, { children: "No items in this inbox." }) : /* @__PURE__ */ l("ul", { className: "sp-catalog-list", children: s.map((n) => {
        const a = ae([n])[0];
        return /* @__PURE__ */ l("li", { children: /* @__PURE__ */ d(
          "button",
          {
            type: "button",
            "aria-label": `insert inbox item ${n.id}`,
            className: "sp-catalog-row sp-catalog-row-inbox",
            onClick: () => o(a),
            children: [
              /* @__PURE__ */ d("span", { className: "sp-catalog-row-label", children: [
                /* @__PURE__ */ l("span", { "aria-hidden": "true", className: "sp-catalog-icon", children: "✉" }),
                n.id
              ] }),
              /* @__PURE__ */ l("span", { className: "sp-catalog-row-sub", children: n.channel })
            ]
          }
        ) }, a.id);
      }) });
    }
    default:
      return null;
  }
}
export {
  Ne as BUILDER_SOURCE_GROUPS,
  te as CATALOG_SECTION_SPECS,
  g as CatalogEmpty,
  ye as CatalogExplorer,
  re as CatalogSchemaTree,
  le as CatalogSection,
  oe as PickerGroup,
  q as READ_SOURCE_GROUPS,
  Y as SQL_SOURCE_ID,
  xe as SourceCombobox,
  ke as SourcePicker,
  J as buildSourceEntries,
  ne as channelEntries,
  fe as datasourceEntries,
  W as extWidgetEntries,
  Q as extensionEntries,
  Z as flowsEntries,
  ae as inboxEntries,
  se as insightEntries,
  j as liveEntries,
  T as loadCatalog,
  ee as loadSourcePicker,
  z as rulesEntries,
  be as schemaColumnEntries,
  ge as schemaTableEntries,
  I as selectionOf,
  we as seriesCatalogEntries,
  G as seriesEntries,
  H as sqlSourceEntry,
  $e as useCatalog,
  he as useSourcePicker,
  U as widgetIdOf
};
