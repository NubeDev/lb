import { useState as w, useRef as D, useEffect as L, useCallback as U, useMemo as B } from "react";
import { jsx as i, jsxs as p } from "react/jsx-runtime";
import { ChevronRight as T, Table2 as G, Inbox as M, Lightbulb as j, Hash as Q, LineChart as W, Database as Z } from "lucide-react";
import * as N from "@radix-ui/react-collapsible";
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
      return (u = l.scope) == null ? void 0 : u.forEach((d) => r.add(d));
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
    source: { tool: "rules.run", args: { rule_id: t.id } },
    writes: !1,
    params: t.params ?? []
  }));
}
const ae = "sql:query";
function ie() {
  return {
    id: ae,
    group: "sql",
    label: "SQL query (direct SurrealDB)",
    source: { tool: "store.query", args: { sql: "" } },
    writes: !1
  };
}
function oe(e) {
  return [
    ...X(e.series ?? []),
    ...Y(e.series ?? []),
    ...ee(e.extensions ?? []),
    ...te(e.extensions ?? []),
    ...se(e.flows ?? [], e.descriptors ?? []),
    ...ne(e.rules ?? []),
    ie()
  ];
}
function _(e) {
  return { id: e.id, source: e.source, action: e.action, viewKey: e.viewKey };
}
const q = {
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
}, le = Object.keys(q);
function re(e) {
  return e instanceof Error ? e.message : String(e);
}
async function ce(e, t) {
  const a = {}, n = (s, o) => {
    a[s] = o, t == null || t((r) => ({ ...r, [s]: o }));
  };
  return await Promise.all(
    le.map(async (s) => {
      const o = await P(e, s);
      o && n(s, o);
    })
  ), a;
}
async function P(e, t) {
  const a = e[q[t]];
  if (a)
    try {
      return { status: "ready", data: await a() };
    } catch (n) {
      return { status: "denied", error: re(n) };
    }
}
async function ue(e) {
  const t = await ce(e), a = g(t.flowSummaries, []), n = g(t.flowDescriptors, []), s = e.getFlow, o = s ? (await Promise.all(a.map((d) => s(d.id).catch(() => null)))).filter((d) => d != null) : [], r = g(t.series, []), l = g(t.extensions, []);
  g(t.datasources, []);
  const u = g(t.rules, []);
  return {
    entries: oe({
      series: r,
      extensions: l,
      flows: o,
      descriptors: n,
      rules: u
    }),
    installed: l
  };
}
function g(e, t) {
  return (e == null ? void 0 : e.status) === "ready" ? e.data : t;
}
function Ee(e, t) {
  const [a, n] = w({
    entries: [],
    installed: [],
    loading: !0
  }), s = D(e);
  return s.current = e, L(() => {
    const o = s.current;
    let r = !1;
    return n((l) => ({ ...l, loading: !0 })), (async () => {
      const { entries: l, installed: u } = await ue(o);
      r || n({ entries: l, installed: u, loading: !1 });
    })(), () => {
      r = !0;
    };
  }, [t]), a;
}
const de = [
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
function Se(e) {
  return e.map((t) => ({
    kind: "datasource",
    id: `datasource:${t.name}`,
    name: t.name,
    rowKind: t.kind,
    endpoint: t.endpoint
  }));
}
function De(e) {
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
function pe(e) {
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
function me(e) {
  return e.map((t) => ({ kind: "inbox", id: `inbox:${t.id}`, channel: t.channel }));
}
function fe(e) {
  const t = [];
  return e.listDatasources && t.push("datasources"), e.readSchema && t.push("schema"), e.listSeries && t.push("series"), e.listChannels && t.push("channels"), e.listInsights && t.push("insights"), e.listInbox && t.push("inbox"), e.listExtensions && t.push("extensions"), e.listRules && t.push("rules"), e.listFlows && t.push("flowSummaries"), e.listFlowNodes && t.push("flowDescriptors"), t;
}
function I(e) {
  const t = {};
  for (const a of fe(e))
    t[a] = { status: "idle" };
  return t;
}
function Ie(e, t) {
  const [a, n] = w(() => I(e)), s = D(e);
  s.current = e, L(() => {
    n(I(s.current));
  }, [t]);
  const o = U((r) => {
    n((l) => {
      const u = l[r];
      if (u && u.status !== "idle") return l;
      const d = { ...l, [r]: { status: "loading" } };
      return P(s.current, r).then((m) => {
        m && n((k) => ({ ...k, [r]: m }));
      }), d;
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
  { group: "rules", label: "Rules" }
], Le = [
  { group: "series", label: "Series" },
  { group: "live", label: "Live (Zenoh)" },
  { group: "sql", label: "Direct SurrealDB" },
  { group: "extension", label: "Installed extension" },
  { group: "action", label: "Action (control)" },
  { group: "widget", label: "Extension widgets" }
];
function Te({
  entries: e,
  value: t = "",
  onSelect: a,
  loading: n = !1,
  groups: s = A,
  "aria-label": o = "source",
  className: r
}) {
  const l = (u) => {
    const d = e.find((m) => m.id === u) ?? null;
    a(d ? _(d) : null);
  };
  return /* @__PURE__ */ i("label", { className: `sp-root${r ? ` ${r}` : ""}`, children: /* @__PURE__ */ p(
    "select",
    {
      className: "sp-select",
      "aria-label": o,
      value: t,
      onChange: (u) => l(u.target.value),
      children: [
        /* @__PURE__ */ i("option", { value: "", children: n ? "loading sources…" : "— pick a source —" }),
        s.map(({ group: u, label: d }) => /* @__PURE__ */ i(ge, { entries: e, group: u, label: d }, u))
      ]
    }
  ) });
}
function ge({
  entries: e,
  group: t,
  label: a
}) {
  const n = e.filter((s) => s.group === t);
  return n.length === 0 ? null : /* @__PURE__ */ i("optgroup", { label: a, children: n.map((s) => /* @__PURE__ */ i("option", { value: s.id, children: s.label }, s.id)) });
}
function _e({
  entries: e,
  value: t = "",
  onSelect: a,
  onSelectEntry: n,
  loading: s = !1,
  groups: o = A,
  "aria-label": r = "source",
  className: l,
  placeholder: u = "Search sources…",
  autoFocus: d = !1
}) {
  const [m, k] = w(""), [v, f] = w(!1), [y, C] = w(0), F = D(null), $ = e.find((c) => c.id === t) ?? null, x = B(() => {
    const c = m.trim().toLowerCase(), h = [];
    for (const { group: S, label: O } of o)
      e.filter(
        (E) => E.group === S && (c === "" || E.label.toLowerCase().includes(c) || O.toLowerCase().includes(c))
      ).forEach((E, z) => h.push({ entry: E, groupLabel: O, firstOfGroup: z === 0 }));
    return h;
  }, [e, o, m]), R = (c) => {
    a(c ? _(c) : null), n == null || n(c), f(!1), k("");
  }, K = (c) => {
    c.key === "ArrowDown" ? (c.preventDefault(), f(!0), C((h) => Math.min(h + 1, x.length - 1))) : c.key === "ArrowUp" ? (c.preventDefault(), C((h) => Math.max(h - 1, 0))) : c.key === "Enter" ? (c.preventDefault(), v && x[y] && R(x[y].entry)) : c.key === "Escape" && f(!1);
  };
  return /* @__PURE__ */ p("div", { className: `sp-root sp-combo${l ? ` ${l}` : ""}`, children: [
    /* @__PURE__ */ i(
      "input",
      {
        className: "sp-combo-input",
        role: "combobox",
        "aria-expanded": v,
        "aria-label": r,
        "aria-autocomplete": "list",
        autoFocus: d,
        value: v ? m : ($ == null ? void 0 : $.label) ?? "",
        placeholder: s ? "loading sources…" : $ ? $.label : u,
        onFocus: () => f(!0),
        onBlur: () => setTimeout(() => f(!1), 120),
        onChange: (c) => {
          k(c.target.value), f(!0), C(0);
        },
        onKeyDown: K
      }
    ),
    v && /* @__PURE__ */ p("ul", { className: "sp-combo-list", role: "listbox", "aria-label": r, ref: F, children: [
      x.length === 0 && /* @__PURE__ */ i("li", { className: "sp-combo-empty", children: "No matching sources" }),
      x.map((c, h) => /* @__PURE__ */ p("li", { role: "presentation", children: [
        c.firstOfGroup && /* @__PURE__ */ i("div", { className: "sp-combo-group", children: c.groupLabel }),
        /* @__PURE__ */ i(
          "button",
          {
            type: "button",
            role: "option",
            "aria-selected": h === y,
            className: `sp-combo-option${h === y ? " is-active" : ""}${c.entry.id === t ? " is-selected" : ""}`,
            onMouseDown: (S) => {
              S.preventDefault(), R(c.entry);
            },
            onMouseEnter: () => C(h),
            children: c.entry.label
          }
        )
      ] }, c.entry.id))
    ] })
  ] });
}
function be({ spec: e, state: t, onOpen: a, defaultOpen: n, children: s }) {
  const [o, r] = w(n ?? t.status !== "idle"), l = t.status === "idle", u = (d) => {
    r(d), d && l && a && a();
  };
  return /* @__PURE__ */ p(
    N.Root,
    {
      className: "sp-catalog-section",
      "aria-label": `section ${e.label}`,
      open: o,
      onOpenChange: u,
      children: [
        /* @__PURE__ */ p(
          N.Trigger,
          {
            className: "sp-catalog-section-head",
            "aria-label": `toggle section ${e.label}`,
            children: [
              /* @__PURE__ */ i(T, { className: "sp-catalog-section-chevron" }),
              /* @__PURE__ */ i("h3", { className: "sp-catalog-section-title", children: e.label }),
              /* @__PURE__ */ i("p", { className: "sp-catalog-section-hint", children: e.hint })
            ]
          }
        ),
        /* @__PURE__ */ i(N.Content, { className: "sp-catalog-section-content", children: we(t, s) })
      ]
    }
  );
}
function we(e, t) {
  return e.status === "idle" ? /* @__PURE__ */ i("p", { className: "sp-catalog-idle", children: "Expand to load." }) : e.status === "loading" ? /* @__PURE__ */ i("div", { "aria-label": "loading", className: "sp-catalog-skeleton" }) : e.status === "denied" ? /* @__PURE__ */ i("p", { "aria-label": "denied", className: "sp-catalog-denied", children: "Not permitted." }) : t(e.data);
}
function b({ children: e }) {
  return /* @__PURE__ */ i("p", { className: "sp-catalog-empty", children: e });
}
function Ne({ schema: e, onSelect: t }) {
  return /* @__PURE__ */ i("ul", { "aria-label": "schema browser", className: "sp-catalog-tree", children: e.tables.map((a) => /* @__PURE__ */ i($e, { name: a.name, columns: a.columns.map((n) => n.name), onSelect: t }, a.name)) });
}
function $e({
  name: e,
  columns: t,
  onSelect: a
}) {
  return /* @__PURE__ */ i("li", { children: /* @__PURE__ */ p(N.Root, { className: "group/collapsible sp-catalog-tree-row", defaultOpen: !1, children: [
    /* @__PURE__ */ p("div", { className: "sp-catalog-tree-row-inner", children: [
      /* @__PURE__ */ i(
        N.Trigger,
        {
          "aria-label": `toggle table ${e}`,
          className: "sp-catalog-toggle",
          children: /* @__PURE__ */ i(T, { className: "sp-catalog-chevron" })
        }
      ),
      /* @__PURE__ */ p(
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
    /* @__PURE__ */ i(N.Content, { className: "sp-catalog-tree-content", children: /* @__PURE__ */ i("ul", { className: "sp-catalog-tree-columns", children: t.length === 0 ? /* @__PURE__ */ i("li", { className: "sp-catalog-tree-no-columns", children: "no columns" }) : t.map((n) => /* @__PURE__ */ i("li", { children: /* @__PURE__ */ i(
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
function qe({
  sections: e,
  onSelect: t,
  onLoadSection: a,
  sectionSpecs: n = de,
  className: s
}) {
  return /* @__PURE__ */ i("div", { "aria-label": "data explorer", className: `sp-root sp-catalog${s ? ` ${s}` : ""}`, children: n.map((o) => {
    const r = e[o.kind];
    return r ? /* @__PURE__ */ i(
      be,
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
      return n.length === 0 ? /* @__PURE__ */ i(b, { children: "No external datasources registered." }) : /* @__PURE__ */ i("ul", { className: "sp-catalog-list", children: n.map((s) => /* @__PURE__ */ i("li", { children: /* @__PURE__ */ p(
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
            /* @__PURE__ */ p("span", { className: "sp-catalog-row-label", children: [
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
      return n.length === 0 ? /* @__PURE__ */ i(b, { children: "No series in this workspace." }) : /* @__PURE__ */ i("ul", { className: "sp-catalog-list", children: n.map((s) => /* @__PURE__ */ i("li", { children: /* @__PURE__ */ p(
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
        const o = pe([s])[0];
        return /* @__PURE__ */ i("li", { children: /* @__PURE__ */ p(
          "button",
          {
            type: "button",
            "aria-label": `insert channel ${s.id}`,
            className: "sp-catalog-row sp-catalog-row-channel",
            onClick: () => a(o),
            children: [
              /* @__PURE__ */ i(Q, { "aria-hidden": "true", className: "sp-catalog-icon", size: 12 }),
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
        return /* @__PURE__ */ i("li", { children: /* @__PURE__ */ p(
          "button",
          {
            type: "button",
            "aria-label": `insert insight ${s.title}`,
            className: "sp-catalog-row sp-catalog-row-insight",
            onClick: () => a(o),
            children: [
              /* @__PURE__ */ p("span", { className: "sp-catalog-row-label", children: [
                /* @__PURE__ */ i(j, { "aria-hidden": "true", className: "sp-catalog-icon", size: 12 }),
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
        const o = me([s])[0];
        return /* @__PURE__ */ i("li", { children: /* @__PURE__ */ p(
          "button",
          {
            type: "button",
            "aria-label": `insert inbox item ${s.id}`,
            className: "sp-catalog-row sp-catalog-row-inbox",
            onClick: () => a(o),
            children: [
              /* @__PURE__ */ p("span", { className: "sp-catalog-row-label", children: [
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
  Le as BUILDER_SOURCE_GROUPS,
  de as CATALOG_SECTION_SPECS,
  b as CatalogEmpty,
  qe as CatalogExplorer,
  Ne as CatalogSchemaTree,
  be as CatalogSection,
  ge as PickerGroup,
  A as READ_SOURCE_GROUPS,
  ae as SQL_SOURCE_ID,
  _e as SourceCombobox,
  Te as SourcePicker,
  oe as buildSourceEntries,
  pe as channelEntries,
  Se as datasourceEntries,
  te as extWidgetEntries,
  ee as extensionEntries,
  se as flowsEntries,
  me as inboxEntries,
  he as insightEntries,
  Y as liveEntries,
  ce as loadCatalog,
  ue as loadSourcePicker,
  ne as rulesEntries,
  Re as schemaColumnEntries,
  De as schemaTableEntries,
  _ as selectionOf,
  Oe as seriesCatalogEntries,
  X as seriesEntries,
  ie as sqlSourceEntry,
  Ie as useCatalog,
  Ee as useSourcePicker,
  H as widgetIdOf
};
