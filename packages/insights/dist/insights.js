import { useState as f, useRef as x, useCallback as $, useEffect as z } from "react";
import { jsx as r, jsxs as d, Fragment as M } from "react/jsx-runtime";
import { X as O, RefreshCw as I, Check as D, CheckCheck as _, Lightbulb as L } from "lucide-react";
const F = ["info", "warning", "critical"];
function Q(e) {
  return F.indexOf(e);
}
function T(e) {
  return e === "critical" ? "destructive" : e === "warning" ? "warning" : "accent-2";
}
function j(e) {
  return e === "open" ? "default" : e === "acked" ? "warning" : "success";
}
function P(e, t = Date.now()) {
  const o = Math.max(1, Math.floor((t - e) / 1e3));
  if (o < 60) return `${o}s ago`;
  const n = Math.floor(o / 60);
  if (n < 60) return o % 60 ? `${n}m ${o % 60}s ago` : `${n}m ago`;
  const s = Math.floor(n / 60);
  return s < 24 ? n % 60 ? `${s}h ${n % 60}m ago` : `${s}h ago` : `${Math.floor(s / 24)}d ago`;
}
function W(e) {
  const t = `${e.kind}:${e.ref}`;
  return e.run ? `${t} · run:${e.run}` : t;
}
function B(e, t) {
  const [o, n] = f([]), [s, l] = f(null), [m, v] = f(!1), [a, g] = f(null), [p, w] = f(null), [b, k] = f(t), u = x(e);
  u.current = e;
  const i = $(async () => {
    v(!0);
    try {
      const c = await u.current.list({ ...b, cursor: void 0 });
      n(c.items), w(c.next ?? null), l(null);
    } catch (c) {
      l(c instanceof Error ? c.message : String(c));
    } finally {
      v(!1);
    }
  }, [b]), N = $(async () => {
    if (p) {
      v(!0);
      try {
        const c = await u.current.list({ ...b, cursor: p });
        n((C) => {
          const R = new Set(C.map((S) => S.id));
          return [...C, ...c.items.filter((S) => !R.has(S.id))];
        }), w(c.next ?? null), l(null);
      } catch (c) {
        l(c instanceof Error ? c.message : String(c));
      } finally {
        v(!1);
      }
    }
  }, [b, p]);
  z(() => {
    i();
  }, [i]);
  const E = x(i);
  E.current = i, z(() => {
    const c = u.current.subscribe;
    return c ? c(() => {
      E.current();
    }) : void 0;
  }, []);
  const y = $((c) => {
    k(c);
  }, []), h = $(
    async (c, C) => {
      g(c);
      try {
        C === "ack" ? await u.current.ack(c) : await u.current.resolve(c), await i();
      } catch (R) {
        l(R instanceof Error ? R.message : String(R));
      } finally {
        g(null);
      }
    },
    [i]
  );
  return {
    items: o,
    error: s,
    loading: m,
    actingOn: a,
    nextCursor: p,
    refresh: i,
    loadMore: N,
    setFilter: y,
    act: h
  };
}
function Z(e, t, o = 50) {
  const [n, s] = f(null), [l, m] = f(null), [v, a] = f(null), [g, p] = f(!0), [w, b] = f(null), [k, u] = f(0), i = x(e);
  i.current = e, z(() => {
    let y = !1;
    return (async () => {
      a(null), p(!0);
      try {
        const [h, c] = await Promise.all([
          i.current.get(t),
          i.current.occurrences(t, void 0, o)
        ]);
        if (y) return;
        s(h), m(c);
      } catch (h) {
        if (y) return;
        a(h instanceof Error ? h.message : String(h));
      } finally {
        y || p(!1);
      }
    })(), () => {
      y = !0;
    };
  }, [t, o, k]);
  const N = $(() => u((y) => y + 1), []), E = $(
    async (y) => {
      b(y), a(null);
      try {
        y === "ack" ? await i.current.ack(t) : await i.current.resolve(t), u((h) => h + 1);
      } catch (h) {
        a(h instanceof Error ? h.message : String(h));
      } finally {
        b(null);
      }
    },
    [t]
  );
  return { insight: n, occurrences: l, error: v, loading: g, actingOn: w, refresh: N, act: E };
}
function V({ severity: e }) {
  return /* @__PURE__ */ r("span", { className: `ins-badge tone-${T(e)}`, children: e });
}
function U({ status: e }) {
  return /* @__PURE__ */ r("span", { className: `ins-badge tone-${j(e)}`, children: e });
}
function X({
  insight: e,
  selected: t,
  onSelect: o,
  showStatus: n = !0,
  showSeverity: s = !1,
  actions: l,
  now: m
}) {
  const v = e.severity === "critical" ? "is-critical" : e.severity === "warning" ? "is-warning" : "is-info", a = /* @__PURE__ */ d(M, { children: [
    /* @__PURE__ */ r("span", { className: `ins-dot ${v}`, role: "img", "aria-label": `severity: ${e.severity}` }),
    /* @__PURE__ */ d("span", { className: "ins-row-main", children: [
      /* @__PURE__ */ r("span", { className: "ins-row-title", children: e.title }),
      /* @__PURE__ */ d("span", { className: "ins-row-meta", children: [
        W(e.origin),
        " · ×",
        e.count
      ] })
    ] }),
    /* @__PURE__ */ d("span", { className: "ins-row-side", children: [
      s && /* @__PURE__ */ r(V, { severity: e.severity }),
      n && /* @__PURE__ */ r(U, { status: e.status }),
      /* @__PURE__ */ r("span", { className: "ins-time", children: P(e.last_ts, m) })
    ] })
  ] });
  return /* @__PURE__ */ d("li", { children: [
    o ? /* @__PURE__ */ r(
      "button",
      {
        type: "button",
        className: `ins-row${t ? " is-selected" : ""}`,
        "aria-selected": t,
        "aria-label": `select insight ${e.dedup_key}`,
        onClick: () => o(e.id),
        children: a
      }
    ) : /* @__PURE__ */ r("div", { className: `ins-row${t ? " is-selected" : ""}`, children: a }),
    l
  ] });
}
function Y({
  insight: e,
  actingOn: t = null,
  onAck: o,
  onResolve: n,
  onDismiss: s
}) {
  const l = t !== null;
  return /* @__PURE__ */ d("div", { className: "ins-actions", children: [
    s && /* @__PURE__ */ d("button", { type: "button", className: "ins-btn", onClick: s, disabled: l, children: [
      /* @__PURE__ */ r(O, { size: 13 }),
      "Dismiss"
    ] }),
    e.status === "open" && o && /* @__PURE__ */ d("button", { type: "button", className: "ins-btn", onClick: o, disabled: l, children: [
      t === "ack" ? /* @__PURE__ */ r(I, { size: 13, className: "ins-spin" }) : /* @__PURE__ */ r(D, { size: 13 }),
      "Ack"
    ] }),
    e.status !== "resolved" && n && /* @__PURE__ */ d(
      "button",
      {
        type: "button",
        className: "ins-btn is-primary",
        onClick: n,
        disabled: l,
        children: [
          t === "resolve" ? /* @__PURE__ */ r(I, { size: 13, className: "ins-spin" }) : /* @__PURE__ */ r(_, { size: 13 }),
          "Resolve"
        ]
      }
    ),
    e.status === "resolved" && /* @__PURE__ */ d("span", { className: "ins-badge tone-success", children: [
      /* @__PURE__ */ r(_, { size: 12 }),
      " Resolved"
    ] })
  ] });
}
const G = { limit: 20 };
function A({
  client: e,
  filter: t = G,
  title: o = "Insights",
  interactive: n = !1,
  showRefresh: s = !0,
  paged: l = !0,
  onSelect: m,
  now: v
}) {
  const a = B(e, t), [g, p] = f(/* @__PURE__ */ new Set()), [w, b] = f(null);
  function k(i, N) {
    b(N), a.act(i, N).finally(() => b(null));
  }
  const u = a.items.filter((i) => !g.has(i.id));
  return /* @__PURE__ */ d("div", { className: "ins-root", children: [
    /* @__PURE__ */ d("div", { className: "ins-header", children: [
      /* @__PURE__ */ d("h3", { className: "ins-header-title", children: [
        /* @__PURE__ */ r(L, { size: 15 }),
        o,
        u.length > 0 && /* @__PURE__ */ d("span", { className: "ins-header-count", children: [
          "(",
          u.length,
          ")"
        ] })
      ] }),
      s && /* @__PURE__ */ r("div", { className: "ins-header-actions", children: /* @__PURE__ */ r(
        "button",
        {
          type: "button",
          className: "ins-btn",
          onClick: () => void a.refresh(),
          disabled: a.loading,
          "aria-label": "Refresh insights",
          children: /* @__PURE__ */ r(I, { size: 13, className: a.loading ? "ins-spin" : void 0 })
        }
      ) })
    ] }),
    a.error && u.length === 0 ? /* @__PURE__ */ r("div", { className: "ins-error", role: "alert", children: a.error }) : u.length === 0 ? /* @__PURE__ */ d("div", { className: "ins-empty", children: [
      /* @__PURE__ */ r(L, { size: 16, className: a.loading ? "ins-spin" : void 0 }),
      a.loading ? "Loading insights…" : "No insights match this filter."
    ] }) : /* @__PURE__ */ r("ul", { className: "ins-list", children: u.map((i) => /* @__PURE__ */ r(
      X,
      {
        insight: i,
        onSelect: m,
        now: v,
        actions: n ? /* @__PURE__ */ r(
          Y,
          {
            insight: i,
            actingOn: a.actingOn === i.id ? w : null,
            onAck: i.status === "open" ? () => k(i.id, "ack") : void 0,
            onResolve: () => k(i.id, "resolve"),
            onDismiss: () => p((N) => new Set(N).add(i.id))
          }
        ) : void 0
      },
      i.id
    )) }),
    l && a.nextCursor !== null && u.length > 0 && /* @__PURE__ */ r("div", { className: "ins-more", children: /* @__PURE__ */ d(
      "button",
      {
        type: "button",
        className: "ins-btn",
        onClick: () => void a.loadMore(),
        disabled: a.loading,
        "aria-label": "Load more insights",
        children: [
          /* @__PURE__ */ r(I, { size: 13, className: a.loading ? "ins-spin" : void 0 }),
          "Load more"
        ]
      }
    ) })
  ] });
}
function q(e) {
  return /* @__PURE__ */ r(A, { ...e, interactive: !1 });
}
function ee(e) {
  return /* @__PURE__ */ r(A, { ...e, interactive: !0 });
}
function ne(e) {
  const t = [...e];
  function o() {
    return [...t].sort((n, s) => s.last_ts - n.last_ts || s.id.localeCompare(n.id));
  }
  return {
    async list(n) {
      let s = o();
      n.status && (s = s.filter((g) => g.status === n.status)), n.severity && (s = s.filter((g) => g.severity === n.severity)), n.origin_ref && (s = s.filter((g) => g.origin.ref.includes(n.origin_ref)));
      const l = n.limit ?? 50, m = s.slice(0, l), v = s.length > l ? { ts: m[m.length - 1].last_ts, id: m[m.length - 1].id } : void 0;
      return { items: m.map(({ evidence: g, ...p }) => p), next: v };
    },
    async get(n) {
      return t.find((s) => s.id === n) ?? null;
    },
    async ack(n) {
      const s = t.find((l) => l.id === n);
      s && (s.status = "acked");
    },
    async resolve(n) {
      const s = t.find((l) => l.id === n);
      s && (s.status = "resolved");
    },
    async occurrences() {
      return { items: [] };
    }
  };
}
function se() {
  const e = () => Promise.reject(new Error("Denied: mcp:insight.list:call"));
  return {
    list: e,
    get: e,
    ack: e,
    resolve: e,
    occurrences: e
  };
}
export {
  Y as InsightActions,
  X as InsightRow,
  ee as InsightsAckWidget,
  q as InsightsReadWidget,
  A as InsightsWidget,
  F as SEVERITY_ORDER,
  V as SeverityBadge,
  U as StatusBadge,
  se as denyClient,
  ne as memoryClient,
  W as originLine,
  Q as severityRank,
  T as severityTone,
  j as statusTone,
  P as timeAgo,
  Z as useInsight,
  B as useInsights
};
