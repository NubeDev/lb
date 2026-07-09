import { useState as f, useRef as x, useCallback as $, useEffect as z } from "react";
import { jsx as i, jsxs as d, Fragment as M } from "react/jsx-runtime";
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
function P(e, r = Date.now()) {
  const o = Math.max(1, Math.floor((r - e) / 1e3));
  if (o < 60) return `${o}s ago`;
  const n = Math.floor(o / 60);
  if (n < 60) return o % 60 ? `${n}m ${o % 60}s ago` : `${n}m ago`;
  const s = Math.floor(n / 60);
  return s < 24 ? n % 60 ? `${s}h ${n % 60}m ago` : `${s}h ago` : `${Math.floor(s / 24)}d ago`;
}
function W(e) {
  const r = `${e.kind}:${e.ref}`;
  return e.run ? `${r} · run:${e.run}` : r;
}
function B(e, r) {
  const [o, n] = f([]), [s, l] = f(null), [m, h] = f(!1), [t, N] = f(null), [y, w] = f(null), [p, k] = f(r), u = x(e);
  u.current = e;
  const a = $(async () => {
    h(!0);
    try {
      const c = await u.current.list({ ...p, cursor: void 0 });
      n(c.items), w(c.next ?? null), l(null);
    } catch (c) {
      l(c instanceof Error ? c.message : String(c));
    } finally {
      h(!1);
    }
  }, [p]), b = $(async () => {
    if (y) {
      h(!0);
      try {
        const c = await u.current.list({ ...p, cursor: y });
        n((C) => {
          const R = new Set(C.map((S) => S.id));
          return [...C, ...c.items.filter((S) => !R.has(S.id))];
        }), w(c.next ?? null), l(null);
      } catch (c) {
        l(c instanceof Error ? c.message : String(c));
      } finally {
        h(!1);
      }
    }
  }, [p, y]);
  z(() => {
    a();
  }, [a]);
  const E = x(a);
  E.current = a, z(() => {
    const c = u.current.subscribe;
    return c ? c(() => {
      E.current();
    }) : void 0;
  }, []);
  const v = $((c) => {
    k(c);
  }, []), g = $(
    async (c, C) => {
      N(c);
      try {
        C === "ack" ? await u.current.ack(c) : await u.current.resolve(c), await a();
      } catch (R) {
        l(R instanceof Error ? R.message : String(R));
      } finally {
        N(null);
      }
    },
    [a]
  );
  return {
    items: o,
    error: s,
    loading: m,
    actingOn: t,
    nextCursor: y,
    refresh: a,
    loadMore: b,
    setFilter: v,
    act: g
  };
}
function Z(e, r, o = 50) {
  const [n, s] = f(null), [l, m] = f(null), [h, t] = f(null), [N, y] = f(!0), [w, p] = f(null), [k, u] = f(0), a = x(e);
  a.current = e, z(() => {
    let v = !1;
    return (async () => {
      t(null), y(!0);
      try {
        const [g, c] = await Promise.all([
          a.current.get(r),
          a.current.occurrences(r, void 0, o)
        ]);
        if (v) return;
        s(g), m(c);
      } catch (g) {
        if (v) return;
        t(g instanceof Error ? g.message : String(g));
      } finally {
        v || y(!1);
      }
    })(), () => {
      v = !0;
    };
  }, [r, o, k]);
  const b = $(() => u((v) => v + 1), []), E = $(
    async (v) => {
      p(v), t(null);
      try {
        v === "ack" ? await a.current.ack(r) : await a.current.resolve(r), u((g) => g + 1);
      } catch (g) {
        t(g instanceof Error ? g.message : String(g));
      } finally {
        p(null);
      }
    },
    [r]
  );
  return { insight: n, occurrences: l, error: h, loading: N, actingOn: w, refresh: b, act: E };
}
function V({ severity: e }) {
  return /* @__PURE__ */ i("span", { className: `ins-badge tone-${T(e)}`, children: e });
}
function U({ status: e }) {
  return /* @__PURE__ */ i("span", { className: `ins-badge tone-${j(e)}`, children: e });
}
function X({
  insight: e,
  selected: r,
  onSelect: o,
  showStatus: n = !0,
  showSeverity: s = !0,
  actions: l,
  now: m
}) {
  const h = e.severity === "critical" ? "is-critical" : e.severity === "warning" ? "is-warning" : "is-info", t = /* @__PURE__ */ d(M, { children: [
    /* @__PURE__ */ i("span", { className: `ins-dot ${h}`, role: "img", "aria-label": `severity: ${e.severity}` }),
    /* @__PURE__ */ d("span", { className: "ins-row-main", children: [
      /* @__PURE__ */ i("span", { className: "ins-row-title", children: e.title }),
      /* @__PURE__ */ d("span", { className: "ins-row-meta", children: [
        W(e.origin),
        " · ×",
        e.count
      ] })
    ] }),
    /* @__PURE__ */ d("span", { className: "ins-row-side", children: [
      s && /* @__PURE__ */ i(V, { severity: e.severity }),
      n && /* @__PURE__ */ i(U, { status: e.status }),
      /* @__PURE__ */ i("span", { className: "ins-time", children: P(e.last_ts, m) })
    ] })
  ] });
  return /* @__PURE__ */ d("li", { children: [
    o ? /* @__PURE__ */ i(
      "button",
      {
        type: "button",
        className: `ins-row${r ? " is-selected" : ""}`,
        "aria-selected": r,
        "aria-label": `select insight ${e.dedup_key}`,
        onClick: () => o(e.id),
        children: t
      }
    ) : /* @__PURE__ */ i("div", { className: `ins-row${r ? " is-selected" : ""}`, children: t }),
    l
  ] });
}
function Y({
  insight: e,
  actingOn: r = null,
  onAck: o,
  onResolve: n,
  onDismiss: s
}) {
  const l = r !== null;
  return /* @__PURE__ */ d("div", { className: "ins-actions", children: [
    s && /* @__PURE__ */ d("button", { type: "button", className: "ins-btn", onClick: s, disabled: l, children: [
      /* @__PURE__ */ i(O, { size: 13 }),
      "Dismiss"
    ] }),
    e.status === "open" && o && /* @__PURE__ */ d("button", { type: "button", className: "ins-btn", onClick: o, disabled: l, children: [
      r === "ack" ? /* @__PURE__ */ i(I, { size: 13, className: "ins-spin" }) : /* @__PURE__ */ i(D, { size: 13 }),
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
          r === "resolve" ? /* @__PURE__ */ i(I, { size: 13, className: "ins-spin" }) : /* @__PURE__ */ i(_, { size: 13 }),
          "Resolve"
        ]
      }
    ),
    e.status === "resolved" && /* @__PURE__ */ d("span", { className: "ins-badge tone-success", children: [
      /* @__PURE__ */ i(_, { size: 12 }),
      " Resolved"
    ] })
  ] });
}
const G = { limit: 20 };
function A({
  client: e,
  filter: r = G,
  title: o = "Insights",
  interactive: n = !1,
  showRefresh: s = !0,
  paged: l = !0,
  onSelect: m,
  now: h
}) {
  const t = B(e, r), [N, y] = f(/* @__PURE__ */ new Set()), [w, p] = f(null);
  function k(a, b) {
    p(b), t.act(a, b).finally(() => p(null));
  }
  const u = t.items.filter((a) => !N.has(a.id));
  return /* @__PURE__ */ d("div", { className: "ins-root", children: [
    /* @__PURE__ */ d("div", { className: "ins-header", children: [
      /* @__PURE__ */ d("h3", { className: "ins-header-title", children: [
        /* @__PURE__ */ i(L, { size: 15 }),
        o,
        u.length > 0 && /* @__PURE__ */ d("span", { className: "ins-header-count", children: [
          "(",
          u.length,
          ")"
        ] })
      ] }),
      s && /* @__PURE__ */ i("div", { className: "ins-header-actions", children: /* @__PURE__ */ i(
        "button",
        {
          type: "button",
          className: "ins-btn",
          onClick: () => void t.refresh(),
          disabled: t.loading,
          "aria-label": "Refresh insights",
          children: /* @__PURE__ */ i(I, { size: 13, className: t.loading ? "ins-spin" : void 0 })
        }
      ) })
    ] }),
    t.error && u.length === 0 ? /* @__PURE__ */ i("div", { className: "ins-error", role: "alert", children: t.error }) : u.length === 0 ? /* @__PURE__ */ d("div", { className: "ins-empty", children: [
      /* @__PURE__ */ i(L, { size: 16, className: t.loading ? "ins-spin" : void 0 }),
      t.loading ? "Loading insights…" : "No insights match this filter."
    ] }) : /* @__PURE__ */ i("ul", { className: "ins-list", children: u.map((a) => /* @__PURE__ */ i(
      X,
      {
        insight: a,
        onSelect: m,
        now: h,
        actions: n ? /* @__PURE__ */ i(
          Y,
          {
            insight: a,
            actingOn: t.actingOn === a.id ? w : null,
            onAck: a.status === "open" ? () => k(a.id, "ack") : void 0,
            onResolve: () => k(a.id, "resolve"),
            onDismiss: () => y((b) => new Set(b).add(a.id))
          }
        ) : void 0
      },
      a.id
    )) }),
    l && t.nextCursor !== null && u.length > 0 && /* @__PURE__ */ i("div", { className: "ins-more", children: /* @__PURE__ */ d(
      "button",
      {
        type: "button",
        className: "ins-btn",
        onClick: () => void t.loadMore(),
        disabled: t.loading,
        "aria-label": "Load more insights",
        children: [
          /* @__PURE__ */ i(I, { size: 13, className: t.loading ? "ins-spin" : void 0 }),
          "Load more"
        ]
      }
    ) })
  ] });
}
function q(e) {
  return /* @__PURE__ */ i(A, { ...e, interactive: !1 });
}
function ee(e) {
  return /* @__PURE__ */ i(A, { ...e, interactive: !0 });
}
function ne(e) {
  const r = [...e];
  function o() {
    return [...r].sort((n, s) => s.last_ts - n.last_ts || s.id.localeCompare(n.id));
  }
  return {
    async list(n) {
      let s = o();
      n.status && (s = s.filter((t) => t.status === n.status)), n.severity && (s = s.filter((t) => t.severity === n.severity)), n.origin_ref && (s = s.filter((t) => t.origin.ref.includes(n.origin_ref)));
      const l = n.limit ?? 50, m = s.slice(0, l), h = s.length > l ? { ts: m[m.length - 1].last_ts, id: m[m.length - 1].id } : void 0;
      return { items: m, next: h };
    },
    async get(n) {
      return r.find((s) => s.id === n) ?? null;
    },
    async ack(n) {
      const s = r.find((l) => l.id === n);
      s && (s.status = "acked");
    },
    async resolve(n) {
      const s = r.find((l) => l.id === n);
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
