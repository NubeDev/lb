import { jsxs as fe, jsx as C, Fragment as un } from "react/jsx-runtime";
import de, { forwardRef as cn, createElement as It, useState as Bt, useRef as eo, useEffect as to } from "react";
import ir from "react-dom";
const ro = 12, fn = 56, no = {
  chart: "timeseries"
};
function Gt(t) {
  return no[t] ?? t;
}
function Cr(t) {
  var e;
  return t.sources && t.sources.length > 0 ? t.sources : (e = t.source) != null && e.tool ? [{ refId: "A", tool: t.source.tool, args: t.source.args, datasource: { type: "surreal" } }] : [];
}
function fs(t) {
  return Cr(t).find((e) => !e.hide) ?? Cr(t)[0];
}
function Je(t) {
  return Gt(t.view || t.widget_type || "timeseries");
}
function ds(t) {
  return t.fieldConfig ?? { defaults: {}, overrides: [] };
}
function Yt(t) {
  var r, n, o;
  return (r = t.title) != null && r.trim() ? t.title.trim() : (n = t.source) != null && n.tool ? t.source.tool : (o = t.action) != null && o.tool ? t.action.tool : Je(t) || t.widget_type || "widget";
}
function ps(t) {
  return "series" in t ? t.series : null;
}
function hs(t) {
  return "find" in t ? t.find.tags : [];
}
function gs() {
  return { defaults: {}, overrides: [] };
}
const ys = 12, ms = 1;
function Oe(t) {
  return Je(t) === "row";
}
function dn(t) {
  var e;
  return Oe(t) && ((e = t.options) == null ? void 0 : e.collapsed) === !0;
}
function oo(t) {
  const e = t.options ?? {};
  return {
    showCount: e.showCount !== !1,
    showLine: e.showLine !== !1,
    collapsed: e.collapsed === !0
  };
}
function ar(t) {
  return t.filter(Oe).sort((e, r) => e.y - r.y || e.x - r.x);
}
function Ve(t, e) {
  if (!Oe(e)) return [];
  const r = ar(t), n = r.findIndex((s) => s.i === e.i);
  if (n < 0) return [];
  const o = e.y, i = r[n + 1], a = i ? i.y : Number.POSITIVE_INFINITY;
  return t.filter((s) => !Oe(s) && s.y >= o && s.y < a);
}
function vs(t) {
  const e = ar(t), r = e.length > 0 ? e[0].y : Number.POSITIVE_INFINITY;
  return t.filter((n) => !Oe(n) && n.y < r);
}
function pn(t) {
  const e = ar(t).filter(dn);
  if (e.length === 0) return t;
  const r = /* @__PURE__ */ new Set();
  for (const n of e)
    for (const o of Ve(t, n)) r.add(o.i);
  return t.filter((n) => !r.has(n.i));
}
function io(t, e) {
  const r = new Map(e.map((o) => [o.i, o])), n = /* @__PURE__ */ new Map();
  for (const o of t) {
    if (!Oe(o)) continue;
    const i = r.get(o.i);
    if (!i) continue;
    const a = i.y - o.y;
    if (a !== 0)
      for (const s of Ve(t, o))
        r.has(s.i) || n.set(s.i, a);
  }
  return t.map((o) => {
    const i = r.get(o.i);
    if (i) return { ...o, x: i.x, y: i.y, w: i.w, h: i.h };
    const a = n.get(o.i);
    return a ? { ...o, y: o.y + a } : o;
  });
}
function ao(t) {
  if (!t || t.hideTimeOverride) return null;
  const e = [], r = t.timeFrom || t.relativeTime;
  return r && e.push(`Last ${r.replace(/^now-/, "")}`), t.timeShift && e.push(`${t.timeShift} earlier`), e.length > 0 ? e.join(", ") : null;
}
const so = "ext:*";
function bs(t) {
  const e = /* @__PURE__ */ new Map(), r = {
    register(n, o) {
      return e.set(Gt(n), o), r;
    },
    resolve(n) {
      const o = Gt(n), i = e.get(o);
      if (i) return i;
      if (o.startsWith("ext:")) return e.get(so);
    },
    resolveCell(n) {
      return r.resolve(Je(n));
    },
    views() {
      return [...e.keys()];
    }
  };
  if (t) for (const [n, o] of Object.entries(t)) r.register(n, o);
  return r;
}
function hn({ view: t }) {
  return /* @__PURE__ */ fe("div", { className: "lbdg-unknown", role: "note", "aria-label": `unknown view ${t}`, children: [
    /* @__PURE__ */ fe("span", { className: "lbdg-unknown-title", children: [
      "No renderer for “",
      t,
      "”"
    ] }),
    /* @__PURE__ */ C("span", { className: "lbdg-unknown-hint", children: "Register one on the widget registry to render this cell." })
  ] });
}
var lo = typeof globalThis < "u" ? globalThis : typeof window < "u" ? window : typeof global < "u" ? global : typeof self < "u" ? self : {};
function uo(t) {
  return t && t.__esModule && Object.prototype.hasOwnProperty.call(t, "default") ? t.default : t;
}
function co(t) {
  if (t.__esModule) return t;
  var e = t.default;
  if (typeof e == "function") {
    var r = function n() {
      return this instanceof n ? Reflect.construct(e, arguments, this.constructor) : e.apply(this, arguments);
    };
    r.prototype = e.prototype;
  } else r = {};
  return Object.defineProperty(r, "__esModule", { value: !0 }), Object.keys(t).forEach(function(n) {
    var o = Object.getOwnPropertyDescriptor(t, n);
    Object.defineProperty(r, n, o.get ? o : {
      enumerable: !0,
      get: function() {
        return t[n];
      }
    });
  }), r;
}
var gn = { exports: {} }, Ze = {}, Ft = { exports: {} };
(function(t, e) {
  (function(r, n) {
    n(e);
  })(lo, function(r) {
    function n(g) {
      return function(S, P, h, q, X, oe, V) {
        return g(S, P, V);
      };
    }
    function o(g) {
      return function(S, P, h, q) {
        if (!S || !P || typeof S != "object" || typeof P != "object")
          return g(S, P, h, q);
        var X = q.get(S), oe = q.get(P);
        if (X && oe)
          return X === P && oe === S;
        q.set(S, P), q.set(P, S);
        var V = g(S, P, h, q);
        return q.delete(S), q.delete(P), V;
      };
    }
    function i(g, b) {
      var S = {};
      for (var P in g)
        S[P] = g[P];
      for (var P in b)
        S[P] = b[P];
      return S;
    }
    function a(g) {
      return g.constructor === Object || g.constructor == null;
    }
    function s(g) {
      return typeof g.then == "function";
    }
    function l(g, b) {
      return g === b || g !== g && b !== b;
    }
    var u = "[object Arguments]", c = "[object Boolean]", f = "[object Date]", d = "[object RegExp]", v = "[object Map]", m = "[object Number]", O = "[object Object]", E = "[object Set]", _ = "[object String]", W = Object.prototype.toString;
    function R(g) {
      var b = g.areArraysEqual, S = g.areDatesEqual, P = g.areMapsEqual, h = g.areObjectsEqual, q = g.areRegExpsEqual, X = g.areSetsEqual, oe = g.createIsNestedEqual, V = oe(pe);
      function pe(F, K, he) {
        if (F === K)
          return !0;
        if (!F || !K || typeof F != "object" || typeof K != "object")
          return F !== F && K !== K;
        if (a(F) && a(K))
          return h(F, K, V, he);
        var Pr = Array.isArray(F), Dr = Array.isArray(K);
        if (Pr || Dr)
          return Pr === Dr && b(F, K, V, he);
        var ge = W.call(F);
        return ge !== W.call(K) ? !1 : ge === f ? S(F, K, V, he) : ge === d ? q(F, K, V, he) : ge === v ? P(F, K, V, he) : ge === E ? X(F, K, V, he) : ge === O || ge === u ? s(F) || s(K) ? !1 : h(F, K, V, he) : ge === c || ge === m || ge === _ ? l(F.valueOf(), K.valueOf()) : !1;
      }
      return pe;
    }
    function D(g, b, S, P) {
      var h = g.length;
      if (b.length !== h)
        return !1;
      for (; h-- > 0; )
        if (!S(g[h], b[h], h, h, g, b, P))
          return !1;
      return !0;
    }
    var p = o(D);
    function U(g, b) {
      return l(g.valueOf(), b.valueOf());
    }
    function Q(g, b, S, P) {
      var h = g.size === b.size;
      if (!h)
        return !1;
      if (!g.size)
        return !0;
      var q = {}, X = 0;
      return g.forEach(function(oe, V) {
        if (h) {
          var pe = !1, F = 0;
          b.forEach(function(K, he) {
            !pe && !q[F] && (pe = S(V, he, X, F, g, b, P) && S(oe, K, V, he, g, b, P)) && (q[F] = !0), F++;
          }), X++, h = pe;
        }
      }), h;
    }
    var j = o(Q), re = "_owner", ie = Object.prototype.hasOwnProperty;
    function le(g, b, S, P) {
      var h = Object.keys(g), q = h.length;
      if (Object.keys(b).length !== q)
        return !1;
      for (var X; q-- > 0; ) {
        if (X = h[q], X === re) {
          var oe = !!g.$$typeof, V = !!b.$$typeof;
          if ((oe || V) && oe !== V)
            return !1;
        }
        if (!ie.call(b, X) || !S(g[X], b[X], X, X, g, b, P))
          return !1;
      }
      return !0;
    }
    var _e = o(le);
    function ve(g, b) {
      return g.source === b.source && g.flags === b.flags;
    }
    function be(g, b, S, P) {
      var h = g.size === b.size;
      if (!h)
        return !1;
      if (!g.size)
        return !0;
      var q = {};
      return g.forEach(function(X, oe) {
        if (h) {
          var V = !1, pe = 0;
          b.forEach(function(F, K) {
            !V && !q[pe] && (V = S(X, F, oe, K, g, b, P)) && (q[pe] = !0), pe++;
          }), h = V;
        }
      }), h;
    }
    var Ge = o(be), ne = Object.freeze({
      areArraysEqual: D,
      areDatesEqual: U,
      areMapsEqual: Q,
      areObjectsEqual: le,
      areRegExpsEqual: ve,
      areSetsEqual: be,
      createIsNestedEqual: n
    }), se = Object.freeze({
      areArraysEqual: p,
      areDatesEqual: U,
      areMapsEqual: j,
      areObjectsEqual: _e,
      areRegExpsEqual: ve,
      areSetsEqual: Ge,
      createIsNestedEqual: n
    }), $e = R(ne);
    function Ye(g, b) {
      return $e(g, b, void 0);
    }
    var y = R(i(ne, { createIsNestedEqual: function() {
      return l;
    } }));
    function w(g, b) {
      return y(g, b, void 0);
    }
    var A = R(se);
    function $(g, b) {
      return A(g, b, /* @__PURE__ */ new WeakMap());
    }
    var z = R(i(se, {
      createIsNestedEqual: function() {
        return l;
      }
    }));
    function H(g, b) {
      return z(g, b, /* @__PURE__ */ new WeakMap());
    }
    function k(g) {
      return R(i(ne, g(ne)));
    }
    function L(g) {
      var b = R(i(se, g(se)));
      return function(S, P, h) {
        return h === void 0 && (h = /* @__PURE__ */ new WeakMap()), b(S, P, h);
      };
    }
    r.circularDeepEqual = $, r.circularShallowEqual = H, r.createCustomCircularEqual = L, r.createCustomEqual = k, r.deepEqual = Ye, r.sameValueZeroEqual = l, r.shallowEqual = w, Object.defineProperty(r, "__esModule", { value: !0 });
  });
})(Ft, Ft.exports);
var sr = Ft.exports, Ut = { exports: {} };
function yn(t) {
  var e, r, n = "";
  if (typeof t == "string" || typeof t == "number") n += t;
  else if (typeof t == "object") if (Array.isArray(t)) {
    var o = t.length;
    for (e = 0; e < o; e++) t[e] && (r = yn(t[e])) && (n && (n += " "), n += r);
  } else for (r in t) t[r] && (n && (n += " "), n += r);
  return n;
}
function zr() {
  for (var t, e, r = 0, n = "", o = arguments.length; r < o; r++) (t = arguments[r]) && (e = yn(t)) && (n && (n += " "), n += e);
  return n;
}
Ut.exports = zr, Ut.exports.clsx = zr;
var ct = Ut.exports, N = {}, fo = function(e, r, n) {
  return e === r ? !0 : e.className === r.className && n(e.style, r.style) && e.width === r.width && e.autoSize === r.autoSize && e.cols === r.cols && e.draggableCancel === r.draggableCancel && e.draggableHandle === r.draggableHandle && n(e.verticalCompact, r.verticalCompact) && n(e.compactType, r.compactType) && n(e.layout, r.layout) && n(e.margin, r.margin) && n(e.containerPadding, r.containerPadding) && e.rowHeight === r.rowHeight && e.maxRows === r.maxRows && e.isBounded === r.isBounded && e.isDraggable === r.isDraggable && e.isResizable === r.isResizable && e.allowOverlap === r.allowOverlap && e.preventCollision === r.preventCollision && e.useCSSTransforms === r.useCSSTransforms && e.transformScale === r.transformScale && e.isDroppable === r.isDroppable && n(e.resizeHandles, r.resizeHandles) && n(e.resizeHandle, r.resizeHandle) && e.onLayoutChange === r.onLayoutChange && e.onDragStart === r.onDragStart && e.onDrag === r.onDrag && e.onDragStop === r.onDragStop && e.onResizeStart === r.onResizeStart && e.onResize === r.onResize && e.onResizeStop === r.onResizeStop && e.onDrop === r.onDrop && n(e.droppingItem, r.droppingItem) && n(e.innerRef, r.innerRef);
};
Object.defineProperty(N, "__esModule", {
  value: !0
});
N.bottom = ft;
N.childrenEqual = bo;
N.cloneLayout = mn;
N.cloneLayoutItem = Te;
N.collides = dt;
N.compact = bn;
N.compactItem = wn;
N.compactType = To;
N.correctBounds = On;
N.fastPositionEqual = wo;
N.fastRGLPropsEqual = void 0;
N.getAllCollisions = Sn;
N.getFirstCollision = ze;
N.getLayoutItem = lr;
N.getStatics = ur;
N.modifyLayout = vn;
N.moveElement = Ue;
N.moveElementAwayFromCollision = Vt;
N.noop = void 0;
N.perc = So;
N.resizeItemInDirection = Do;
N.setTopLeft = zo;
N.setTransform = Co;
N.sortLayoutItems = gr;
N.sortLayoutItemsByColRow = Pn;
N.sortLayoutItemsByRowCol = xn;
N.synchronizeLayoutWithChildren = jo;
N.validateLayout = Dn;
N.withLayoutItem = vo;
var jr = sr, Fe = po(de);
function po(t) {
  return t && t.__esModule ? t : { default: t };
}
function Tr(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function at(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? Tr(Object(r), !0).forEach(function(n) {
      ho(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : Tr(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function ho(t, e, r) {
  return (e = go(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function go(t) {
  var e = yo(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function yo(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
const mo = process.env.NODE_ENV === "production";
function ft(t) {
  let e = 0, r;
  for (let n = 0, o = t.length; n < o; n++)
    r = t[n].y + t[n].h, r > e && (e = r);
  return e;
}
function mn(t) {
  const e = Array(t.length);
  for (let r = 0, n = t.length; r < n; r++)
    e[r] = Te(t[r]);
  return e;
}
function vn(t, e) {
  const r = Array(t.length);
  for (let n = 0, o = t.length; n < o; n++)
    e.i === t[n].i ? r[n] = e : r[n] = t[n];
  return r;
}
function vo(t, e, r) {
  let n = lr(t, e);
  return n ? (n = r(Te(n)), t = vn(t, n), [t, n]) : [t, null];
}
function Te(t) {
  return {
    w: t.w,
    h: t.h,
    x: t.x,
    y: t.y,
    i: t.i,
    minW: t.minW,
    maxW: t.maxW,
    minH: t.minH,
    maxH: t.maxH,
    moved: !!t.moved,
    static: !!t.static,
    // These can be null/undefined
    isDraggable: t.isDraggable,
    isResizable: t.isResizable,
    resizeHandles: t.resizeHandles,
    isBounded: t.isBounded
  };
}
function bo(t, e) {
  return (0, jr.deepEqual)(Fe.default.Children.map(t, (r) => r == null ? void 0 : r.key), Fe.default.Children.map(e, (r) => r == null ? void 0 : r.key)) && (0, jr.deepEqual)(Fe.default.Children.map(t, (r) => r == null ? void 0 : r.props["data-grid"]), Fe.default.Children.map(e, (r) => r == null ? void 0 : r.props["data-grid"]));
}
N.fastRGLPropsEqual = fo;
function wo(t, e) {
  return t.left === e.left && t.top === e.top && t.width === e.width && t.height === e.height;
}
function dt(t, e) {
  return !(t.i === e.i || t.x + t.w <= e.x || t.x >= e.x + e.w || t.y + t.h <= e.y || t.y >= e.y + e.h);
}
function bn(t, e, r, n) {
  const o = ur(t);
  let i = ft(o);
  const a = gr(t, e), s = Array(t.length);
  for (let l = 0, u = a.length; l < u; l++) {
    let c = Te(a[l]);
    c.static || (c = wn(o, c, e, r, a, n, i), i = Math.max(i, c.y + c.h), o.push(c)), s[t.indexOf(a[l])] = c, c.moved = !1;
  }
  return s;
}
const Oo = {
  x: "w",
  y: "h"
};
function Xt(t, e, r, n) {
  const o = Oo[n];
  e[n] += 1;
  const i = t.map((a) => a.i).indexOf(e.i);
  for (let a = i + 1; a < t.length; a++) {
    const s = t[a];
    if (!s.static) {
      if (s.y > e.y + e.h) break;
      dt(e, s) && Xt(t, s, r + e[o], n);
    }
  }
  e[n] = r;
}
function wn(t, e, r, n, o, i, a) {
  const s = r === "vertical", l = r === "horizontal";
  if (s)
    for (typeof a == "number" ? e.y = Math.min(a, e.y) : e.y = Math.min(ft(t), e.y); e.y > 0 && !ze(t, e); )
      e.y--;
  else if (l)
    for (; e.x > 0 && !ze(t, e); )
      e.x--;
  let u;
  for (; (u = ze(t, e)) && !(r === null && i); )
    if (l ? Xt(o, e, u.x + u.w, "x") : Xt(o, e, u.y + u.h, "y"), l && e.x + e.w > n)
      for (e.x = n - e.w, e.y++; e.x > 0 && !ze(t, e); )
        e.x--;
  return e.y = Math.max(e.y, 0), e.x = Math.max(e.x, 0), e;
}
function On(t, e) {
  const r = ur(t);
  for (let n = 0, o = t.length; n < o; n++) {
    const i = t[n];
    if (i.x + i.w > e.cols && (i.x = e.cols - i.w), i.x < 0 && (i.x = 0, i.w = e.cols), !i.static) r.push(i);
    else
      for (; ze(r, i); )
        i.y++;
  }
  return t;
}
function lr(t, e) {
  for (let r = 0, n = t.length; r < n; r++)
    if (t[r].i === e) return t[r];
}
function ze(t, e) {
  for (let r = 0, n = t.length; r < n; r++)
    if (dt(t[r], e)) return t[r];
}
function Sn(t, e) {
  return t.filter((r) => dt(r, e));
}
function ur(t) {
  return t.filter((e) => e.static);
}
function Ue(t, e, r, n, o, i, a, s, l) {
  if (e.static && e.isDraggable !== !0 || e.y === n && e.x === r) return t;
  "Moving element ".concat(e.i, " to [").concat(String(r), ",").concat(String(n), "] from [").concat(e.x, ",").concat(e.y, "]");
  const u = e.x, c = e.y;
  typeof r == "number" && (e.x = r), typeof n == "number" && (e.y = n), e.moved = !0;
  let f = gr(t, a);
  (a === "vertical" && typeof n == "number" ? c >= n : a === "horizontal" && typeof r == "number" ? u >= r : !1) && (f = f.reverse());
  const v = Sn(f, e), m = v.length > 0;
  if (m && l)
    return mn(t);
  if (m && i)
    return "Collision prevented on ".concat(e.i, ", reverting."), e.x = u, e.y = c, e.moved = !1, t;
  for (let O = 0, E = v.length; O < E; O++) {
    const _ = v[O];
    "Resolving collision between ".concat(e.i, " at [").concat(e.x, ",").concat(e.y, "] and ").concat(_.i, " at [").concat(_.x, ",").concat(_.y, "]"), !_.moved && (_.static ? t = Vt(t, _, e, o, a) : t = Vt(t, e, _, o, a));
  }
  return t;
}
function Vt(t, e, r, n, o, i) {
  const a = o === "horizontal", s = o === "vertical", l = e.static;
  if (n) {
    n = !1;
    const f = {
      x: a ? Math.max(e.x - r.w, 0) : r.x,
      y: s ? Math.max(e.y - r.h, 0) : r.y,
      w: r.w,
      h: r.h,
      i: "-1"
    }, d = ze(t, f), v = d && d.y + d.h > e.y, m = d && e.x + e.w > d.x;
    if (d) {
      if (v && s)
        return Ue(t, r, void 0, r.y + 1, n, l, o);
      if (v && o == null)
        return e.y = r.y, r.y = r.y + r.h, t;
      if (m && a)
        return Ue(t, e, r.x, void 0, n, l, o);
    } else return "Doing reverse collision on ".concat(r.i, " up to [").concat(f.x, ",").concat(f.y, "]."), Ue(t, r, a ? f.x : void 0, s ? f.y : void 0, n, l, o);
  }
  const u = a ? r.x + 1 : void 0, c = s ? r.y + 1 : void 0;
  return u == null && c == null ? t : Ue(t, r, a ? r.x + 1 : void 0, s ? r.y + 1 : void 0, n, l, o);
}
function So(t) {
  return t * 100 + "%";
}
const _n = (t, e, r, n) => t + r > n ? e : r, Rn = (t, e, r) => t < 0 ? e : r, En = (t) => Math.max(0, t), cr = (t) => Math.max(0, t), fr = (t, e, r) => {
  let {
    left: n,
    height: o,
    width: i
  } = e;
  const a = t.top - (o - t.height);
  return {
    left: n,
    width: i,
    height: Rn(a, t.height, o),
    top: cr(a)
  };
}, dr = (t, e, r) => {
  let {
    top: n,
    left: o,
    height: i,
    width: a
  } = e;
  return {
    top: n,
    height: i,
    width: _n(t.left, t.width, a, r),
    left: En(o)
  };
}, pr = (t, e, r) => {
  let {
    top: n,
    height: o,
    width: i
  } = e;
  const a = t.left - (i - t.width);
  return {
    height: o,
    width: a < 0 ? t.width : _n(t.left, t.width, i, r),
    top: cr(n),
    left: En(a)
  };
}, hr = (t, e, r) => {
  let {
    top: n,
    left: o,
    height: i,
    width: a
  } = e;
  return {
    width: a,
    left: o,
    height: Rn(n, t.height, i),
    top: cr(n)
  };
}, _o = function() {
  return fr(arguments.length <= 0 ? void 0 : arguments[0], dr(...arguments));
}, Ro = function() {
  return fr(arguments.length <= 0 ? void 0 : arguments[0], pr(...arguments));
}, Eo = function() {
  return hr(arguments.length <= 0 ? void 0 : arguments[0], dr(...arguments));
}, xo = function() {
  return hr(arguments.length <= 0 ? void 0 : arguments[0], pr(...arguments));
}, Po = {
  n: fr,
  ne: _o,
  e: dr,
  se: Eo,
  s: hr,
  sw: xo,
  w: pr,
  nw: Ro
};
function Do(t, e, r, n) {
  const o = Po[t];
  return o ? o(e, at(at({}, e), r), n) : r;
}
function Co(t) {
  let {
    top: e,
    left: r,
    width: n,
    height: o
  } = t;
  const i = "translate(".concat(r, "px,").concat(e, "px)");
  return {
    transform: i,
    WebkitTransform: i,
    MozTransform: i,
    msTransform: i,
    OTransform: i,
    width: "".concat(n, "px"),
    height: "".concat(o, "px"),
    position: "absolute"
  };
}
function zo(t) {
  let {
    top: e,
    left: r,
    width: n,
    height: o
  } = t;
  return {
    top: "".concat(e, "px"),
    left: "".concat(r, "px"),
    width: "".concat(n, "px"),
    height: "".concat(o, "px"),
    position: "absolute"
  };
}
function gr(t, e) {
  return e === "horizontal" ? Pn(t) : e === "vertical" ? xn(t) : t;
}
function xn(t) {
  return t.slice(0).sort(function(e, r) {
    return e.y > r.y || e.y === r.y && e.x > r.x ? 1 : e.y === r.y && e.x === r.x ? 0 : -1;
  });
}
function Pn(t) {
  return t.slice(0).sort(function(e, r) {
    return e.x > r.x || e.x === r.x && e.y > r.y ? 1 : -1;
  });
}
function jo(t, e, r, n, o) {
  t = t || [];
  const i = [];
  Fe.default.Children.forEach(e, (s) => {
    if ((s == null ? void 0 : s.key) == null) return;
    const l = lr(t, String(s.key)), u = s.props["data-grid"];
    l && u == null ? i.push(Te(l)) : u ? (mo || Dn([u], "ReactGridLayout.children"), i.push(Te(at(at({}, u), {}, {
      i: s.key
    })))) : i.push(Te({
      w: 1,
      h: 1,
      x: 0,
      y: ft(i),
      i: String(s.key)
    }));
  });
  const a = On(i, {
    cols: r
  });
  return o ? a : bn(a, n, r);
}
function Dn(t) {
  let e = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : "Layout";
  const r = ["x", "y", "w", "h"];
  if (!Array.isArray(t)) throw new Error(e + " must be an array!");
  for (let n = 0, o = t.length; n < o; n++) {
    const i = t[n];
    for (let a = 0; a < r.length; a++) {
      const s = r[a], l = i[s];
      if (typeof l != "number" || Number.isNaN(l))
        throw new Error("ReactGridLayout: ".concat(e, "[").concat(n, "].").concat(s, " must be a number! Received: ").concat(l, " (").concat(typeof l, ")"));
    }
    if (typeof i.i < "u" && typeof i.i != "string")
      throw new Error("ReactGridLayout: ".concat(e, "[").concat(n, "].i must be a string! Received: ").concat(i.i, " (").concat(typeof i.i, ")"));
  }
}
function To(t) {
  const {
    verticalCompact: e,
    compactType: r
  } = t || {};
  return e === !1 ? null : r;
}
const Mo = () => {
};
N.noop = Mo;
var me = {};
Object.defineProperty(me, "__esModule", {
  value: !0
});
me.calcGridColWidth = pt;
me.calcGridItemPosition = ko;
me.calcGridItemWHPx = Kt;
me.calcWH = Lo;
me.calcXY = $o;
me.clamp = je;
function pt(t) {
  const {
    margin: e,
    containerPadding: r,
    containerWidth: n,
    cols: o
  } = t;
  return (n - e[0] * (o - 1) - r[0] * 2) / o;
}
function Kt(t, e, r) {
  return Number.isFinite(t) ? Math.round(e * t + Math.max(0, t - 1) * r) : t;
}
function ko(t, e, r, n, o, i) {
  const {
    margin: a,
    containerPadding: s,
    rowHeight: l
  } = t, u = pt(t), c = {};
  return i && i.resizing ? (c.width = Math.round(i.resizing.width), c.height = Math.round(i.resizing.height)) : (c.width = Kt(n, u, a[0]), c.height = Kt(o, l, a[1])), i && i.dragging ? (c.top = Math.round(i.dragging.top), c.left = Math.round(i.dragging.left)) : i && i.resizing && typeof i.resizing.top == "number" && typeof i.resizing.left == "number" ? (c.top = Math.round(i.resizing.top), c.left = Math.round(i.resizing.left)) : (c.top = Math.round((l + a[1]) * r + s[1]), c.left = Math.round((u + a[0]) * e + s[0])), c;
}
function $o(t, e, r, n, o) {
  const {
    margin: i,
    containerPadding: a,
    cols: s,
    rowHeight: l,
    maxRows: u
  } = t, c = pt(t);
  let f = Math.round((r - a[0]) / (c + i[0])), d = Math.round((e - a[1]) / (l + i[1]));
  return f = je(f, 0, s - n), d = je(d, 0, u - o), {
    x: f,
    y: d
  };
}
function Lo(t, e, r, n, o, i) {
  const {
    margin: a,
    maxRows: s,
    cols: l,
    rowHeight: u
  } = t, c = pt(t);
  let f = Math.round((e + a[0]) / (c + a[0])), d = Math.round((r + a[1]) / (u + a[1])), v = je(f, 0, l - n), m = je(d, 0, s - o);
  return ["sw", "w", "nw"].indexOf(i) !== -1 && (v = je(f, 0, l)), ["nw", "n", "ne"].indexOf(i) !== -1 && (m = je(d, 0, s)), {
    w: v,
    h: m
  };
}
function je(t, e, r) {
  return Math.max(Math.min(t, r), e);
}
var ht = {}, Jt = { exports: {} }, tt = { exports: {} }, B = {};
/** @license React v16.13.1
 * react-is.production.min.js
 *
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var Mr;
function No() {
  if (Mr) return B;
  Mr = 1;
  var t = typeof Symbol == "function" && Symbol.for, e = t ? Symbol.for("react.element") : 60103, r = t ? Symbol.for("react.portal") : 60106, n = t ? Symbol.for("react.fragment") : 60107, o = t ? Symbol.for("react.strict_mode") : 60108, i = t ? Symbol.for("react.profiler") : 60114, a = t ? Symbol.for("react.provider") : 60109, s = t ? Symbol.for("react.context") : 60110, l = t ? Symbol.for("react.async_mode") : 60111, u = t ? Symbol.for("react.concurrent_mode") : 60111, c = t ? Symbol.for("react.forward_ref") : 60112, f = t ? Symbol.for("react.suspense") : 60113, d = t ? Symbol.for("react.suspense_list") : 60120, v = t ? Symbol.for("react.memo") : 60115, m = t ? Symbol.for("react.lazy") : 60116, O = t ? Symbol.for("react.block") : 60121, E = t ? Symbol.for("react.fundamental") : 60117, _ = t ? Symbol.for("react.responder") : 60118, W = t ? Symbol.for("react.scope") : 60119;
  function R(p) {
    if (typeof p == "object" && p !== null) {
      var U = p.$$typeof;
      switch (U) {
        case e:
          switch (p = p.type, p) {
            case l:
            case u:
            case n:
            case i:
            case o:
            case f:
              return p;
            default:
              switch (p = p && p.$$typeof, p) {
                case s:
                case c:
                case m:
                case v:
                case a:
                  return p;
                default:
                  return U;
              }
          }
        case r:
          return U;
      }
    }
  }
  function D(p) {
    return R(p) === u;
  }
  return B.AsyncMode = l, B.ConcurrentMode = u, B.ContextConsumer = s, B.ContextProvider = a, B.Element = e, B.ForwardRef = c, B.Fragment = n, B.Lazy = m, B.Memo = v, B.Portal = r, B.Profiler = i, B.StrictMode = o, B.Suspense = f, B.isAsyncMode = function(p) {
    return D(p) || R(p) === l;
  }, B.isConcurrentMode = D, B.isContextConsumer = function(p) {
    return R(p) === s;
  }, B.isContextProvider = function(p) {
    return R(p) === a;
  }, B.isElement = function(p) {
    return typeof p == "object" && p !== null && p.$$typeof === e;
  }, B.isForwardRef = function(p) {
    return R(p) === c;
  }, B.isFragment = function(p) {
    return R(p) === n;
  }, B.isLazy = function(p) {
    return R(p) === m;
  }, B.isMemo = function(p) {
    return R(p) === v;
  }, B.isPortal = function(p) {
    return R(p) === r;
  }, B.isProfiler = function(p) {
    return R(p) === i;
  }, B.isStrictMode = function(p) {
    return R(p) === o;
  }, B.isSuspense = function(p) {
    return R(p) === f;
  }, B.isValidElementType = function(p) {
    return typeof p == "string" || typeof p == "function" || p === n || p === u || p === i || p === o || p === f || p === d || typeof p == "object" && p !== null && (p.$$typeof === m || p.$$typeof === v || p.$$typeof === a || p.$$typeof === s || p.$$typeof === c || p.$$typeof === E || p.$$typeof === _ || p.$$typeof === W || p.$$typeof === O);
  }, B.typeOf = R, B;
}
var G = {};
/** @license React v16.13.1
 * react-is.development.js
 *
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var kr;
function Ao() {
  return kr || (kr = 1, process.env.NODE_ENV !== "production" && function() {
    var t = typeof Symbol == "function" && Symbol.for, e = t ? Symbol.for("react.element") : 60103, r = t ? Symbol.for("react.portal") : 60106, n = t ? Symbol.for("react.fragment") : 60107, o = t ? Symbol.for("react.strict_mode") : 60108, i = t ? Symbol.for("react.profiler") : 60114, a = t ? Symbol.for("react.provider") : 60109, s = t ? Symbol.for("react.context") : 60110, l = t ? Symbol.for("react.async_mode") : 60111, u = t ? Symbol.for("react.concurrent_mode") : 60111, c = t ? Symbol.for("react.forward_ref") : 60112, f = t ? Symbol.for("react.suspense") : 60113, d = t ? Symbol.for("react.suspense_list") : 60120, v = t ? Symbol.for("react.memo") : 60115, m = t ? Symbol.for("react.lazy") : 60116, O = t ? Symbol.for("react.block") : 60121, E = t ? Symbol.for("react.fundamental") : 60117, _ = t ? Symbol.for("react.responder") : 60118, W = t ? Symbol.for("react.scope") : 60119;
    function R(h) {
      return typeof h == "string" || typeof h == "function" || // Note: its typeof might be other than 'symbol' or 'number' if it's a polyfill.
      h === n || h === u || h === i || h === o || h === f || h === d || typeof h == "object" && h !== null && (h.$$typeof === m || h.$$typeof === v || h.$$typeof === a || h.$$typeof === s || h.$$typeof === c || h.$$typeof === E || h.$$typeof === _ || h.$$typeof === W || h.$$typeof === O);
    }
    function D(h) {
      if (typeof h == "object" && h !== null) {
        var q = h.$$typeof;
        switch (q) {
          case e:
            var X = h.type;
            switch (X) {
              case l:
              case u:
              case n:
              case i:
              case o:
              case f:
                return X;
              default:
                var oe = X && X.$$typeof;
                switch (oe) {
                  case s:
                  case c:
                  case m:
                  case v:
                  case a:
                    return oe;
                  default:
                    return q;
                }
            }
          case r:
            return q;
        }
      }
    }
    var p = l, U = u, Q = s, j = a, re = e, ie = c, le = n, _e = m, ve = v, be = r, Ge = i, ne = o, se = f, $e = !1;
    function Ye(h) {
      return $e || ($e = !0, console.warn("The ReactIs.isAsyncMode() alias has been deprecated, and will be removed in React 17+. Update your code to use ReactIs.isConcurrentMode() instead. It has the exact same API.")), y(h) || D(h) === l;
    }
    function y(h) {
      return D(h) === u;
    }
    function w(h) {
      return D(h) === s;
    }
    function A(h) {
      return D(h) === a;
    }
    function $(h) {
      return typeof h == "object" && h !== null && h.$$typeof === e;
    }
    function z(h) {
      return D(h) === c;
    }
    function H(h) {
      return D(h) === n;
    }
    function k(h) {
      return D(h) === m;
    }
    function L(h) {
      return D(h) === v;
    }
    function g(h) {
      return D(h) === r;
    }
    function b(h) {
      return D(h) === i;
    }
    function S(h) {
      return D(h) === o;
    }
    function P(h) {
      return D(h) === f;
    }
    G.AsyncMode = p, G.ConcurrentMode = U, G.ContextConsumer = Q, G.ContextProvider = j, G.Element = re, G.ForwardRef = ie, G.Fragment = le, G.Lazy = _e, G.Memo = ve, G.Portal = be, G.Profiler = Ge, G.StrictMode = ne, G.Suspense = se, G.isAsyncMode = Ye, G.isConcurrentMode = y, G.isContextConsumer = w, G.isContextProvider = A, G.isElement = $, G.isForwardRef = z, G.isFragment = H, G.isLazy = k, G.isMemo = L, G.isPortal = g, G.isProfiler = b, G.isStrictMode = S, G.isSuspense = P, G.isValidElementType = R, G.typeOf = D;
  }()), G;
}
var $r;
function Cn() {
  return $r || ($r = 1, process.env.NODE_ENV === "production" ? tt.exports = No() : tt.exports = Ao()), tt.exports;
}
/*
object-assign
(c) Sindre Sorhus
@license MIT
*/
var _t, Lr;
function Ho() {
  if (Lr) return _t;
  Lr = 1;
  var t = Object.getOwnPropertySymbols, e = Object.prototype.hasOwnProperty, r = Object.prototype.propertyIsEnumerable;
  function n(i) {
    if (i == null)
      throw new TypeError("Object.assign cannot be called with null or undefined");
    return Object(i);
  }
  function o() {
    try {
      if (!Object.assign)
        return !1;
      var i = new String("abc");
      if (i[5] = "de", Object.getOwnPropertyNames(i)[0] === "5")
        return !1;
      for (var a = {}, s = 0; s < 10; s++)
        a["_" + String.fromCharCode(s)] = s;
      var l = Object.getOwnPropertyNames(a).map(function(c) {
        return a[c];
      });
      if (l.join("") !== "0123456789")
        return !1;
      var u = {};
      return "abcdefghijklmnopqrst".split("").forEach(function(c) {
        u[c] = c;
      }), Object.keys(Object.assign({}, u)).join("") === "abcdefghijklmnopqrst";
    } catch {
      return !1;
    }
  }
  return _t = o() ? Object.assign : function(i, a) {
    for (var s, l = n(i), u, c = 1; c < arguments.length; c++) {
      s = Object(arguments[c]);
      for (var f in s)
        e.call(s, f) && (l[f] = s[f]);
      if (t) {
        u = t(s);
        for (var d = 0; d < u.length; d++)
          r.call(s, u[d]) && (l[u[d]] = s[u[d]]);
      }
    }
    return l;
  }, _t;
}
var Rt, Nr;
function yr() {
  if (Nr) return Rt;
  Nr = 1;
  var t = "SECRET_DO_NOT_PASS_THIS_OR_YOU_WILL_BE_FIRED";
  return Rt = t, Rt;
}
var Et, Ar;
function zn() {
  return Ar || (Ar = 1, Et = Function.call.bind(Object.prototype.hasOwnProperty)), Et;
}
var xt, Hr;
function Wo() {
  if (Hr) return xt;
  Hr = 1;
  var t = function() {
  };
  if (process.env.NODE_ENV !== "production") {
    var e = yr(), r = {}, n = zn();
    t = function(i) {
      var a = "Warning: " + i;
      typeof console < "u" && console.error(a);
      try {
        throw new Error(a);
      } catch {
      }
    };
  }
  function o(i, a, s, l, u) {
    if (process.env.NODE_ENV !== "production") {
      for (var c in i)
        if (n(i, c)) {
          var f;
          try {
            if (typeof i[c] != "function") {
              var d = Error(
                (l || "React class") + ": " + s + " type `" + c + "` is invalid; it must be a function, usually from the `prop-types` package, but received `" + typeof i[c] + "`.This often happens because of typos such as `PropTypes.function` instead of `PropTypes.func`."
              );
              throw d.name = "Invariant Violation", d;
            }
            f = i[c](a, c, l, s, null, e);
          } catch (m) {
            f = m;
          }
          if (f && !(f instanceof Error) && t(
            (l || "React class") + ": type specification of " + s + " `" + c + "` is invalid; the type checker function must return `null` or an `Error` but returned a " + typeof f + ". You may have forgotten to pass an argument to the type checker creator (arrayOf, instanceOf, objectOf, oneOf, oneOfType, and shape all require an argument)."
          ), f instanceof Error && !(f.message in r)) {
            r[f.message] = !0;
            var v = u ? u() : "";
            t(
              "Failed " + s + " type: " + f.message + (v ?? "")
            );
          }
        }
    }
  }
  return o.resetWarningCache = function() {
    process.env.NODE_ENV !== "production" && (r = {});
  }, xt = o, xt;
}
var Pt, Wr;
function qo() {
  if (Wr) return Pt;
  Wr = 1;
  var t = Cn(), e = Ho(), r = yr(), n = zn(), o = Wo(), i = function() {
  };
  process.env.NODE_ENV !== "production" && (i = function(s) {
    var l = "Warning: " + s;
    typeof console < "u" && console.error(l);
    try {
      throw new Error(l);
    } catch {
    }
  });
  function a() {
    return null;
  }
  return Pt = function(s, l) {
    var u = typeof Symbol == "function" && Symbol.iterator, c = "@@iterator";
    function f(y) {
      var w = y && (u && y[u] || y[c]);
      if (typeof w == "function")
        return w;
    }
    var d = "<<anonymous>>", v = {
      array: _("array"),
      bigint: _("bigint"),
      bool: _("boolean"),
      func: _("function"),
      number: _("number"),
      object: _("object"),
      string: _("string"),
      symbol: _("symbol"),
      any: W(),
      arrayOf: R,
      element: D(),
      elementType: p(),
      instanceOf: U,
      node: ie(),
      objectOf: j,
      oneOf: Q,
      oneOfType: re,
      shape: _e,
      exact: ve
    };
    function m(y, w) {
      return y === w ? y !== 0 || 1 / y === 1 / w : y !== y && w !== w;
    }
    function O(y, w) {
      this.message = y, this.data = w && typeof w == "object" ? w : {}, this.stack = "";
    }
    O.prototype = Error.prototype;
    function E(y) {
      if (process.env.NODE_ENV !== "production")
        var w = {}, A = 0;
      function $(H, k, L, g, b, S, P) {
        if (g = g || d, S = S || L, P !== r) {
          if (l) {
            var h = new Error(
              "Calling PropTypes validators directly is not supported by the `prop-types` package. Use `PropTypes.checkPropTypes()` to call them. Read more at http://fb.me/use-check-prop-types"
            );
            throw h.name = "Invariant Violation", h;
          } else if (process.env.NODE_ENV !== "production" && typeof console < "u") {
            var q = g + ":" + L;
            !w[q] && // Avoid spamming the console because they are often not actionable except for lib authors
            A < 3 && (i(
              "You are manually calling a React.PropTypes validation function for the `" + S + "` prop on `" + g + "`. This is deprecated and will throw in the standalone `prop-types` package. You may be seeing this warning due to a third-party PropTypes library. See https://fb.me/react-warning-dont-call-proptypes for details."
            ), w[q] = !0, A++);
          }
        }
        return k[L] == null ? H ? k[L] === null ? new O("The " + b + " `" + S + "` is marked as required " + ("in `" + g + "`, but its value is `null`.")) : new O("The " + b + " `" + S + "` is marked as required in " + ("`" + g + "`, but its value is `undefined`.")) : null : y(k, L, g, b, S);
      }
      var z = $.bind(null, !1);
      return z.isRequired = $.bind(null, !0), z;
    }
    function _(y) {
      function w(A, $, z, H, k, L) {
        var g = A[$], b = ne(g);
        if (b !== y) {
          var S = se(g);
          return new O(
            "Invalid " + H + " `" + k + "` of type " + ("`" + S + "` supplied to `" + z + "`, expected ") + ("`" + y + "`."),
            { expectedType: y }
          );
        }
        return null;
      }
      return E(w);
    }
    function W() {
      return E(a);
    }
    function R(y) {
      function w(A, $, z, H, k) {
        if (typeof y != "function")
          return new O("Property `" + k + "` of component `" + z + "` has invalid PropType notation inside arrayOf.");
        var L = A[$];
        if (!Array.isArray(L)) {
          var g = ne(L);
          return new O("Invalid " + H + " `" + k + "` of type " + ("`" + g + "` supplied to `" + z + "`, expected an array."));
        }
        for (var b = 0; b < L.length; b++) {
          var S = y(L, b, z, H, k + "[" + b + "]", r);
          if (S instanceof Error)
            return S;
        }
        return null;
      }
      return E(w);
    }
    function D() {
      function y(w, A, $, z, H) {
        var k = w[A];
        if (!s(k)) {
          var L = ne(k);
          return new O("Invalid " + z + " `" + H + "` of type " + ("`" + L + "` supplied to `" + $ + "`, expected a single ReactElement."));
        }
        return null;
      }
      return E(y);
    }
    function p() {
      function y(w, A, $, z, H) {
        var k = w[A];
        if (!t.isValidElementType(k)) {
          var L = ne(k);
          return new O("Invalid " + z + " `" + H + "` of type " + ("`" + L + "` supplied to `" + $ + "`, expected a single ReactElement type."));
        }
        return null;
      }
      return E(y);
    }
    function U(y) {
      function w(A, $, z, H, k) {
        if (!(A[$] instanceof y)) {
          var L = y.name || d, g = Ye(A[$]);
          return new O("Invalid " + H + " `" + k + "` of type " + ("`" + g + "` supplied to `" + z + "`, expected ") + ("instance of `" + L + "`."));
        }
        return null;
      }
      return E(w);
    }
    function Q(y) {
      if (!Array.isArray(y))
        return process.env.NODE_ENV !== "production" && (arguments.length > 1 ? i(
          "Invalid arguments supplied to oneOf, expected an array, got " + arguments.length + " arguments. A common mistake is to write oneOf(x, y, z) instead of oneOf([x, y, z])."
        ) : i("Invalid argument supplied to oneOf, expected an array.")), a;
      function w(A, $, z, H, k) {
        for (var L = A[$], g = 0; g < y.length; g++)
          if (m(L, y[g]))
            return null;
        var b = JSON.stringify(y, function(P, h) {
          var q = se(h);
          return q === "symbol" ? String(h) : h;
        });
        return new O("Invalid " + H + " `" + k + "` of value `" + String(L) + "` " + ("supplied to `" + z + "`, expected one of " + b + "."));
      }
      return E(w);
    }
    function j(y) {
      function w(A, $, z, H, k) {
        if (typeof y != "function")
          return new O("Property `" + k + "` of component `" + z + "` has invalid PropType notation inside objectOf.");
        var L = A[$], g = ne(L);
        if (g !== "object")
          return new O("Invalid " + H + " `" + k + "` of type " + ("`" + g + "` supplied to `" + z + "`, expected an object."));
        for (var b in L)
          if (n(L, b)) {
            var S = y(L, b, z, H, k + "." + b, r);
            if (S instanceof Error)
              return S;
          }
        return null;
      }
      return E(w);
    }
    function re(y) {
      if (!Array.isArray(y))
        return process.env.NODE_ENV !== "production" && i("Invalid argument supplied to oneOfType, expected an instance of array."), a;
      for (var w = 0; w < y.length; w++) {
        var A = y[w];
        if (typeof A != "function")
          return i(
            "Invalid argument supplied to oneOfType. Expected an array of check functions, but received " + $e(A) + " at index " + w + "."
          ), a;
      }
      function $(z, H, k, L, g) {
        for (var b = [], S = 0; S < y.length; S++) {
          var P = y[S], h = P(z, H, k, L, g, r);
          if (h == null)
            return null;
          h.data && n(h.data, "expectedType") && b.push(h.data.expectedType);
        }
        var q = b.length > 0 ? ", expected one of type [" + b.join(", ") + "]" : "";
        return new O("Invalid " + L + " `" + g + "` supplied to " + ("`" + k + "`" + q + "."));
      }
      return E($);
    }
    function ie() {
      function y(w, A, $, z, H) {
        return be(w[A]) ? null : new O("Invalid " + z + " `" + H + "` supplied to " + ("`" + $ + "`, expected a ReactNode."));
      }
      return E(y);
    }
    function le(y, w, A, $, z) {
      return new O(
        (y || "React class") + ": " + w + " type `" + A + "." + $ + "` is invalid; it must be a function, usually from the `prop-types` package, but received `" + z + "`."
      );
    }
    function _e(y) {
      function w(A, $, z, H, k) {
        var L = A[$], g = ne(L);
        if (g !== "object")
          return new O("Invalid " + H + " `" + k + "` of type `" + g + "` " + ("supplied to `" + z + "`, expected `object`."));
        for (var b in y) {
          var S = y[b];
          if (typeof S != "function")
            return le(z, H, k, b, se(S));
          var P = S(L, b, z, H, k + "." + b, r);
          if (P)
            return P;
        }
        return null;
      }
      return E(w);
    }
    function ve(y) {
      function w(A, $, z, H, k) {
        var L = A[$], g = ne(L);
        if (g !== "object")
          return new O("Invalid " + H + " `" + k + "` of type `" + g + "` " + ("supplied to `" + z + "`, expected `object`."));
        var b = e({}, A[$], y);
        for (var S in b) {
          var P = y[S];
          if (n(y, S) && typeof P != "function")
            return le(z, H, k, S, se(P));
          if (!P)
            return new O(
              "Invalid " + H + " `" + k + "` key `" + S + "` supplied to `" + z + "`.\nBad object: " + JSON.stringify(A[$], null, "  ") + `
Valid keys: ` + JSON.stringify(Object.keys(y), null, "  ")
            );
          var h = P(L, S, z, H, k + "." + S, r);
          if (h)
            return h;
        }
        return null;
      }
      return E(w);
    }
    function be(y) {
      switch (typeof y) {
        case "number":
        case "string":
        case "undefined":
          return !0;
        case "boolean":
          return !y;
        case "object":
          if (Array.isArray(y))
            return y.every(be);
          if (y === null || s(y))
            return !0;
          var w = f(y);
          if (w) {
            var A = w.call(y), $;
            if (w !== y.entries) {
              for (; !($ = A.next()).done; )
                if (!be($.value))
                  return !1;
            } else
              for (; !($ = A.next()).done; ) {
                var z = $.value;
                if (z && !be(z[1]))
                  return !1;
              }
          } else
            return !1;
          return !0;
        default:
          return !1;
      }
    }
    function Ge(y, w) {
      return y === "symbol" ? !0 : w ? w["@@toStringTag"] === "Symbol" || typeof Symbol == "function" && w instanceof Symbol : !1;
    }
    function ne(y) {
      var w = typeof y;
      return Array.isArray(y) ? "array" : y instanceof RegExp ? "object" : Ge(w, y) ? "symbol" : w;
    }
    function se(y) {
      if (typeof y > "u" || y === null)
        return "" + y;
      var w = ne(y);
      if (w === "object") {
        if (y instanceof Date)
          return "date";
        if (y instanceof RegExp)
          return "regexp";
      }
      return w;
    }
    function $e(y) {
      var w = se(y);
      switch (w) {
        case "array":
        case "object":
          return "an " + w;
        case "boolean":
        case "date":
        case "regexp":
          return "a " + w;
        default:
          return w;
      }
    }
    function Ye(y) {
      return !y.constructor || !y.constructor.name ? d : y.constructor.name;
    }
    return v.checkPropTypes = o, v.resetWarningCache = o.resetWarningCache, v.PropTypes = v, v;
  }, Pt;
}
var Dt, qr;
function Io() {
  if (qr) return Dt;
  qr = 1;
  var t = yr();
  function e() {
  }
  function r() {
  }
  return r.resetWarningCache = e, Dt = function() {
    function n(a, s, l, u, c, f) {
      if (f !== t) {
        var d = new Error(
          "Calling PropTypes validators directly is not supported by the `prop-types` package. Use PropTypes.checkPropTypes() to call them. Read more at http://fb.me/use-check-prop-types"
        );
        throw d.name = "Invariant Violation", d;
      }
    }
    n.isRequired = n;
    function o() {
      return n;
    }
    var i = {
      array: n,
      bigint: n,
      bool: n,
      func: n,
      number: n,
      object: n,
      string: n,
      symbol: n,
      any: n,
      arrayOf: o,
      element: n,
      elementType: n,
      instanceOf: o,
      node: n,
      objectOf: o,
      oneOf: o,
      oneOfType: o,
      shape: o,
      exact: o,
      checkPropTypes: r,
      resetWarningCache: e
    };
    return i.PropTypes = i, i;
  }, Dt;
}
if (process.env.NODE_ENV !== "production") {
  var Bo = Cn(), Go = !0;
  Jt.exports = qo()(Bo.isElement, Go);
} else
  Jt.exports = Io()();
var Ee = Jt.exports, gt = { exports: {} }, Yo = Object.create, yt = Object.defineProperty, Fo = Object.getOwnPropertyDescriptor, Uo = Object.getOwnPropertyNames, Xo = Object.getPrototypeOf, Vo = Object.prototype.hasOwnProperty, Ko = (t, e) => {
  for (var r in e)
    yt(t, r, { get: e[r], enumerable: !0 });
}, jn = (t, e, r, n) => {
  if (e && typeof e == "object" || typeof e == "function")
    for (let o of Uo(e))
      !Vo.call(t, o) && o !== r && yt(t, o, { get: () => e[o], enumerable: !(n = Fo(e, o)) || n.enumerable });
  return t;
}, Be = (t, e, r) => (r = t != null ? Yo(Xo(t)) : {}, jn(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  !t || !t.__esModule ? yt(r, "default", { value: t, enumerable: !0 }) : r,
  t
)), Jo = (t) => jn(yt({}, "__esModule", { value: !0 }), t), Tn = {};
Ko(Tn, {
  DraggableCore: () => Me,
  default: () => mt
});
var Zo = Jo(Tn), rt = Be(de), Y = Be(Ee), Qo = Be(ir), ei = ct;
function Zt(t, e) {
  for (let r = 0, n = t.length; r < n; r++)
    if (e.apply(e, [t[r], r, t])) return t[r];
}
function Ir(t) {
  return typeof t == "function" || Object.prototype.toString.call(t) === "[object Function]";
}
function Xe(t) {
  return typeof t == "number" && !isNaN(t);
}
function te(t) {
  return parseInt(t, 10);
}
function We(t, e, r) {
  if (t[e])
    return new Error(`Invalid prop ${e} passed to ${r} - do not set this, set it on the child.`);
}
var Ct = ["Moz", "Webkit", "O", "ms"];
function ti(t = "transform") {
  var e, r;
  if (typeof window > "u") return "";
  const n = (r = (e = window.document) == null ? void 0 : e.documentElement) == null ? void 0 : r.style;
  if (!n || t in n) return "";
  for (let o = 0; o < Ct.length; o++)
    if (Mn(t, Ct[o]) in n) return Ct[o];
  return "";
}
function Mn(t, e) {
  return e ? `${e}${ri(t)}` : t;
}
function ri(t) {
  let e = "", r = !0;
  for (let n = 0; n < t.length; n++)
    r ? (e += t[n].toUpperCase(), r = !1) : t[n] === "-" ? r = !0 : e += t[n];
  return e;
}
var ni = ti(), zt = "";
function oi(t, e) {
  var r;
  zt || (zt = (r = Zt([
    "matches",
    "webkitMatchesSelector",
    "mozMatchesSelector",
    "msMatchesSelector",
    "oMatchesSelector"
  ], function(o) {
    return Ir(t[o]);
  })) != null ? r : "");
  const n = t[zt];
  return Ir(n) ? !!n.call(t, e) : !1;
}
function Br(t, e, r) {
  let n = t;
  do {
    if (oi(n, e)) return !0;
    if (n === r) return !1;
    n = n.parentNode;
  } while (n);
  return !1;
}
function jt(t, e, r, n) {
  if (!t) return;
  const o = { capture: !0, ...n }, i = r;
  t.addEventListener ? t.addEventListener(e, i, o) : t.attachEvent ? t.attachEvent("on" + e, i) : t["on" + e] = i;
}
function xe(t, e, r, n) {
  if (!t) return;
  const o = { capture: !0, ...n }, i = r;
  t.removeEventListener ? t.removeEventListener(e, i, o) : t.detachEvent ? t.detachEvent("on" + e, i) : t["on" + e] = null;
}
function ii(t) {
  let e = t.clientHeight;
  const r = t.ownerDocument.defaultView.getComputedStyle(t);
  return e += te(r.borderTopWidth), e += te(r.borderBottomWidth), e;
}
function ai(t) {
  let e = t.clientWidth;
  const r = t.ownerDocument.defaultView.getComputedStyle(t);
  return e += te(r.borderLeftWidth), e += te(r.borderRightWidth), e;
}
function si(t) {
  let e = t.clientHeight;
  const r = t.ownerDocument.defaultView.getComputedStyle(t);
  return e -= te(r.paddingTop), e -= te(r.paddingBottom), e;
}
function li(t) {
  let e = t.clientWidth;
  const r = t.ownerDocument.defaultView.getComputedStyle(t);
  return e -= te(r.paddingLeft), e -= te(r.paddingRight), e;
}
function ui(t, e, r) {
  const o = e === e.ownerDocument.body ? { left: 0, top: 0 } : e.getBoundingClientRect(), i = (t.clientX + e.scrollLeft - o.left) / r, a = (t.clientY + e.scrollTop - o.top) / r;
  return { x: i, y: a };
}
function ci(t, e) {
  const r = kn(t, e, "px");
  return { [Mn("transform", ni)]: r };
}
function fi(t, e) {
  return kn(t, e, "");
}
function kn({ x: t, y: e }, r, n) {
  let o = `translate(${t}${n},${e}${n})`;
  if (r) {
    const i = `${typeof r.x == "string" ? r.x : r.x + n}`, a = `${typeof r.y == "string" ? r.y : r.y + n}`;
    o = `translate(${i}, ${a})` + o;
  }
  return o;
}
function di(t, e) {
  return t.targetTouches && Zt(t.targetTouches, (r) => e === r.identifier) || t.changedTouches && Zt(t.changedTouches, (r) => e === r.identifier);
}
function pi(t) {
  if (t.targetTouches && t.targetTouches[0]) return t.targetTouches[0].identifier;
  if (t.changedTouches && t.changedTouches[0]) return t.changedTouches[0].identifier;
}
function hi() {
  return typeof __webpack_nonce__ < "u" ? __webpack_nonce__ : void 0;
}
function gi(t, e) {
  if (!t) return;
  let r = t.getElementById("react-draggable-style-el");
  if (!r) {
    r = t.createElement("style"), r.type = "text/css", r.id = "react-draggable-style-el";
    const n = e ?? hi();
    n && r.setAttribute("nonce", n), r.innerHTML = `.react-draggable-transparent-selection *::-moz-selection {all: inherit;}
`, r.innerHTML += `.react-draggable-transparent-selection *::selection {all: inherit;}
`, t.getElementsByTagName("head")[0].appendChild(r);
  }
  t.body && yi(t.body, "react-draggable-transparent-selection");
}
function Gr(t) {
  window.requestAnimationFrame ? window.requestAnimationFrame(() => {
    Yr(t);
  }) : Yr(t);
}
function Yr(t) {
  if (t)
    try {
      t.body && mi(t.body, "react-draggable-transparent-selection");
      const e = t.selection;
      if (e)
        e.empty();
      else {
        const r = (t.defaultView || window).getSelection();
        r && r.type !== "Caret" && r.removeAllRanges();
      }
    } catch {
    }
}
function yi(t, e) {
  t.classList ? t.classList.add(e) : t.className.match(new RegExp(`(?:^|\\s)${e}(?!\\S)`)) || (t.className += ` ${e}`);
}
function mi(t, e) {
  t.classList ? t.classList.remove(e) : t.className = t.className.replace(new RegExp(`(?:^|\\s)${e}(?!\\S)`, "g"), "");
}
function vi(t, e, r) {
  if (!t.props.bounds) return [e, r];
  let { bounds: n } = t.props;
  n = typeof n == "string" ? n : Oi(n);
  const o = mr(t);
  if (typeof n == "string") {
    const { ownerDocument: i } = o, a = i.defaultView;
    if (!a)
      throw new Error("Cannot resolve the owner window of the draggable node.");
    let s;
    if (n === "parent" ? s = o.parentNode : s = o.getRootNode().querySelector(n), !(s instanceof a.HTMLElement))
      throw new Error('Bounds selector "' + n + '" could not find an element.');
    const l = s, u = a.getComputedStyle(o), c = a.getComputedStyle(l);
    n = {
      left: -o.offsetLeft + te(c.paddingLeft) + te(u.marginLeft),
      top: -o.offsetTop + te(c.paddingTop) + te(u.marginTop),
      right: li(l) - ai(o) - o.offsetLeft + te(c.paddingRight) - te(u.marginRight),
      bottom: si(l) - ii(o) - o.offsetTop + te(c.paddingBottom) - te(u.marginBottom)
    };
  }
  return Xe(n.right) && (e = Math.min(e, n.right)), Xe(n.bottom) && (r = Math.min(r, n.bottom)), Xe(n.left) && (e = Math.max(e, n.left)), Xe(n.top) && (r = Math.max(r, n.top)), [e, r];
}
function Fr(t, e, r) {
  const n = Math.round(e / t[0]) * t[0], o = Math.round(r / t[1]) * t[1];
  return [n, o];
}
function bi(t) {
  return t.props.axis === "both" || t.props.axis === "x";
}
function wi(t) {
  return t.props.axis === "both" || t.props.axis === "y";
}
function Tt(t, e, r) {
  const n = typeof e == "number" ? di(t, e) : null;
  if (typeof e == "number" && !n) return null;
  const o = mr(r), i = r.props.offsetParent || o.offsetParent || o.ownerDocument.body;
  return ui(n || t, i, r.props.scale);
}
function Mt(t, e, r) {
  const n = !Xe(t.lastX), o = mr(t);
  return n ? {
    node: o,
    deltaX: 0,
    deltaY: 0,
    lastX: e,
    lastY: r,
    x: e,
    y: r
  } : {
    node: o,
    deltaX: e - t.lastX,
    deltaY: r - t.lastY,
    lastX: t.lastX,
    lastY: t.lastY,
    x: e,
    y: r
  };
}
function kt(t, e) {
  const r = t.props.scale;
  return {
    node: e.node,
    x: t.state.x + e.deltaX / r,
    y: t.state.y + e.deltaY / r,
    deltaX: e.deltaX / r,
    deltaY: e.deltaY / r,
    lastX: t.state.x,
    lastY: t.state.y
  };
}
function Oi(t) {
  return {
    left: t.left,
    top: t.top,
    right: t.right,
    bottom: t.bottom
  };
}
function mr(t) {
  const e = t.findDOMNode();
  if (!e)
    throw new Error("<DraggableCore>: Unmounted during event!");
  return e;
}
var $t = Be(de), ee = Be(Ee), Si = Be(ir);
function ye(...t) {
  process.env.DRAGGABLE_DEBUG && console.log(...t);
}
var ue = {
  touch: {
    start: "touchstart",
    move: "touchmove",
    stop: "touchend"
  },
  mouse: {
    start: "mousedown",
    move: "mousemove",
    stop: "mouseup"
  }
}, Re = ue.mouse, Me = class extends $t.Component {
  constructor() {
    super(...arguments), this.dragging = !1, this.lastX = NaN, this.lastY = NaN, this.touchIdentifier = null, this.mounted = !1, this.handleDragStart = (e) => {
      if (this.props.onMouseDown(e), !this.props.allowAnyClick && (typeof e.button == "number" && e.button !== 0 || e.ctrlKey)) return !1;
      const r = this.findDOMNode();
      if (!r || !r.ownerDocument || !r.ownerDocument.body)
        throw new Error("<DraggableCore> not mounted on DragStart!");
      const { ownerDocument: n } = r;
      if (this.props.disabled || !(e.target instanceof n.defaultView.Node) || this.props.handle && !Br(e.target, this.props.handle, r) || this.props.cancel && Br(e.target, this.props.cancel, r))
        return;
      e.type === "touchstart" && !this.props.allowMobileScroll && e.preventDefault();
      const o = pi(e);
      this.touchIdentifier = o;
      const i = Tt(e, o, this);
      if (i == null) return;
      const { x: a, y: s } = i, l = Mt(this, a, s);
      ye("DraggableCore: handleDragStart: %j", l), ye("calling", this.props.onStart), !(this.props.onStart(e, l) === !1 || this.mounted === !1) && (this.props.enableUserSelectHack && gi(n, this.props.nonce), this.dragging = !0, this.lastX = a, this.lastY = s, jt(n, Re.move, this.handleDrag), jt(n, Re.stop, this.handleDragStop));
    }, this.handleDrag = (e) => {
      const r = Tt(e, this.touchIdentifier, this);
      if (r == null) return;
      let { x: n, y: o } = r;
      if (Array.isArray(this.props.grid)) {
        let s = n - this.lastX, l = o - this.lastY;
        if ([s, l] = Fr(this.props.grid, s, l), !s && !l) return;
        n = this.lastX + s, o = this.lastY + l;
      }
      const i = Mt(this, n, o);
      if (ye("DraggableCore: handleDrag: %j", i), this.props.onDrag(e, i) === !1 || this.mounted === !1) {
        try {
          this.handleDragStop(new MouseEvent("mouseup"));
        } catch {
          const s = document.createEvent("MouseEvents");
          s.initMouseEvent("mouseup", !0, !0, window, 0, 0, 0, 0, 0, !1, !1, !1, !1, 0, null), this.handleDragStop(s);
        }
        return;
      }
      this.lastX = n, this.lastY = o;
    }, this.handleDragStop = (e) => {
      if (!this.dragging) return;
      const r = Tt(e, this.touchIdentifier, this);
      if (r == null) return;
      let { x: n, y: o } = r;
      if (Array.isArray(this.props.grid)) {
        let l = n - this.lastX || 0, u = o - this.lastY || 0;
        [l, u] = Fr(this.props.grid, l, u), n = this.lastX + l, o = this.lastY + u;
      }
      const i = Mt(this, n, o);
      if (this.props.onStop(e, i) === !1 || this.mounted === !1) return !1;
      const s = this.findDOMNode();
      s && this.props.enableUserSelectHack && Gr(s.ownerDocument), ye("DraggableCore: handleDragStop: %j", i), this.dragging = !1, this.lastX = NaN, this.lastY = NaN, s && (ye("DraggableCore: Removing handlers"), xe(s.ownerDocument, Re.move, this.handleDrag), xe(s.ownerDocument, Re.stop, this.handleDragStop));
    }, this.onMouseDown = (e) => (Re = ue.mouse, this.handleDragStart(e)), this.onMouseUp = (e) => (Re = ue.mouse, this.handleDragStop(e)), this.onTouchStart = (e) => (Re = ue.touch, this.handleDragStart(e)), this.onTouchEnd = (e) => (Re = ue.touch, this.handleDragStop(e));
  }
  componentDidMount() {
    this.mounted = !0;
    const e = this.findDOMNode();
    e && jt(e, ue.touch.start, this.onTouchStart, { passive: !1 });
  }
  componentWillUnmount() {
    this.mounted = !1;
    const e = this.findDOMNode();
    if (e) {
      const { ownerDocument: r } = e;
      xe(r, ue.mouse.move, this.handleDrag), xe(r, ue.touch.move, this.handleDrag), xe(r, ue.mouse.stop, this.handleDragStop), xe(r, ue.touch.stop, this.handleDragStop), xe(e, ue.touch.start, this.onTouchStart, { passive: !1 }), this.props.enableUserSelectHack && Gr(r);
    }
  }
  // React 19 removed ReactDOM.findDOMNode, so nodeRef is now required.
  // For backward compatibility with React 18 and earlier, we still support findDOMNode if available.
  findDOMNode() {
    var e;
    if ((e = this.props) != null && e.nodeRef)
      return this.props.nodeRef.current;
    const r = Si.default;
    return typeof r.findDOMNode == "function" ? r.findDOMNode(this) : (ye(
      "react-draggable: ReactDOM.findDOMNode is not available in React 19+. You must provide a nodeRef prop. See: https://github.com/react-grid-layout/react-draggable#noderef"
    ), null);
  }
  render() {
    return $t.cloneElement($t.Children.only(this.props.children), {
      // Note: mouseMove handler is attached to document so it will still function
      // when the user drags quickly and leaves the bounds of the element.
      onMouseDown: this.onMouseDown,
      onMouseUp: this.onMouseUp,
      // onTouchStart is added on `componentDidMount` so they can be added with
      // {passive: false}, which allows it to cancel. See
      // https://developers.google.com/web/updates/2017/01/scrolling-intervention
      onTouchEnd: this.onTouchEnd
    });
  }
};
Me.displayName = "DraggableCore";
Me.propTypes = {
  /**
   * `allowAnyClick` allows dragging using any mouse button.
   * By default, we only accept the left button.
   *
   * Defaults to `false`.
   */
  allowAnyClick: ee.default.bool,
  /**
   * `allowMobileScroll` turns off cancellation of the 'touchstart' event
   * on mobile devices. Only enable this if you are having trouble with click
   * events. Prefer using 'handle' / 'cancel' instead.
   *
   * Defaults to `false`.
   */
  allowMobileScroll: ee.default.bool,
  children: ee.default.node.isRequired,
  /**
   * `disabled`, if true, stops the <Draggable> from dragging. All handlers,
   * with the exception of `onMouseDown`, will not fire.
   */
  disabled: ee.default.bool,
  /**
   * By default, we add 'user-select:none' attributes to the document body
   * to prevent ugly text selection during drag. If this is causing problems
   * for your app, set this to `false`.
   */
  enableUserSelectHack: ee.default.bool,
  /**
   * `offsetParent`, if set, uses the passed DOM node to compute drag offsets
   * instead of using the parent node.
   */
  offsetParent: function(t, e) {
    if (t[e] && t[e].nodeType !== 1)
      throw new Error("Draggable's offsetParent must be a DOM Node.");
  },
  /**
   * `grid` specifies the x and y that dragging should snap to.
   */
  grid: ee.default.arrayOf(ee.default.number),
  /**
   * `handle` specifies a selector to be used as the handle that initiates drag.
   *
   * Example:
   *
   * ```jsx
   *   let App = React.createClass({
   *       render: function () {
   *         return (
   *            <Draggable handle=".handle">
   *              <div>
   *                  <div className="handle">Click me to drag</div>
   *                  <div>This is some other content</div>
   *              </div>
   *           </Draggable>
   *         );
   *       }
   *   });
   * ```
   */
  handle: ee.default.string,
  /**
   * `cancel` specifies a selector to be used to prevent drag initialization.
   *
   * Example:
   *
   * ```jsx
   *   let App = React.createClass({
   *       render: function () {
   *           return(
   *               <Draggable cancel=".cancel">
   *                   <div>
   *                     <div className="cancel">You can't drag from here</div>
   *                     <div>Dragging here works fine</div>
   *                   </div>
   *               </Draggable>
   *           );
   *       }
   *   });
   * ```
   */
  cancel: ee.default.string,
  /* If running in React Strict mode, ReactDOM.findDOMNode() is deprecated.
   * Unfortunately, in order for <Draggable> to work properly, we need raw access
   * to the underlying DOM node. If you want to avoid the warning, pass a `nodeRef`
   * as in this example:
   *
   * function MyComponent() {
   *   const nodeRef = React.useRef(null);
   *   return (
   *     <Draggable nodeRef={nodeRef}>
   *       <div ref={nodeRef}>Example Target</div>
   *     </Draggable>
   *   );
   * }
   *
   * This can be used for arbitrarily nested components, so long as the ref ends up
   * pointing to the actual child DOM node and not a custom component.
   */
  nodeRef: ee.default.object,
  /**
   * `nonce` is applied to the dynamically-injected <style> element used by the
   * user-select hack, so it isn't blocked under a strict Content Security
   * Policy (`style-src` without `'unsafe-inline'`). If omitted, webpack's
   * `__webpack_nonce__` global is used when available.
   */
  nonce: ee.default.string,
  /**
   * Called when dragging starts.
   * If this function returns the boolean false, dragging will be canceled.
   */
  onStart: ee.default.func,
  /**
   * Called while dragging.
   * If this function returns the boolean false, dragging will be canceled.
   */
  onDrag: ee.default.func,
  /**
   * Called when dragging stops.
   * If this function returns the boolean false, the drag will remain active.
   */
  onStop: ee.default.func,
  /**
   * A workaround option which can be passed if onMouseDown needs to be accessed,
   * since it'll always be blocked (as there is internal use of onMouseDown)
   */
  onMouseDown: ee.default.func,
  /**
   * `scale`, if set, applies scaling while dragging an element
   */
  scale: ee.default.number,
  /**
   * These properties should be defined on the child, not here.
   */
  className: We,
  style: We,
  transform: We
};
Me.defaultProps = {
  allowAnyClick: !1,
  // by default only accept left click
  allowMobileScroll: !1,
  disabled: !1,
  enableUserSelectHack: !0,
  onStart: function() {
  },
  onDrag: function() {
  },
  onStop: function() {
  },
  onMouseDown: function() {
  },
  scale: 1
};
var mt = class extends rt.Component {
  constructor(e) {
    super(e), this.onDragStart = (r, n) => {
      if (ye("Draggable: onDragStart: %j", n), this.props.onStart(r, kt(this, n)) === !1) return !1;
      this.setState({ dragging: !0, dragged: !0 });
    }, this.onDrag = (r, n) => {
      if (!this.state.dragging) return !1;
      ye("Draggable: onDrag: %j", n);
      const o = kt(this, n), i = {
        x: o.x,
        y: o.y,
        slackX: 0,
        slackY: 0
      };
      if (this.props.bounds) {
        const { x: s, y: l } = i;
        i.x += this.state.slackX, i.y += this.state.slackY;
        const [u, c] = vi(this, i.x, i.y);
        i.x = u, i.y = c, i.slackX = this.state.slackX + (s - i.x), i.slackY = this.state.slackY + (l - i.y), o.x = i.x, o.y = i.y, o.deltaX = i.x - this.state.x, o.deltaY = i.y - this.state.y;
      }
      if (this.props.onDrag(r, o) === !1) return !1;
      this.setState(i);
    }, this.onDragStop = (r, n) => {
      if (!this.state.dragging || this.props.onStop(r, kt(this, n)) === !1) return !1;
      ye("Draggable: onDragStop: %j", n);
      const i = {
        dragging: !1,
        slackX: 0,
        slackY: 0
      };
      if (!!this.props.position) {
        const { x: s, y: l } = this.props.position;
        i.x = s, i.y = l;
      }
      this.setState(i);
    }, this.state = {
      // Whether or not we are currently dragging.
      dragging: !1,
      // Whether or not we have been dragged before.
      dragged: !1,
      // Current transform x and y.
      x: e.position ? e.position.x : e.defaultPosition.x,
      y: e.position ? e.position.y : e.defaultPosition.y,
      prevPropsPosition: { ...e.position },
      // Used for compensating for out-of-bounds drags
      slackX: 0,
      slackY: 0,
      // Can only determine if SVG after mounting
      isElementSVG: !1
    }, e.position && !(e.onDrag || e.onStop) && console.warn("A `position` was applied to this <Draggable>, without drag handlers. This will make this component effectively undraggable. Please attach `onDrag` or `onStop` handlers so you can adjust the `position` of this element.");
  }
  // React 16.3+
  // Arity (props, state)
  static getDerivedStateFromProps({ position: e }, { prevPropsPosition: r }) {
    return e && (!r || e.x !== r.x || e.y !== r.y) ? (ye("Draggable: getDerivedStateFromProps %j", { position: e, prevPropsPosition: r }), {
      x: e.x,
      y: e.y,
      prevPropsPosition: { ...e }
    }) : null;
  }
  componentDidMount() {
    typeof window.SVGElement < "u" && this.findDOMNode() instanceof window.SVGElement && this.setState({ isElementSVG: !0 });
  }
  componentWillUnmount() {
    this.state.dragging && this.setState({ dragging: !1 });
  }
  // React 19 removed ReactDOM.findDOMNode, so nodeRef is now required.
  // For backward compatibility with React 18 and earlier, we still support findDOMNode if available.
  findDOMNode() {
    var e;
    if ((e = this.props) != null && e.nodeRef)
      return this.props.nodeRef.current;
    const r = Qo.default;
    return typeof r.findDOMNode == "function" ? r.findDOMNode(this) : null;
  }
  render() {
    const {
      axis: e,
      bounds: r,
      children: n,
      defaultPosition: o,
      defaultClassName: i,
      defaultClassNameDragging: a,
      defaultClassNameDragged: s,
      position: l,
      positionOffset: u,
      scale: c,
      ...f
    } = this.props;
    let d = {}, v = null;
    const O = !!!l || this.state.dragging, E = l || o, _ = {
      // Set left if horizontal drag is enabled
      x: bi(this) && O ? this.state.x : E.x,
      // Set top if vertical drag is enabled
      y: wi(this) && O ? this.state.y : E.y
    };
    this.state.isElementSVG ? v = fi(_, u) : d = ci(_, u);
    const W = rt.Children.only(n), R = (0, ei.clsx)(W.props.className || "", i, {
      [a]: this.state.dragging,
      [s]: this.state.dragged
    });
    return /* @__PURE__ */ rt.createElement(Me, { ...f, onStart: this.onDragStart, onDrag: this.onDrag, onStop: this.onDragStop }, rt.cloneElement(W, {
      className: R,
      style: { ...W.props.style, ...d },
      transform: v
    }));
  }
};
mt.displayName = "Draggable";
mt.propTypes = {
  // Accepts all props <DraggableCore> accepts.
  ...Me.propTypes,
  /**
   * `axis` determines which axis the draggable can move.
   *
   *  Note that all callbacks will still return data as normal. This only
   *  controls flushing to the DOM.
   *
   * 'both' allows movement horizontally and vertically.
   * 'x' limits movement to horizontal axis.
   * 'y' limits movement to vertical axis.
   * 'none' limits all movement.
   *
   * Defaults to 'both'.
   */
  axis: Y.default.oneOf(["both", "x", "y", "none"]),
  /**
   * `bounds` determines the range of movement available to the element.
   * Available values are:
   *
   * 'parent' restricts movement within the Draggable's parent node.
   *
   * Alternatively, pass an object with the following properties, all of which are optional:
   *
   * {left: LEFT_BOUND, right: RIGHT_BOUND, bottom: BOTTOM_BOUND, top: TOP_BOUND}
   *
   * All values are in px.
   *
   * Example:
   *
   * ```jsx
   *   let App = React.createClass({
   *       render: function () {
   *         return (
   *            <Draggable bounds={{right: 300, bottom: 300}}>
   *              <div>Content</div>
   *           </Draggable>
   *         );
   *       }
   *   });
   * ```
   */
  bounds: Y.default.oneOfType([
    Y.default.shape({
      left: Y.default.number,
      right: Y.default.number,
      top: Y.default.number,
      bottom: Y.default.number
    }),
    Y.default.string,
    Y.default.oneOf([!1])
  ]),
  defaultClassName: Y.default.string,
  defaultClassNameDragging: Y.default.string,
  defaultClassNameDragged: Y.default.string,
  /**
   * `defaultPosition` specifies the x and y that the dragged item should start at
   *
   * Example:
   *
   * ```jsx
   *      let App = React.createClass({
   *          render: function () {
   *              return (
   *                  <Draggable defaultPosition={{x: 25, y: 25}}>
   *                      <div>I start with transformX: 25px and transformY: 25px;</div>
   *                  </Draggable>
   *              );
   *          }
   *      });
   * ```
   */
  defaultPosition: Y.default.shape({
    x: Y.default.number,
    y: Y.default.number
  }),
  positionOffset: Y.default.shape({
    x: Y.default.oneOfType([Y.default.number, Y.default.string]),
    y: Y.default.oneOfType([Y.default.number, Y.default.string])
  }),
  /**
   * `position`, if present, defines the current position of the element.
   *
   *  This is similar to how form elements in React work - if no `position` is supplied, the component
   *  is uncontrolled.
   *
   * Example:
   *
   * ```jsx
   *      let App = React.createClass({
   *          render: function () {
   *              return (
   *                  <Draggable position={{x: 25, y: 25}}>
   *                      <div>I start with transformX: 25px and transformY: 25px;</div>
   *                  </Draggable>
   *              );
   *          }
   *      });
   * ```
   */
  position: Y.default.shape({
    x: Y.default.number,
    y: Y.default.number
  }),
  /**
   * These properties should be defined on the child, not here.
   */
  className: We,
  style: We,
  transform: We
};
mt.defaultProps = {
  ...Me.defaultProps,
  axis: "both",
  bounds: !1,
  defaultClassName: "react-draggable",
  defaultClassNameDragging: "react-draggable-dragging",
  defaultClassNameDragged: "react-draggable-dragged",
  defaultPosition: { x: 0, y: 0 },
  scale: 1
};
const Qt = Zo, _i = Qt.DraggableCore, $n = Qt.default || Qt;
gt.exports = $n;
gt.exports.default = $n;
gt.exports.DraggableCore = _i;
var Ln = gt.exports, vt = { exports: {} }, Qe = {}, vr = {};
vr.__esModule = !0;
vr.cloneElement = Ci;
var Ri = Ei(de);
function Ei(t) {
  return t && t.__esModule ? t : { default: t };
}
function Ur(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Xr(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? Ur(Object(r), !0).forEach(function(n) {
      xi(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : Ur(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function xi(t, e, r) {
  return (e = Pi(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function Pi(t) {
  var e = Di(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function Di(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
function Ci(t, e) {
  return e.style && t.props.style && (e.style = Xr(Xr({}, t.props.style), e.style)), e.className && t.props.className && (e.className = t.props.className + " " + e.className), /* @__PURE__ */ Ri.default.cloneElement(t, e);
}
var et = {};
et.__esModule = !0;
et.resizableProps = void 0;
var T = zi(Ee);
function zi(t) {
  return t && t.__esModule ? t : { default: t };
}
et.resizableProps = {
  /*
  * Restricts resizing to a particular axis (default: 'both')
  * 'both' - allows resizing by width or height
  * 'x' - only allows the width to be changed
  * 'y' - only allows the height to be changed
  * 'none' - disables resizing altogether
  * */
  axis: T.default.oneOf(["both", "x", "y", "none"]),
  className: T.default.string,
  /*
  * Require that one and only one child be present.
  * */
  children: T.default.element.isRequired,
  /*
  * These will be passed wholesale to react-draggable's DraggableCore
  * */
  draggableOpts: T.default.shape({
    allowAnyClick: T.default.bool,
    cancel: T.default.string,
    children: T.default.node,
    disabled: T.default.bool,
    enableUserSelectHack: T.default.bool,
    // #251: Check for Element to support SSR environments where DOM globals don't exist
    offsetParent: typeof Element < "u" ? T.default.instanceOf(Element) : T.default.any,
    grid: T.default.arrayOf(T.default.number),
    handle: T.default.string,
    nodeRef: T.default.object,
    onStart: T.default.func,
    onDrag: T.default.func,
    onStop: T.default.func,
    onMouseDown: T.default.func,
    scale: T.default.number
  }),
  /*
  * Initial height
  * */
  height: function() {
    for (var t = arguments.length, e = new Array(t), r = 0; r < t; r++)
      e[r] = arguments[r];
    const n = e[0];
    return n.axis === "both" || n.axis === "y" ? T.default.number.isRequired(...e) : T.default.number(...e);
  },
  /*
  * Customize cursor resize handle
  * */
  handle: T.default.oneOfType([T.default.node, T.default.func]),
  /*
  * If you change this, be sure to update your css
  * */
  handleSize: T.default.arrayOf(T.default.number),
  lockAspectRatio: T.default.bool,
  /*
  * Max X & Y measure
  * */
  maxConstraints: T.default.arrayOf(T.default.number),
  /*
  * Min X & Y measure
  * */
  minConstraints: T.default.arrayOf(T.default.number),
  /*
  * Called on stop resize event
  * */
  onResizeStop: T.default.func,
  /*
  * Called on start resize event
  * */
  onResizeStart: T.default.func,
  /*
  * Called on resize event
  * */
  onResize: T.default.func,
  /*
  * Defines which resize handles should be rendered (default: 'se')
  * 's' - South handle (bottom-center)
  * 'w' - West handle (left-center)
  * 'e' - East handle (right-center)
  * 'n' - North handle (top-center)
  * 'sw' - Southwest handle (bottom-left)
  * 'nw' - Northwest handle (top-left)
  * 'se' - Southeast handle (bottom-right)
  * 'ne' - Northeast handle (top-center)
  * */
  resizeHandles: T.default.arrayOf(T.default.oneOf(["s", "w", "e", "n", "sw", "nw", "se", "ne"])),
  /*
  * If `transform: scale(n)` is set on the parent, this should be set to `n`.
  * */
  transformScale: T.default.number,
  /*
   * Initial width
   */
  width: function() {
    for (var t = arguments.length, e = new Array(t), r = 0; r < t; r++)
      e[r] = arguments[r];
    const n = e[0];
    return n.axis === "both" || n.axis === "x" ? T.default.number.isRequired(...e) : T.default.number(...e);
  }
};
Qe.__esModule = !0;
Qe.default = void 0;
var Le = Nn(de), ji = Ln, Ti = vr, Mi = et;
const ki = ["children", "className", "draggableOpts", "width", "height", "handle", "handleSize", "lockAspectRatio", "axis", "minConstraints", "maxConstraints", "onResize", "onResizeStop", "onResizeStart", "resizeHandles", "transformScale"];
function Nn(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (Nn = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var a, s, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (a = i ? n : r) {
      if (a.has(o)) return a.get(o);
      a.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((s = (a = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (s.get || s.set) ? a(l, u, s) : l[u] = o[u]);
    return l;
  })(t, e);
}
function er() {
  return er = Object.assign ? Object.assign.bind() : function(t) {
    for (var e = 1; e < arguments.length; e++) {
      var r = arguments[e];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (t[n] = r[n]);
    }
    return t;
  }, er.apply(null, arguments);
}
function $i(t, e) {
  if (t == null) return {};
  var r = {};
  for (var n in t) if ({}.hasOwnProperty.call(t, n)) {
    if (e.indexOf(n) !== -1) continue;
    r[n] = t[n];
  }
  return r;
}
function Vr(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Lt(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? Vr(Object(r), !0).forEach(function(n) {
      Li(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : Vr(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function Li(t, e, r) {
  return (e = Ni(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function Ni(t) {
  var e = Ai(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function Ai(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
class br extends Le.Component {
  constructor() {
    super(...arguments), this.handleRefs = {}, this.lastHandleRect = null, this.slack = null, this.lastSize = null;
  }
  componentWillUnmount() {
    this.resetData();
  }
  resetData() {
    this.lastHandleRect = this.slack = this.lastSize = null;
  }
  // Clamp width and height within provided constraints
  runConstraints(e, r) {
    const n = this.props, o = n.minConstraints, i = n.maxConstraints, a = n.lockAspectRatio;
    if (!o && !i && !a) return [e, r];
    if (a) {
      const d = this.props.width / this.props.height, v = e - this.props.width, m = r - this.props.height;
      Math.abs(v) > Math.abs(m * d) ? r = e / d : e = r * d;
    }
    const s = e, l = r;
    let u = this.slack || [0, 0], c = u[0], f = u[1];
    return e += c, r += f, o && (e = Math.max(o[0], e), r = Math.max(o[1], r)), i && (e = Math.min(i[0], e), r = Math.min(i[1], r)), this.slack = [c + (s - e), f + (l - r)], [e, r];
  }
  /**
   * Wrapper around drag events to provide more useful data.
   *
   * @param  {String} handlerName Handler name to wrap.
   * @return {Function}           Handler function.
   */
  resizeHandler(e, r) {
    return (n, o) => {
      var i, a, s, l;
      let u = o.node, c = o.deltaX, f = o.deltaY;
      e === "onResizeStart" && this.resetData();
      const d = (this.props.axis === "both" || this.props.axis === "x") && r !== "n" && r !== "s", v = (this.props.axis === "both" || this.props.axis === "y") && r !== "e" && r !== "w";
      if (!d && !v) return;
      const m = r[0], O = r[r.length - 1], E = u.getBoundingClientRect();
      if (this.lastHandleRect != null) {
        if (O === "w") {
          const ie = E.left - this.lastHandleRect.left;
          c += ie;
        }
        if (m === "n") {
          const ie = E.top - this.lastHandleRect.top;
          f += ie;
        }
      }
      this.lastHandleRect = E, O === "w" && (c = -c), m === "n" && (f = -f);
      const _ = (i = (a = this.lastSize) == null ? void 0 : a.width) != null ? i : this.props.width, W = (s = (l = this.lastSize) == null ? void 0 : l.height) != null ? s : this.props.height;
      let R = _ + (d ? c / this.props.transformScale : 0), D = W + (v ? f / this.props.transformScale : 0);
      var p = this.runConstraints(R, D);
      if (R = p[0], D = p[1], e === "onResizeStop" && this.lastSize) {
        var U = this.lastSize;
        R = U.width, D = U.height;
      }
      const Q = R !== _ || D !== W;
      e !== "onResizeStop" && (this.lastSize = {
        width: R,
        height: D
      });
      const j = typeof this.props[e] == "function" ? this.props[e] : null;
      j && !(e === "onResize" && !Q) && (n.persist == null || n.persist(), j(n, {
        node: u,
        size: {
          width: R,
          height: D
        },
        handle: r
      })), e === "onResizeStop" && this.resetData();
    };
  }
  // Render a resize handle given an axis & DOM ref. Ref *must* be attached for
  // the underlying draggable library to work properly.
  renderResizeHandle(e, r) {
    const n = this.props.handle;
    if (!n)
      return /* @__PURE__ */ Le.createElement("span", {
        className: "react-resizable-handle react-resizable-handle-" + e,
        ref: r
      });
    if (typeof n == "function")
      return n(e, r);
    const o = typeof n.type == "string", i = Lt({
      ref: r
    }, o ? {} : {
      handleAxis: e
    });
    return /* @__PURE__ */ Le.cloneElement(n, i);
  }
  render() {
    const e = this.props, r = e.children, n = e.className, o = e.draggableOpts;
    e.width, e.height, e.handle, e.handleSize, e.lockAspectRatio, e.axis, e.minConstraints, e.maxConstraints, e.onResize, e.onResizeStop, e.onResizeStart;
    const i = e.resizeHandles;
    e.transformScale;
    const a = $i(e, ki);
    return (0, Ti.cloneElement)(r, Lt(Lt({}, a), {}, {
      className: (n ? n + " " : "") + "react-resizable",
      children: [...Le.Children.toArray(r.props.children), ...i.map((s) => {
        var l;
        const u = (l = this.handleRefs[s]) != null ? l : this.handleRefs[s] = /* @__PURE__ */ Le.createRef();
        return /* @__PURE__ */ Le.createElement(ji.DraggableCore, er({}, o, {
          nodeRef: u,
          key: "resizableHandle-" + s,
          onStop: this.resizeHandler("onResizeStop", s),
          onStart: this.resizeHandler("onResizeStart", s),
          onDrag: this.resizeHandler("onResize", s)
        }), this.renderResizeHandle(s, u));
      })]
    }));
  }
}
Qe.default = br;
br.propTypes = Mi.resizableProps;
br.defaultProps = {
  axis: "both",
  handleSize: [20, 20],
  lockAspectRatio: !1,
  minConstraints: [20, 20],
  maxConstraints: [1 / 0, 1 / 0],
  resizeHandles: ["se"],
  transformScale: 1
};
var bt = {};
bt.__esModule = !0;
bt.default = void 0;
var Nt = Hn(de), Hi = An(Ee), Wi = An(Qe), qi = et;
const Ii = ["handle", "handleSize", "onResize", "onResizeStart", "onResizeStop", "draggableOpts", "minConstraints", "maxConstraints", "lockAspectRatio", "axis", "width", "height", "resizeHandles", "style", "transformScale"];
function An(t) {
  return t && t.__esModule ? t : { default: t };
}
function Hn(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (Hn = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var a, s, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (a = i ? n : r) {
      if (a.has(o)) return a.get(o);
      a.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((s = (a = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (s.get || s.set) ? a(l, u, s) : l[u] = o[u]);
    return l;
  })(t, e);
}
function tr() {
  return tr = Object.assign ? Object.assign.bind() : function(t) {
    for (var e = 1; e < arguments.length; e++) {
      var r = arguments[e];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (t[n] = r[n]);
    }
    return t;
  }, tr.apply(null, arguments);
}
function Kr(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function st(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? Kr(Object(r), !0).forEach(function(n) {
      Bi(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : Kr(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function Bi(t, e, r) {
  return (e = Gi(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function Gi(t) {
  var e = Yi(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function Yi(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
function Fi(t, e) {
  if (t == null) return {};
  var r = {};
  for (var n in t) if ({}.hasOwnProperty.call(t, n)) {
    if (e.indexOf(n) !== -1) continue;
    r[n] = t[n];
  }
  return r;
}
class Wn extends Nt.Component {
  constructor() {
    super(...arguments), this.state = {
      width: this.props.width,
      height: this.props.height,
      propsWidth: this.props.width,
      propsHeight: this.props.height
    }, this.onResize = (e, r) => {
      const n = r.size;
      this.props.onResize ? (e.persist == null || e.persist(), this.setState(n, () => this.props.onResize && this.props.onResize(e, r))) : this.setState(n);
    };
  }
  static getDerivedStateFromProps(e, r) {
    return r.propsWidth !== e.width || r.propsHeight !== e.height ? {
      width: e.width,
      height: e.height,
      propsWidth: e.width,
      propsHeight: e.height
    } : null;
  }
  render() {
    const e = this.props, r = e.handle, n = e.handleSize;
    e.onResize;
    const o = e.onResizeStart, i = e.onResizeStop, a = e.draggableOpts, s = e.minConstraints, l = e.maxConstraints, u = e.lockAspectRatio, c = e.axis;
    e.width, e.height;
    const f = e.resizeHandles, d = e.style, v = e.transformScale, m = Fi(e, Ii);
    return /* @__PURE__ */ Nt.createElement(Wi.default, {
      axis: c,
      draggableOpts: a,
      handle: r,
      handleSize: n,
      height: this.state.height,
      lockAspectRatio: u,
      maxConstraints: l,
      minConstraints: s,
      onResizeStart: o,
      onResize: this.onResize,
      onResizeStop: i,
      resizeHandles: f,
      transformScale: v,
      width: this.state.width
    }, /* @__PURE__ */ Nt.createElement("div", tr({}, m, {
      style: st(st({}, d), {}, {
        width: this.state.width + "px",
        height: this.state.height + "px"
      })
    })));
  }
}
bt.default = Wn;
Wn.propTypes = st(st({}, qi.resizableProps), {}, {
  children: Hi.default.element
});
vt.exports = function() {
  throw new Error("Don't instantiate Resizable directly! Use require('react-resizable').Resizable");
};
vt.exports.Resizable = Qe.default;
vt.exports.ResizableBox = bt.default;
var Ui = vt.exports, we = {};
Object.defineProperty(we, "__esModule", {
  value: !0
});
we.resizeHandleType = we.resizeHandleAxesType = we.default = void 0;
var M = qn(Ee), Xi = qn(de);
function qn(t) {
  return t && t.__esModule ? t : { default: t };
}
const Vi = we.resizeHandleAxesType = M.default.arrayOf(M.default.oneOf(["s", "w", "e", "n", "sw", "nw", "se", "ne"])), Ki = we.resizeHandleType = M.default.oneOfType([M.default.node, M.default.func]);
we.default = {
  //
  // Basic props
  //
  className: M.default.string,
  style: M.default.object,
  // This can be set explicitly. If it is not set, it will automatically
  // be set to the container width. Note that resizes will *not* cause this to adjust.
  // If you need that behavior, use WidthProvider.
  width: M.default.number,
  // If true, the container height swells and contracts to fit contents
  autoSize: M.default.bool,
  // # of cols.
  cols: M.default.number,
  // A selector that will not be draggable.
  draggableCancel: M.default.string,
  // A selector for the draggable handler
  draggableHandle: M.default.string,
  // Deprecated
  verticalCompact: function(t) {
    t.verticalCompact === !1 && process.env.NODE_ENV !== "production" && console.warn(
      // eslint-disable-line no-console
      '`verticalCompact` on <ReactGridLayout> is deprecated and will be removed soon. Use `compactType`: "horizontal" | "vertical" | null.'
    );
  },
  // Choose vertical or hotizontal compaction
  compactType: M.default.oneOf(["vertical", "horizontal"]),
  // layout is an array of object with the format:
  // {x: Number, y: Number, w: Number, h: Number, i: String}
  layout: function(t) {
    var e = t.layout;
    e !== void 0 && N.validateLayout(e, "layout");
  },
  //
  // Grid Dimensions
  //
  // Margin between items [x, y] in px
  margin: M.default.arrayOf(M.default.number),
  // Padding inside the container [x, y] in px
  containerPadding: M.default.arrayOf(M.default.number),
  // Rows have a static height, but you can change this based on breakpoints if you like
  rowHeight: M.default.number,
  // Default Infinity, but you can specify a max here if you like.
  // Note that this isn't fully fleshed out and won't error if you specify a layout that
  // extends beyond the row capacity. It will, however, not allow users to drag/resize
  // an item past the barrier. They can push items beyond the barrier, though.
  // Intentionally not documented for this reason.
  maxRows: M.default.number,
  //
  // Flags
  //
  isBounded: M.default.bool,
  isDraggable: M.default.bool,
  isResizable: M.default.bool,
  // If true, grid can be placed one over the other.
  allowOverlap: M.default.bool,
  // If true, grid items won't change position when being dragged over.
  preventCollision: M.default.bool,
  // Use CSS transforms instead of top/left
  useCSSTransforms: M.default.bool,
  // parent layout transform scale
  transformScale: M.default.number,
  // If true, an external element can trigger onDrop callback with a specific grid position as a parameter
  isDroppable: M.default.bool,
  // Resize handle options
  resizeHandles: Vi,
  resizeHandle: Ki,
  //
  // Callbacks
  //
  // Callback so you can save the layout. Calls after each drag & resize stops.
  onLayoutChange: M.default.func,
  // Calls when drag starts. Callback is of the signature (layout, oldItem, newItem, placeholder, e, ?node).
  // All callbacks below have the same signature. 'start' and 'stop' callbacks omit the 'placeholder'.
  onDragStart: M.default.func,
  // Calls on each drag movement.
  onDrag: M.default.func,
  // Calls when drag is complete.
  onDragStop: M.default.func,
  //Calls when resize starts.
  onResizeStart: M.default.func,
  // Calls when resize movement happens.
  onResize: M.default.func,
  // Calls when resize is complete.
  onResizeStop: M.default.func,
  // Calls when some element is dropped.
  onDrop: M.default.func,
  //
  // Other validations
  //
  droppingItem: M.default.shape({
    i: M.default.string.isRequired,
    w: M.default.number.isRequired,
    h: M.default.number.isRequired
  }),
  // Children must not have duplicate keys.
  children: function(t, e) {
    const r = t[e], n = {};
    Xi.default.Children.forEach(r, function(o) {
      if ((o == null ? void 0 : o.key) != null) {
        if (n[o.key])
          throw new Error('Duplicate child key "' + o.key + '" found! This will cause problems in ReactGridLayout.');
        n[o.key] = !0;
      }
    });
  },
  // Optional ref for getting a reference for the wrapping div.
  innerRef: M.default.any
};
Object.defineProperty(ht, "__esModule", {
  value: !0
});
ht.default = void 0;
var Ne = wr(de), Jr = ir, I = wr(Ee), Ji = Ln, Zi = Ui, Ae = N, J = me, Zr = we, Qi = wr(ct);
function wr(t) {
  return t && t.__esModule ? t : { default: t };
}
function Qr(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function At(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? Qr(Object(r), !0).forEach(function(n) {
      ce(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : Qr(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function ce(t, e, r) {
  return (e = ea(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function ea(t) {
  var e = ta(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function ta(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
class Or extends Ne.default.Component {
  constructor() {
    super(...arguments), ce(this, "state", {
      resizing: null,
      dragging: null,
      className: ""
    }), ce(this, "elementRef", /* @__PURE__ */ Ne.default.createRef()), ce(this, "onDragStart", (e, r) => {
      let {
        node: n
      } = r;
      const {
        onDragStart: o,
        transformScale: i
      } = this.props;
      if (!o) return;
      const a = {
        top: 0,
        left: 0
      }, {
        offsetParent: s
      } = n;
      if (!s) return;
      const l = s.getBoundingClientRect(), u = n.getBoundingClientRect(), c = u.left / i, f = l.left / i, d = u.top / i, v = l.top / i;
      a.left = c - f + s.scrollLeft, a.top = d - v + s.scrollTop, this.setState({
        dragging: a
      });
      const {
        x: m,
        y: O
      } = (0, J.calcXY)(this.getPositionParams(), a.top, a.left, this.props.w, this.props.h);
      return o.call(this, this.props.i, m, O, {
        e,
        node: n,
        newPosition: a
      });
    }), ce(this, "onDrag", (e, r, n) => {
      let {
        node: o,
        deltaX: i,
        deltaY: a
      } = r;
      const {
        onDrag: s
      } = this.props;
      if (!s) return;
      if (!this.state.dragging)
        throw new Error("onDrag called before onDragStart.");
      let l = this.state.dragging.top + a, u = this.state.dragging.left + i;
      const {
        isBounded: c,
        i: f,
        w: d,
        h: v,
        containerWidth: m
      } = this.props, O = this.getPositionParams();
      if (c) {
        const {
          offsetParent: R
        } = o;
        if (R) {
          const {
            margin: D,
            rowHeight: p
          } = this.props, U = R.clientHeight - (0, J.calcGridItemWHPx)(v, p, D[1]);
          l = (0, J.clamp)(l, 0, U);
          const Q = (0, J.calcGridColWidth)(O), j = m - (0, J.calcGridItemWHPx)(d, Q, D[0]);
          u = (0, J.clamp)(u, 0, j);
        }
      }
      const E = {
        top: l,
        left: u
      };
      n ? this.setState({
        dragging: E
      }) : (0, Jr.flushSync)(() => {
        this.setState({
          dragging: E
        });
      });
      const {
        x: _,
        y: W
      } = (0, J.calcXY)(O, l, u, d, v);
      return s.call(this, f, _, W, {
        e,
        node: o,
        newPosition: E
      });
    }), ce(this, "onDragStop", (e, r) => {
      let {
        node: n
      } = r;
      const {
        onDragStop: o
      } = this.props;
      if (!o) return;
      if (!this.state.dragging)
        throw new Error("onDragEnd called before onDragStart.");
      const {
        w: i,
        h: a,
        i: s
      } = this.props, {
        left: l,
        top: u
      } = this.state.dragging, c = {
        top: u,
        left: l
      };
      this.setState({
        dragging: null
      });
      const {
        x: f,
        y: d
      } = (0, J.calcXY)(this.getPositionParams(), u, l, i, a);
      return o.call(this, s, f, d, {
        e,
        node: n,
        newPosition: c
      });
    }), ce(this, "onResizeStop", (e, r, n) => this.onResizeHandler(e, r, n, "onResizeStop")), ce(this, "onResizeStart", (e, r, n) => this.onResizeHandler(e, r, n, "onResizeStart")), ce(this, "onResize", (e, r, n) => this.onResizeHandler(e, r, n, "onResize"));
  }
  shouldComponentUpdate(e, r) {
    if (this.props.children !== e.children || this.props.droppingPosition !== e.droppingPosition) return !0;
    const n = (0, J.calcGridItemPosition)(this.getPositionParams(this.props), this.props.x, this.props.y, this.props.w, this.props.h, this.state), o = (0, J.calcGridItemPosition)(this.getPositionParams(e), e.x, e.y, e.w, e.h, r);
    return !(0, Ae.fastPositionEqual)(n, o) || this.props.useCSSTransforms !== e.useCSSTransforms;
  }
  componentDidMount() {
    this.moveDroppingItem({});
  }
  componentDidUpdate(e) {
    this.moveDroppingItem(e);
  }
  // When a droppingPosition is present, this means we should fire a move event, as if we had moved
  // this element by `x, y` pixels.
  moveDroppingItem(e) {
    const {
      droppingPosition: r
    } = this.props;
    if (!r) return;
    const n = this.elementRef.current;
    if (!n) return;
    const o = e.droppingPosition || {
      left: 0,
      top: 0
    }, {
      dragging: i
    } = this.state, a = i && r.left !== o.left || r.top !== o.top;
    if (!i)
      this.onDragStart(r.e, {
        node: n,
        deltaX: r.left,
        deltaY: r.top
      });
    else if (a) {
      const s = r.left - i.left, l = r.top - i.top;
      this.onDrag(
        r.e,
        {
          node: n,
          deltaX: s,
          deltaY: l
        },
        !0
        // dontFLush: avoid flushSync to temper warnings
      );
    }
  }
  getPositionParams() {
    let e = arguments.length > 0 && arguments[0] !== void 0 ? arguments[0] : this.props;
    return {
      cols: e.cols,
      containerPadding: e.containerPadding,
      containerWidth: e.containerWidth,
      margin: e.margin,
      maxRows: e.maxRows,
      rowHeight: e.rowHeight
    };
  }
  /**
   * This is where we set the grid item's absolute placement. It gets a little tricky because we want to do it
   * well when server rendering, and the only way to do that properly is to use percentage width/left because
   * we don't know exactly what the browser viewport is.
   * Unfortunately, CSS Transforms, which are great for performance, break in this instance because a percentage
   * left is relative to the item itself, not its container! So we cannot use them on the server rendering pass.
   *
   * @param  {Object} pos Position object with width, height, left, top.
   * @return {Object}     Style object.
   */
  createStyle(e) {
    const {
      usePercentages: r,
      containerWidth: n,
      useCSSTransforms: o
    } = this.props;
    let i;
    return o ? i = (0, Ae.setTransform)(e) : (i = (0, Ae.setTopLeft)(e), r && (i.left = (0, Ae.perc)(e.left / n), i.width = (0, Ae.perc)(e.width / n))), i;
  }
  /**
   * Mix a Draggable instance into a child.
   * @param  {Element} child    Child element.
   * @return {Element}          Child wrapped in Draggable.
   */
  mixinDraggable(e, r) {
    return /* @__PURE__ */ Ne.default.createElement(Ji.DraggableCore, {
      disabled: !r,
      onStart: this.onDragStart,
      onDrag: this.onDrag,
      onStop: this.onDragStop,
      handle: this.props.handle,
      cancel: ".react-resizable-handle" + (this.props.cancel ? "," + this.props.cancel : ""),
      scale: this.props.transformScale,
      nodeRef: this.elementRef
    }, e);
  }
  /**
   * Utility function to setup callback handler definitions for
   * similarily structured resize events.
   */
  curryResizeHandler(e, r) {
    return (n, o) => (
      /*: Function*/
      r(n, o, e)
    );
  }
  /**
   * Mix a Resizable instance into a child.
   * @param  {Element} child    Child element.
   * @param  {Object} position  Position object (pixel values)
   * @return {Element}          Child wrapped in Resizable.
   */
  mixinResizable(e, r, n) {
    const {
      cols: o,
      minW: i,
      minH: a,
      maxW: s,
      maxH: l,
      transformScale: u,
      resizeHandles: c,
      resizeHandle: f
    } = this.props, d = this.getPositionParams(), v = (0, J.calcGridItemPosition)(d, 0, 0, o, 0).width, m = (0, J.calcGridItemPosition)(d, 0, 0, i, a), O = (0, J.calcGridItemPosition)(d, 0, 0, s, l), E = [m.width, m.height], _ = [Math.min(O.width, v), Math.min(O.height, 1 / 0)];
    return /* @__PURE__ */ Ne.default.createElement(
      Zi.Resizable,
      {
        draggableOpts: {
          disabled: !n
        },
        className: n ? void 0 : "react-resizable-hide",
        width: r.width,
        height: r.height,
        minConstraints: E,
        maxConstraints: _,
        onResizeStop: this.curryResizeHandler(r, this.onResizeStop),
        onResizeStart: this.curryResizeHandler(r, this.onResizeStart),
        onResize: this.curryResizeHandler(r, this.onResize),
        transformScale: u,
        resizeHandles: c,
        handle: f
      },
      e
    );
  }
  /**
   * Wrapper around resize events to provide more useful data.
   */
  onResizeHandler(e, r, n, o) {
    let {
      node: i,
      size: a,
      handle: s
    } = r;
    const l = this.props[o];
    if (!l) return;
    const {
      x: u,
      y: c,
      i: f,
      maxH: d,
      minH: v,
      containerWidth: m
    } = this.props, {
      minW: O,
      maxW: E
    } = this.props;
    let _ = a;
    i && (_ = (0, Ae.resizeItemInDirection)(s, n, a, m), (0, Jr.flushSync)(() => {
      this.setState({
        resizing: o === "onResizeStop" ? null : _
      });
    }));
    let {
      w: W,
      h: R
    } = (0, J.calcWH)(this.getPositionParams(), _.width, _.height, u, c, s);
    W = (0, J.clamp)(W, Math.max(O, 1), E), R = (0, J.clamp)(R, v, d), l.call(this, f, W, R, {
      e,
      node: i,
      size: _,
      handle: s
    });
  }
  render() {
    const {
      x: e,
      y: r,
      w: n,
      h: o,
      isDraggable: i,
      isResizable: a,
      droppingPosition: s,
      useCSSTransforms: l
    } = this.props, u = (0, J.calcGridItemPosition)(this.getPositionParams(), e, r, n, o, this.state), c = Ne.default.Children.only(this.props.children);
    let f = /* @__PURE__ */ Ne.default.cloneElement(c, {
      ref: this.elementRef,
      className: (0, Qi.default)("react-grid-item", c.props.className, this.props.className, {
        static: this.props.static,
        resizing: !!this.state.resizing,
        "react-draggable": i,
        "react-draggable-dragging": !!this.state.dragging,
        dropping: !!s,
        cssTransforms: l
      }),
      // We can set the width and height on the child, but unfortunately we can't set the position.
      style: At(At(At({}, this.props.style), c.props.style), this.createStyle(u))
    });
    return f = this.mixinResizable(f, u, a), f = this.mixinDraggable(f, i), f;
  }
}
ht.default = Or;
ce(Or, "propTypes", {
  // Children must be only a single element
  children: I.default.element,
  // General grid attributes
  cols: I.default.number.isRequired,
  containerWidth: I.default.number.isRequired,
  rowHeight: I.default.number.isRequired,
  margin: I.default.array.isRequired,
  maxRows: I.default.number.isRequired,
  containerPadding: I.default.array.isRequired,
  // These are all in grid units
  x: I.default.number.isRequired,
  y: I.default.number.isRequired,
  w: I.default.number.isRequired,
  h: I.default.number.isRequired,
  // All optional
  minW: function(t, e) {
    const r = t[e];
    if (typeof r != "number") return new Error("minWidth not Number");
    if (r > t.w || r > t.maxW) return new Error("minWidth larger than item width/maxWidth");
  },
  maxW: function(t, e) {
    const r = t[e];
    if (typeof r != "number") return new Error("maxWidth not Number");
    if (r < t.w || r < t.minW) return new Error("maxWidth smaller than item width/minWidth");
  },
  minH: function(t, e) {
    const r = t[e];
    if (typeof r != "number") return new Error("minHeight not Number");
    if (r > t.h || r > t.maxH) return new Error("minHeight larger than item height/maxHeight");
  },
  maxH: function(t, e) {
    const r = t[e];
    if (typeof r != "number") return new Error("maxHeight not Number");
    if (r < t.h || r < t.minH) return new Error("maxHeight smaller than item height/minHeight");
  },
  // ID is nice to have for callbacks
  i: I.default.string.isRequired,
  // Resize handle options
  resizeHandles: Zr.resizeHandleAxesType,
  resizeHandle: Zr.resizeHandleType,
  // Functions
  onDragStop: I.default.func,
  onDragStart: I.default.func,
  onDrag: I.default.func,
  onResizeStop: I.default.func,
  onResizeStart: I.default.func,
  onResize: I.default.func,
  // Flags
  isDraggable: I.default.bool.isRequired,
  isResizable: I.default.bool.isRequired,
  isBounded: I.default.bool.isRequired,
  static: I.default.bool,
  // Use CSS transforms instead of top/left
  useCSSTransforms: I.default.bool.isRequired,
  transformScale: I.default.number,
  // Others
  className: I.default.string,
  // Selector for draggable handle
  handle: I.default.string,
  // Selector for draggable cancel (see react-draggable)
  cancel: I.default.string,
  // Current position of a dropping element
  droppingPosition: I.default.shape({
    e: I.default.object.isRequired,
    left: I.default.number.isRequired,
    top: I.default.number.isRequired
  })
});
ce(Or, "defaultProps", {
  className: "",
  cancel: "",
  handle: "",
  minH: 1,
  minW: 1,
  maxH: 1 / 0,
  maxW: 1 / 0,
  transformScale: 1
});
Object.defineProperty(Ze, "__esModule", {
  value: !0
});
Ze.default = void 0;
var Pe = In(de), Ht = sr, ra = Sr(ct), x = N, na = me, en = Sr(ht), oa = Sr(we);
function Sr(t) {
  return t && t.__esModule ? t : { default: t };
}
function In(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (In = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var a, s, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (a = i ? n : r) {
      if (a.has(o)) return a.get(o);
      a.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((s = (a = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (s.get || s.set) ? a(l, u, s) : l[u] = o[u]);
    return l;
  })(t, e);
}
function tn(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function De(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? tn(Object(r), !0).forEach(function(n) {
      Z(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : tn(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function Z(t, e, r) {
  return (e = ia(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function ia(t) {
  var e = aa(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function aa(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
const rn = "react-grid-layout";
let Bn = !1;
try {
  Bn = /firefox/i.test(navigator.userAgent);
} catch {
}
class wt extends Pe.Component {
  constructor() {
    super(...arguments), Z(this, "state", {
      activeDrag: null,
      layout: (0, x.synchronizeLayoutWithChildren)(
        this.props.layout,
        this.props.children,
        this.props.cols,
        // Legacy support for verticalCompact: false
        (0, x.compactType)(this.props),
        this.props.allowOverlap
      ),
      mounted: !1,
      oldDragItem: null,
      oldLayout: null,
      oldResizeItem: null,
      resizing: !1,
      droppingDOMNode: null,
      children: []
    }), Z(this, "dragEnterCounter", 0), Z(this, "onDragStart", (e, r, n, o) => {
      let {
        e: i,
        node: a
      } = o;
      const {
        layout: s
      } = this.state, l = (0, x.getLayoutItem)(s, e);
      if (!l) return;
      const u = {
        w: l.w,
        h: l.h,
        x: l.x,
        y: l.y,
        placeholder: !0,
        i: e
      };
      return this.setState({
        oldDragItem: (0, x.cloneLayoutItem)(l),
        oldLayout: s,
        activeDrag: u
      }), this.props.onDragStart(s, l, l, null, i, a);
    }), Z(this, "onDrag", (e, r, n, o) => {
      let {
        e: i,
        node: a
      } = o;
      const {
        oldDragItem: s
      } = this.state;
      let {
        layout: l
      } = this.state;
      const {
        cols: u,
        allowOverlap: c,
        preventCollision: f
      } = this.props, d = (0, x.getLayoutItem)(l, e);
      if (!d) return;
      const v = {
        w: d.w,
        h: d.h,
        x: d.x,
        y: d.y,
        placeholder: !0,
        i: e
      };
      l = (0, x.moveElement)(l, d, r, n, !0, f, (0, x.compactType)(this.props), u, c), this.props.onDrag(l, s, d, v, i, a), this.setState({
        layout: c ? l : (0, x.compact)(l, (0, x.compactType)(this.props), u),
        activeDrag: v
      });
    }), Z(this, "onDragStop", (e, r, n, o) => {
      let {
        e: i,
        node: a
      } = o;
      if (!this.state.activeDrag) return;
      const {
        oldDragItem: s
      } = this.state;
      let {
        layout: l
      } = this.state;
      const {
        cols: u,
        preventCollision: c,
        allowOverlap: f
      } = this.props, d = (0, x.getLayoutItem)(l, e);
      if (!d) return;
      l = (0, x.moveElement)(l, d, r, n, !0, c, (0, x.compactType)(this.props), u, f);
      const m = f ? l : (0, x.compact)(l, (0, x.compactType)(this.props), u);
      this.props.onDragStop(m, s, d, null, i, a);
      const {
        oldLayout: O
      } = this.state;
      this.setState({
        activeDrag: null,
        layout: m,
        oldDragItem: null,
        oldLayout: null
      }), this.onLayoutMaybeChanged(m, O);
    }), Z(this, "onResizeStart", (e, r, n, o) => {
      let {
        e: i,
        node: a
      } = o;
      const {
        layout: s
      } = this.state, l = (0, x.getLayoutItem)(s, e);
      l && (this.setState({
        oldResizeItem: (0, x.cloneLayoutItem)(l),
        oldLayout: this.state.layout,
        resizing: !0
      }), this.props.onResizeStart(s, l, l, null, i, a));
    }), Z(this, "onResize", (e, r, n, o) => {
      let {
        e: i,
        node: a,
        size: s,
        handle: l
      } = o;
      const {
        oldResizeItem: u
      } = this.state, {
        layout: c
      } = this.state, {
        cols: f,
        preventCollision: d,
        allowOverlap: v
      } = this.props;
      let m = !1, O, E, _;
      const [W, R] = (0, x.withLayoutItem)(c, e, (p) => {
        let U;
        return E = p.x, _ = p.y, ["sw", "w", "nw", "n", "ne"].indexOf(l) !== -1 && (["sw", "nw", "w"].indexOf(l) !== -1 && (E = p.x + (p.w - r), r = p.x !== E && E < 0 ? p.w : r, E = E < 0 ? 0 : E), ["ne", "n", "nw"].indexOf(l) !== -1 && (_ = p.y + (p.h - n), n = p.y !== _ && _ < 0 ? p.h : n, _ = _ < 0 ? 0 : _), m = !0), d && !v && (U = (0, x.getAllCollisions)(c, De(De({}, p), {}, {
          w: r,
          h: n,
          x: E,
          y: _
        })).filter((j) => j.i !== p.i).length > 0, U && (_ = p.y, n = p.h, E = p.x, r = p.w, m = !1)), p.w = r, p.h = n, p;
      });
      if (!R) return;
      O = W, m && (O = (0, x.moveElement)(W, R, E, _, !0, this.props.preventCollision, (0, x.compactType)(this.props), f, v));
      const D = {
        w: R.w,
        h: R.h,
        x: R.x,
        y: R.y,
        static: !0,
        i: e
      };
      this.props.onResize(O, u, R, D, i, a), this.setState({
        layout: v ? O : (0, x.compact)(O, (0, x.compactType)(this.props), f),
        activeDrag: D
      });
    }), Z(this, "onResizeStop", (e, r, n, o) => {
      let {
        e: i,
        node: a
      } = o;
      const {
        layout: s,
        oldResizeItem: l
      } = this.state, {
        cols: u,
        allowOverlap: c
      } = this.props, f = (0, x.getLayoutItem)(s, e), d = c ? s : (0, x.compact)(s, (0, x.compactType)(this.props), u);
      this.props.onResizeStop(d, l, f, null, i, a);
      const {
        oldLayout: v
      } = this.state;
      this.setState({
        activeDrag: null,
        layout: d,
        oldResizeItem: null,
        oldLayout: null,
        resizing: !1
      }), this.onLayoutMaybeChanged(d, v);
    }), Z(this, "onDragOver", (e) => {
      var r;
      if (e.preventDefault(), e.stopPropagation(), Bn && // $FlowIgnore can't figure this out
      !((r = e.nativeEvent.target) !== null && r !== void 0 && r.classList.contains(rn)))
        return !1;
      const {
        droppingItem: n,
        onDropDragOver: o,
        margin: i,
        cols: a,
        rowHeight: s,
        maxRows: l,
        width: u,
        containerPadding: c,
        transformScale: f
      } = this.props, d = o == null ? void 0 : o(e);
      if (d === !1)
        return this.state.droppingDOMNode && this.removeDroppingPlaceholder(), !1;
      const v = De(De({}, n), d), {
        layout: m
      } = this.state, O = e.currentTarget.getBoundingClientRect(), E = e.clientX - O.left, _ = e.clientY - O.top, W = {
        left: E / f,
        top: _ / f,
        e
      };
      if (this.state.droppingDOMNode) {
        if (this.state.droppingPosition) {
          const {
            left: R,
            top: D
          } = this.state.droppingPosition;
          (R != E || D != _) && this.setState({
            droppingPosition: W
          });
        }
      } else {
        const R = {
          cols: a,
          margin: i,
          maxRows: l,
          rowHeight: s,
          containerWidth: u,
          containerPadding: c || i
        }, D = (0, na.calcXY)(R, _, E, v.w, v.h);
        this.setState({
          droppingDOMNode: /* @__PURE__ */ Pe.createElement("div", {
            key: v.i
          }),
          droppingPosition: W,
          layout: [...m, De(De({}, v), {}, {
            x: D.x,
            y: D.y,
            static: !1,
            isDraggable: !0
          })]
        });
      }
    }), Z(this, "removeDroppingPlaceholder", () => {
      const {
        droppingItem: e,
        cols: r
      } = this.props, {
        layout: n
      } = this.state, o = (0, x.compact)(n.filter((i) => i.i !== e.i), (0, x.compactType)(this.props), r, this.props.allowOverlap);
      this.setState({
        layout: o,
        droppingDOMNode: null,
        activeDrag: null,
        droppingPosition: void 0
      });
    }), Z(this, "onDragLeave", (e) => {
      e.preventDefault(), e.stopPropagation(), this.dragEnterCounter--, this.dragEnterCounter === 0 && this.removeDroppingPlaceholder();
    }), Z(this, "onDragEnter", (e) => {
      e.preventDefault(), e.stopPropagation(), this.dragEnterCounter++;
    }), Z(this, "onDrop", (e) => {
      e.preventDefault(), e.stopPropagation();
      const {
        droppingItem: r
      } = this.props, {
        layout: n
      } = this.state, o = n.find((i) => i.i === r.i);
      this.dragEnterCounter = 0, this.removeDroppingPlaceholder(), this.props.onDrop(n, o, e);
    });
  }
  componentDidMount() {
    this.setState({
      mounted: !0
    }), this.onLayoutMaybeChanged(this.state.layout, this.props.layout);
  }
  static getDerivedStateFromProps(e, r) {
    let n;
    return r.activeDrag ? null : (!(0, Ht.deepEqual)(e.layout, r.propsLayout) || e.compactType !== r.compactType ? n = e.layout : (0, x.childrenEqual)(e.children, r.children) || (n = r.layout), n ? {
      layout: (0, x.synchronizeLayoutWithChildren)(n, e.children, e.cols, (0, x.compactType)(e), e.allowOverlap),
      // We need to save these props to state for using
      // getDerivedStateFromProps instead of componentDidMount (in which we would get extra rerender)
      compactType: e.compactType,
      children: e.children,
      propsLayout: e.layout
    } : null);
  }
  shouldComponentUpdate(e, r) {
    return (
      // NOTE: this is almost always unequal. Therefore the only way to get better performance
      // from SCU is if the user intentionally memoizes children. If they do, and they can
      // handle changes properly, performance will increase.
      this.props.children !== e.children || !(0, x.fastRGLPropsEqual)(this.props, e, Ht.deepEqual) || this.state.activeDrag !== r.activeDrag || this.state.mounted !== r.mounted || this.state.droppingPosition !== r.droppingPosition
    );
  }
  componentDidUpdate(e, r) {
    if (!this.state.activeDrag) {
      const n = this.state.layout, o = r.layout;
      this.onLayoutMaybeChanged(n, o);
    }
  }
  /**
   * Calculates a pixel value for the container.
   * @return {String} Container height in pixels.
   */
  containerHeight() {
    if (!this.props.autoSize) return;
    const e = (0, x.bottom)(this.state.layout), r = this.props.containerPadding ? this.props.containerPadding[1] : this.props.margin[1];
    return e * this.props.rowHeight + (e - 1) * this.props.margin[1] + r * 2 + "px";
  }
  onLayoutMaybeChanged(e, r) {
    r || (r = this.state.layout), (0, Ht.deepEqual)(r, e) || this.props.onLayoutChange(e);
  }
  /**
   * Create a placeholder object.
   * @return {Element} Placeholder div.
   */
  placeholder() {
    const {
      activeDrag: e
    } = this.state;
    if (!e) return null;
    const {
      width: r,
      cols: n,
      margin: o,
      containerPadding: i,
      rowHeight: a,
      maxRows: s,
      useCSSTransforms: l,
      transformScale: u
    } = this.props;
    return /* @__PURE__ */ Pe.createElement(en.default, {
      w: e.w,
      h: e.h,
      x: e.x,
      y: e.y,
      i: e.i,
      className: "react-grid-placeholder ".concat(this.state.resizing ? "placeholder-resizing" : ""),
      containerWidth: r,
      cols: n,
      margin: o,
      containerPadding: i || o,
      maxRows: s,
      rowHeight: a,
      isDraggable: !1,
      isResizable: !1,
      isBounded: !1,
      useCSSTransforms: l,
      transformScale: u
    }, /* @__PURE__ */ Pe.createElement("div", null));
  }
  /**
   * Given a grid item, set its style attributes & surround in a <Draggable>.
   * @param  {Element} child React element.
   * @return {Element}       Element wrapped in draggable and properly placed.
   */
  processGridItem(e, r) {
    if (!e || !e.key) return;
    const n = (0, x.getLayoutItem)(this.state.layout, String(e.key));
    if (!n) return null;
    const {
      width: o,
      cols: i,
      margin: a,
      containerPadding: s,
      rowHeight: l,
      maxRows: u,
      isDraggable: c,
      isResizable: f,
      isBounded: d,
      useCSSTransforms: v,
      transformScale: m,
      draggableCancel: O,
      draggableHandle: E,
      resizeHandles: _,
      resizeHandle: W
    } = this.props, {
      mounted: R,
      droppingPosition: D
    } = this.state, p = typeof n.isDraggable == "boolean" ? n.isDraggable : !n.static && c, U = typeof n.isResizable == "boolean" ? n.isResizable : !n.static && f, Q = n.resizeHandles || _, j = p && d && n.isBounded !== !1;
    return /* @__PURE__ */ Pe.createElement(en.default, {
      containerWidth: o,
      cols: i,
      margin: a,
      containerPadding: s || a,
      maxRows: u,
      rowHeight: l,
      cancel: O,
      handle: E,
      onDragStop: this.onDragStop,
      onDragStart: this.onDragStart,
      onDrag: this.onDrag,
      onResizeStart: this.onResizeStart,
      onResize: this.onResize,
      onResizeStop: this.onResizeStop,
      isDraggable: p,
      isResizable: U,
      isBounded: j,
      useCSSTransforms: v && R,
      usePercentages: !R,
      transformScale: m,
      w: n.w,
      h: n.h,
      x: n.x,
      y: n.y,
      i: n.i,
      minH: n.minH,
      minW: n.minW,
      maxH: n.maxH,
      maxW: n.maxW,
      static: n.static,
      droppingPosition: r ? D : void 0,
      resizeHandles: Q,
      resizeHandle: W
    }, e);
  }
  render() {
    const {
      className: e,
      style: r,
      isDroppable: n,
      innerRef: o
    } = this.props, i = (0, ra.default)(rn, e), a = De({
      height: this.containerHeight()
    }, r);
    return /* @__PURE__ */ Pe.createElement("div", {
      ref: o,
      className: i,
      style: a,
      onDrop: n ? this.onDrop : x.noop,
      onDragLeave: n ? this.onDragLeave : x.noop,
      onDragEnter: n ? this.onDragEnter : x.noop,
      onDragOver: n ? this.onDragOver : x.noop
    }, Pe.Children.map(this.props.children, (s) => this.processGridItem(s)), n && this.state.droppingDOMNode && this.processGridItem(this.state.droppingDOMNode, !0), this.placeholder());
  }
}
Ze.default = wt;
Z(wt, "displayName", "ReactGridLayout");
Z(wt, "propTypes", oa.default);
Z(wt, "defaultProps", {
  autoSize: !0,
  cols: 12,
  className: "",
  style: {},
  draggableHandle: "",
  draggableCancel: "",
  containerPadding: null,
  rowHeight: 150,
  maxRows: 1 / 0,
  // infinite vertical growth
  layout: [],
  margin: [10, 10],
  isBounded: !1,
  isDraggable: !0,
  isResizable: !0,
  allowOverlap: !1,
  isDroppable: !1,
  useCSSTransforms: !0,
  transformScale: 1,
  verticalCompact: !0,
  compactType: "vertical",
  preventCollision: !1,
  droppingItem: {
    i: "__dropping-elem__",
    h: 1,
    w: 1
  },
  resizeHandles: ["se"],
  onLayoutChange: x.noop,
  onDragStart: x.noop,
  onDrag: x.noop,
  onDragStop: x.noop,
  onResizeStart: x.noop,
  onResize: x.noop,
  onResizeStop: x.noop,
  onDrop: x.noop,
  onDropDragOver: x.noop
});
var Ot = {}, ke = {};
Object.defineProperty(ke, "__esModule", {
  value: !0
});
ke.findOrGenerateResponsiveLayout = ua;
ke.getBreakpointFromWidth = sa;
ke.getColsFromBreakpoint = la;
ke.sortBreakpoints = _r;
var nt = N;
function sa(t, e) {
  const r = _r(t);
  let n = r[0];
  for (let o = 1, i = r.length; o < i; o++) {
    const a = r[o];
    e > t[a] && (n = a);
  }
  return n;
}
function la(t, e) {
  if (!e[t])
    throw new Error("ResponsiveReactGridLayout: `cols` entry for breakpoint " + t + " is missing!");
  return e[t];
}
function ua(t, e, r, n, o, i) {
  if (t[r]) return (0, nt.cloneLayout)(t[r]);
  let a = t[n];
  const s = _r(e), l = s.slice(s.indexOf(r));
  for (let u = 0, c = l.length; u < c; u++) {
    const f = l[u];
    if (t[f]) {
      a = t[f];
      break;
    }
  }
  return a = (0, nt.cloneLayout)(a || []), (0, nt.compact)((0, nt.correctBounds)(a, {
    cols: o
  }), i, o);
}
function _r(t) {
  return Object.keys(t).sort(function(r, n) {
    return t[r] - t[n];
  });
}
Object.defineProperty(Ot, "__esModule", {
  value: !0
});
Ot.default = void 0;
var nn = Yn(de), ae = Gn(Ee), Wt = sr, qe = N, Ce = ke, ca = Gn(Ze);
const fa = ["breakpoint", "breakpoints", "cols", "layouts", "margin", "containerPadding", "onBreakpointChange", "onLayoutChange", "onWidthChange"];
function Gn(t) {
  return t && t.__esModule ? t : { default: t };
}
function Yn(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (Yn = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var a, s, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (a = i ? n : r) {
      if (a.has(o)) return a.get(o);
      a.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((s = (a = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (s.get || s.set) ? a(l, u, s) : l[u] = o[u]);
    return l;
  })(t, e);
}
function rr() {
  return rr = Object.assign ? Object.assign.bind() : function(t) {
    for (var e = 1; e < arguments.length; e++) {
      var r = arguments[e];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (t[n] = r[n]);
    }
    return t;
  }, rr.apply(null, arguments);
}
function da(t, e) {
  if (t == null) return {};
  var r, n, o = pa(t, e);
  if (Object.getOwnPropertySymbols) {
    var i = Object.getOwnPropertySymbols(t);
    for (n = 0; n < i.length; n++) r = i[n], e.indexOf(r) === -1 && {}.propertyIsEnumerable.call(t, r) && (o[r] = t[r]);
  }
  return o;
}
function pa(t, e) {
  if (t == null) return {};
  var r = {};
  for (var n in t) if ({}.hasOwnProperty.call(t, n)) {
    if (e.indexOf(n) !== -1) continue;
    r[n] = t[n];
  }
  return r;
}
function on(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function qt(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? on(Object(r), !0).forEach(function(n) {
      Ke(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : on(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function Ke(t, e, r) {
  return (e = ha(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function ha(t) {
  var e = ga(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function ga(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
const an = (t) => Object.prototype.toString.call(t);
function ot(t, e) {
  return t == null ? null : Array.isArray(t) ? t : t[e];
}
class Rr extends nn.Component {
  constructor() {
    super(...arguments), Ke(this, "state", this.generateInitialState()), Ke(this, "onLayoutChange", (e) => {
      this.props.onLayoutChange(e, qt(qt({}, this.props.layouts), {}, {
        [this.state.breakpoint]: e
      }));
    });
  }
  generateInitialState() {
    const {
      width: e,
      breakpoints: r,
      layouts: n,
      cols: o
    } = this.props, i = (0, Ce.getBreakpointFromWidth)(r, e), a = (0, Ce.getColsFromBreakpoint)(i, o), s = this.props.verticalCompact === !1 ? null : this.props.compactType;
    return {
      layout: (0, Ce.findOrGenerateResponsiveLayout)(n, r, i, i, a, s),
      breakpoint: i,
      cols: a
    };
  }
  static getDerivedStateFromProps(e, r) {
    if (!(0, Wt.deepEqual)(e.layouts, r.layouts)) {
      const {
        breakpoint: n,
        cols: o
      } = r;
      return {
        layout: (0, Ce.findOrGenerateResponsiveLayout)(e.layouts, e.breakpoints, n, n, o, e.compactType),
        layouts: e.layouts
      };
    }
    return null;
  }
  componentDidUpdate(e) {
    (this.props.width != e.width || this.props.breakpoint !== e.breakpoint || !(0, Wt.deepEqual)(this.props.breakpoints, e.breakpoints) || !(0, Wt.deepEqual)(this.props.cols, e.cols)) && this.onWidthChange(e);
  }
  /**
   * When the width changes work through breakpoints and reset state with the new width & breakpoint.
   * Width changes are necessary to figure out the widget widths.
   */
  onWidthChange(e) {
    const {
      breakpoints: r,
      cols: n,
      layouts: o,
      compactType: i
    } = this.props, a = this.props.breakpoint || (0, Ce.getBreakpointFromWidth)(this.props.breakpoints, this.props.width), s = this.state.breakpoint, l = (0, Ce.getColsFromBreakpoint)(a, n), u = qt({}, o);
    if (s !== a || e.breakpoints !== r || e.cols !== n) {
      s in u || (u[s] = (0, qe.cloneLayout)(this.state.layout));
      let d = (0, Ce.findOrGenerateResponsiveLayout)(u, r, a, s, l, i);
      d = (0, qe.synchronizeLayoutWithChildren)(d, this.props.children, l, i, this.props.allowOverlap), u[a] = d, this.props.onBreakpointChange(a, l), this.props.onLayoutChange(d, u), this.setState({
        breakpoint: a,
        layout: d,
        cols: l
      });
    }
    const c = ot(this.props.margin, a), f = ot(this.props.containerPadding, a);
    this.props.onWidthChange(this.props.width, c, l, f);
  }
  render() {
    const e = this.props, {
      breakpoint: r,
      breakpoints: n,
      cols: o,
      layouts: i,
      margin: a,
      containerPadding: s,
      onBreakpointChange: l,
      onLayoutChange: u,
      onWidthChange: c
    } = e, f = da(e, fa);
    return /* @__PURE__ */ nn.createElement(ca.default, rr({}, f, {
      // $FlowIgnore should allow nullable here due to DefaultProps
      margin: ot(a, this.state.breakpoint),
      containerPadding: ot(s, this.state.breakpoint),
      onLayoutChange: this.onLayoutChange,
      layout: this.state.layout,
      cols: this.state.cols
    }));
  }
}
Ot.default = Rr;
Ke(Rr, "propTypes", {
  //
  // Basic props
  //
  // Optional, but if you are managing width yourself you may want to set the breakpoint
  // yourself as well.
  breakpoint: ae.default.string,
  // {name: pxVal}, e.g. {lg: 1200, md: 996, sm: 768, xs: 480}
  breakpoints: ae.default.object,
  allowOverlap: ae.default.bool,
  // # of cols. This is a breakpoint -> cols map
  cols: ae.default.object,
  // # of margin. This is a breakpoint -> margin map
  // e.g. { lg: [5, 5], md: [10, 10], sm: [15, 15] }
  // Margin between items [x, y] in px
  // e.g. [10, 10]
  margin: ae.default.oneOfType([ae.default.array, ae.default.object]),
  // # of containerPadding. This is a breakpoint -> containerPadding map
  // e.g. { lg: [5, 5], md: [10, 10], sm: [15, 15] }
  // Padding inside the container [x, y] in px
  // e.g. [10, 10]
  containerPadding: ae.default.oneOfType([ae.default.array, ae.default.object]),
  // layouts is an object mapping breakpoints to layouts.
  // e.g. {lg: Layout, md: Layout, ...}
  layouts(t, e) {
    if (an(t[e]) !== "[object Object]")
      throw new Error("Layout property must be an object. Received: " + an(t[e]));
    Object.keys(t[e]).forEach((r) => {
      if (!(r in t.breakpoints))
        throw new Error("Each key in layouts must align with a key in breakpoints.");
      (0, qe.validateLayout)(t.layouts[r], "layouts." + r);
    });
  },
  // The width of this component.
  // Required in this propTypes stanza because generateInitialState() will fail without it.
  width: ae.default.number.isRequired,
  //
  // Callbacks
  //
  // Calls back with breakpoint and new # cols
  onBreakpointChange: ae.default.func,
  // Callback so you can save the layout.
  // Calls back with (currentLayout, allLayouts). allLayouts are keyed by breakpoint.
  onLayoutChange: ae.default.func,
  // Calls back with (containerWidth, margin, cols, containerPadding)
  onWidthChange: ae.default.func
});
Ke(Rr, "defaultProps", {
  breakpoints: {
    lg: 1200,
    md: 996,
    sm: 768,
    xs: 480,
    xxs: 0
  },
  cols: {
    lg: 12,
    md: 10,
    sm: 6,
    xs: 4,
    xxs: 2
  },
  containerPadding: {
    lg: null,
    md: null,
    sm: null,
    xs: null,
    xxs: null
  },
  layouts: {},
  margin: [10, 10],
  allowOverlap: !1,
  onBreakpointChange: qe.noop,
  onLayoutChange: qe.noop,
  onWidthChange: qe.noop
});
var Er = {}, Fn = function() {
  if (typeof Map < "u")
    return Map;
  function t(e, r) {
    var n = -1;
    return e.some(function(o, i) {
      return o[0] === r ? (n = i, !0) : !1;
    }), n;
  }
  return (
    /** @class */
    function() {
      function e() {
        this.__entries__ = [];
      }
      return Object.defineProperty(e.prototype, "size", {
        /**
         * @returns {boolean}
         */
        get: function() {
          return this.__entries__.length;
        },
        enumerable: !0,
        configurable: !0
      }), e.prototype.get = function(r) {
        var n = t(this.__entries__, r), o = this.__entries__[n];
        return o && o[1];
      }, e.prototype.set = function(r, n) {
        var o = t(this.__entries__, r);
        ~o ? this.__entries__[o][1] = n : this.__entries__.push([r, n]);
      }, e.prototype.delete = function(r) {
        var n = this.__entries__, o = t(n, r);
        ~o && n.splice(o, 1);
      }, e.prototype.has = function(r) {
        return !!~t(this.__entries__, r);
      }, e.prototype.clear = function() {
        this.__entries__.splice(0);
      }, e.prototype.forEach = function(r, n) {
        n === void 0 && (n = null);
        for (var o = 0, i = this.__entries__; o < i.length; o++) {
          var a = i[o];
          r.call(n, a[1], a[0]);
        }
      }, e;
    }()
  );
}(), nr = typeof window < "u" && typeof document < "u" && window.document === document, lt = function() {
  return typeof global < "u" && global.Math === Math ? global : typeof self < "u" && self.Math === Math ? self : typeof window < "u" && window.Math === Math ? window : Function("return this")();
}(), ya = function() {
  return typeof requestAnimationFrame == "function" ? requestAnimationFrame.bind(lt) : function(t) {
    return setTimeout(function() {
      return t(Date.now());
    }, 1e3 / 60);
  };
}(), ma = 2;
function va(t, e) {
  var r = !1, n = !1, o = 0;
  function i() {
    r && (r = !1, t()), n && s();
  }
  function a() {
    ya(i);
  }
  function s() {
    var l = Date.now();
    if (r) {
      if (l - o < ma)
        return;
      n = !0;
    } else
      r = !0, n = !1, setTimeout(a, e);
    o = l;
  }
  return s;
}
var ba = 20, wa = ["top", "right", "bottom", "left", "width", "height", "size", "weight"], Oa = typeof MutationObserver < "u", Sa = (
  /** @class */
  function() {
    function t() {
      this.connected_ = !1, this.mutationEventsAdded_ = !1, this.mutationsObserver_ = null, this.observers_ = [], this.onTransitionEnd_ = this.onTransitionEnd_.bind(this), this.refresh = va(this.refresh.bind(this), ba);
    }
    return t.prototype.addObserver = function(e) {
      ~this.observers_.indexOf(e) || this.observers_.push(e), this.connected_ || this.connect_();
    }, t.prototype.removeObserver = function(e) {
      var r = this.observers_, n = r.indexOf(e);
      ~n && r.splice(n, 1), !r.length && this.connected_ && this.disconnect_();
    }, t.prototype.refresh = function() {
      var e = this.updateObservers_();
      e && this.refresh();
    }, t.prototype.updateObservers_ = function() {
      var e = this.observers_.filter(function(r) {
        return r.gatherActive(), r.hasActive();
      });
      return e.forEach(function(r) {
        return r.broadcastActive();
      }), e.length > 0;
    }, t.prototype.connect_ = function() {
      !nr || this.connected_ || (document.addEventListener("transitionend", this.onTransitionEnd_), window.addEventListener("resize", this.refresh), Oa ? (this.mutationsObserver_ = new MutationObserver(this.refresh), this.mutationsObserver_.observe(document, {
        attributes: !0,
        childList: !0,
        characterData: !0,
        subtree: !0
      })) : (document.addEventListener("DOMSubtreeModified", this.refresh), this.mutationEventsAdded_ = !0), this.connected_ = !0);
    }, t.prototype.disconnect_ = function() {
      !nr || !this.connected_ || (document.removeEventListener("transitionend", this.onTransitionEnd_), window.removeEventListener("resize", this.refresh), this.mutationsObserver_ && this.mutationsObserver_.disconnect(), this.mutationEventsAdded_ && document.removeEventListener("DOMSubtreeModified", this.refresh), this.mutationsObserver_ = null, this.mutationEventsAdded_ = !1, this.connected_ = !1);
    }, t.prototype.onTransitionEnd_ = function(e) {
      var r = e.propertyName, n = r === void 0 ? "" : r, o = wa.some(function(i) {
        return !!~n.indexOf(i);
      });
      o && this.refresh();
    }, t.getInstance = function() {
      return this.instance_ || (this.instance_ = new t()), this.instance_;
    }, t.instance_ = null, t;
  }()
), Un = function(t, e) {
  for (var r = 0, n = Object.keys(e); r < n.length; r++) {
    var o = n[r];
    Object.defineProperty(t, o, {
      value: e[o],
      enumerable: !1,
      writable: !1,
      configurable: !0
    });
  }
  return t;
}, Ie = function(t) {
  var e = t && t.ownerDocument && t.ownerDocument.defaultView;
  return e || lt;
}, Xn = St(0, 0, 0, 0);
function ut(t) {
  return parseFloat(t) || 0;
}
function sn(t) {
  for (var e = [], r = 1; r < arguments.length; r++)
    e[r - 1] = arguments[r];
  return e.reduce(function(n, o) {
    var i = t["border-" + o + "-width"];
    return n + ut(i);
  }, 0);
}
function _a(t) {
  for (var e = ["top", "right", "bottom", "left"], r = {}, n = 0, o = e; n < o.length; n++) {
    var i = o[n], a = t["padding-" + i];
    r[i] = ut(a);
  }
  return r;
}
function Ra(t) {
  var e = t.getBBox();
  return St(0, 0, e.width, e.height);
}
function Ea(t) {
  var e = t.clientWidth, r = t.clientHeight;
  if (!e && !r)
    return Xn;
  var n = Ie(t).getComputedStyle(t), o = _a(n), i = o.left + o.right, a = o.top + o.bottom, s = ut(n.width), l = ut(n.height);
  if (n.boxSizing === "border-box" && (Math.round(s + i) !== e && (s -= sn(n, "left", "right") + i), Math.round(l + a) !== r && (l -= sn(n, "top", "bottom") + a)), !Pa(t)) {
    var u = Math.round(s + i) - e, c = Math.round(l + a) - r;
    Math.abs(u) !== 1 && (s -= u), Math.abs(c) !== 1 && (l -= c);
  }
  return St(o.left, o.top, s, l);
}
var xa = /* @__PURE__ */ function() {
  return typeof SVGGraphicsElement < "u" ? function(t) {
    return t instanceof Ie(t).SVGGraphicsElement;
  } : function(t) {
    return t instanceof Ie(t).SVGElement && typeof t.getBBox == "function";
  };
}();
function Pa(t) {
  return t === Ie(t).document.documentElement;
}
function Da(t) {
  return nr ? xa(t) ? Ra(t) : Ea(t) : Xn;
}
function Ca(t) {
  var e = t.x, r = t.y, n = t.width, o = t.height, i = typeof DOMRectReadOnly < "u" ? DOMRectReadOnly : Object, a = Object.create(i.prototype);
  return Un(a, {
    x: e,
    y: r,
    width: n,
    height: o,
    top: r,
    right: e + n,
    bottom: o + r,
    left: e
  }), a;
}
function St(t, e, r, n) {
  return { x: t, y: e, width: r, height: n };
}
var za = (
  /** @class */
  function() {
    function t(e) {
      this.broadcastWidth = 0, this.broadcastHeight = 0, this.contentRect_ = St(0, 0, 0, 0), this.target = e;
    }
    return t.prototype.isActive = function() {
      var e = Da(this.target);
      return this.contentRect_ = e, e.width !== this.broadcastWidth || e.height !== this.broadcastHeight;
    }, t.prototype.broadcastRect = function() {
      var e = this.contentRect_;
      return this.broadcastWidth = e.width, this.broadcastHeight = e.height, e;
    }, t;
  }()
), ja = (
  /** @class */
  /* @__PURE__ */ function() {
    function t(e, r) {
      var n = Ca(r);
      Un(this, { target: e, contentRect: n });
    }
    return t;
  }()
), Ta = (
  /** @class */
  function() {
    function t(e, r, n) {
      if (this.activeObservations_ = [], this.observations_ = new Fn(), typeof e != "function")
        throw new TypeError("The callback provided as parameter 1 is not a function.");
      this.callback_ = e, this.controller_ = r, this.callbackCtx_ = n;
    }
    return t.prototype.observe = function(e) {
      if (!arguments.length)
        throw new TypeError("1 argument required, but only 0 present.");
      if (!(typeof Element > "u" || !(Element instanceof Object))) {
        if (!(e instanceof Ie(e).Element))
          throw new TypeError('parameter 1 is not of type "Element".');
        var r = this.observations_;
        r.has(e) || (r.set(e, new za(e)), this.controller_.addObserver(this), this.controller_.refresh());
      }
    }, t.prototype.unobserve = function(e) {
      if (!arguments.length)
        throw new TypeError("1 argument required, but only 0 present.");
      if (!(typeof Element > "u" || !(Element instanceof Object))) {
        if (!(e instanceof Ie(e).Element))
          throw new TypeError('parameter 1 is not of type "Element".');
        var r = this.observations_;
        r.has(e) && (r.delete(e), r.size || this.controller_.removeObserver(this));
      }
    }, t.prototype.disconnect = function() {
      this.clearActive(), this.observations_.clear(), this.controller_.removeObserver(this);
    }, t.prototype.gatherActive = function() {
      var e = this;
      this.clearActive(), this.observations_.forEach(function(r) {
        r.isActive() && e.activeObservations_.push(r);
      });
    }, t.prototype.broadcastActive = function() {
      if (this.hasActive()) {
        var e = this.callbackCtx_, r = this.activeObservations_.map(function(n) {
          return new ja(n.target, n.broadcastRect());
        });
        this.callback_.call(e, r, e), this.clearActive();
      }
    }, t.prototype.clearActive = function() {
      this.activeObservations_.splice(0);
    }, t.prototype.hasActive = function() {
      return this.activeObservations_.length > 0;
    }, t;
  }()
), Vn = typeof WeakMap < "u" ? /* @__PURE__ */ new WeakMap() : new Fn(), Kn = (
  /** @class */
  /* @__PURE__ */ function() {
    function t(e) {
      if (!(this instanceof t))
        throw new TypeError("Cannot call a class as a function.");
      if (!arguments.length)
        throw new TypeError("1 argument required, but only 0 present.");
      var r = Sa.getInstance(), n = new Ta(e, r, this);
      Vn.set(this, n);
    }
    return t;
  }()
);
[
  "observe",
  "unobserve",
  "disconnect"
].forEach(function(t) {
  Kn.prototype[t] = function() {
    var e;
    return (e = Vn.get(this))[t].apply(e, arguments);
  };
});
var Ma = function() {
  return typeof lt.ResizeObserver < "u" ? lt.ResizeObserver : Kn;
}();
const ka = /* @__PURE__ */ Object.freeze(/* @__PURE__ */ Object.defineProperty({
  __proto__: null,
  default: Ma
}, Symbol.toStringTag, { value: "Module" })), $a = /* @__PURE__ */ co(ka);
Object.defineProperty(Er, "__esModule", {
  value: !0
});
Er.default = Ya;
var it = Jn(de), La = xr(Ee), Na = xr($a), Aa = xr(ct);
const Ha = ["measureBeforeMount"];
function xr(t) {
  return t && t.__esModule ? t : { default: t };
}
function Jn(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (Jn = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var a, s, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (a = i ? n : r) {
      if (a.has(o)) return a.get(o);
      a.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((s = (a = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (s.get || s.set) ? a(l, u, s) : l[u] = o[u]);
    return l;
  })(t, e);
}
function or() {
  return or = Object.assign ? Object.assign.bind() : function(t) {
    for (var e = 1; e < arguments.length; e++) {
      var r = arguments[e];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (t[n] = r[n]);
    }
    return t;
  }, or.apply(null, arguments);
}
function Wa(t, e) {
  if (t == null) return {};
  var r, n, o = qa(t, e);
  if (Object.getOwnPropertySymbols) {
    var i = Object.getOwnPropertySymbols(t);
    for (n = 0; n < i.length; n++) r = i[n], e.indexOf(r) === -1 && {}.propertyIsEnumerable.call(t, r) && (o[r] = t[r]);
  }
  return o;
}
function qa(t, e) {
  if (t == null) return {};
  var r = {};
  for (var n in t) if ({}.hasOwnProperty.call(t, n)) {
    if (e.indexOf(n) !== -1) continue;
    r[n] = t[n];
  }
  return r;
}
function He(t, e, r) {
  return (e = Ia(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function Ia(t) {
  var e = Ba(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function Ba(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
const Ga = "react-grid-layout";
function Ya(t) {
  var e;
  return e = class extends it.Component {
    constructor() {
      super(...arguments), He(this, "state", {
        width: 1280
      }), He(this, "elementRef", /* @__PURE__ */ it.createRef()), He(this, "mounted", !1), He(this, "resizeObserver", void 0);
    }
    componentDidMount() {
      this.mounted = !0, this.resizeObserver = new Na.default((o) => {
        if (this.elementRef.current instanceof HTMLElement) {
          const a = o[0].contentRect.width;
          this.setState({
            width: a
          });
        }
      });
      const n = this.elementRef.current;
      n instanceof HTMLElement && this.resizeObserver.observe(n);
    }
    componentWillUnmount() {
      this.mounted = !1;
      const n = this.elementRef.current;
      n instanceof HTMLElement && this.resizeObserver.unobserve(n), this.resizeObserver.disconnect();
    }
    render() {
      const n = this.props, {
        measureBeforeMount: o
      } = n, i = Wa(n, Ha);
      return o && !this.mounted ? /* @__PURE__ */ it.createElement("div", {
        className: (0, Aa.default)(this.props.className, Ga),
        style: this.props.style,
        ref: this.elementRef
      }) : /* @__PURE__ */ it.createElement(t, or({
        innerRef: this.elementRef
      }, i, this.state));
    }
  }, He(e, "defaultProps", {
    measureBeforeMount: !1
  }), He(e, "propTypes", {
    // If true, will not render children until mounted. Useful for getting the exact width before
    // rendering, to prevent any unsightly resizing.
    measureBeforeMount: La.default.bool
  }), e;
}
(function(t) {
  t.exports = Ze.default, t.exports.utils = N, t.exports.calculateUtils = me, t.exports.Responsive = Ot.default, t.exports.Responsive.utils = ke, t.exports.WidthProvider = Er.default;
})(gn);
var Fa = gn.exports;
const Ua = /* @__PURE__ */ uo(Fa);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Xa = (t) => t.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase(), Zn = (...t) => t.filter((e, r, n) => !!e && e.trim() !== "" && n.indexOf(e) === r).join(" ").trim();
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
var Va = {
  xmlns: "http://www.w3.org/2000/svg",
  width: 24,
  height: 24,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2,
  strokeLinecap: "round",
  strokeLinejoin: "round"
};
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Ka = cn(
  ({
    color: t = "currentColor",
    size: e = 24,
    strokeWidth: r = 2,
    absoluteStrokeWidth: n,
    className: o = "",
    children: i,
    iconNode: a,
    ...s
  }, l) => It(
    "svg",
    {
      ref: l,
      ...Va,
      width: e,
      height: e,
      stroke: t,
      strokeWidth: n ? Number(r) * 24 / Number(e) : r,
      className: Zn("lucide", o),
      ...s
    },
    [
      ...a.map(([u, c]) => It(u, c)),
      ...Array.isArray(i) ? i : [i]
    ]
  )
);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Se = (t, e) => {
  const r = cn(
    ({ className: n, ...o }, i) => It(Ka, {
      ref: i,
      iconNode: e,
      className: Zn(`lucide-${Xa(t)}`, n),
      ...o
    })
  );
  return r.displayName = `${t}`, r;
};
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const ln = Se("ChevronDown", [
  ["path", { d: "m6 9 6 6 6-6", key: "qrunsl" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Ja = Se("ChevronRight", [
  ["path", { d: "m9 18 6-6-6-6", key: "mthhwq" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Za = Se("Copy", [
  ["rect", { width: "14", height: "14", x: "8", y: "8", rx: "2", ry: "2", key: "17jyea" }],
  ["path", { d: "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2", key: "zix9uf" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Qa = Se("Download", [
  ["path", { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", key: "ih7n3h" }],
  ["polyline", { points: "7 10 12 15 17 10", key: "2ggqvy" }],
  ["line", { x1: "12", x2: "12", y1: "15", y2: "3", key: "1vk2je" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const es = Se("GripHorizontal", [
  ["circle", { cx: "12", cy: "9", r: "1", key: "124mty" }],
  ["circle", { cx: "19", cy: "9", r: "1", key: "1ruzo2" }],
  ["circle", { cx: "5", cy: "9", r: "1", key: "1a8b28" }],
  ["circle", { cx: "12", cy: "15", r: "1", key: "1e56xg" }],
  ["circle", { cx: "19", cy: "15", r: "1", key: "1a92ep" }],
  ["circle", { cx: "5", cy: "15", r: "1", key: "5r1jwy" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const ts = Se("GripVertical", [
  ["circle", { cx: "9", cy: "12", r: "1", key: "1vctgf" }],
  ["circle", { cx: "9", cy: "5", r: "1", key: "hp0tcf" }],
  ["circle", { cx: "9", cy: "19", r: "1", key: "fkjjf6" }],
  ["circle", { cx: "15", cy: "12", r: "1", key: "1tmaij" }],
  ["circle", { cx: "15", cy: "5", r: "1", key: "19l28e" }],
  ["circle", { cx: "15", cy: "19", r: "1", key: "f4zoj3" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const rs = Se("Link", [
  ["path", { d: "M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71", key: "1cjeqo" }],
  ["path", { d: "M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71", key: "19qd67" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const ns = Se("Pencil", [
  [
    "path",
    {
      d: "M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z",
      key: "1a8usu"
    }
  ],
  ["path", { d: "m15 5 4 4", key: "1mk7zo" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Qn = Se("X", [
  ["path", { d: "M18 6 6 18", key: "1bl5f8" }],
  ["path", { d: "m6 6 12 12", key: "d8bk6v" }]
]);
function os({
  cell: t,
  memberCount: e,
  editable: r,
  onToggleCollapse: n,
  onRename: o,
  onRemove: i
}) {
  const a = dn(t), s = oo(t), [l, u] = Bt(!1), [c, f] = Bt(""), d = Yt(t), v = () => {
    const m = c.trim();
    m && m !== d && (o == null || o(t.i, m)), u(!1);
  };
  return /* @__PURE__ */ fe(
    "div",
    {
      "data-row-header": "",
      className: `lbdg-row-header${s.showLine ? " lbdg-row-header--line" : ""}`,
      "aria-label": `row ${d}`,
      children: [
        r && /* @__PURE__ */ C(
          "span",
          {
            "aria-label": `move cell ${t.i}`,
            title: "Move row",
            className: "lbdg-drag-handle lbdg-row-grip",
            children: /* @__PURE__ */ C(ts, { size: 13 })
          }
        ),
        l && r ? /* @__PURE__ */ fe(un, { children: [
          /* @__PURE__ */ C("span", { className: "lbdg-row-chevron", children: /* @__PURE__ */ C(ln, { size: 16 }) }),
          /* @__PURE__ */ C(
            "input",
            {
              autoFocus: !0,
              "aria-label": "row title",
              className: "lbdg-no-drag lbdg-row-rename",
              defaultValue: d,
              onChange: (m) => f(m.target.value),
              onBlur: v,
              onKeyDown: (m) => {
                m.key === "Enter" && v(), m.key === "Escape" && u(!1);
              }
            }
          )
        ] }) : /* @__PURE__ */ fe(
          "button",
          {
            type: "button",
            "aria-label": a ? `expand row ${d}` : `collapse row ${d}`,
            "aria-expanded": !a,
            title: a ? "Expand row" : "Collapse row",
            className: "lbdg-row-toggle",
            onClick: () => n == null ? void 0 : n(t.i),
            onDoubleClick: (m) => {
              !r || !o || (m.stopPropagation(), f(d), u(!0));
            },
            children: [
              /* @__PURE__ */ C("span", { className: "lbdg-row-chevron", children: a ? /* @__PURE__ */ C(Ja, { size: 16 }) : /* @__PURE__ */ C(ln, { size: 16 }) }),
              /* @__PURE__ */ C("span", { className: "lbdg-row-title", children: d }),
              s.showCount && e > 0 && /* @__PURE__ */ fe("span", { className: "lbdg-row-count", children: [
                "· ",
                e,
                " panel",
                e === 1 ? "" : "s"
              ] })
            ]
          }
        ),
        r && i && /* @__PURE__ */ C(
          "button",
          {
            type: "button",
            "aria-label": `remove cell ${t.i}`,
            title: "Remove row",
            className: "lbdg-no-drag lbdg-btn lbdg-btn--danger lbdg-row-remove",
            onClick: () => i(t.i),
            children: /* @__PURE__ */ C(Qn, { size: 13 })
          }
        )
      ]
    }
  );
}
function is({
  cells: t,
  registry: e,
  range: r,
  scope: n,
  refreshKey: o
}) {
  const i = [...pn(t)].sort((a, s) => a.y - s.y || a.x - s.x);
  return /* @__PURE__ */ C("div", { className: "lbdg-stack", "aria-label": "dashboard stack", children: i.map(
    (a) => Oe(a) ? /* @__PURE__ */ fe("div", { className: "lbdg-stack-row", "aria-label": `row ${Yt(a)}`, children: [
      /* @__PURE__ */ C("span", { className: "lbdg-row-title", children: Yt(a) }),
      Ve(t, a).length > 0 && /* @__PURE__ */ fe("span", { className: "lbdg-row-count", children: [
        "· ",
        Ve(t, a).length
      ] })
    ] }, a.i) : /* @__PURE__ */ C(
      "div",
      {
        className: a.transparent ? "lbdg-cell lbdg-cell--transparent" : "lbdg-cell lbdg-cell--framed",
        style: { minHeight: `${a.h * fn}px` },
        "aria-label": `cell ${a.i}`,
        children: /* @__PURE__ */ C("div", { className: "lbdg-cell-body", children: (() => {
          const s = e.resolveCell(a);
          return s ? /* @__PURE__ */ C(s, { cell: a, range: r, scope: n, refreshKey: o, editable: !1 }) : /* @__PURE__ */ C(hn, { view: Je(a) });
        })() })
      },
      a.i
    )
  ) });
}
const as = 1200;
function _s({
  cells: t,
  editable: e,
  registry: r,
  range: n,
  scope: o,
  refreshKey: i,
  onLayout: a,
  onRemove: s,
  onDuplicate: l,
  onToggleRow: u,
  onRenameRow: c,
  onEditPanel: f,
  onExportCell: d,
  stackBelow: v = 768,
  droppable: m,
  droppingItem: O,
  onDrop: E
}) {
  const _ = eo(null), [W, R] = Bt(as);
  to(() => {
    const j = () => {
      const le = _.current;
      if (!le) return;
      const _e = window.getComputedStyle(le), ve = le.clientWidth - (parseFloat(_e.paddingLeft) || 0) - (parseFloat(_e.paddingRight) || 0);
      ve > 0 && R(ve);
    };
    j();
    const re = _.current;
    if (!re || typeof ResizeObserver > "u")
      return window.addEventListener("resize", j), () => window.removeEventListener("resize", j);
    const ie = new ResizeObserver(j);
    return ie.observe(re), () => ie.disconnect();
  }, []);
  const D = pn(t), p = D.map((j) => ({
    i: j.i,
    x: j.x,
    y: j.y,
    w: j.w,
    h: j.h,
    // A row header is a fixed-height full-width bar — it may move but never resize.
    ...Oe(j) ? { isResizable: !1 } : {}
  })), U = (j) => a(io(t, j));
  if (v > 0 && W < v)
    return /* @__PURE__ */ C("div", { ref: _, className: "lbdg-root", "aria-label": "dashboard grid", children: /* @__PURE__ */ C(is, { cells: t, registry: r, range: n, scope: o, refreshKey: i }) });
  const Q = !!(m && e && E);
  return /* @__PURE__ */ C(
    "div",
    {
      ref: _,
      className: "lbdg-root lbdg-canvas",
      "aria-label": "dashboard grid",
      "data-droppable": Q ? "true" : void 0,
      children: /* @__PURE__ */ C(
        Ua,
        {
          className: "layout",
          layout: p,
          cols: ro,
          rowHeight: fn,
          width: W,
          isDraggable: e,
          isResizable: e,
          onDragStop: U,
          onResizeStop: U,
          draggableHandle: ".lbdg-drag-handle",
          draggableCancel: ".lbdg-no-drag",
          isDroppable: Q,
          droppingItem: O,
          onDrop: (j, re, ie) => {
            re && E && E({ x: re.x, y: re.y, w: re.w, h: re.h }, ie);
          },
          children: D.map(
            (j) => Oe(j) ? (
              // A row header: a full-width, flat, full-bleed section bar — NOT a widget frame. The
              // bar owns its own chrome (drag handle + rename + collapse + remove) inline.
              /* @__PURE__ */ C("div", { "data-row": "", className: "lbdg-row-item", "aria-label": `row cell ${j.i}`, children: /* @__PURE__ */ C(
                os,
                {
                  cell: j,
                  memberCount: Ve(t, j).length,
                  editable: e,
                  onToggleCollapse: u,
                  onRename: c,
                  onRemove: s
                }
              ) }, j.i)
            ) : /* @__PURE__ */ C(
              "div",
              {
                className: j.transparent ? "lbdg-cell lbdg-cell--transparent" : "lbdg-cell lbdg-cell--framed",
                "data-transparent": j.transparent ? "true" : void 0,
                "aria-label": `cell ${j.i}`,
                children: /* @__PURE__ */ C(
                  ss,
                  {
                    cell: j,
                    editable: e,
                    registry: r,
                    range: n,
                    scope: o,
                    refreshKey: i,
                    onRemove: s,
                    onDuplicate: l,
                    onEditPanel: f,
                    onExportCell: d
                  }
                )
              },
              j.i
            )
          )
        }
      )
    }
  );
}
function ss({
  cell: t,
  editable: e,
  registry: r,
  range: n,
  scope: o,
  refreshKey: i,
  onRemove: a,
  onDuplicate: s,
  onEditPanel: l,
  onExportCell: u
}) {
  const c = ao(t.queryOptions), f = t.links ?? [], d = Je(t), v = r.resolveCell(t);
  return /* @__PURE__ */ fe(un, { children: [
    c && /* @__PURE__ */ C("div", { className: "lbdg-badge", "aria-label": `time override for cell ${t.i}`, children: c }),
    f.length > 0 && /* @__PURE__ */ C("div", { className: "lbdg-no-drag lbdg-links", children: f.map((m, O) => /* @__PURE__ */ fe(
      "a",
      {
        href: m.url,
        title: m.title || m.url,
        "aria-label": `panel link ${m.title || m.url}`,
        ...m.targetBlank === !1 ? {} : { target: "_blank", rel: "noreferrer" },
        className: "lbdg-link",
        children: [
          /* @__PURE__ */ C(rs, { size: 11 }),
          /* @__PURE__ */ C("span", { className: "lbdg-link-title", children: m.title || m.url })
        ]
      },
      `${m.url}-${O}`
    )) }),
    e && /* @__PURE__ */ C(
      "button",
      {
        type: "button",
        "aria-label": `move cell ${t.i}`,
        title: "Move widget",
        className: "lbdg-drag-handle lbdg-btn lbdg-move",
        children: /* @__PURE__ */ C(es, { size: 13 })
      }
    ),
    e && (l || s || u || a) && /* @__PURE__ */ fe("div", { className: "lbdg-no-drag lbdg-chrome", children: [
      l && /* @__PURE__ */ C(
        "button",
        {
          type: "button",
          "aria-label": `edit cell ${t.i}`,
          title: "Edit panel",
          className: "lbdg-btn",
          onClick: () => l(t.i),
          children: /* @__PURE__ */ C(ns, { size: 13 })
        }
      ),
      s && /* @__PURE__ */ C(
        "button",
        {
          type: "button",
          "aria-label": `duplicate cell ${t.i}`,
          title: "Duplicate widget",
          className: "lbdg-btn",
          onClick: () => s(t.i),
          children: /* @__PURE__ */ C(Za, { size: 13 })
        }
      ),
      u && /* @__PURE__ */ C(
        "button",
        {
          type: "button",
          "aria-label": `export cell ${t.i}`,
          title: "Export widget",
          className: "lbdg-btn",
          onClick: () => u(t.i),
          children: /* @__PURE__ */ C(Qa, { size: 13 })
        }
      ),
      a && /* @__PURE__ */ C(
        "button",
        {
          type: "button",
          "aria-label": `remove cell ${t.i}`,
          title: "Remove widget",
          className: "lbdg-btn lbdg-btn--danger",
          onClick: () => a(t.i),
          children: /* @__PURE__ */ C(Qn, { size: 13 })
        }
      )
    ] }),
    /* @__PURE__ */ C("div", { className: "lbdg-cell-body", children: v ? /* @__PURE__ */ C(v, { cell: t, range: n, scope: o, refreshKey: i, editable: e }) : /* @__PURE__ */ C(hn, { view: d }) })
  ] });
}
export {
  _s as DashboardGrid,
  is as DashboardStack,
  so as EXT_WILDCARD,
  as as FALLBACK_WIDTH,
  ro as GRID_COLS,
  fn as GRID_ROW_PX,
  ms as ROW_H,
  ys as ROW_W,
  os as RowHeader,
  hn as UnknownView,
  ps as bindingSeries,
  hs as bindingTags,
  Gt as canonicalView,
  ds as cellFieldConfig,
  Yt as cellLabel,
  fs as cellPrimaryTarget,
  Cr as cellSources,
  Je as cellView,
  bs as createRegistry,
  gs as emptyFieldConfig,
  dn as isCollapsed,
  Oe as isRow,
  io as mergeLayout,
  Ve as rowMembers,
  oo as rowOptions,
  ar as rows,
  ao as timeOverrideBadge,
  vs as ungroupedCells,
  pn as visibleCells
};
