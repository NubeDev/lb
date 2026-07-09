import { useState as w, useRef as D, useEffect as I, useCallback as U, useMemo as B } from "react";
import { jsx as i, jsxs as d } from "react/jsx-runtime";
import { ChevronRight as L, Table2 as G, Inbox as M, Lightbulb as Q, Hash as j, LineChart as W, Database as Z } from "lucide-react";
import * as $ from "@radix-ui/react-collapsible";
function H(e) {
  return e.label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
function J(e) {
  return /\.(publish|write|enqueue|command|set|send|record|create|delete|resolve|derive|simulate)$/.test(
    e
  );
}
function V(e, t) {
  const a = t.startsWith(`${e}.`) ? t.slice(e.length + 1) : t;
  return `${e} · ${a}`;
}
function X(e) {
  return e.map((t) => ({
    id: `series:${t}`,
    group: "series",
    label: t,
    source: { tool: "series.read", args: { series: t } },
    writes: !1
  }));
}
function Y(e) {
  return e.map((t) => ({
    id: `live:${t}`,
    group: "live",
    label: `${t} (live)`,
    source: { tool: "series.watch", args: { series: t } },
    writes: !1
  }));
}
function ee(e) {
  var a, n, s;
  const t = [];
  for (const o of e) {
    if (!o.enabled) continue;
    const r = /* @__PURE__ */ new Set();
    (n = (a = o.ui) == null ? void 0 : a.scope) == null || n.forEach((l) => r.add(l)), (s = o.widgets) == null || s.forEach((l) => {
      var u;
      return (u = l.scope) == null ? void 0 : u.forEach((p) => r.add(p));
    });
    for (const l of r) {
      const u = J(l);
      t.push({
        id: `ext:${o.ext}:${l}`,
        group: u ? "action" : "extension",
        label: V(o.ext, l),
        source: u ? void 0 : { tool: l, args: {} },
        action: u ? { tool: l, argsTemplate: {} } : void 0,
        writes: u
      });
    }
  }
  return t;
}
function te(e) {
  const t = [];
  for (const a of e)
    if (a.enabled)
      for (const n of a.widgets ?? []) {
        const s = H(n);
        t.push({
          id: `widget:${a.ext}/${s}`,
          group: "widget",
          label: `${a.ext} · ${n.label}`,
          icon: n.icon,
          viewKey: `ext:${a.ext}/${s}`,
          data: n.data === !0,
          writes: !1
        });
      }
  return t;
}
function se(e, t) {
  const a = new Map(t.map((s) => [s.type, s])), n = [];
  for (const s of e)
    for (const o of s.nodes ?? []) {
      const r = a.get(o.type);
      if (r) {
        for (const l of r.inputs ?? [])
          n.push({
            id: `flows:in:${s.id}:${o.id}:${l}`,
            group: "flows",
            label: `${s.name || s.id} › ${o.id} › ${l} (input)`,
            action: {
              tool: "flows.inject",
              argsTemplate: { id: s.id, node: o.id, port: l, value: "{{value}}" }
            },
            writes: !0
          });
        for (const l of r.outputs ?? [])
          n.push({
            id: `flows:out:${s.id}:${o.id}:${l}`,
            group: "flows",
            label: `${s.name || s.id} › ${o.id} › ${l} (output)`,
            source: {
              tool: "flows.node_state",
              args: { id: s.id, __flowNode: o.id, __flowPort: l }
            },
            writes: !1
          });
      }
    }
  return n;
}
function ne(e) {
  return e.map((t) => ({
    id: `rule:${t.id}`,
    group: "rules",
    label: t.name || t.id,
    source: { tool: "rules.run", args: { rule_id: t.id, route: !1 } },
    writes: !1,
    params: t.params ?? []
  }));
}
function ae(e) {
  return e.map((t) => ({
    id: `query:${t.id}`,
    group: "queries",
    label: t.name || t.id,
    source: { tool: "query.run", args: { id: t.id } },
    writes: !1
  }));
}
const ie = "sql:query";
function oe() {
  return {
    id: ie,
    group: "sql",
    label: "SQL query (direct SurrealDB)",
    source: { tool: "store.query", args: { sql: "" } },
    writes: !1
  };
}
function le(e) {
  return [
    ...X(e.series ?? []),
    ...Y(e.series ?? []),
    ...ee(e.extensions ?? []),
    ...te(e.extensions ?? []),
    ...se(e.flows ?? [], e.descriptors ?? []),
    ...ne(e.rules ?? []),
    ...ae(e.queries ?? []),
    oe()
  ];
}
function T(e) {
  return { id: e.id, source: e.source, action: e.action, viewKey: e.viewKey };
}
const _ = {
  datasources: "listDatasources",
  schema: "readSchema",
  series: "listSeries",
  channels: "listChannels",
  insights: "listInsights",
  inbox: "listInbox",
  queries: "listQueries",
  extensions: "listExtensions",
  rules: "listRules",
  flowSummaries: "listFlows",
  flowDescriptors: "listFlowNodes"
}, re = Object.keys(_);
function ce(e) {
  return e instanceof Error ? e.message : String(e);
}
async function ue(e, t) {
  const a = {}, n = (s, o) => {
    a[s] = o, t == null || t((r) => ({ ...r, [s]: o }));
  };
  return await Promise.all(
    re.map(async (s) => {
      const o = await P(e, s);
      o && n(s, o);
    })
  ), a;
}
async function P(e, t) {
  const a = e[_[t]];
  if (a)
    try {
      return { status: "ready", data: await a() };
    } catch (n) {
      return { status: "denied", error: ce(n) };
    }
}
async function de(e) {
  const t = await ue(e), a = f(t.flowSummaries, []), n = f(t.flowDescriptors, []), s = e.getFlow, o = s ? (await Promise.all(a.map((m) => s(m.id).catch(() => null)))).filter((m) => m != null) : [], r = f(t.series, []), l = f(t.extensions, []);
  f(t.datasources, []);
  const u = f(t.rules, []), p = f(t.queries, []);
  return {
    entries: le({
      series: r,
      extensions: l,
      flows: o,
      descriptors: n,
      rules: u,
      queries: p
    }),
    installed: l
  };
}
function f(e, t) {
  return (e == null ? void 0 : e.status) === "ready" ? e.data : t;
}
function Se(e, t) {
  const [a, n] = w({
    entries: [],
    installed: [],
    loading: !0
  }), s = D(e);
  return s.current = e, I(() => {
    const o = s.current;
    let r = !1;
    return n((l) => ({ ...l, loading: !0 })), (async () => {
      const { entries: l, installed: u } = await de(o);
      r || n({ entries: l, installed: u, loading: !1 });
    })(), () => {
      r = !0;
    };
  }, [t]), a;
}
const pe = [
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
  },
  {
    kind: "queries",
    label: "Saved queries",
    hint: "Saved PRQL/raw queries — click to run or reference one."
  }
];
function De(e) {
  return e.map((t) => ({
    kind: "datasource",
    id: `datasource:${t.name}`,
    name: t.name,
    rowKind: t.kind,
    endpoint: t.endpoint
  }));
}
function qe(e) {
  return e.tables.map((t) => ({
    kind: "table",
    id: `table:${t.name}`,
    table: t.name
  }));
}
function Re(e) {
  const t = [];
  for (const a of e.tables)
    for (const n of a.columns)
      t.push({
        kind: "column",
        id: `column:${a.name}.${n.name}`,
        table: a.name,
        column: n.name
      });
  return t;
}
function Oe(e) {
  return e.map((t) => ({ kind: "series", id: `series:${t}`, name: t }));
}
function me(e) {
  return e.map((t) => ({ kind: "channel", id: `channel:${t.id}`, name: t.id }));
}
function he(e) {
  return e.map((t) => ({
    kind: "insight",
    id: `insight:${t.id}`,
    title: t.title,
    severity: t.severity,
    status: t.status
  }));
}
function fe(e) {
  return e.map((t) => ({ kind: "inbox", id: `inbox:${t.id}`, channel: t.channel }));
}
function Ie(e) {
  return e.map((t) => ({
    kind: "query",
    id: `query:${t.id}`,
    name: t.name || t.id,
    target: t.target
  }));
}
function ge(e) {
  const t = [];
  return e.listDatasources && t.push("datasources"), e.readSchema && t.push("schema"), e.listSeries && t.push("series"), e.listChannels && t.push("channels"), e.listInsights && t.push("insights"), e.listInbox && t.push("inbox"), e.listQueries && t.push("queries"), e.listExtensions && t.push("extensions"), e.listRules && t.push("rules"), e.listFlows && t.push("flowSummaries"), e.listFlowNodes && t.push("flowDescriptors"), t;
}
function O(e) {
  const t = {};
  for (const a of ge(e))
    t[a] = { status: "idle" };
  return t;
}
function Le(e, t) {
  const [a, n] = w(() => O(e)), s = D(e);
  s.current = e, I(() => {
    n(O(s.current));
  }, [t]);
  const o = U((r) => {
    n((l) => {
      const u = l[r];
      if (u && u.status !== "idle") return l;
      const p = { ...l, [r]: { status: "loading" } };
      return P(s.current, r).then((m) => {
        m && n((x) => ({ ...x, [r]: m }));
      }), p;
    });
  }, []);
  return { sections: a, loadSection: o };
}
const A = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "widget", label: "Extension widgets" },
  { group: "flows", label: "Flows" },
  { group: "rules", label: "Rules" },
  { group: "queries", label: "Saved queries" }
], Te = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "action", label: "Action (control)" },
  { group: "widget", label: "Extension widgets" }
];
function _e({
  entries: e,
  value: t = "",
  onSelect: a,
  loading: n = !1,
  groups: s = A,
  "aria-label": o = "source",
  className: r
}) {
  const l = (u) => {
    const p = e.find((m) => m.id === u) ?? null;
    a(p ? T(p) : null);
  };
  return /* @__PURE__ */ i("label", { className: `sp-root${r ? ` ${r}` : ""}`, children: /* @__PURE__ */ d(
    "select",
    {
      className: "sp-select",
      "aria-label": o,
      value: t,
      onChange: (u) => l(u.target.value),
      children: [
        /* @__PURE__ */ i("option", { value: "", children: n ? "loading sources…" : "— pick a source —" }),
        s.map(({ group: u, label: p }) => /* @__PURE__ */ i(be, { entries: e, group: u, label: p }, u))
      ]
    }
  ) });
}
function be({
  entries: e,
  group: t,
  label: a
}) {
  const n = e.filter((s) => s.group === t);
  return n.length === 0 ? null : /* @__PURE__ */ i("optgroup", { label: a, children: n.map((s) => /* @__PURE__ */ i("option", { value: s.id, children: s.label }, s.id)) });
}
function Pe({
  entries: e,
  value: t = "",
  onSelect: a,
  onSelectEntry: n,
  loading: s = !1,
  groups: o = A,
  "aria-label": r = "source",
  className: l,
  placeholder: u = "Search sources…",
  autoFocus: p = !1
}) {
  const [m, x] = w(""), [y, g] = w(!1), [v, C] = w(0), F = D(null), N = e.find((c) => c.id === t) ?? null, k = B(() => {
    const c = m.trim().toLowerCase(), h = [];
    for (const { group: S, label: R } of o)
      e.filter(
        (E) => E.group === S && (c === "" || E.label.toLowerCase().includes(c) || R.toLowerCase().includes(c))
      ).forEach((E, z) => h.push({ entry: E, groupLabel: R, firstOfGroup: z === 0 }));
    return h;
  }, [e, o, m]), q = (c) => {
    a(c ? T(c) : null), n == null || n(c), g(!1), x("");
  }, K = (c) => {
    c.key === "ArrowDown" ? (c.preventDefault(), g(!0), C((h) => Math.min(h + 1, k.length - 1))) : c.key === "ArrowUp" ? (c.preventDefault(), C((h) => Math.max(h - 1, 0))) : c.key === "Enter" ? (c.preventDefault(), y && k[v] && q(k[v].entry)) : c.key === "Escape" && g(!1);
  };
  return /* @__PURE__ */ d("div", { className: `sp-root sp-combo${l ? ` ${l}` : ""}`, children: [
    /* @__PURE__ */ i(
      "input",
      {
        className: "sp-combo-input",
        role: "combobox",
        "aria-expanded": y,
        "aria-label": r,
        "aria-autocomplete": "list",
        autoFocus: p,
        value: y ? m : (N == null ? void 0 : N.label) ?? "",
        placeholder: s ? "loading sources…" : N ? N.label : u,
        onFocus: () => g(!0),
        onBlur: () => setTimeout(() => g(!1), 120),
        onChange: (c) => {
          x(c.target.value), g(!0), C(0);
        },
        onKeyDown: K
      }
    ),
    y && /* @__PURE__ */ d("ul", { className: "sp-combo-list", role: "listbox", "aria-label": r, ref: F, children: [
      k.length === 0 && /* @__PURE__ */ i("li", { className: "sp-combo-empty", children: "No matching sources" }),
      k.map((c, h) => /* @__PURE__ */ d("li", { role: "presentation", children: [
        c.firstOfGroup && /* @__PURE__ */ i("div", { className: "sp-combo-group", children: c.groupLabel }),
        /* @__PURE__ */ i(
          "button",
          {
            type: "button",
            role: "option",
            "aria-selected": h === v,
            className: `sp-combo-option${h === v ? " is-active" : ""}${c.entry.id === t ? " is-selected" : ""}`,
            onMouseDown: (S) => {
              S.preventDefault(), q(c.entry);
            },
            onMouseEnter: () => C(h),
            children: c.entry.label
          }
        )
      ] }, c.entry.id))
    ] })
  ] });
}
function we({ spec: e, state: t, onOpen: a, defaultOpen: n, children: s }) {
  const [o, r] = w(n ?? t.status !== "idle"), l = t.status === "idle", u = (p) => {
    r(p), p && l && a && a();
  };
  return /* @__PURE__ */ d(
    $.Root,
    {
      className: "sp-catalog-section",
      "aria-label": `section ${e.label}`,
      open: o,
      onOpenChange: u,
      children: [
        /* @__PURE__ */ d(
          $.Trigger,
          {
            className: "sp-catalog-section-head",
            "aria-label": `toggle section ${e.label}`,
            children: [
              /* @__PURE__ */ i(L, { className: "sp-catalog-section-chevron" }),
              /* @__PURE__ */ i("h3", { className: "sp-catalog-section-title", children: e.label }),
              /* @__PURE__ */ i("p", { className: "sp-catalog-section-hint", children: e.hint })
            ]
          }
        ),
        /* @__PURE__ */ i($.Content, { className: "sp-catalog-section-content", children: $e(t, s) })
      ]
    }
  );
}
function $e(e, t) {
  return e.status === "idle" ? /* @__PURE__ */ i("p", { className: "sp-catalog-idle", children: "Expand to load." }) : e.status === "loading" ? /* @__PURE__ */ i("div", { "aria-label": "loading", className: "sp-catalog-skeleton" }) : e.status === "denied" ? /* @__PURE__ */ i("p", { "aria-label": "denied", className: "sp-catalog-denied", children: "Not permitted." }) : t(e.data);
}
function b({ children: e }) {
  return /* @__PURE__ */ i("p", { className: "sp-catalog-empty", children: e });
}
function Ne({ schema: e, onSelect: t }) {
  return /* @__PURE__ */ i("ul", { "aria-label": "schema browser", className: "sp-catalog-tree", children: e.tables.map((a) => /* @__PURE__ */ i(ke, { name: a.name, columns: a.columns.map((n) => n.name), onSelect: t }, a.name)) });
}
function ke({
  name: e,
  columns: t,
  onSelect: a
}) {
  return /* @__PURE__ */ i("li", { children: /* @__PURE__ */ d($.Root, { className: "group/collapsible sp-catalog-tree-row", defaultOpen: !1, children: [
    /* @__PURE__ */ d("div", { className: "sp-catalog-tree-row-inner", children: [
      /* @__PURE__ */ i(
        $.Trigger,
        {
          "aria-label": `toggle table ${e}`,
          className: "sp-catalog-toggle",
          children: /* @__PURE__ */ i(L, { className: "sp-catalog-chevron" })
        }
      ),
      /* @__PURE__ */ d(
        "button",
        {
          type: "button",
          "aria-label": `insert table ${e}`,
          className: "sp-catalog-tree-table",
          onClick: () => a({ kind: "table", id: `table:${e}`, table: e }),
          children: [
            /* @__PURE__ */ i(G, { "aria-hidden": "true", className: "sp-catalog-icon", size: 12 }),
            /* @__PURE__ */ i("span", { className: "sp-catalog-tree-table-name", children: e })
          ]
        }
      )
    ] }),
    /* @__PURE__ */ i($.Content, { className: "sp-catalog-tree-content", children: /* @__PURE__ */ i("ul", { className: "sp-catalog-tree-columns", children: t.length === 0 ? /* @__PURE__ */ i("li", { className: "sp-catalog-tree-no-columns", children: "no columns" }) : t.map((n) => /* @__PURE__ */ i("li", { children: /* @__PURE__ */ i(
      "button",
      {
        type: "button",
        "aria-label": `insert column ${e}.${n}`,
        className: "sp-catalog-tree-column",
        onClick: () => a({ kind: "column", id: `column:${e}.${n}`, table: e, column: n }),
        children: n
      }
    ) }, n)) }) })
  ] }) });
}
function Ae({
  sections: e,
  onSelect: t,
  onLoadSection: a,
  sectionSpecs: n = pe,
  className: s
}) {
  return /* @__PURE__ */ i("div", { "aria-label": "data explorer", className: `sp-root sp-catalog${s ? ` ${s}` : ""}`, children: n.map((o) => {
    const r = e[o.kind];
    return r ? /* @__PURE__ */ i(
      we,
      {
        spec: o,
        state: r,
        onOpen: a ? () => a(o.kind) : void 0,
        children: (l) => xe(o.kind, l, t)
      },
      o.kind
    ) : null;
  }) });
}
function xe(e, t, a) {
  switch (e) {
    case "datasources": {
      const n = t ?? [];
      return n.length === 0 ? /* @__PURE__ */ i(b, { children: "No external datasources registered." }) : /* @__PURE__ */ i("ul", { className: "sp-catalog-list", children: n.map((s) => /* @__PURE__ */ i("li", { children: /* @__PURE__ */ d(
        "button",
        {
          type: "button",
          "aria-label": `insert datasource ${s.name}`,
          className: "sp-catalog-row sp-catalog-row-datasource",
          onClick: () => a({
            kind: "datasource",
            id: `datasource:${s.name}`,
            name: s.name,
            rowKind: s.kind,
            endpoint: s.endpoint
          }),
          children: [
            /* @__PURE__ */ d("span", { className: "sp-catalog-row-label", children: [
              /* @__PURE__ */ i(Z, { "aria-hidden": "true", className: "sp-catalog-icon", size: 12 }),
              s.name
            ] }),
            /* @__PURE__ */ i("span", { className: "sp-catalog-row-sub", children: s.endpoint ? `${s.kind} · ${s.endpoint}` : s.kind })
          ]
        }
      ) }, s.name)) });
    }
    case "schema": {
      const n = t;
      return n.tables.length === 0 ? /* @__PURE__ */ i(b, { children: "No local tables yet." }) : /* @__PURE__ */ i(Ne, { schema: n, onSelect: a });
    }
    case "series": {
      const n = t ?? [];
      return n.length === 0 ? /* @__PURE__ */ i(b, { children: "No series in this workspace." }) : /* @__PURE__ */ i("ul", { className: "sp-catalog-list", children: n.map((s) => /* @__PURE__ */ i("li", { children: /* @__PURE__ */ d(
        "button",
        {
          type: "button",
          "aria-label": `insert series ${s}`,
          className: "sp-catalog-row sp-catalog-row-series",
          onClick: () => a({ kind: "series", id: `series:${s}`, name: s }),
          children: [
            /* @__PURE__ */ i(W, { "aria-hidden": "true", className: "sp-catalog-icon", size: 12 }),
            s
          ]
        }
      ) }, s)) });
    }
    case "channels": {
      const n = t ?? [];
      return n.length === 0 ? /* @__PURE__ */ i(b, { children: "No channels registered." }) : /* @__PURE__ */ i("ul", { className: "sp-catalog-list", children: n.map((s) => {
        const o = me([s])[0];
        return /* @__PURE__ */ i("li", { children: /* @__PURE__ */ d(
          "button",
          {
            type: "button",
            "aria-label": `insert channel ${s.id}`,
            className: "sp-catalog-row sp-catalog-row-channel",
            onClick: () => a(o),
            children: [
              /* @__PURE__ */ i(j, { "aria-hidden": "true", className: "sp-catalog-icon", size: 12 }),
              s.id
            ]
          }
        ) }, o.id);
      }) });
    }
    case "insights": {
      const n = t ?? [];
      return n.length === 0 ? /* @__PURE__ */ i(b, { children: "No insights in this workspace." }) : /* @__PURE__ */ i("ul", { className: "sp-catalog-list", children: n.map((s) => {
        const o = he([s])[0];
        return /* @__PURE__ */ i("li", { children: /* @__PURE__ */ d(
          "button",
          {
            type: "button",
            "aria-label": `insert insight ${s.title}`,
            className: "sp-catalog-row sp-catalog-row-insight",
            onClick: () => a(o),
            children: [
              /* @__PURE__ */ d("span", { className: "sp-catalog-row-label", children: [
                /* @__PURE__ */ i(Q, { "aria-hidden": "true", className: "sp-catalog-icon", size: 12 }),
                s.title
              ] }),
              (s.severity || s.status) && /* @__PURE__ */ i("span", { className: "sp-catalog-row-sub", children: [s.severity, s.status].filter(Boolean).join(" · ") })
            ]
          }
        ) }, o.id);
      }) });
    }
    case "inbox": {
      const n = t ?? [];
      return n.length === 0 ? /* @__PURE__ */ i(b, { children: "No items in this inbox." }) : /* @__PURE__ */ i("ul", { className: "sp-catalog-list", children: n.map((s) => {
        const o = fe([s])[0];
        return /* @__PURE__ */ i("li", { children: /* @__PURE__ */ d(
          "button",
          {
            type: "button",
            "aria-label": `insert inbox item ${s.id}`,
            className: "sp-catalog-row sp-catalog-row-inbox",
            onClick: () => a(o),
            children: [
              /* @__PURE__ */ d("span", { className: "sp-catalog-row-label", children: [
                /* @__PURE__ */ i(M, { "aria-hidden": "true", className: "sp-catalog-icon", size: 12 }),
                s.id
              ] }),
              /* @__PURE__ */ i("span", { className: "sp-catalog-row-sub", children: s.channel })
            ]
          }
        ) }, o.id);
      }) });
    }
    default:
      return null;
  }
}
export {
  Te as BUILDER_SOURCE_GROUPS,
  pe as CATALOG_SECTION_SPECS,
  b as CatalogEmpty,
  Ae as CatalogExplorer,
  Ne as CatalogSchemaTree,
  we as CatalogSection,
  be as PickerGroup,
  A as READ_SOURCE_GROUPS,
  ie as SQL_SOURCE_ID,
  Pe as SourceCombobox,
  _e as SourcePicker,
  le as buildSourceEntries,
  me as channelEntries,
  De as datasourceEntries,
  te as extWidgetEntries,
  ee as extensionEntries,
  se as flowsEntries,
  fe as inboxEntries,
  he as insightEntries,
  Y as liveEntries,
  ue as loadCatalog,
  de as loadSourcePicker,
  Ie as queryCatalogEntries,
  ae as queryEntries,
  ne as rulesEntries,
  Re as schemaColumnEntries,
  qe as schemaTableEntries,
  T as selectionOf,
  Oe as seriesCatalogEntries,
  X as seriesEntries,
  oe as sqlSourceEntry,
  Le as useCatalog,
  Se as useSourcePicker,
  H as widgetIdOf
};
