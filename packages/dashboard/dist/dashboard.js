import { jsxs as J, jsx as v, Fragment as Cr } from "react/jsx-runtime";
import Q, { forwardRef as jr, createElement as Ot, useState as St, useRef as Mn, useEffect as Ln } from "react";
import Ht from "react-dom";
const Nn = 12, kr = 56, Tn = {
  chart: "timeseries"
};
function _t(t) {
  return Tn[t] ?? t;
}
function sr(t) {
  var e;
  return t.sources && t.sources.length > 0 ? t.sources : (e = t.source) != null && e.tool ? [{ refId: "A", tool: t.source.tool, args: t.source.args, datasource: { type: "surreal" } }] : [];
}
function As(t) {
  return sr(t).find((e) => !e.hide) ?? sr(t)[0];
}
function Ne(t) {
  return _t(t.view || t.widget_type || "timeseries");
}
function Bs(t) {
  return t.fieldConfig ?? { defaults: {}, overrides: [] };
}
function Rt(t) {
  var r, n, o;
  return (r = t.title) != null && r.trim() ? t.title.trim() : (n = t.source) != null && n.tool ? t.source.tool : (o = t.action) != null && o.tool ? t.action.tool : Ne(t) || t.widget_type || "widget";
}
function Gs(t) {
  return "series" in t ? t.series : null;
}
function Ys(t) {
  return "find" in t ? t.find.tags : [];
}
function Fs() {
  return { defaults: {}, overrides: [] };
}
const Xs = 12, Us = 1;
function ie(t) {
  return Ne(t) === "row";
}
function Mr(t) {
  var e;
  return ie(t) && ((e = t.options) == null ? void 0 : e.collapsed) === !0;
}
function Hn(t) {
  const e = t.options ?? {};
  return {
    showCount: e.showCount !== !1,
    showLine: e.showLine !== !1,
    collapsed: e.collapsed === !0
  };
}
function $t(t) {
  return t.filter(ie).sort((e, r) => e.y - r.y || e.x - r.x);
}
function Me(t, e) {
  if (!ie(e)) return [];
  const r = $t(t), n = r.findIndex((a) => a.i === e.i);
  if (n < 0) return [];
  const o = e.y, i = r[n + 1], s = i ? i.y : Number.POSITIVE_INFINITY;
  return t.filter((a) => !ie(a) && a.y >= o && a.y < s);
}
function Vs(t) {
  const e = $t(t), r = e.length > 0 ? e[0].y : Number.POSITIVE_INFINITY;
  return t.filter((n) => !ie(n) && n.y < r);
}
function Lr(t) {
  const e = $t(t).filter(Mr);
  if (e.length === 0) return t;
  const r = /* @__PURE__ */ new Set();
  for (const n of e)
    for (const o of Me(t, n)) r.add(o.i);
  return t.filter((n) => !r.has(n.i));
}
function $n(t, e) {
  const r = new Map(e.map((o) => [o.i, o])), n = /* @__PURE__ */ new Map();
  for (const o of t) {
    if (!ie(o)) continue;
    const i = r.get(o.i);
    if (!i) continue;
    const s = i.y - o.y;
    if (s !== 0)
      for (const a of Me(t, o))
        r.has(a.i) || n.set(a.i, s);
  }
  return t.map((o) => {
    const i = r.get(o.i);
    if (i) return { ...o, x: i.x, y: i.y, w: i.w, h: i.h };
    const s = n.get(o.i);
    return s ? { ...o, y: o.y + s } : o;
  });
}
function Wn(t) {
  if (!t || t.hideTimeOverride) return null;
  const e = [], r = t.timeFrom || t.relativeTime;
  return r && e.push(`Last ${r.replace(/^now-/, "")}`), t.timeShift && e.push(`${t.timeShift} earlier`), e.length > 0 ? e.join(", ") : null;
}
const qn = "ext:*";
function Ks(t) {
  const e = /* @__PURE__ */ new Map(), r = {
    register(n, o) {
      return e.set(_t(n), o), r;
    },
    resolve(n) {
      const o = _t(n), i = e.get(o);
      if (i) return i;
      if (o.startsWith("ext:")) return e.get(qn);
    },
    resolveCell(n) {
      return r.resolve(Ne(n));
    },
    views() {
      return [...e.keys()];
    }
  };
  if (t) for (const [n, o] of Object.entries(t)) r.register(n, o);
  return r;
}
function Nr({ view: t }) {
  return /* @__PURE__ */ J("div", { className: "lbdg-unknown", role: "note", "aria-label": `unknown view ${t}`, children: [
    /* @__PURE__ */ J("span", { className: "lbdg-unknown-title", children: [
      "No renderer for “",
      t,
      "”"
    ] }),
    /* @__PURE__ */ v("span", { className: "lbdg-unknown-hint", children: "Register one on the widget registry to render this cell." })
  ] });
}
var In = typeof globalThis < "u" ? globalThis : typeof window < "u" ? window : typeof global < "u" ? global : typeof self < "u" ? self : {};
function An(t) {
  return t && t.__esModule && Object.prototype.hasOwnProperty.call(t, "default") ? t.default : t;
}
function Bn(t) {
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
var Tr = { exports: {} }, Te = {}, Dt = { exports: {} };
(function(t, e) {
  (function(r, n) {
    n(e);
  })(In, function(r) {
    function n(h) {
      return function(k, z, C, T, W, V, H) {
        return h(k, z, H);
      };
    }
    function o(h) {
      return function(k, z, C, T) {
        if (!k || !z || typeof k != "object" || typeof z != "object")
          return h(k, z, C, T);
        var W = T.get(k), V = T.get(z);
        if (W && V)
          return W === z && V === k;
        T.set(k, z), T.set(z, k);
        var H = h(k, z, C, T);
        return T.delete(k), T.delete(z), H;
      };
    }
    function i(h, b) {
      var k = {};
      for (var z in h)
        k[z] = h[z];
      for (var z in b)
        k[z] = b[z];
      return k;
    }
    function s(h) {
      return h.constructor === Object || h.constructor == null;
    }
    function a(h) {
      return typeof h.then == "function";
    }
    function l(h, b) {
      return h === b || h !== h && b !== b;
    }
    var u = "[object Arguments]", c = "[object Boolean]", d = "[object Date]", f = "[object RegExp]", g = "[object Map]", p = "[object Number]", _ = "[object Object]", D = "[object Set]", m = "[object String]", j = Object.prototype.toString;
    function w(h) {
      var b = h.areArraysEqual, k = h.areDatesEqual, z = h.areMapsEqual, C = h.areObjectsEqual, T = h.areRegExpsEqual, W = h.areSetsEqual, V = h.createIsNestedEqual, H = V(ee);
      function ee(N, $, te) {
        if (N === $)
          return !0;
        if (!N || !$ || typeof N != "object" || typeof $ != "object")
          return N !== N && $ !== $;
        if (s(N) && s($))
          return C(N, $, H, te);
        var or = Array.isArray(N), ir = Array.isArray($);
        if (or || ir)
          return or === ir && b(N, $, H, te);
        var re = j.call(N);
        return re !== j.call($) ? !1 : re === d ? k(N, $, H, te) : re === f ? T(N, $, H, te) : re === g ? z(N, $, H, te) : re === D ? W(N, $, H, te) : re === _ || re === u ? a(N) || a($) ? !1 : C(N, $, H, te) : re === c || re === p || re === m ? l(N.valueOf(), $.valueOf()) : !1;
      }
      return ee;
    }
    function L(h, b, k, z) {
      var C = h.length;
      if (b.length !== C)
        return !1;
      for (; C-- > 0; )
        if (!k(h[C], b[C], C, C, h, b, z))
          return !1;
      return !0;
    }
    var x = o(L);
    function G(h, b) {
      return l(h.valueOf(), b.valueOf());
    }
    function U(h, b, k, z) {
      var C = h.size === b.size;
      if (!C)
        return !1;
      if (!h.size)
        return !0;
      var T = {}, W = 0;
      return h.forEach(function(V, H) {
        if (C) {
          var ee = !1, N = 0;
          b.forEach(function($, te) {
            !ee && !T[N] && (ee = k(H, te, W, N, h, b, z) && k(V, $, H, te, h, b, z)) && (T[N] = !0), N++;
          }), W++, C = ee;
        }
      }), C;
    }
    var X = o(U), P = "_owner", Y = Object.prototype.hasOwnProperty;
    function ue(h, b, k, z) {
      var C = Object.keys(h), T = C.length;
      if (Object.keys(b).length !== T)
        return !1;
      for (var W; T-- > 0; ) {
        if (W = C[T], W === P) {
          var V = !!h.$$typeof, H = !!b.$$typeof;
          if ((V || H) && V !== H)
            return !1;
        }
        if (!Y.call(b, W) || !k(h[W], b[W], W, W, h, b, z))
          return !1;
      }
      return !0;
    }
    var Pe = o(ue);
    function Ee(h, b) {
      return h.source === b.source && h.flags === b.flags;
    }
    function ze(h, b, k, z) {
      var C = h.size === b.size;
      if (!C)
        return !1;
      if (!h.size)
        return !0;
      var T = {};
      return h.forEach(function(W, V) {
        if (C) {
          var H = !1, ee = 0;
          b.forEach(function(N, $) {
            !H && !T[ee] && (H = k(W, N, V, $, h, b, z)) && (T[ee] = !0), ee++;
          }), C = H;
        }
      }), C;
    }
    var Sn = o(ze), We = Object.freeze({
      areArraysEqual: L,
      areDatesEqual: G,
      areMapsEqual: U,
      areObjectsEqual: ue,
      areRegExpsEqual: Ee,
      areSetsEqual: ze,
      createIsNestedEqual: n
    }), qe = Object.freeze({
      areArraysEqual: x,
      areDatesEqual: G,
      areMapsEqual: X,
      areObjectsEqual: Pe,
      areRegExpsEqual: Ee,
      areSetsEqual: Sn,
      createIsNestedEqual: n
    }), _n = w(We);
    function Rn(h, b) {
      return _n(h, b, void 0);
    }
    var Dn = w(i(We, { createIsNestedEqual: function() {
      return l;
    } }));
    function xn(h, b) {
      return Dn(h, b, void 0);
    }
    var Pn = w(qe);
    function En(h, b) {
      return Pn(h, b, /* @__PURE__ */ new WeakMap());
    }
    var zn = w(i(qe, {
      createIsNestedEqual: function() {
        return l;
      }
    }));
    function Cn(h, b) {
      return zn(h, b, /* @__PURE__ */ new WeakMap());
    }
    function jn(h) {
      return w(i(We, h(We)));
    }
    function kn(h) {
      var b = w(i(qe, h(qe)));
      return function(k, z, C) {
        return C === void 0 && (C = /* @__PURE__ */ new WeakMap()), b(k, z, C);
      };
    }
    r.circularDeepEqual = En, r.circularShallowEqual = Cn, r.createCustomCircularEqual = kn, r.createCustomEqual = jn, r.deepEqual = Rn, r.sameValueZeroEqual = l, r.shallowEqual = xn, Object.defineProperty(r, "__esModule", { value: !0 });
  });
})(Dt, Dt.exports);
var Wt = Dt.exports, xt = { exports: {} };
function Hr(t) {
  var e, r, n = "";
  if (typeof t == "string" || typeof t == "number") n += t;
  else if (typeof t == "object") if (Array.isArray(t)) {
    var o = t.length;
    for (e = 0; e < o; e++) t[e] && (r = Hr(t[e])) && (n && (n += " "), n += r);
  } else for (r in t) t[r] && (n && (n += " "), n += r);
  return n;
}
function ar() {
  for (var t, e, r = 0, n = "", o = arguments.length; r < o; r++) (t = arguments[r]) && (e = Hr(t)) && (n && (n += " "), n += e);
  return n;
}
xt.exports = ar, xt.exports.clsx = ar;
var Ve = xt.exports, R = {}, Gn = function(e, r, n) {
  return e === r ? !0 : e.className === r.className && n(e.style, r.style) && e.width === r.width && e.autoSize === r.autoSize && e.cols === r.cols && e.draggableCancel === r.draggableCancel && e.draggableHandle === r.draggableHandle && n(e.verticalCompact, r.verticalCompact) && n(e.compactType, r.compactType) && n(e.layout, r.layout) && n(e.margin, r.margin) && n(e.containerPadding, r.containerPadding) && e.rowHeight === r.rowHeight && e.maxRows === r.maxRows && e.isBounded === r.isBounded && e.isDraggable === r.isDraggable && e.isResizable === r.isResizable && e.allowOverlap === r.allowOverlap && e.preventCollision === r.preventCollision && e.useCSSTransforms === r.useCSSTransforms && e.transformScale === r.transformScale && e.isDroppable === r.isDroppable && n(e.resizeHandles, r.resizeHandles) && n(e.resizeHandle, r.resizeHandle) && e.onLayoutChange === r.onLayoutChange && e.onDragStart === r.onDragStart && e.onDrag === r.onDrag && e.onDragStop === r.onDragStop && e.onResizeStart === r.onResizeStart && e.onResize === r.onResize && e.onResizeStop === r.onResizeStop && e.onDrop === r.onDrop && n(e.droppingItem, r.droppingItem) && n(e.innerRef, r.innerRef);
};
Object.defineProperty(R, "__esModule", {
  value: !0
});
R.bottom = Ke;
R.childrenEqual = Kn;
R.cloneLayout = $r;
R.cloneLayoutItem = me;
R.collides = Ze;
R.compact = qr;
R.compactItem = Ir;
R.compactType = co;
R.correctBounds = Ar;
R.fastPositionEqual = Zn;
R.fastRGLPropsEqual = void 0;
R.getAllCollisions = Br;
R.getFirstCollision = he;
R.getLayoutItem = qt;
R.getStatics = It;
R.modifyLayout = Wr;
R.moveElement = je;
R.moveElementAwayFromCollision = Et;
R.noop = void 0;
R.perc = Qn;
R.resizeItemInDirection = io;
R.setTopLeft = ao;
R.setTransform = so;
R.sortLayoutItems = Xt;
R.sortLayoutItemsByColRow = Ur;
R.sortLayoutItemsByRowCol = Xr;
R.synchronizeLayoutWithChildren = lo;
R.validateLayout = uo;
R.withLayoutItem = Vn;
var lr = Wt, Ce = Yn(Q);
function Yn(t) {
  return t && t.__esModule ? t : { default: t };
}
function ur(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Ye(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? ur(Object(r), !0).forEach(function(n) {
      Fn(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : ur(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function Fn(t, e, r) {
  return (e = Xn(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function Xn(t) {
  var e = Un(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function Un(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
function Ke(t) {
  let e = 0, r;
  for (let n = 0, o = t.length; n < o; n++)
    r = t[n].y + t[n].h, r > e && (e = r);
  return e;
}
function $r(t) {
  const e = Array(t.length);
  for (let r = 0, n = t.length; r < n; r++)
    e[r] = me(t[r]);
  return e;
}
function Wr(t, e) {
  const r = Array(t.length);
  for (let n = 0, o = t.length; n < o; n++)
    e.i === t[n].i ? r[n] = e : r[n] = t[n];
  return r;
}
function Vn(t, e, r) {
  let n = qt(t, e);
  return n ? (n = r(me(n)), t = Wr(t, n), [t, n]) : [t, null];
}
function me(t) {
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
function Kn(t, e) {
  return (0, lr.deepEqual)(Ce.default.Children.map(t, (r) => r == null ? void 0 : r.key), Ce.default.Children.map(e, (r) => r == null ? void 0 : r.key)) && (0, lr.deepEqual)(Ce.default.Children.map(t, (r) => r == null ? void 0 : r.props["data-grid"]), Ce.default.Children.map(e, (r) => r == null ? void 0 : r.props["data-grid"]));
}
R.fastRGLPropsEqual = Gn;
function Zn(t, e) {
  return t.left === e.left && t.top === e.top && t.width === e.width && t.height === e.height;
}
function Ze(t, e) {
  return !(t.i === e.i || t.x + t.w <= e.x || t.x >= e.x + e.w || t.y + t.h <= e.y || t.y >= e.y + e.h);
}
function qr(t, e, r, n) {
  const o = It(t);
  let i = Ke(o);
  const s = Xt(t, e), a = Array(t.length);
  for (let l = 0, u = s.length; l < u; l++) {
    let c = me(s[l]);
    c.static || (c = Ir(o, c, e, r, s, n, i), i = Math.max(i, c.y + c.h), o.push(c)), a[t.indexOf(s[l])] = c, c.moved = !1;
  }
  return a;
}
const Jn = {
  x: "w",
  y: "h"
};
function Pt(t, e, r, n) {
  const o = Jn[n];
  e[n] += 1;
  const i = t.map((s) => s.i).indexOf(e.i);
  for (let s = i + 1; s < t.length; s++) {
    const a = t[s];
    if (!a.static) {
      if (a.y > e.y + e.h) break;
      Ze(e, a) && Pt(t, a, r + e[o], n);
    }
  }
  e[n] = r;
}
function Ir(t, e, r, n, o, i, s) {
  const a = r === "vertical", l = r === "horizontal";
  if (a)
    for (typeof s == "number" ? e.y = Math.min(s, e.y) : e.y = Math.min(Ke(t), e.y); e.y > 0 && !he(t, e); )
      e.y--;
  else if (l)
    for (; e.x > 0 && !he(t, e); )
      e.x--;
  let u;
  for (; (u = he(t, e)) && !(r === null && i); )
    if (l ? Pt(o, e, u.x + u.w, "x") : Pt(o, e, u.y + u.h, "y"), l && e.x + e.w > n)
      for (e.x = n - e.w, e.y++; e.x > 0 && !he(t, e); )
        e.x--;
  return e.y = Math.max(e.y, 0), e.x = Math.max(e.x, 0), e;
}
function Ar(t, e) {
  const r = It(t);
  for (let n = 0, o = t.length; n < o; n++) {
    const i = t[n];
    if (i.x + i.w > e.cols && (i.x = e.cols - i.w), i.x < 0 && (i.x = 0, i.w = e.cols), !i.static) r.push(i);
    else
      for (; he(r, i); )
        i.y++;
  }
  return t;
}
function qt(t, e) {
  for (let r = 0, n = t.length; r < n; r++)
    if (t[r].i === e) return t[r];
}
function he(t, e) {
  for (let r = 0, n = t.length; r < n; r++)
    if (Ze(t[r], e)) return t[r];
}
function Br(t, e) {
  return t.filter((r) => Ze(r, e));
}
function It(t) {
  return t.filter((e) => e.static);
}
function je(t, e, r, n, o, i, s, a, l) {
  if (e.static && e.isDraggable !== !0 || e.y === n && e.x === r) return t;
  "Moving element ".concat(e.i, " to [").concat(String(r), ",").concat(String(n), "] from [").concat(e.x, ",").concat(e.y, "]");
  const u = e.x, c = e.y;
  typeof r == "number" && (e.x = r), typeof n == "number" && (e.y = n), e.moved = !0;
  let d = Xt(t, s);
  (s === "vertical" && typeof n == "number" ? c >= n : s === "horizontal" && typeof r == "number" ? u >= r : !1) && (d = d.reverse());
  const g = Br(d, e), p = g.length > 0;
  if (p && l)
    return $r(t);
  if (p && i)
    return "Collision prevented on ".concat(e.i, ", reverting."), e.x = u, e.y = c, e.moved = !1, t;
  for (let _ = 0, D = g.length; _ < D; _++) {
    const m = g[_];
    "Resolving collision between ".concat(e.i, " at [").concat(e.x, ",").concat(e.y, "] and ").concat(m.i, " at [").concat(m.x, ",").concat(m.y, "]"), !m.moved && (m.static ? t = Et(t, m, e, o, s) : t = Et(t, e, m, o, s));
  }
  return t;
}
function Et(t, e, r, n, o, i) {
  const s = o === "horizontal", a = o === "vertical", l = e.static;
  if (n) {
    n = !1;
    const d = {
      x: s ? Math.max(e.x - r.w, 0) : r.x,
      y: a ? Math.max(e.y - r.h, 0) : r.y,
      w: r.w,
      h: r.h,
      i: "-1"
    }, f = he(t, d), g = f && f.y + f.h > e.y, p = f && e.x + e.w > f.x;
    if (f) {
      if (g && a)
        return je(t, r, void 0, r.y + 1, n, l, o);
      if (g && o == null)
        return e.y = r.y, r.y = r.y + r.h, t;
      if (p && s)
        return je(t, e, r.x, void 0, n, l, o);
    } else return "Doing reverse collision on ".concat(r.i, " up to [").concat(d.x, ",").concat(d.y, "]."), je(t, r, s ? d.x : void 0, a ? d.y : void 0, n, l, o);
  }
  const u = s ? r.x + 1 : void 0, c = a ? r.y + 1 : void 0;
  return u == null && c == null ? t : je(t, r, s ? r.x + 1 : void 0, a ? r.y + 1 : void 0, n, l, o);
}
function Qn(t) {
  return t * 100 + "%";
}
const Gr = (t, e, r, n) => t + r > n ? e : r, Yr = (t, e, r) => t < 0 ? e : r, Fr = (t) => Math.max(0, t), At = (t) => Math.max(0, t), Bt = (t, e, r) => {
  let {
    left: n,
    height: o,
    width: i
  } = e;
  const s = t.top - (o - t.height);
  return {
    left: n,
    width: i,
    height: Yr(s, t.height, o),
    top: At(s)
  };
}, Gt = (t, e, r) => {
  let {
    top: n,
    left: o,
    height: i,
    width: s
  } = e;
  return {
    top: n,
    height: i,
    width: Gr(t.left, t.width, s, r),
    left: Fr(o)
  };
}, Yt = (t, e, r) => {
  let {
    top: n,
    height: o,
    width: i
  } = e;
  const s = t.left - (i - t.width);
  return {
    height: o,
    width: s < 0 ? t.width : Gr(t.left, t.width, i, r),
    top: At(n),
    left: Fr(s)
  };
}, Ft = (t, e, r) => {
  let {
    top: n,
    left: o,
    height: i,
    width: s
  } = e;
  return {
    width: s,
    left: o,
    height: Yr(n, t.height, i),
    top: At(n)
  };
}, eo = function() {
  return Bt(arguments.length <= 0 ? void 0 : arguments[0], Gt(...arguments));
}, to = function() {
  return Bt(arguments.length <= 0 ? void 0 : arguments[0], Yt(...arguments));
}, ro = function() {
  return Ft(arguments.length <= 0 ? void 0 : arguments[0], Gt(...arguments));
}, no = function() {
  return Ft(arguments.length <= 0 ? void 0 : arguments[0], Yt(...arguments));
}, oo = {
  n: Bt,
  ne: eo,
  e: Gt,
  se: ro,
  s: Ft,
  sw: no,
  w: Yt,
  nw: to
};
function io(t, e, r, n) {
  const o = oo[t];
  return o ? o(e, Ye(Ye({}, e), r), n) : r;
}
function so(t) {
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
function ao(t) {
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
function Xt(t, e) {
  return e === "horizontal" ? Ur(t) : e === "vertical" ? Xr(t) : t;
}
function Xr(t) {
  return t.slice(0).sort(function(e, r) {
    return e.y > r.y || e.y === r.y && e.x > r.x ? 1 : e.y === r.y && e.x === r.x ? 0 : -1;
  });
}
function Ur(t) {
  return t.slice(0).sort(function(e, r) {
    return e.x > r.x || e.x === r.x && e.y > r.y ? 1 : -1;
  });
}
function lo(t, e, r, n, o) {
  t = t || [];
  const i = [];
  Ce.default.Children.forEach(e, (a) => {
    if ((a == null ? void 0 : a.key) == null) return;
    const l = qt(t, String(a.key)), u = a.props["data-grid"];
    l && u == null ? i.push(me(l)) : u ? i.push(me(Ye(Ye({}, u), {}, {
      i: a.key
    }))) : i.push(me({
      w: 1,
      h: 1,
      x: 0,
      y: Ke(i),
      i: String(a.key)
    }));
  });
  const s = Ar(i, {
    cols: r
  });
  return o ? s : qr(s, n, r);
}
function uo(t) {
  let e = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : "Layout";
  const r = ["x", "y", "w", "h"];
  if (!Array.isArray(t)) throw new Error(e + " must be an array!");
  for (let n = 0, o = t.length; n < o; n++) {
    const i = t[n];
    for (let s = 0; s < r.length; s++) {
      const a = r[s], l = i[a];
      if (typeof l != "number" || Number.isNaN(l))
        throw new Error("ReactGridLayout: ".concat(e, "[").concat(n, "].").concat(a, " must be a number! Received: ").concat(l, " (").concat(typeof l, ")"));
    }
    if (typeof i.i < "u" && typeof i.i != "string")
      throw new Error("ReactGridLayout: ".concat(e, "[").concat(n, "].i must be a string! Received: ").concat(i.i, " (").concat(typeof i.i, ")"));
  }
}
function co(t) {
  const {
    verticalCompact: e,
    compactType: r
  } = t || {};
  return e === !1 ? null : r;
}
const fo = () => {
};
R.noop = fo;
var ne = {};
Object.defineProperty(ne, "__esModule", {
  value: !0
});
ne.calcGridColWidth = Je;
ne.calcGridItemPosition = po;
ne.calcGridItemWHPx = zt;
ne.calcWH = go;
ne.calcXY = ho;
ne.clamp = ge;
function Je(t) {
  const {
    margin: e,
    containerPadding: r,
    containerWidth: n,
    cols: o
  } = t;
  return (n - e[0] * (o - 1) - r[0] * 2) / o;
}
function zt(t, e, r) {
  return Number.isFinite(t) ? Math.round(e * t + Math.max(0, t - 1) * r) : t;
}
function po(t, e, r, n, o, i) {
  const {
    margin: s,
    containerPadding: a,
    rowHeight: l
  } = t, u = Je(t), c = {};
  return i && i.resizing ? (c.width = Math.round(i.resizing.width), c.height = Math.round(i.resizing.height)) : (c.width = zt(n, u, s[0]), c.height = zt(o, l, s[1])), i && i.dragging ? (c.top = Math.round(i.dragging.top), c.left = Math.round(i.dragging.left)) : i && i.resizing && typeof i.resizing.top == "number" && typeof i.resizing.left == "number" ? (c.top = Math.round(i.resizing.top), c.left = Math.round(i.resizing.left)) : (c.top = Math.round((l + s[1]) * r + a[1]), c.left = Math.round((u + s[0]) * e + a[0])), c;
}
function ho(t, e, r, n, o) {
  const {
    margin: i,
    containerPadding: s,
    cols: a,
    rowHeight: l,
    maxRows: u
  } = t, c = Je(t);
  let d = Math.round((r - s[0]) / (c + i[0])), f = Math.round((e - s[1]) / (l + i[1]));
  return d = ge(d, 0, a - n), f = ge(f, 0, u - o), {
    x: d,
    y: f
  };
}
function go(t, e, r, n, o, i) {
  const {
    margin: s,
    maxRows: a,
    cols: l,
    rowHeight: u
  } = t, c = Je(t);
  let d = Math.round((e + s[0]) / (c + s[0])), f = Math.round((r + s[1]) / (u + s[1])), g = ge(d, 0, l - n), p = ge(f, 0, a - o);
  return ["sw", "w", "nw"].indexOf(i) !== -1 && (g = ge(d, 0, l)), ["nw", "n", "ne"].indexOf(i) !== -1 && (p = ge(f, 0, a)), {
    w: g,
    h: p
  };
}
function ge(t, e, r) {
  return Math.max(Math.min(t, r), e);
}
var Qe = {}, Vr = { exports: {} }, mo = "SECRET_DO_NOT_PASS_THIS_OR_YOU_WILL_BE_FIRED", yo = mo, bo = yo;
function Kr() {
}
function Zr() {
}
Zr.resetWarningCache = Kr;
var vo = function() {
  function t(n, o, i, s, a, l) {
    if (l !== bo) {
      var u = new Error(
        "Calling PropTypes validators directly is not supported by the `prop-types` package. Use PropTypes.checkPropTypes() to call them. Read more at http://fb.me/use-check-prop-types"
      );
      throw u.name = "Invariant Violation", u;
    }
  }
  t.isRequired = t;
  function e() {
    return t;
  }
  var r = {
    array: t,
    bigint: t,
    bool: t,
    func: t,
    number: t,
    object: t,
    string: t,
    symbol: t,
    any: t,
    arrayOf: e,
    element: t,
    elementType: t,
    instanceOf: e,
    node: t,
    objectOf: e,
    oneOf: e,
    oneOfType: e,
    shape: e,
    exact: e,
    checkPropTypes: Zr,
    resetWarningCache: Kr
  };
  return r.PropTypes = r, r;
};
Vr.exports = vo();
var le = Vr.exports, et = { exports: {} }, wo = Object.create, tt = Object.defineProperty, Oo = Object.getOwnPropertyDescriptor, So = Object.getOwnPropertyNames, _o = Object.getPrototypeOf, Ro = Object.prototype.hasOwnProperty, Do = (t, e) => {
  for (var r in e)
    tt(t, r, { get: e[r], enumerable: !0 });
}, Jr = (t, e, r, n) => {
  if (e && typeof e == "object" || typeof e == "function")
    for (let o of So(e))
      !Ro.call(t, o) && o !== r && tt(t, o, { get: () => e[o], enumerable: !(n = Oo(e, o)) || n.enumerable });
  return t;
}, xe = (t, e, r) => (r = t != null ? wo(_o(t)) : {}, Jr(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  !t || !t.__esModule ? tt(r, "default", { value: t, enumerable: !0 }) : r,
  t
)), xo = (t) => Jr(tt({}, "__esModule", { value: !0 }), t), Qr = {};
Do(Qr, {
  DraggableCore: () => ye,
  default: () => rt
});
var Po = xo(Qr), Ie = xe(Q), M = xe(le), Eo = xe(Ht), zo = Ve;
function Ct(t, e) {
  for (let r = 0, n = t.length; r < n; r++)
    if (e.apply(e, [t[r], r, t])) return t[r];
}
function cr(t) {
  return typeof t == "function" || Object.prototype.toString.call(t) === "[object Function]";
}
function ke(t) {
  return typeof t == "number" && !isNaN(t);
}
function B(t) {
  return parseInt(t, 10);
}
function _e(t, e, r) {
  if (t[e])
    return new Error(`Invalid prop ${e} passed to ${r} - do not set this, set it on the child.`);
}
var lt = ["Moz", "Webkit", "O", "ms"];
function Co(t = "transform") {
  var e, r;
  if (typeof window > "u") return "";
  const n = (r = (e = window.document) == null ? void 0 : e.documentElement) == null ? void 0 : r.style;
  if (!n || t in n) return "";
  for (let o = 0; o < lt.length; o++)
    if (en(t, lt[o]) in n) return lt[o];
  return "";
}
function en(t, e) {
  return e ? `${e}${jo(t)}` : t;
}
function jo(t) {
  let e = "", r = !0;
  for (let n = 0; n < t.length; n++)
    r ? (e += t[n].toUpperCase(), r = !1) : t[n] === "-" ? r = !0 : e += t[n];
  return e;
}
var ko = Co(), ut = "";
function Mo(t, e) {
  var r;
  ut || (ut = (r = Ct([
    "matches",
    "webkitMatchesSelector",
    "mozMatchesSelector",
    "msMatchesSelector",
    "oMatchesSelector"
  ], function(o) {
    return cr(t[o]);
  })) != null ? r : "");
  const n = t[ut];
  return cr(n) ? !!n.call(t, e) : !1;
}
function fr(t, e, r) {
  let n = t;
  do {
    if (Mo(n, e)) return !0;
    if (n === r) return !1;
    n = n.parentNode;
  } while (n);
  return !1;
}
function ct(t, e, r, n) {
  if (!t) return;
  const o = { capture: !0, ...n }, i = r;
  t.addEventListener ? t.addEventListener(e, i, o) : t.attachEvent ? t.attachEvent("on" + e, i) : t["on" + e] = i;
}
function ce(t, e, r, n) {
  if (!t) return;
  const o = { capture: !0, ...n }, i = r;
  t.removeEventListener ? t.removeEventListener(e, i, o) : t.detachEvent ? t.detachEvent("on" + e, i) : t["on" + e] = null;
}
function Lo(t) {
  let e = t.clientHeight;
  const r = t.ownerDocument.defaultView.getComputedStyle(t);
  return e += B(r.borderTopWidth), e += B(r.borderBottomWidth), e;
}
function No(t) {
  let e = t.clientWidth;
  const r = t.ownerDocument.defaultView.getComputedStyle(t);
  return e += B(r.borderLeftWidth), e += B(r.borderRightWidth), e;
}
function To(t) {
  let e = t.clientHeight;
  const r = t.ownerDocument.defaultView.getComputedStyle(t);
  return e -= B(r.paddingTop), e -= B(r.paddingBottom), e;
}
function Ho(t) {
  let e = t.clientWidth;
  const r = t.ownerDocument.defaultView.getComputedStyle(t);
  return e -= B(r.paddingLeft), e -= B(r.paddingRight), e;
}
function $o(t, e, r) {
  const o = e === e.ownerDocument.body ? { left: 0, top: 0 } : e.getBoundingClientRect(), i = (t.clientX + e.scrollLeft - o.left) / r, s = (t.clientY + e.scrollTop - o.top) / r;
  return { x: i, y: s };
}
function Wo(t, e) {
  const r = tn(t, e, "px");
  return { [en("transform", ko)]: r };
}
function qo(t, e) {
  return tn(t, e, "");
}
function tn({ x: t, y: e }, r, n) {
  let o = `translate(${t}${n},${e}${n})`;
  if (r) {
    const i = `${typeof r.x == "string" ? r.x : r.x + n}`, s = `${typeof r.y == "string" ? r.y : r.y + n}`;
    o = `translate(${i}, ${s})` + o;
  }
  return o;
}
function Io(t, e) {
  return t.targetTouches && Ct(t.targetTouches, (r) => e === r.identifier) || t.changedTouches && Ct(t.changedTouches, (r) => e === r.identifier);
}
function Ao(t) {
  if (t.targetTouches && t.targetTouches[0]) return t.targetTouches[0].identifier;
  if (t.changedTouches && t.changedTouches[0]) return t.changedTouches[0].identifier;
}
function Bo() {
  return typeof __webpack_nonce__ < "u" ? __webpack_nonce__ : void 0;
}
function Go(t, e) {
  if (!t) return;
  let r = t.getElementById("react-draggable-style-el");
  if (!r) {
    r = t.createElement("style"), r.type = "text/css", r.id = "react-draggable-style-el";
    const n = e ?? Bo();
    n && r.setAttribute("nonce", n), r.innerHTML = `.react-draggable-transparent-selection *::-moz-selection {all: inherit;}
`, r.innerHTML += `.react-draggable-transparent-selection *::selection {all: inherit;}
`, t.getElementsByTagName("head")[0].appendChild(r);
  }
  t.body && Yo(t.body, "react-draggable-transparent-selection");
}
function dr(t) {
  window.requestAnimationFrame ? window.requestAnimationFrame(() => {
    pr(t);
  }) : pr(t);
}
function pr(t) {
  if (t)
    try {
      t.body && Fo(t.body, "react-draggable-transparent-selection");
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
function Yo(t, e) {
  t.classList ? t.classList.add(e) : t.className.match(new RegExp(`(?:^|\\s)${e}(?!\\S)`)) || (t.className += ` ${e}`);
}
function Fo(t, e) {
  t.classList ? t.classList.remove(e) : t.className = t.className.replace(new RegExp(`(?:^|\\s)${e}(?!\\S)`, "g"), "");
}
function Xo(t, e, r) {
  if (!t.props.bounds) return [e, r];
  let { bounds: n } = t.props;
  n = typeof n == "string" ? n : Ko(n);
  const o = Ut(t);
  if (typeof n == "string") {
    const { ownerDocument: i } = o, s = i.defaultView;
    if (!s)
      throw new Error("Cannot resolve the owner window of the draggable node.");
    let a;
    if (n === "parent" ? a = o.parentNode : a = o.getRootNode().querySelector(n), !(a instanceof s.HTMLElement))
      throw new Error('Bounds selector "' + n + '" could not find an element.');
    const l = a, u = s.getComputedStyle(o), c = s.getComputedStyle(l);
    n = {
      left: -o.offsetLeft + B(c.paddingLeft) + B(u.marginLeft),
      top: -o.offsetTop + B(c.paddingTop) + B(u.marginTop),
      right: Ho(l) - No(o) - o.offsetLeft + B(c.paddingRight) - B(u.marginRight),
      bottom: To(l) - Lo(o) - o.offsetTop + B(c.paddingBottom) - B(u.marginBottom)
    };
  }
  return ke(n.right) && (e = Math.min(e, n.right)), ke(n.bottom) && (r = Math.min(r, n.bottom)), ke(n.left) && (e = Math.max(e, n.left)), ke(n.top) && (r = Math.max(r, n.top)), [e, r];
}
function hr(t, e, r) {
  const n = Math.round(e / t[0]) * t[0], o = Math.round(r / t[1]) * t[1];
  return [n, o];
}
function Uo(t) {
  return t.props.axis === "both" || t.props.axis === "x";
}
function Vo(t) {
  return t.props.axis === "both" || t.props.axis === "y";
}
function ft(t, e, r) {
  const n = typeof e == "number" ? Io(t, e) : null;
  if (typeof e == "number" && !n) return null;
  const o = Ut(r), i = r.props.offsetParent || o.offsetParent || o.ownerDocument.body;
  return $o(n || t, i, r.props.scale);
}
function dt(t, e, r) {
  const n = !ke(t.lastX), o = Ut(t);
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
function pt(t, e) {
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
function Ko(t) {
  return {
    left: t.left,
    top: t.top,
    right: t.right,
    bottom: t.bottom
  };
}
function Ut(t) {
  const e = t.findDOMNode();
  if (!e)
    throw new Error("<DraggableCore>: Unmounted during event!");
  return e;
}
var ht = xe(Q), A = xe(le), Zo = xe(Ht);
function Jo(...t) {
}
var K = {
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
}, ae = K.mouse, ye = class extends ht.Component {
  constructor() {
    super(...arguments), this.dragging = !1, this.lastX = NaN, this.lastY = NaN, this.touchIdentifier = null, this.mounted = !1, this.handleDragStart = (e) => {
      if (this.props.onMouseDown(e), !this.props.allowAnyClick && (typeof e.button == "number" && e.button !== 0 || e.ctrlKey)) return !1;
      const r = this.findDOMNode();
      if (!r || !r.ownerDocument || !r.ownerDocument.body)
        throw new Error("<DraggableCore> not mounted on DragStart!");
      const { ownerDocument: n } = r;
      if (this.props.disabled || !(e.target instanceof n.defaultView.Node) || this.props.handle && !fr(e.target, this.props.handle, r) || this.props.cancel && fr(e.target, this.props.cancel, r))
        return;
      e.type === "touchstart" && !this.props.allowMobileScroll && e.preventDefault();
      const o = Ao(e);
      this.touchIdentifier = o;
      const i = ft(e, o, this);
      if (i == null) return;
      const { x: s, y: a } = i, l = dt(this, s, a);
      Jo("calling", this.props.onStart), !(this.props.onStart(e, l) === !1 || this.mounted === !1) && (this.props.enableUserSelectHack && Go(n, this.props.nonce), this.dragging = !0, this.lastX = s, this.lastY = a, ct(n, ae.move, this.handleDrag), ct(n, ae.stop, this.handleDragStop));
    }, this.handleDrag = (e) => {
      const r = ft(e, this.touchIdentifier, this);
      if (r == null) return;
      let { x: n, y: o } = r;
      if (Array.isArray(this.props.grid)) {
        let a = n - this.lastX, l = o - this.lastY;
        if ([a, l] = hr(this.props.grid, a, l), !a && !l) return;
        n = this.lastX + a, o = this.lastY + l;
      }
      const i = dt(this, n, o);
      if (this.props.onDrag(e, i) === !1 || this.mounted === !1) {
        try {
          this.handleDragStop(new MouseEvent("mouseup"));
        } catch {
          const a = document.createEvent("MouseEvents");
          a.initMouseEvent("mouseup", !0, !0, window, 0, 0, 0, 0, 0, !1, !1, !1, !1, 0, null), this.handleDragStop(a);
        }
        return;
      }
      this.lastX = n, this.lastY = o;
    }, this.handleDragStop = (e) => {
      if (!this.dragging) return;
      const r = ft(e, this.touchIdentifier, this);
      if (r == null) return;
      let { x: n, y: o } = r;
      if (Array.isArray(this.props.grid)) {
        let l = n - this.lastX || 0, u = o - this.lastY || 0;
        [l, u] = hr(this.props.grid, l, u), n = this.lastX + l, o = this.lastY + u;
      }
      const i = dt(this, n, o);
      if (this.props.onStop(e, i) === !1 || this.mounted === !1) return !1;
      const a = this.findDOMNode();
      a && this.props.enableUserSelectHack && dr(a.ownerDocument), this.dragging = !1, this.lastX = NaN, this.lastY = NaN, a && (ce(a.ownerDocument, ae.move, this.handleDrag), ce(a.ownerDocument, ae.stop, this.handleDragStop));
    }, this.onMouseDown = (e) => (ae = K.mouse, this.handleDragStart(e)), this.onMouseUp = (e) => (ae = K.mouse, this.handleDragStop(e)), this.onTouchStart = (e) => (ae = K.touch, this.handleDragStart(e)), this.onTouchEnd = (e) => (ae = K.touch, this.handleDragStop(e));
  }
  componentDidMount() {
    this.mounted = !0;
    const e = this.findDOMNode();
    e && ct(e, K.touch.start, this.onTouchStart, { passive: !1 });
  }
  componentWillUnmount() {
    this.mounted = !1;
    const e = this.findDOMNode();
    if (e) {
      const { ownerDocument: r } = e;
      ce(r, K.mouse.move, this.handleDrag), ce(r, K.touch.move, this.handleDrag), ce(r, K.mouse.stop, this.handleDragStop), ce(r, K.touch.stop, this.handleDragStop), ce(e, K.touch.start, this.onTouchStart, { passive: !1 }), this.props.enableUserSelectHack && dr(r);
    }
  }
  // React 19 removed ReactDOM.findDOMNode, so nodeRef is now required.
  // For backward compatibility with React 18 and earlier, we still support findDOMNode if available.
  findDOMNode() {
    var e;
    if ((e = this.props) != null && e.nodeRef)
      return this.props.nodeRef.current;
    const r = Zo.default;
    return typeof r.findDOMNode == "function" ? r.findDOMNode(this) : null;
  }
  render() {
    return ht.cloneElement(ht.Children.only(this.props.children), {
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
ye.displayName = "DraggableCore";
ye.propTypes = {
  /**
   * `allowAnyClick` allows dragging using any mouse button.
   * By default, we only accept the left button.
   *
   * Defaults to `false`.
   */
  allowAnyClick: A.default.bool,
  /**
   * `allowMobileScroll` turns off cancellation of the 'touchstart' event
   * on mobile devices. Only enable this if you are having trouble with click
   * events. Prefer using 'handle' / 'cancel' instead.
   *
   * Defaults to `false`.
   */
  allowMobileScroll: A.default.bool,
  children: A.default.node.isRequired,
  /**
   * `disabled`, if true, stops the <Draggable> from dragging. All handlers,
   * with the exception of `onMouseDown`, will not fire.
   */
  disabled: A.default.bool,
  /**
   * By default, we add 'user-select:none' attributes to the document body
   * to prevent ugly text selection during drag. If this is causing problems
   * for your app, set this to `false`.
   */
  enableUserSelectHack: A.default.bool,
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
  grid: A.default.arrayOf(A.default.number),
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
  handle: A.default.string,
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
  cancel: A.default.string,
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
  nodeRef: A.default.object,
  /**
   * `nonce` is applied to the dynamically-injected <style> element used by the
   * user-select hack, so it isn't blocked under a strict Content Security
   * Policy (`style-src` without `'unsafe-inline'`). If omitted, webpack's
   * `__webpack_nonce__` global is used when available.
   */
  nonce: A.default.string,
  /**
   * Called when dragging starts.
   * If this function returns the boolean false, dragging will be canceled.
   */
  onStart: A.default.func,
  /**
   * Called while dragging.
   * If this function returns the boolean false, dragging will be canceled.
   */
  onDrag: A.default.func,
  /**
   * Called when dragging stops.
   * If this function returns the boolean false, the drag will remain active.
   */
  onStop: A.default.func,
  /**
   * A workaround option which can be passed if onMouseDown needs to be accessed,
   * since it'll always be blocked (as there is internal use of onMouseDown)
   */
  onMouseDown: A.default.func,
  /**
   * `scale`, if set, applies scaling while dragging an element
   */
  scale: A.default.number,
  /**
   * These properties should be defined on the child, not here.
   */
  className: _e,
  style: _e,
  transform: _e
};
ye.defaultProps = {
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
var rt = class extends Ie.Component {
  constructor(e) {
    super(e), this.onDragStart = (r, n) => {
      if (this.props.onStart(r, pt(this, n)) === !1) return !1;
      this.setState({ dragging: !0, dragged: !0 });
    }, this.onDrag = (r, n) => {
      if (!this.state.dragging) return !1;
      const o = pt(this, n), i = {
        x: o.x,
        y: o.y,
        slackX: 0,
        slackY: 0
      };
      if (this.props.bounds) {
        const { x: a, y: l } = i;
        i.x += this.state.slackX, i.y += this.state.slackY;
        const [u, c] = Xo(this, i.x, i.y);
        i.x = u, i.y = c, i.slackX = this.state.slackX + (a - i.x), i.slackY = this.state.slackY + (l - i.y), o.x = i.x, o.y = i.y, o.deltaX = i.x - this.state.x, o.deltaY = i.y - this.state.y;
      }
      if (this.props.onDrag(r, o) === !1) return !1;
      this.setState(i);
    }, this.onDragStop = (r, n) => {
      if (!this.state.dragging || this.props.onStop(r, pt(this, n)) === !1) return !1;
      const i = {
        dragging: !1,
        slackX: 0,
        slackY: 0
      };
      if (!!this.props.position) {
        const { x: a, y: l } = this.props.position;
        i.x = a, i.y = l;
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
    return e && (!r || e.x !== r.x || e.y !== r.y) ? {
      x: e.x,
      y: e.y,
      prevPropsPosition: { ...e }
    } : null;
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
    const r = Eo.default;
    return typeof r.findDOMNode == "function" ? r.findDOMNode(this) : null;
  }
  render() {
    const {
      axis: e,
      bounds: r,
      children: n,
      defaultPosition: o,
      defaultClassName: i,
      defaultClassNameDragging: s,
      defaultClassNameDragged: a,
      position: l,
      positionOffset: u,
      scale: c,
      ...d
    } = this.props;
    let f = {}, g = null;
    const _ = !!!l || this.state.dragging, D = l || o, m = {
      // Set left if horizontal drag is enabled
      x: Uo(this) && _ ? this.state.x : D.x,
      // Set top if vertical drag is enabled
      y: Vo(this) && _ ? this.state.y : D.y
    };
    this.state.isElementSVG ? g = qo(m, u) : f = Wo(m, u);
    const j = Ie.Children.only(n), w = (0, zo.clsx)(j.props.className || "", i, {
      [s]: this.state.dragging,
      [a]: this.state.dragged
    });
    return /* @__PURE__ */ Ie.createElement(ye, { ...d, onStart: this.onDragStart, onDrag: this.onDrag, onStop: this.onDragStop }, Ie.cloneElement(j, {
      className: w,
      style: { ...j.props.style, ...f },
      transform: g
    }));
  }
};
rt.displayName = "Draggable";
rt.propTypes = {
  // Accepts all props <DraggableCore> accepts.
  ...ye.propTypes,
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
  axis: M.default.oneOf(["both", "x", "y", "none"]),
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
  bounds: M.default.oneOfType([
    M.default.shape({
      left: M.default.number,
      right: M.default.number,
      top: M.default.number,
      bottom: M.default.number
    }),
    M.default.string,
    M.default.oneOf([!1])
  ]),
  defaultClassName: M.default.string,
  defaultClassNameDragging: M.default.string,
  defaultClassNameDragged: M.default.string,
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
  defaultPosition: M.default.shape({
    x: M.default.number,
    y: M.default.number
  }),
  positionOffset: M.default.shape({
    x: M.default.oneOfType([M.default.number, M.default.string]),
    y: M.default.oneOfType([M.default.number, M.default.string])
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
  position: M.default.shape({
    x: M.default.number,
    y: M.default.number
  }),
  /**
   * These properties should be defined on the child, not here.
   */
  className: _e,
  style: _e,
  transform: _e
};
rt.defaultProps = {
  ...ye.defaultProps,
  axis: "both",
  bounds: !1,
  defaultClassName: "react-draggable",
  defaultClassNameDragging: "react-draggable-dragging",
  defaultClassNameDragged: "react-draggable-dragged",
  defaultPosition: { x: 0, y: 0 },
  scale: 1
};
const jt = Po, Qo = jt.DraggableCore, rn = jt.default || jt;
et.exports = rn;
et.exports.default = rn;
et.exports.DraggableCore = Qo;
var nn = et.exports, nt = { exports: {} }, He = {}, Vt = {};
Vt.__esModule = !0;
Vt.cloneElement = ii;
var ei = ti(Q);
function ti(t) {
  return t && t.__esModule ? t : { default: t };
}
function gr(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function mr(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? gr(Object(r), !0).forEach(function(n) {
      ri(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : gr(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function ri(t, e, r) {
  return (e = ni(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function ni(t) {
  var e = oi(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function oi(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
function ii(t, e) {
  return e.style && t.props.style && (e.style = mr(mr({}, t.props.style), e.style)), e.className && t.props.className && (e.className = t.props.className + " " + e.className), /* @__PURE__ */ ei.default.cloneElement(t, e);
}
var $e = {};
$e.__esModule = !0;
$e.resizableProps = void 0;
var O = si(le);
function si(t) {
  return t && t.__esModule ? t : { default: t };
}
$e.resizableProps = {
  /*
  * Restricts resizing to a particular axis (default: 'both')
  * 'both' - allows resizing by width or height
  * 'x' - only allows the width to be changed
  * 'y' - only allows the height to be changed
  * 'none' - disables resizing altogether
  * */
  axis: O.default.oneOf(["both", "x", "y", "none"]),
  className: O.default.string,
  /*
  * Require that one and only one child be present.
  * */
  children: O.default.element.isRequired,
  /*
  * These will be passed wholesale to react-draggable's DraggableCore
  * */
  draggableOpts: O.default.shape({
    allowAnyClick: O.default.bool,
    cancel: O.default.string,
    children: O.default.node,
    disabled: O.default.bool,
    enableUserSelectHack: O.default.bool,
    // #251: Check for Element to support SSR environments where DOM globals don't exist
    offsetParent: typeof Element < "u" ? O.default.instanceOf(Element) : O.default.any,
    grid: O.default.arrayOf(O.default.number),
    handle: O.default.string,
    nodeRef: O.default.object,
    onStart: O.default.func,
    onDrag: O.default.func,
    onStop: O.default.func,
    onMouseDown: O.default.func,
    scale: O.default.number
  }),
  /*
  * Initial height
  * */
  height: function() {
    for (var t = arguments.length, e = new Array(t), r = 0; r < t; r++)
      e[r] = arguments[r];
    const n = e[0];
    return n.axis === "both" || n.axis === "y" ? O.default.number.isRequired(...e) : O.default.number(...e);
  },
  /*
  * Customize cursor resize handle
  * */
  handle: O.default.oneOfType([O.default.node, O.default.func]),
  /*
  * If you change this, be sure to update your css
  * */
  handleSize: O.default.arrayOf(O.default.number),
  lockAspectRatio: O.default.bool,
  /*
  * Max X & Y measure
  * */
  maxConstraints: O.default.arrayOf(O.default.number),
  /*
  * Min X & Y measure
  * */
  minConstraints: O.default.arrayOf(O.default.number),
  /*
  * Called on stop resize event
  * */
  onResizeStop: O.default.func,
  /*
  * Called on start resize event
  * */
  onResizeStart: O.default.func,
  /*
  * Called on resize event
  * */
  onResize: O.default.func,
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
  resizeHandles: O.default.arrayOf(O.default.oneOf(["s", "w", "e", "n", "sw", "nw", "se", "ne"])),
  /*
  * If `transform: scale(n)` is set on the parent, this should be set to `n`.
  * */
  transformScale: O.default.number,
  /*
   * Initial width
   */
  width: function() {
    for (var t = arguments.length, e = new Array(t), r = 0; r < t; r++)
      e[r] = arguments[r];
    const n = e[0];
    return n.axis === "both" || n.axis === "x" ? O.default.number.isRequired(...e) : O.default.number(...e);
  }
};
He.__esModule = !0;
He.default = void 0;
var ve = on(Q), ai = nn, li = Vt, ui = $e;
const ci = ["children", "className", "draggableOpts", "width", "height", "handle", "handleSize", "lockAspectRatio", "axis", "minConstraints", "maxConstraints", "onResize", "onResizeStop", "onResizeStart", "resizeHandles", "transformScale"];
function on(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (on = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var s, a, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (s = i ? n : r) {
      if (s.has(o)) return s.get(o);
      s.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((a = (s = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (a.get || a.set) ? s(l, u, a) : l[u] = o[u]);
    return l;
  })(t, e);
}
function kt() {
  return kt = Object.assign ? Object.assign.bind() : function(t) {
    for (var e = 1; e < arguments.length; e++) {
      var r = arguments[e];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (t[n] = r[n]);
    }
    return t;
  }, kt.apply(null, arguments);
}
function fi(t, e) {
  if (t == null) return {};
  var r = {};
  for (var n in t) if ({}.hasOwnProperty.call(t, n)) {
    if (e.indexOf(n) !== -1) continue;
    r[n] = t[n];
  }
  return r;
}
function yr(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function gt(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? yr(Object(r), !0).forEach(function(n) {
      di(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : yr(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function di(t, e, r) {
  return (e = pi(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function pi(t) {
  var e = hi(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function hi(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
class Kt extends ve.Component {
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
    const n = this.props, o = n.minConstraints, i = n.maxConstraints, s = n.lockAspectRatio;
    if (!o && !i && !s) return [e, r];
    if (s) {
      const f = this.props.width / this.props.height, g = e - this.props.width, p = r - this.props.height;
      Math.abs(g) > Math.abs(p * f) ? r = e / f : e = r * f;
    }
    const a = e, l = r;
    let u = this.slack || [0, 0], c = u[0], d = u[1];
    return e += c, r += d, o && (e = Math.max(o[0], e), r = Math.max(o[1], r)), i && (e = Math.min(i[0], e), r = Math.min(i[1], r)), this.slack = [c + (a - e), d + (l - r)], [e, r];
  }
  /**
   * Wrapper around drag events to provide more useful data.
   *
   * @param  {String} handlerName Handler name to wrap.
   * @return {Function}           Handler function.
   */
  resizeHandler(e, r) {
    return (n, o) => {
      var i, s, a, l;
      let u = o.node, c = o.deltaX, d = o.deltaY;
      e === "onResizeStart" && this.resetData();
      const f = (this.props.axis === "both" || this.props.axis === "x") && r !== "n" && r !== "s", g = (this.props.axis === "both" || this.props.axis === "y") && r !== "e" && r !== "w";
      if (!f && !g) return;
      const p = r[0], _ = r[r.length - 1], D = u.getBoundingClientRect();
      if (this.lastHandleRect != null) {
        if (_ === "w") {
          const Y = D.left - this.lastHandleRect.left;
          c += Y;
        }
        if (p === "n") {
          const Y = D.top - this.lastHandleRect.top;
          d += Y;
        }
      }
      this.lastHandleRect = D, _ === "w" && (c = -c), p === "n" && (d = -d);
      const m = (i = (s = this.lastSize) == null ? void 0 : s.width) != null ? i : this.props.width, j = (a = (l = this.lastSize) == null ? void 0 : l.height) != null ? a : this.props.height;
      let w = m + (f ? c / this.props.transformScale : 0), L = j + (g ? d / this.props.transformScale : 0);
      var x = this.runConstraints(w, L);
      if (w = x[0], L = x[1], e === "onResizeStop" && this.lastSize) {
        var G = this.lastSize;
        w = G.width, L = G.height;
      }
      const U = w !== m || L !== j;
      e !== "onResizeStop" && (this.lastSize = {
        width: w,
        height: L
      });
      const X = typeof this.props[e] == "function" ? this.props[e] : null;
      X && !(e === "onResize" && !U) && (n.persist == null || n.persist(), X(n, {
        node: u,
        size: {
          width: w,
          height: L
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
      return /* @__PURE__ */ ve.createElement("span", {
        className: "react-resizable-handle react-resizable-handle-" + e,
        ref: r
      });
    if (typeof n == "function")
      return n(e, r);
    const o = typeof n.type == "string", i = gt({
      ref: r
    }, o ? {} : {
      handleAxis: e
    });
    return /* @__PURE__ */ ve.cloneElement(n, i);
  }
  render() {
    const e = this.props, r = e.children, n = e.className, o = e.draggableOpts;
    e.width, e.height, e.handle, e.handleSize, e.lockAspectRatio, e.axis, e.minConstraints, e.maxConstraints, e.onResize, e.onResizeStop, e.onResizeStart;
    const i = e.resizeHandles;
    e.transformScale;
    const s = fi(e, ci);
    return (0, li.cloneElement)(r, gt(gt({}, s), {}, {
      className: (n ? n + " " : "") + "react-resizable",
      children: [...ve.Children.toArray(r.props.children), ...i.map((a) => {
        var l;
        const u = (l = this.handleRefs[a]) != null ? l : this.handleRefs[a] = /* @__PURE__ */ ve.createRef();
        return /* @__PURE__ */ ve.createElement(ai.DraggableCore, kt({}, o, {
          nodeRef: u,
          key: "resizableHandle-" + a,
          onStop: this.resizeHandler("onResizeStop", a),
          onStart: this.resizeHandler("onResizeStart", a),
          onDrag: this.resizeHandler("onResize", a)
        }), this.renderResizeHandle(a, u));
      })]
    }));
  }
}
He.default = Kt;
Kt.propTypes = ui.resizableProps;
Kt.defaultProps = {
  axis: "both",
  handleSize: [20, 20],
  lockAspectRatio: !1,
  minConstraints: [20, 20],
  maxConstraints: [1 / 0, 1 / 0],
  resizeHandles: ["se"],
  transformScale: 1
};
var ot = {};
ot.__esModule = !0;
ot.default = void 0;
var mt = an(Q), gi = sn(le), mi = sn(He), yi = $e;
const bi = ["handle", "handleSize", "onResize", "onResizeStart", "onResizeStop", "draggableOpts", "minConstraints", "maxConstraints", "lockAspectRatio", "axis", "width", "height", "resizeHandles", "style", "transformScale"];
function sn(t) {
  return t && t.__esModule ? t : { default: t };
}
function an(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (an = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var s, a, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (s = i ? n : r) {
      if (s.has(o)) return s.get(o);
      s.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((a = (s = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (a.get || a.set) ? s(l, u, a) : l[u] = o[u]);
    return l;
  })(t, e);
}
function Mt() {
  return Mt = Object.assign ? Object.assign.bind() : function(t) {
    for (var e = 1; e < arguments.length; e++) {
      var r = arguments[e];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (t[n] = r[n]);
    }
    return t;
  }, Mt.apply(null, arguments);
}
function br(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Fe(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? br(Object(r), !0).forEach(function(n) {
      vi(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : br(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function vi(t, e, r) {
  return (e = wi(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function wi(t) {
  var e = Oi(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function Oi(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
function Si(t, e) {
  if (t == null) return {};
  var r = {};
  for (var n in t) if ({}.hasOwnProperty.call(t, n)) {
    if (e.indexOf(n) !== -1) continue;
    r[n] = t[n];
  }
  return r;
}
class ln extends mt.Component {
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
    const o = e.onResizeStart, i = e.onResizeStop, s = e.draggableOpts, a = e.minConstraints, l = e.maxConstraints, u = e.lockAspectRatio, c = e.axis;
    e.width, e.height;
    const d = e.resizeHandles, f = e.style, g = e.transformScale, p = Si(e, bi);
    return /* @__PURE__ */ mt.createElement(mi.default, {
      axis: c,
      draggableOpts: s,
      handle: r,
      handleSize: n,
      height: this.state.height,
      lockAspectRatio: u,
      maxConstraints: l,
      minConstraints: a,
      onResizeStart: o,
      onResize: this.onResize,
      onResizeStop: i,
      resizeHandles: d,
      transformScale: g,
      width: this.state.width
    }, /* @__PURE__ */ mt.createElement("div", Mt({}, p, {
      style: Fe(Fe({}, f), {}, {
        width: this.state.width + "px",
        height: this.state.height + "px"
      })
    })));
  }
}
ot.default = ln;
ln.propTypes = Fe(Fe({}, yi.resizableProps), {}, {
  children: gi.default.element
});
nt.exports = function() {
  throw new Error("Don't instantiate Resizable directly! Use require('react-resizable').Resizable");
};
nt.exports.Resizable = He.default;
nt.exports.ResizableBox = ot.default;
var _i = nt.exports, oe = {};
Object.defineProperty(oe, "__esModule", {
  value: !0
});
oe.resizeHandleType = oe.resizeHandleAxesType = oe.default = void 0;
var S = un(le), Ri = un(Q);
function un(t) {
  return t && t.__esModule ? t : { default: t };
}
const Di = oe.resizeHandleAxesType = S.default.arrayOf(S.default.oneOf(["s", "w", "e", "n", "sw", "nw", "se", "ne"])), xi = oe.resizeHandleType = S.default.oneOfType([S.default.node, S.default.func]);
oe.default = {
  //
  // Basic props
  //
  className: S.default.string,
  style: S.default.object,
  // This can be set explicitly. If it is not set, it will automatically
  // be set to the container width. Note that resizes will *not* cause this to adjust.
  // If you need that behavior, use WidthProvider.
  width: S.default.number,
  // If true, the container height swells and contracts to fit contents
  autoSize: S.default.bool,
  // # of cols.
  cols: S.default.number,
  // A selector that will not be draggable.
  draggableCancel: S.default.string,
  // A selector for the draggable handler
  draggableHandle: S.default.string,
  // Deprecated
  verticalCompact: function(t) {
    t.verticalCompact;
  },
  // Choose vertical or hotizontal compaction
  compactType: S.default.oneOf(["vertical", "horizontal"]),
  // layout is an array of object with the format:
  // {x: Number, y: Number, w: Number, h: Number, i: String}
  layout: function(t) {
    var e = t.layout;
    e !== void 0 && R.validateLayout(e, "layout");
  },
  //
  // Grid Dimensions
  //
  // Margin between items [x, y] in px
  margin: S.default.arrayOf(S.default.number),
  // Padding inside the container [x, y] in px
  containerPadding: S.default.arrayOf(S.default.number),
  // Rows have a static height, but you can change this based on breakpoints if you like
  rowHeight: S.default.number,
  // Default Infinity, but you can specify a max here if you like.
  // Note that this isn't fully fleshed out and won't error if you specify a layout that
  // extends beyond the row capacity. It will, however, not allow users to drag/resize
  // an item past the barrier. They can push items beyond the barrier, though.
  // Intentionally not documented for this reason.
  maxRows: S.default.number,
  //
  // Flags
  //
  isBounded: S.default.bool,
  isDraggable: S.default.bool,
  isResizable: S.default.bool,
  // If true, grid can be placed one over the other.
  allowOverlap: S.default.bool,
  // If true, grid items won't change position when being dragged over.
  preventCollision: S.default.bool,
  // Use CSS transforms instead of top/left
  useCSSTransforms: S.default.bool,
  // parent layout transform scale
  transformScale: S.default.number,
  // If true, an external element can trigger onDrop callback with a specific grid position as a parameter
  isDroppable: S.default.bool,
  // Resize handle options
  resizeHandles: Di,
  resizeHandle: xi,
  //
  // Callbacks
  //
  // Callback so you can save the layout. Calls after each drag & resize stops.
  onLayoutChange: S.default.func,
  // Calls when drag starts. Callback is of the signature (layout, oldItem, newItem, placeholder, e, ?node).
  // All callbacks below have the same signature. 'start' and 'stop' callbacks omit the 'placeholder'.
  onDragStart: S.default.func,
  // Calls on each drag movement.
  onDrag: S.default.func,
  // Calls when drag is complete.
  onDragStop: S.default.func,
  //Calls when resize starts.
  onResizeStart: S.default.func,
  // Calls when resize movement happens.
  onResize: S.default.func,
  // Calls when resize is complete.
  onResizeStop: S.default.func,
  // Calls when some element is dropped.
  onDrop: S.default.func,
  //
  // Other validations
  //
  droppingItem: S.default.shape({
    i: S.default.string.isRequired,
    w: S.default.number.isRequired,
    h: S.default.number.isRequired
  }),
  // Children must not have duplicate keys.
  children: function(t, e) {
    const r = t[e], n = {};
    Ri.default.Children.forEach(r, function(o) {
      if ((o == null ? void 0 : o.key) != null) {
        if (n[o.key])
          throw new Error('Duplicate child key "' + o.key + '" found! This will cause problems in ReactGridLayout.');
        n[o.key] = !0;
      }
    });
  },
  // Optional ref for getting a reference for the wrapping div.
  innerRef: S.default.any
};
Object.defineProperty(Qe, "__esModule", {
  value: !0
});
Qe.default = void 0;
var we = Zt(Q), vr = Ht, E = Zt(le), Pi = nn, Ei = _i, Oe = R, q = ne, wr = oe, zi = Zt(Ve);
function Zt(t) {
  return t && t.__esModule ? t : { default: t };
}
function Or(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function yt(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? Or(Object(r), !0).forEach(function(n) {
      Z(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : Or(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function Z(t, e, r) {
  return (e = Ci(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function Ci(t) {
  var e = ji(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function ji(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
class Jt extends we.default.Component {
  constructor() {
    super(...arguments), Z(this, "state", {
      resizing: null,
      dragging: null,
      className: ""
    }), Z(this, "elementRef", /* @__PURE__ */ we.default.createRef()), Z(this, "onDragStart", (e, r) => {
      let {
        node: n
      } = r;
      const {
        onDragStart: o,
        transformScale: i
      } = this.props;
      if (!o) return;
      const s = {
        top: 0,
        left: 0
      }, {
        offsetParent: a
      } = n;
      if (!a) return;
      const l = a.getBoundingClientRect(), u = n.getBoundingClientRect(), c = u.left / i, d = l.left / i, f = u.top / i, g = l.top / i;
      s.left = c - d + a.scrollLeft, s.top = f - g + a.scrollTop, this.setState({
        dragging: s
      });
      const {
        x: p,
        y: _
      } = (0, q.calcXY)(this.getPositionParams(), s.top, s.left, this.props.w, this.props.h);
      return o.call(this, this.props.i, p, _, {
        e,
        node: n,
        newPosition: s
      });
    }), Z(this, "onDrag", (e, r, n) => {
      let {
        node: o,
        deltaX: i,
        deltaY: s
      } = r;
      const {
        onDrag: a
      } = this.props;
      if (!a) return;
      if (!this.state.dragging)
        throw new Error("onDrag called before onDragStart.");
      let l = this.state.dragging.top + s, u = this.state.dragging.left + i;
      const {
        isBounded: c,
        i: d,
        w: f,
        h: g,
        containerWidth: p
      } = this.props, _ = this.getPositionParams();
      if (c) {
        const {
          offsetParent: w
        } = o;
        if (w) {
          const {
            margin: L,
            rowHeight: x
          } = this.props, G = w.clientHeight - (0, q.calcGridItemWHPx)(g, x, L[1]);
          l = (0, q.clamp)(l, 0, G);
          const U = (0, q.calcGridColWidth)(_), X = p - (0, q.calcGridItemWHPx)(f, U, L[0]);
          u = (0, q.clamp)(u, 0, X);
        }
      }
      const D = {
        top: l,
        left: u
      };
      n ? this.setState({
        dragging: D
      }) : (0, vr.flushSync)(() => {
        this.setState({
          dragging: D
        });
      });
      const {
        x: m,
        y: j
      } = (0, q.calcXY)(_, l, u, f, g);
      return a.call(this, d, m, j, {
        e,
        node: o,
        newPosition: D
      });
    }), Z(this, "onDragStop", (e, r) => {
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
        h: s,
        i: a
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
        x: d,
        y: f
      } = (0, q.calcXY)(this.getPositionParams(), u, l, i, s);
      return o.call(this, a, d, f, {
        e,
        node: n,
        newPosition: c
      });
    }), Z(this, "onResizeStop", (e, r, n) => this.onResizeHandler(e, r, n, "onResizeStop")), Z(this, "onResizeStart", (e, r, n) => this.onResizeHandler(e, r, n, "onResizeStart")), Z(this, "onResize", (e, r, n) => this.onResizeHandler(e, r, n, "onResize"));
  }
  shouldComponentUpdate(e, r) {
    if (this.props.children !== e.children || this.props.droppingPosition !== e.droppingPosition) return !0;
    const n = (0, q.calcGridItemPosition)(this.getPositionParams(this.props), this.props.x, this.props.y, this.props.w, this.props.h, this.state), o = (0, q.calcGridItemPosition)(this.getPositionParams(e), e.x, e.y, e.w, e.h, r);
    return !(0, Oe.fastPositionEqual)(n, o) || this.props.useCSSTransforms !== e.useCSSTransforms;
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
    } = this.state, s = i && r.left !== o.left || r.top !== o.top;
    if (!i)
      this.onDragStart(r.e, {
        node: n,
        deltaX: r.left,
        deltaY: r.top
      });
    else if (s) {
      const a = r.left - i.left, l = r.top - i.top;
      this.onDrag(
        r.e,
        {
          node: n,
          deltaX: a,
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
    return o ? i = (0, Oe.setTransform)(e) : (i = (0, Oe.setTopLeft)(e), r && (i.left = (0, Oe.perc)(e.left / n), i.width = (0, Oe.perc)(e.width / n))), i;
  }
  /**
   * Mix a Draggable instance into a child.
   * @param  {Element} child    Child element.
   * @return {Element}          Child wrapped in Draggable.
   */
  mixinDraggable(e, r) {
    return /* @__PURE__ */ we.default.createElement(Pi.DraggableCore, {
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
      minH: s,
      maxW: a,
      maxH: l,
      transformScale: u,
      resizeHandles: c,
      resizeHandle: d
    } = this.props, f = this.getPositionParams(), g = (0, q.calcGridItemPosition)(f, 0, 0, o, 0).width, p = (0, q.calcGridItemPosition)(f, 0, 0, i, s), _ = (0, q.calcGridItemPosition)(f, 0, 0, a, l), D = [p.width, p.height], m = [Math.min(_.width, g), Math.min(_.height, 1 / 0)];
    return /* @__PURE__ */ we.default.createElement(
      Ei.Resizable,
      {
        draggableOpts: {
          disabled: !n
        },
        className: n ? void 0 : "react-resizable-hide",
        width: r.width,
        height: r.height,
        minConstraints: D,
        maxConstraints: m,
        onResizeStop: this.curryResizeHandler(r, this.onResizeStop),
        onResizeStart: this.curryResizeHandler(r, this.onResizeStart),
        onResize: this.curryResizeHandler(r, this.onResize),
        transformScale: u,
        resizeHandles: c,
        handle: d
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
      size: s,
      handle: a
    } = r;
    const l = this.props[o];
    if (!l) return;
    const {
      x: u,
      y: c,
      i: d,
      maxH: f,
      minH: g,
      containerWidth: p
    } = this.props, {
      minW: _,
      maxW: D
    } = this.props;
    let m = s;
    i && (m = (0, Oe.resizeItemInDirection)(a, n, s, p), (0, vr.flushSync)(() => {
      this.setState({
        resizing: o === "onResizeStop" ? null : m
      });
    }));
    let {
      w: j,
      h: w
    } = (0, q.calcWH)(this.getPositionParams(), m.width, m.height, u, c, a);
    j = (0, q.clamp)(j, Math.max(_, 1), D), w = (0, q.clamp)(w, g, f), l.call(this, d, j, w, {
      e,
      node: i,
      size: m,
      handle: a
    });
  }
  render() {
    const {
      x: e,
      y: r,
      w: n,
      h: o,
      isDraggable: i,
      isResizable: s,
      droppingPosition: a,
      useCSSTransforms: l
    } = this.props, u = (0, q.calcGridItemPosition)(this.getPositionParams(), e, r, n, o, this.state), c = we.default.Children.only(this.props.children);
    let d = /* @__PURE__ */ we.default.cloneElement(c, {
      ref: this.elementRef,
      className: (0, zi.default)("react-grid-item", c.props.className, this.props.className, {
        static: this.props.static,
        resizing: !!this.state.resizing,
        "react-draggable": i,
        "react-draggable-dragging": !!this.state.dragging,
        dropping: !!a,
        cssTransforms: l
      }),
      // We can set the width and height on the child, but unfortunately we can't set the position.
      style: yt(yt(yt({}, this.props.style), c.props.style), this.createStyle(u))
    });
    return d = this.mixinResizable(d, u, s), d = this.mixinDraggable(d, i), d;
  }
}
Qe.default = Jt;
Z(Jt, "propTypes", {
  // Children must be only a single element
  children: E.default.element,
  // General grid attributes
  cols: E.default.number.isRequired,
  containerWidth: E.default.number.isRequired,
  rowHeight: E.default.number.isRequired,
  margin: E.default.array.isRequired,
  maxRows: E.default.number.isRequired,
  containerPadding: E.default.array.isRequired,
  // These are all in grid units
  x: E.default.number.isRequired,
  y: E.default.number.isRequired,
  w: E.default.number.isRequired,
  h: E.default.number.isRequired,
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
  i: E.default.string.isRequired,
  // Resize handle options
  resizeHandles: wr.resizeHandleAxesType,
  resizeHandle: wr.resizeHandleType,
  // Functions
  onDragStop: E.default.func,
  onDragStart: E.default.func,
  onDrag: E.default.func,
  onResizeStop: E.default.func,
  onResizeStart: E.default.func,
  onResize: E.default.func,
  // Flags
  isDraggable: E.default.bool.isRequired,
  isResizable: E.default.bool.isRequired,
  isBounded: E.default.bool.isRequired,
  static: E.default.bool,
  // Use CSS transforms instead of top/left
  useCSSTransforms: E.default.bool.isRequired,
  transformScale: E.default.number,
  // Others
  className: E.default.string,
  // Selector for draggable handle
  handle: E.default.string,
  // Selector for draggable cancel (see react-draggable)
  cancel: E.default.string,
  // Current position of a dropping element
  droppingPosition: E.default.shape({
    e: E.default.object.isRequired,
    left: E.default.number.isRequired,
    top: E.default.number.isRequired
  })
});
Z(Jt, "defaultProps", {
  className: "",
  cancel: "",
  handle: "",
  minH: 1,
  minW: 1,
  maxH: 1 / 0,
  maxW: 1 / 0,
  transformScale: 1
});
Object.defineProperty(Te, "__esModule", {
  value: !0
});
Te.default = void 0;
var fe = cn(Q), bt = Wt, ki = Qt(Ve), y = R, Mi = ne, Sr = Qt(Qe), Li = Qt(oe);
function Qt(t) {
  return t && t.__esModule ? t : { default: t };
}
function cn(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (cn = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var s, a, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (s = i ? n : r) {
      if (s.has(o)) return s.get(o);
      s.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((a = (s = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (a.get || a.set) ? s(l, u, a) : l[u] = o[u]);
    return l;
  })(t, e);
}
function _r(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function de(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? _r(Object(r), !0).forEach(function(n) {
      I(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : _r(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function I(t, e, r) {
  return (e = Ni(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function Ni(t) {
  var e = Ti(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function Ti(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
const Rr = "react-grid-layout";
let fn = !1;
try {
  fn = /firefox/i.test(navigator.userAgent);
} catch {
}
class it extends fe.Component {
  constructor() {
    super(...arguments), I(this, "state", {
      activeDrag: null,
      layout: (0, y.synchronizeLayoutWithChildren)(
        this.props.layout,
        this.props.children,
        this.props.cols,
        // Legacy support for verticalCompact: false
        (0, y.compactType)(this.props),
        this.props.allowOverlap
      ),
      mounted: !1,
      oldDragItem: null,
      oldLayout: null,
      oldResizeItem: null,
      resizing: !1,
      droppingDOMNode: null,
      children: []
    }), I(this, "dragEnterCounter", 0), I(this, "onDragStart", (e, r, n, o) => {
      let {
        e: i,
        node: s
      } = o;
      const {
        layout: a
      } = this.state, l = (0, y.getLayoutItem)(a, e);
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
        oldDragItem: (0, y.cloneLayoutItem)(l),
        oldLayout: a,
        activeDrag: u
      }), this.props.onDragStart(a, l, l, null, i, s);
    }), I(this, "onDrag", (e, r, n, o) => {
      let {
        e: i,
        node: s
      } = o;
      const {
        oldDragItem: a
      } = this.state;
      let {
        layout: l
      } = this.state;
      const {
        cols: u,
        allowOverlap: c,
        preventCollision: d
      } = this.props, f = (0, y.getLayoutItem)(l, e);
      if (!f) return;
      const g = {
        w: f.w,
        h: f.h,
        x: f.x,
        y: f.y,
        placeholder: !0,
        i: e
      };
      l = (0, y.moveElement)(l, f, r, n, !0, d, (0, y.compactType)(this.props), u, c), this.props.onDrag(l, a, f, g, i, s), this.setState({
        layout: c ? l : (0, y.compact)(l, (0, y.compactType)(this.props), u),
        activeDrag: g
      });
    }), I(this, "onDragStop", (e, r, n, o) => {
      let {
        e: i,
        node: s
      } = o;
      if (!this.state.activeDrag) return;
      const {
        oldDragItem: a
      } = this.state;
      let {
        layout: l
      } = this.state;
      const {
        cols: u,
        preventCollision: c,
        allowOverlap: d
      } = this.props, f = (0, y.getLayoutItem)(l, e);
      if (!f) return;
      l = (0, y.moveElement)(l, f, r, n, !0, c, (0, y.compactType)(this.props), u, d);
      const p = d ? l : (0, y.compact)(l, (0, y.compactType)(this.props), u);
      this.props.onDragStop(p, a, f, null, i, s);
      const {
        oldLayout: _
      } = this.state;
      this.setState({
        activeDrag: null,
        layout: p,
        oldDragItem: null,
        oldLayout: null
      }), this.onLayoutMaybeChanged(p, _);
    }), I(this, "onResizeStart", (e, r, n, o) => {
      let {
        e: i,
        node: s
      } = o;
      const {
        layout: a
      } = this.state, l = (0, y.getLayoutItem)(a, e);
      l && (this.setState({
        oldResizeItem: (0, y.cloneLayoutItem)(l),
        oldLayout: this.state.layout,
        resizing: !0
      }), this.props.onResizeStart(a, l, l, null, i, s));
    }), I(this, "onResize", (e, r, n, o) => {
      let {
        e: i,
        node: s,
        size: a,
        handle: l
      } = o;
      const {
        oldResizeItem: u
      } = this.state, {
        layout: c
      } = this.state, {
        cols: d,
        preventCollision: f,
        allowOverlap: g
      } = this.props;
      let p = !1, _, D, m;
      const [j, w] = (0, y.withLayoutItem)(c, e, (x) => {
        let G;
        return D = x.x, m = x.y, ["sw", "w", "nw", "n", "ne"].indexOf(l) !== -1 && (["sw", "nw", "w"].indexOf(l) !== -1 && (D = x.x + (x.w - r), r = x.x !== D && D < 0 ? x.w : r, D = D < 0 ? 0 : D), ["ne", "n", "nw"].indexOf(l) !== -1 && (m = x.y + (x.h - n), n = x.y !== m && m < 0 ? x.h : n, m = m < 0 ? 0 : m), p = !0), f && !g && (G = (0, y.getAllCollisions)(c, de(de({}, x), {}, {
          w: r,
          h: n,
          x: D,
          y: m
        })).filter((X) => X.i !== x.i).length > 0, G && (m = x.y, n = x.h, D = x.x, r = x.w, p = !1)), x.w = r, x.h = n, x;
      });
      if (!w) return;
      _ = j, p && (_ = (0, y.moveElement)(j, w, D, m, !0, this.props.preventCollision, (0, y.compactType)(this.props), d, g));
      const L = {
        w: w.w,
        h: w.h,
        x: w.x,
        y: w.y,
        static: !0,
        i: e
      };
      this.props.onResize(_, u, w, L, i, s), this.setState({
        layout: g ? _ : (0, y.compact)(_, (0, y.compactType)(this.props), d),
        activeDrag: L
      });
    }), I(this, "onResizeStop", (e, r, n, o) => {
      let {
        e: i,
        node: s
      } = o;
      const {
        layout: a,
        oldResizeItem: l
      } = this.state, {
        cols: u,
        allowOverlap: c
      } = this.props, d = (0, y.getLayoutItem)(a, e), f = c ? a : (0, y.compact)(a, (0, y.compactType)(this.props), u);
      this.props.onResizeStop(f, l, d, null, i, s);
      const {
        oldLayout: g
      } = this.state;
      this.setState({
        activeDrag: null,
        layout: f,
        oldResizeItem: null,
        oldLayout: null,
        resizing: !1
      }), this.onLayoutMaybeChanged(f, g);
    }), I(this, "onDragOver", (e) => {
      var r;
      if (e.preventDefault(), e.stopPropagation(), fn && // $FlowIgnore can't figure this out
      !((r = e.nativeEvent.target) !== null && r !== void 0 && r.classList.contains(Rr)))
        return !1;
      const {
        droppingItem: n,
        onDropDragOver: o,
        margin: i,
        cols: s,
        rowHeight: a,
        maxRows: l,
        width: u,
        containerPadding: c,
        transformScale: d
      } = this.props, f = o == null ? void 0 : o(e);
      if (f === !1)
        return this.state.droppingDOMNode && this.removeDroppingPlaceholder(), !1;
      const g = de(de({}, n), f), {
        layout: p
      } = this.state, _ = e.currentTarget.getBoundingClientRect(), D = e.clientX - _.left, m = e.clientY - _.top, j = {
        left: D / d,
        top: m / d,
        e
      };
      if (this.state.droppingDOMNode) {
        if (this.state.droppingPosition) {
          const {
            left: w,
            top: L
          } = this.state.droppingPosition;
          (w != D || L != m) && this.setState({
            droppingPosition: j
          });
        }
      } else {
        const w = {
          cols: s,
          margin: i,
          maxRows: l,
          rowHeight: a,
          containerWidth: u,
          containerPadding: c || i
        }, L = (0, Mi.calcXY)(w, m, D, g.w, g.h);
        this.setState({
          droppingDOMNode: /* @__PURE__ */ fe.createElement("div", {
            key: g.i
          }),
          droppingPosition: j,
          layout: [...p, de(de({}, g), {}, {
            x: L.x,
            y: L.y,
            static: !1,
            isDraggable: !0
          })]
        });
      }
    }), I(this, "removeDroppingPlaceholder", () => {
      const {
        droppingItem: e,
        cols: r
      } = this.props, {
        layout: n
      } = this.state, o = (0, y.compact)(n.filter((i) => i.i !== e.i), (0, y.compactType)(this.props), r, this.props.allowOverlap);
      this.setState({
        layout: o,
        droppingDOMNode: null,
        activeDrag: null,
        droppingPosition: void 0
      });
    }), I(this, "onDragLeave", (e) => {
      e.preventDefault(), e.stopPropagation(), this.dragEnterCounter--, this.dragEnterCounter === 0 && this.removeDroppingPlaceholder();
    }), I(this, "onDragEnter", (e) => {
      e.preventDefault(), e.stopPropagation(), this.dragEnterCounter++;
    }), I(this, "onDrop", (e) => {
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
    return r.activeDrag ? null : (!(0, bt.deepEqual)(e.layout, r.propsLayout) || e.compactType !== r.compactType ? n = e.layout : (0, y.childrenEqual)(e.children, r.children) || (n = r.layout), n ? {
      layout: (0, y.synchronizeLayoutWithChildren)(n, e.children, e.cols, (0, y.compactType)(e), e.allowOverlap),
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
      this.props.children !== e.children || !(0, y.fastRGLPropsEqual)(this.props, e, bt.deepEqual) || this.state.activeDrag !== r.activeDrag || this.state.mounted !== r.mounted || this.state.droppingPosition !== r.droppingPosition
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
    const e = (0, y.bottom)(this.state.layout), r = this.props.containerPadding ? this.props.containerPadding[1] : this.props.margin[1];
    return e * this.props.rowHeight + (e - 1) * this.props.margin[1] + r * 2 + "px";
  }
  onLayoutMaybeChanged(e, r) {
    r || (r = this.state.layout), (0, bt.deepEqual)(r, e) || this.props.onLayoutChange(e);
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
      rowHeight: s,
      maxRows: a,
      useCSSTransforms: l,
      transformScale: u
    } = this.props;
    return /* @__PURE__ */ fe.createElement(Sr.default, {
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
      maxRows: a,
      rowHeight: s,
      isDraggable: !1,
      isResizable: !1,
      isBounded: !1,
      useCSSTransforms: l,
      transformScale: u
    }, /* @__PURE__ */ fe.createElement("div", null));
  }
  /**
   * Given a grid item, set its style attributes & surround in a <Draggable>.
   * @param  {Element} child React element.
   * @return {Element}       Element wrapped in draggable and properly placed.
   */
  processGridItem(e, r) {
    if (!e || !e.key) return;
    const n = (0, y.getLayoutItem)(this.state.layout, String(e.key));
    if (!n) return null;
    const {
      width: o,
      cols: i,
      margin: s,
      containerPadding: a,
      rowHeight: l,
      maxRows: u,
      isDraggable: c,
      isResizable: d,
      isBounded: f,
      useCSSTransforms: g,
      transformScale: p,
      draggableCancel: _,
      draggableHandle: D,
      resizeHandles: m,
      resizeHandle: j
    } = this.props, {
      mounted: w,
      droppingPosition: L
    } = this.state, x = typeof n.isDraggable == "boolean" ? n.isDraggable : !n.static && c, G = typeof n.isResizable == "boolean" ? n.isResizable : !n.static && d, U = n.resizeHandles || m, X = x && f && n.isBounded !== !1;
    return /* @__PURE__ */ fe.createElement(Sr.default, {
      containerWidth: o,
      cols: i,
      margin: s,
      containerPadding: a || s,
      maxRows: u,
      rowHeight: l,
      cancel: _,
      handle: D,
      onDragStop: this.onDragStop,
      onDragStart: this.onDragStart,
      onDrag: this.onDrag,
      onResizeStart: this.onResizeStart,
      onResize: this.onResize,
      onResizeStop: this.onResizeStop,
      isDraggable: x,
      isResizable: G,
      isBounded: X,
      useCSSTransforms: g && w,
      usePercentages: !w,
      transformScale: p,
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
      droppingPosition: r ? L : void 0,
      resizeHandles: U,
      resizeHandle: j
    }, e);
  }
  render() {
    const {
      className: e,
      style: r,
      isDroppable: n,
      innerRef: o
    } = this.props, i = (0, ki.default)(Rr, e), s = de({
      height: this.containerHeight()
    }, r);
    return /* @__PURE__ */ fe.createElement("div", {
      ref: o,
      className: i,
      style: s,
      onDrop: n ? this.onDrop : y.noop,
      onDragLeave: n ? this.onDragLeave : y.noop,
      onDragEnter: n ? this.onDragEnter : y.noop,
      onDragOver: n ? this.onDragOver : y.noop
    }, fe.Children.map(this.props.children, (a) => this.processGridItem(a)), n && this.state.droppingDOMNode && this.processGridItem(this.state.droppingDOMNode, !0), this.placeholder());
  }
}
Te.default = it;
I(it, "displayName", "ReactGridLayout");
I(it, "propTypes", Li.default);
I(it, "defaultProps", {
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
  onLayoutChange: y.noop,
  onDragStart: y.noop,
  onDrag: y.noop,
  onDragStop: y.noop,
  onResizeStart: y.noop,
  onResize: y.noop,
  onResizeStop: y.noop,
  onDrop: y.noop,
  onDropDragOver: y.noop
});
var st = {}, be = {};
Object.defineProperty(be, "__esModule", {
  value: !0
});
be.findOrGenerateResponsiveLayout = Wi;
be.getBreakpointFromWidth = Hi;
be.getColsFromBreakpoint = $i;
be.sortBreakpoints = er;
var Ae = R;
function Hi(t, e) {
  const r = er(t);
  let n = r[0];
  for (let o = 1, i = r.length; o < i; o++) {
    const s = r[o];
    e > t[s] && (n = s);
  }
  return n;
}
function $i(t, e) {
  if (!e[t])
    throw new Error("ResponsiveReactGridLayout: `cols` entry for breakpoint " + t + " is missing!");
  return e[t];
}
function Wi(t, e, r, n, o, i) {
  if (t[r]) return (0, Ae.cloneLayout)(t[r]);
  let s = t[n];
  const a = er(e), l = a.slice(a.indexOf(r));
  for (let u = 0, c = l.length; u < c; u++) {
    const d = l[u];
    if (t[d]) {
      s = t[d];
      break;
    }
  }
  return s = (0, Ae.cloneLayout)(s || []), (0, Ae.compact)((0, Ae.correctBounds)(s, {
    cols: o
  }), i, o);
}
function er(t) {
  return Object.keys(t).sort(function(r, n) {
    return t[r] - t[n];
  });
}
Object.defineProperty(st, "__esModule", {
  value: !0
});
st.default = void 0;
var Dr = pn(Q), F = dn(le), vt = Wt, Re = R, pe = be, qi = dn(Te);
const Ii = ["breakpoint", "breakpoints", "cols", "layouts", "margin", "containerPadding", "onBreakpointChange", "onLayoutChange", "onWidthChange"];
function dn(t) {
  return t && t.__esModule ? t : { default: t };
}
function pn(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (pn = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var s, a, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (s = i ? n : r) {
      if (s.has(o)) return s.get(o);
      s.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((a = (s = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (a.get || a.set) ? s(l, u, a) : l[u] = o[u]);
    return l;
  })(t, e);
}
function Lt() {
  return Lt = Object.assign ? Object.assign.bind() : function(t) {
    for (var e = 1; e < arguments.length; e++) {
      var r = arguments[e];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (t[n] = r[n]);
    }
    return t;
  }, Lt.apply(null, arguments);
}
function Ai(t, e) {
  if (t == null) return {};
  var r, n, o = Bi(t, e);
  if (Object.getOwnPropertySymbols) {
    var i = Object.getOwnPropertySymbols(t);
    for (n = 0; n < i.length; n++) r = i[n], e.indexOf(r) === -1 && {}.propertyIsEnumerable.call(t, r) && (o[r] = t[r]);
  }
  return o;
}
function Bi(t, e) {
  if (t == null) return {};
  var r = {};
  for (var n in t) if ({}.hasOwnProperty.call(t, n)) {
    if (e.indexOf(n) !== -1) continue;
    r[n] = t[n];
  }
  return r;
}
function xr(t, e) {
  var r = Object.keys(t);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(t);
    e && (n = n.filter(function(o) {
      return Object.getOwnPropertyDescriptor(t, o).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function wt(t) {
  for (var e = 1; e < arguments.length; e++) {
    var r = arguments[e] != null ? arguments[e] : {};
    e % 2 ? xr(Object(r), !0).forEach(function(n) {
      Le(t, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(t, Object.getOwnPropertyDescriptors(r)) : xr(Object(r)).forEach(function(n) {
      Object.defineProperty(t, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return t;
}
function Le(t, e, r) {
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
const Pr = (t) => Object.prototype.toString.call(t);
function Be(t, e) {
  return t == null ? null : Array.isArray(t) ? t : t[e];
}
class tr extends Dr.Component {
  constructor() {
    super(...arguments), Le(this, "state", this.generateInitialState()), Le(this, "onLayoutChange", (e) => {
      this.props.onLayoutChange(e, wt(wt({}, this.props.layouts), {}, {
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
    } = this.props, i = (0, pe.getBreakpointFromWidth)(r, e), s = (0, pe.getColsFromBreakpoint)(i, o), a = this.props.verticalCompact === !1 ? null : this.props.compactType;
    return {
      layout: (0, pe.findOrGenerateResponsiveLayout)(n, r, i, i, s, a),
      breakpoint: i,
      cols: s
    };
  }
  static getDerivedStateFromProps(e, r) {
    if (!(0, vt.deepEqual)(e.layouts, r.layouts)) {
      const {
        breakpoint: n,
        cols: o
      } = r;
      return {
        layout: (0, pe.findOrGenerateResponsiveLayout)(e.layouts, e.breakpoints, n, n, o, e.compactType),
        layouts: e.layouts
      };
    }
    return null;
  }
  componentDidUpdate(e) {
    (this.props.width != e.width || this.props.breakpoint !== e.breakpoint || !(0, vt.deepEqual)(this.props.breakpoints, e.breakpoints) || !(0, vt.deepEqual)(this.props.cols, e.cols)) && this.onWidthChange(e);
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
    } = this.props, s = this.props.breakpoint || (0, pe.getBreakpointFromWidth)(this.props.breakpoints, this.props.width), a = this.state.breakpoint, l = (0, pe.getColsFromBreakpoint)(s, n), u = wt({}, o);
    if (a !== s || e.breakpoints !== r || e.cols !== n) {
      a in u || (u[a] = (0, Re.cloneLayout)(this.state.layout));
      let f = (0, pe.findOrGenerateResponsiveLayout)(u, r, s, a, l, i);
      f = (0, Re.synchronizeLayoutWithChildren)(f, this.props.children, l, i, this.props.allowOverlap), u[s] = f, this.props.onBreakpointChange(s, l), this.props.onLayoutChange(f, u), this.setState({
        breakpoint: s,
        layout: f,
        cols: l
      });
    }
    const c = Be(this.props.margin, s), d = Be(this.props.containerPadding, s);
    this.props.onWidthChange(this.props.width, c, l, d);
  }
  render() {
    const e = this.props, {
      breakpoint: r,
      breakpoints: n,
      cols: o,
      layouts: i,
      margin: s,
      containerPadding: a,
      onBreakpointChange: l,
      onLayoutChange: u,
      onWidthChange: c
    } = e, d = Ai(e, Ii);
    return /* @__PURE__ */ Dr.createElement(qi.default, Lt({}, d, {
      // $FlowIgnore should allow nullable here due to DefaultProps
      margin: Be(s, this.state.breakpoint),
      containerPadding: Be(a, this.state.breakpoint),
      onLayoutChange: this.onLayoutChange,
      layout: this.state.layout,
      cols: this.state.cols
    }));
  }
}
st.default = tr;
Le(tr, "propTypes", {
  //
  // Basic props
  //
  // Optional, but if you are managing width yourself you may want to set the breakpoint
  // yourself as well.
  breakpoint: F.default.string,
  // {name: pxVal}, e.g. {lg: 1200, md: 996, sm: 768, xs: 480}
  breakpoints: F.default.object,
  allowOverlap: F.default.bool,
  // # of cols. This is a breakpoint -> cols map
  cols: F.default.object,
  // # of margin. This is a breakpoint -> margin map
  // e.g. { lg: [5, 5], md: [10, 10], sm: [15, 15] }
  // Margin between items [x, y] in px
  // e.g. [10, 10]
  margin: F.default.oneOfType([F.default.array, F.default.object]),
  // # of containerPadding. This is a breakpoint -> containerPadding map
  // e.g. { lg: [5, 5], md: [10, 10], sm: [15, 15] }
  // Padding inside the container [x, y] in px
  // e.g. [10, 10]
  containerPadding: F.default.oneOfType([F.default.array, F.default.object]),
  // layouts is an object mapping breakpoints to layouts.
  // e.g. {lg: Layout, md: Layout, ...}
  layouts(t, e) {
    if (Pr(t[e]) !== "[object Object]")
      throw new Error("Layout property must be an object. Received: " + Pr(t[e]));
    Object.keys(t[e]).forEach((r) => {
      if (!(r in t.breakpoints))
        throw new Error("Each key in layouts must align with a key in breakpoints.");
      (0, Re.validateLayout)(t.layouts[r], "layouts." + r);
    });
  },
  // The width of this component.
  // Required in this propTypes stanza because generateInitialState() will fail without it.
  width: F.default.number.isRequired,
  //
  // Callbacks
  //
  // Calls back with breakpoint and new # cols
  onBreakpointChange: F.default.func,
  // Callback so you can save the layout.
  // Calls back with (currentLayout, allLayouts). allLayouts are keyed by breakpoint.
  onLayoutChange: F.default.func,
  // Calls back with (containerWidth, margin, cols, containerPadding)
  onWidthChange: F.default.func
});
Le(tr, "defaultProps", {
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
  onBreakpointChange: Re.noop,
  onLayoutChange: Re.noop,
  onWidthChange: Re.noop
});
var rr = {}, hn = function() {
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
          var s = i[o];
          r.call(n, s[1], s[0]);
        }
      }, e;
    }()
  );
}(), Nt = typeof window < "u" && typeof document < "u" && window.document === document, Xe = function() {
  return typeof global < "u" && global.Math === Math ? global : typeof self < "u" && self.Math === Math ? self : typeof window < "u" && window.Math === Math ? window : Function("return this")();
}(), Fi = function() {
  return typeof requestAnimationFrame == "function" ? requestAnimationFrame.bind(Xe) : function(t) {
    return setTimeout(function() {
      return t(Date.now());
    }, 1e3 / 60);
  };
}(), Xi = 2;
function Ui(t, e) {
  var r = !1, n = !1, o = 0;
  function i() {
    r && (r = !1, t()), n && a();
  }
  function s() {
    Fi(i);
  }
  function a() {
    var l = Date.now();
    if (r) {
      if (l - o < Xi)
        return;
      n = !0;
    } else
      r = !0, n = !1, setTimeout(s, e);
    o = l;
  }
  return a;
}
var Vi = 20, Ki = ["top", "right", "bottom", "left", "width", "height", "size", "weight"], Zi = typeof MutationObserver < "u", Ji = (
  /** @class */
  function() {
    function t() {
      this.connected_ = !1, this.mutationEventsAdded_ = !1, this.mutationsObserver_ = null, this.observers_ = [], this.onTransitionEnd_ = this.onTransitionEnd_.bind(this), this.refresh = Ui(this.refresh.bind(this), Vi);
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
      !Nt || this.connected_ || (document.addEventListener("transitionend", this.onTransitionEnd_), window.addEventListener("resize", this.refresh), Zi ? (this.mutationsObserver_ = new MutationObserver(this.refresh), this.mutationsObserver_.observe(document, {
        attributes: !0,
        childList: !0,
        characterData: !0,
        subtree: !0
      })) : (document.addEventListener("DOMSubtreeModified", this.refresh), this.mutationEventsAdded_ = !0), this.connected_ = !0);
    }, t.prototype.disconnect_ = function() {
      !Nt || !this.connected_ || (document.removeEventListener("transitionend", this.onTransitionEnd_), window.removeEventListener("resize", this.refresh), this.mutationsObserver_ && this.mutationsObserver_.disconnect(), this.mutationEventsAdded_ && document.removeEventListener("DOMSubtreeModified", this.refresh), this.mutationsObserver_ = null, this.mutationEventsAdded_ = !1, this.connected_ = !1);
    }, t.prototype.onTransitionEnd_ = function(e) {
      var r = e.propertyName, n = r === void 0 ? "" : r, o = Ki.some(function(i) {
        return !!~n.indexOf(i);
      });
      o && this.refresh();
    }, t.getInstance = function() {
      return this.instance_ || (this.instance_ = new t()), this.instance_;
    }, t.instance_ = null, t;
  }()
), gn = function(t, e) {
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
}, De = function(t) {
  var e = t && t.ownerDocument && t.ownerDocument.defaultView;
  return e || Xe;
}, mn = at(0, 0, 0, 0);
function Ue(t) {
  return parseFloat(t) || 0;
}
function Er(t) {
  for (var e = [], r = 1; r < arguments.length; r++)
    e[r - 1] = arguments[r];
  return e.reduce(function(n, o) {
    var i = t["border-" + o + "-width"];
    return n + Ue(i);
  }, 0);
}
function Qi(t) {
  for (var e = ["top", "right", "bottom", "left"], r = {}, n = 0, o = e; n < o.length; n++) {
    var i = o[n], s = t["padding-" + i];
    r[i] = Ue(s);
  }
  return r;
}
function es(t) {
  var e = t.getBBox();
  return at(0, 0, e.width, e.height);
}
function ts(t) {
  var e = t.clientWidth, r = t.clientHeight;
  if (!e && !r)
    return mn;
  var n = De(t).getComputedStyle(t), o = Qi(n), i = o.left + o.right, s = o.top + o.bottom, a = Ue(n.width), l = Ue(n.height);
  if (n.boxSizing === "border-box" && (Math.round(a + i) !== e && (a -= Er(n, "left", "right") + i), Math.round(l + s) !== r && (l -= Er(n, "top", "bottom") + s)), !ns(t)) {
    var u = Math.round(a + i) - e, c = Math.round(l + s) - r;
    Math.abs(u) !== 1 && (a -= u), Math.abs(c) !== 1 && (l -= c);
  }
  return at(o.left, o.top, a, l);
}
var rs = /* @__PURE__ */ function() {
  return typeof SVGGraphicsElement < "u" ? function(t) {
    return t instanceof De(t).SVGGraphicsElement;
  } : function(t) {
    return t instanceof De(t).SVGElement && typeof t.getBBox == "function";
  };
}();
function ns(t) {
  return t === De(t).document.documentElement;
}
function os(t) {
  return Nt ? rs(t) ? es(t) : ts(t) : mn;
}
function is(t) {
  var e = t.x, r = t.y, n = t.width, o = t.height, i = typeof DOMRectReadOnly < "u" ? DOMRectReadOnly : Object, s = Object.create(i.prototype);
  return gn(s, {
    x: e,
    y: r,
    width: n,
    height: o,
    top: r,
    right: e + n,
    bottom: o + r,
    left: e
  }), s;
}
function at(t, e, r, n) {
  return { x: t, y: e, width: r, height: n };
}
var ss = (
  /** @class */
  function() {
    function t(e) {
      this.broadcastWidth = 0, this.broadcastHeight = 0, this.contentRect_ = at(0, 0, 0, 0), this.target = e;
    }
    return t.prototype.isActive = function() {
      var e = os(this.target);
      return this.contentRect_ = e, e.width !== this.broadcastWidth || e.height !== this.broadcastHeight;
    }, t.prototype.broadcastRect = function() {
      var e = this.contentRect_;
      return this.broadcastWidth = e.width, this.broadcastHeight = e.height, e;
    }, t;
  }()
), as = (
  /** @class */
  /* @__PURE__ */ function() {
    function t(e, r) {
      var n = is(r);
      gn(this, { target: e, contentRect: n });
    }
    return t;
  }()
), ls = (
  /** @class */
  function() {
    function t(e, r, n) {
      if (this.activeObservations_ = [], this.observations_ = new hn(), typeof e != "function")
        throw new TypeError("The callback provided as parameter 1 is not a function.");
      this.callback_ = e, this.controller_ = r, this.callbackCtx_ = n;
    }
    return t.prototype.observe = function(e) {
      if (!arguments.length)
        throw new TypeError("1 argument required, but only 0 present.");
      if (!(typeof Element > "u" || !(Element instanceof Object))) {
        if (!(e instanceof De(e).Element))
          throw new TypeError('parameter 1 is not of type "Element".');
        var r = this.observations_;
        r.has(e) || (r.set(e, new ss(e)), this.controller_.addObserver(this), this.controller_.refresh());
      }
    }, t.prototype.unobserve = function(e) {
      if (!arguments.length)
        throw new TypeError("1 argument required, but only 0 present.");
      if (!(typeof Element > "u" || !(Element instanceof Object))) {
        if (!(e instanceof De(e).Element))
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
          return new as(n.target, n.broadcastRect());
        });
        this.callback_.call(e, r, e), this.clearActive();
      }
    }, t.prototype.clearActive = function() {
      this.activeObservations_.splice(0);
    }, t.prototype.hasActive = function() {
      return this.activeObservations_.length > 0;
    }, t;
  }()
), yn = typeof WeakMap < "u" ? /* @__PURE__ */ new WeakMap() : new hn(), bn = (
  /** @class */
  /* @__PURE__ */ function() {
    function t(e) {
      if (!(this instanceof t))
        throw new TypeError("Cannot call a class as a function.");
      if (!arguments.length)
        throw new TypeError("1 argument required, but only 0 present.");
      var r = Ji.getInstance(), n = new ls(e, r, this);
      yn.set(this, n);
    }
    return t;
  }()
);
[
  "observe",
  "unobserve",
  "disconnect"
].forEach(function(t) {
  bn.prototype[t] = function() {
    var e;
    return (e = yn.get(this))[t].apply(e, arguments);
  };
});
var us = function() {
  return typeof Xe.ResizeObserver < "u" ? Xe.ResizeObserver : bn;
}();
const cs = /* @__PURE__ */ Object.freeze(/* @__PURE__ */ Object.defineProperty({
  __proto__: null,
  default: us
}, Symbol.toStringTag, { value: "Module" })), fs = /* @__PURE__ */ Bn(cs);
Object.defineProperty(rr, "__esModule", {
  value: !0
});
rr.default = Os;
var Ge = vn(Q), ds = nr(le), ps = nr(fs), hs = nr(Ve);
const gs = ["measureBeforeMount"];
function nr(t) {
  return t && t.__esModule ? t : { default: t };
}
function vn(t, e) {
  if (typeof WeakMap == "function") var r = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap();
  return (vn = function(o, i) {
    if (!i && o && o.__esModule) return o;
    var s, a, l = { __proto__: null, default: o };
    if (o === null || typeof o != "object" && typeof o != "function") return l;
    if (s = i ? n : r) {
      if (s.has(o)) return s.get(o);
      s.set(o, l);
    }
    for (const u in o) u !== "default" && {}.hasOwnProperty.call(o, u) && ((a = (s = Object.defineProperty) && Object.getOwnPropertyDescriptor(o, u)) && (a.get || a.set) ? s(l, u, a) : l[u] = o[u]);
    return l;
  })(t, e);
}
function Tt() {
  return Tt = Object.assign ? Object.assign.bind() : function(t) {
    for (var e = 1; e < arguments.length; e++) {
      var r = arguments[e];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (t[n] = r[n]);
    }
    return t;
  }, Tt.apply(null, arguments);
}
function ms(t, e) {
  if (t == null) return {};
  var r, n, o = ys(t, e);
  if (Object.getOwnPropertySymbols) {
    var i = Object.getOwnPropertySymbols(t);
    for (n = 0; n < i.length; n++) r = i[n], e.indexOf(r) === -1 && {}.propertyIsEnumerable.call(t, r) && (o[r] = t[r]);
  }
  return o;
}
function ys(t, e) {
  if (t == null) return {};
  var r = {};
  for (var n in t) if ({}.hasOwnProperty.call(t, n)) {
    if (e.indexOf(n) !== -1) continue;
    r[n] = t[n];
  }
  return r;
}
function Se(t, e, r) {
  return (e = bs(e)) in t ? Object.defineProperty(t, e, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : t[e] = r, t;
}
function bs(t) {
  var e = vs(t, "string");
  return typeof e == "symbol" ? e : e + "";
}
function vs(t, e) {
  if (typeof t != "object" || !t) return t;
  var r = t[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(t, e);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (e === "string" ? String : Number)(t);
}
const ws = "react-grid-layout";
function Os(t) {
  var e;
  return e = class extends Ge.Component {
    constructor() {
      super(...arguments), Se(this, "state", {
        width: 1280
      }), Se(this, "elementRef", /* @__PURE__ */ Ge.createRef()), Se(this, "mounted", !1), Se(this, "resizeObserver", void 0);
    }
    componentDidMount() {
      this.mounted = !0, this.resizeObserver = new ps.default((o) => {
        if (this.elementRef.current instanceof HTMLElement) {
          const s = o[0].contentRect.width;
          this.setState({
            width: s
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
      } = n, i = ms(n, gs);
      return o && !this.mounted ? /* @__PURE__ */ Ge.createElement("div", {
        className: (0, hs.default)(this.props.className, ws),
        style: this.props.style,
        ref: this.elementRef
      }) : /* @__PURE__ */ Ge.createElement(t, Tt({
        innerRef: this.elementRef
      }, i, this.state));
    }
  }, Se(e, "defaultProps", {
    measureBeforeMount: !1
  }), Se(e, "propTypes", {
    // If true, will not render children until mounted. Useful for getting the exact width before
    // rendering, to prevent any unsightly resizing.
    measureBeforeMount: ds.default.bool
  }), e;
}
(function(t) {
  t.exports = Te.default, t.exports.utils = R, t.exports.calculateUtils = ne, t.exports.Responsive = st.default, t.exports.Responsive.utils = be, t.exports.WidthProvider = rr.default;
})(Tr);
var Ss = Tr.exports;
const _s = /* @__PURE__ */ An(Ss);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Rs = (t) => t.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase(), wn = (...t) => t.filter((e, r, n) => !!e && e.trim() !== "" && n.indexOf(e) === r).join(" ").trim();
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
var Ds = {
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
const xs = jr(
  ({
    color: t = "currentColor",
    size: e = 24,
    strokeWidth: r = 2,
    absoluteStrokeWidth: n,
    className: o = "",
    children: i,
    iconNode: s,
    ...a
  }, l) => Ot(
    "svg",
    {
      ref: l,
      ...Ds,
      width: e,
      height: e,
      stroke: t,
      strokeWidth: n ? Number(r) * 24 / Number(e) : r,
      className: wn("lucide", o),
      ...a
    },
    [
      ...s.map(([u, c]) => Ot(u, c)),
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
const se = (t, e) => {
  const r = jr(
    ({ className: n, ...o }, i) => Ot(xs, {
      ref: i,
      iconNode: e,
      className: wn(`lucide-${Rs(t)}`, n),
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
const zr = se("ChevronDown", [
  ["path", { d: "m6 9 6 6 6-6", key: "qrunsl" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Ps = se("ChevronRight", [
  ["path", { d: "m9 18 6-6-6-6", key: "mthhwq" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Es = se("Copy", [
  ["rect", { width: "14", height: "14", x: "8", y: "8", rx: "2", ry: "2", key: "17jyea" }],
  ["path", { d: "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2", key: "zix9uf" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const zs = se("Download", [
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
const Cs = se("GripHorizontal", [
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
const js = se("GripVertical", [
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
const ks = se("Link", [
  ["path", { d: "M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71", key: "1cjeqo" }],
  ["path", { d: "M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71", key: "19qd67" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Ms = se("Pencil", [
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
const On = se("X", [
  ["path", { d: "M18 6 6 18", key: "1bl5f8" }],
  ["path", { d: "m6 6 12 12", key: "d8bk6v" }]
]);
function Ls({
  cell: t,
  memberCount: e,
  editable: r,
  onToggleCollapse: n,
  onRename: o,
  onRemove: i
}) {
  const s = Mr(t), a = Hn(t), [l, u] = St(!1), [c, d] = St(""), f = Rt(t), g = () => {
    const p = c.trim();
    p && p !== f && (o == null || o(t.i, p)), u(!1);
  };
  return /* @__PURE__ */ J(
    "div",
    {
      "data-row-header": "",
      className: `lbdg-row-header${a.showLine ? " lbdg-row-header--line" : ""}`,
      "aria-label": `row ${f}`,
      children: [
        r && /* @__PURE__ */ v(
          "span",
          {
            "aria-label": `move cell ${t.i}`,
            title: "Move row",
            className: "lbdg-drag-handle lbdg-row-grip",
            children: /* @__PURE__ */ v(js, { size: 13 })
          }
        ),
        l && r ? /* @__PURE__ */ J(Cr, { children: [
          /* @__PURE__ */ v("span", { className: "lbdg-row-chevron", children: /* @__PURE__ */ v(zr, { size: 16 }) }),
          /* @__PURE__ */ v(
            "input",
            {
              autoFocus: !0,
              "aria-label": "row title",
              className: "lbdg-no-drag lbdg-row-rename",
              defaultValue: f,
              onChange: (p) => d(p.target.value),
              onBlur: g,
              onKeyDown: (p) => {
                p.key === "Enter" && g(), p.key === "Escape" && u(!1);
              }
            }
          )
        ] }) : /* @__PURE__ */ J(
          "button",
          {
            type: "button",
            "aria-label": s ? `expand row ${f}` : `collapse row ${f}`,
            "aria-expanded": !s,
            title: s ? "Expand row" : "Collapse row",
            className: "lbdg-row-toggle",
            onClick: () => n == null ? void 0 : n(t.i),
            onDoubleClick: (p) => {
              !r || !o || (p.stopPropagation(), d(f), u(!0));
            },
            children: [
              /* @__PURE__ */ v("span", { className: "lbdg-row-chevron", children: s ? /* @__PURE__ */ v(Ps, { size: 16 }) : /* @__PURE__ */ v(zr, { size: 16 }) }),
              /* @__PURE__ */ v("span", { className: "lbdg-row-title", children: f }),
              a.showCount && e > 0 && /* @__PURE__ */ J("span", { className: "lbdg-row-count", children: [
                "· ",
                e,
                " panel",
                e === 1 ? "" : "s"
              ] })
            ]
          }
        ),
        r && i && /* @__PURE__ */ v(
          "button",
          {
            type: "button",
            "aria-label": `remove cell ${t.i}`,
            title: "Remove row",
            className: "lbdg-no-drag lbdg-btn lbdg-btn--danger lbdg-row-remove",
            onClick: () => i(t.i),
            children: /* @__PURE__ */ v(On, { size: 13 })
          }
        )
      ]
    }
  );
}
function Ns({
  cells: t,
  registry: e,
  range: r,
  scope: n,
  refreshKey: o
}) {
  const i = [...Lr(t)].sort((s, a) => s.y - a.y || s.x - a.x);
  return /* @__PURE__ */ v("div", { className: "lbdg-stack", "aria-label": "dashboard stack", children: i.map(
    (s) => ie(s) ? /* @__PURE__ */ J("div", { className: "lbdg-stack-row", "aria-label": `row ${Rt(s)}`, children: [
      /* @__PURE__ */ v("span", { className: "lbdg-row-title", children: Rt(s) }),
      Me(t, s).length > 0 && /* @__PURE__ */ J("span", { className: "lbdg-row-count", children: [
        "· ",
        Me(t, s).length
      ] })
    ] }, s.i) : /* @__PURE__ */ v(
      "div",
      {
        className: s.transparent ? "lbdg-cell lbdg-cell--transparent" : "lbdg-cell lbdg-cell--framed",
        style: { minHeight: `${s.h * kr}px` },
        "aria-label": `cell ${s.i}`,
        children: /* @__PURE__ */ v("div", { className: "lbdg-cell-body", children: (() => {
          const a = e.resolveCell(s);
          return a ? /* @__PURE__ */ v(a, { cell: s, range: r, scope: n, refreshKey: o, editable: !1 }) : /* @__PURE__ */ v(Nr, { view: Ne(s) });
        })() })
      },
      s.i
    )
  ) });
}
const Ts = 1200, Hs = ["s", "w", "e", "n", "sw", "nw", "se", "ne"];
function ea({
  cells: t,
  editable: e,
  registry: r,
  range: n,
  scope: o,
  refreshKey: i,
  onLayout: s,
  onRemove: a,
  onDuplicate: l,
  onToggleRow: u,
  onRenameRow: c,
  onEditPanel: d,
  onExportCell: f,
  stackBelow: g = 768,
  resizeHandles: p = Hs,
  droppable: _,
  droppingItem: D,
  onDrop: m
}) {
  const j = Mn(null), [w, L] = St(Ts);
  Ln(() => {
    const P = () => {
      const Pe = j.current;
      if (!Pe) return;
      const Ee = window.getComputedStyle(Pe), ze = Pe.clientWidth - (parseFloat(Ee.paddingLeft) || 0) - (parseFloat(Ee.paddingRight) || 0);
      ze > 0 && L(ze);
    };
    P();
    const Y = j.current;
    if (!Y || typeof ResizeObserver > "u")
      return window.addEventListener("resize", P), () => window.removeEventListener("resize", P);
    const ue = new ResizeObserver(P);
    return ue.observe(Y), () => ue.disconnect();
  }, []);
  const x = Lr(t), G = x.map((P) => ({
    i: P.i,
    x: P.x,
    y: P.y,
    w: P.w,
    h: P.h,
    // A row header is a fixed-height full-width bar — it may move but never resize. Widget cells
    // clamp resizing to their per-cell minimums (absent ⇒ react-grid-layout's 1×1 default), so a
    // chart can't be dragged down to an unreadable sliver, and carry the grip set PER ITEM: RGL
    // renders a handle span for every axis in a grid-level `resizeHandles` even where the item is
    // non-resizable, so a read-only board or a row would sprout dead grips — set them here where
    // `editable`/`isRow` already gate the rest of the item's behaviour.
    ...ie(P) ? { isResizable: !1, resizeHandles: [] } : {
      resizeHandles: e ? p : [],
      ...P.minW !== void 0 ? { minW: P.minW } : {},
      ...P.minH !== void 0 ? { minH: P.minH } : {}
    }
  })), U = (P) => s($n(t, P));
  if (g > 0 && w < g)
    return /* @__PURE__ */ v("div", { ref: j, className: "lbdg-root", "aria-label": "dashboard grid", children: /* @__PURE__ */ v(Ns, { cells: t, registry: r, range: n, scope: o, refreshKey: i }) });
  const X = !!(_ && e && m);
  return /* @__PURE__ */ v(
    "div",
    {
      ref: j,
      className: "lbdg-root lbdg-canvas",
      "aria-label": "dashboard grid",
      "data-droppable": X ? "true" : void 0,
      children: /* @__PURE__ */ v(
        _s,
        {
          className: "layout",
          layout: G,
          cols: Nn,
          rowHeight: kr,
          width: w,
          isDraggable: e,
          isResizable: e,
          onDragStop: U,
          onResizeStop: U,
          draggableHandle: ".lbdg-drag-handle",
          draggableCancel: ".lbdg-no-drag",
          isDroppable: X,
          droppingItem: D,
          onDrop: (P, Y, ue) => {
            Y && m && m({ x: Y.x, y: Y.y, w: Y.w, h: Y.h }, ue);
          },
          children: x.map(
            (P) => ie(P) ? (
              // A row header: a full-width, flat, full-bleed section bar — NOT a widget frame. The
              // bar owns its own chrome (drag handle + rename + collapse + remove) inline.
              /* @__PURE__ */ v("div", { "data-row": "", className: "lbdg-row-item", "aria-label": `row cell ${P.i}`, children: /* @__PURE__ */ v(
                Ls,
                {
                  cell: P,
                  memberCount: Me(t, P).length,
                  editable: e,
                  onToggleCollapse: u,
                  onRename: c,
                  onRemove: a
                }
              ) }, P.i)
            ) : /* @__PURE__ */ v(
              "div",
              {
                className: P.transparent ? "lbdg-cell lbdg-cell--transparent" : "lbdg-cell lbdg-cell--framed",
                "data-transparent": P.transparent ? "true" : void 0,
                "aria-label": `cell ${P.i}`,
                children: /* @__PURE__ */ v(
                  $s,
                  {
                    cell: P,
                    editable: e,
                    registry: r,
                    range: n,
                    scope: o,
                    refreshKey: i,
                    onRemove: a,
                    onDuplicate: l,
                    onEditPanel: d,
                    onExportCell: f
                  }
                )
              },
              P.i
            )
          )
        }
      )
    }
  );
}
function $s({
  cell: t,
  editable: e,
  registry: r,
  range: n,
  scope: o,
  refreshKey: i,
  onRemove: s,
  onDuplicate: a,
  onEditPanel: l,
  onExportCell: u
}) {
  const c = Wn(t.queryOptions), d = t.links ?? [], f = Ne(t), g = r.resolveCell(t);
  return /* @__PURE__ */ J(Cr, { children: [
    c && /* @__PURE__ */ v("div", { className: "lbdg-badge", "aria-label": `time override for cell ${t.i}`, children: c }),
    d.length > 0 && /* @__PURE__ */ v("div", { className: "lbdg-no-drag lbdg-links", children: d.map((p, _) => /* @__PURE__ */ J(
      "a",
      {
        href: p.url,
        title: p.title || p.url,
        "aria-label": `panel link ${p.title || p.url}`,
        ...p.targetBlank === !1 ? {} : { target: "_blank", rel: "noreferrer" },
        className: "lbdg-link",
        children: [
          /* @__PURE__ */ v(ks, { size: 11 }),
          /* @__PURE__ */ v("span", { className: "lbdg-link-title", children: p.title || p.url })
        ]
      },
      `${p.url}-${_}`
    )) }),
    e && /* @__PURE__ */ v(
      "button",
      {
        type: "button",
        "aria-label": `move cell ${t.i}`,
        title: "Move widget",
        className: "lbdg-drag-handle lbdg-btn lbdg-move",
        children: /* @__PURE__ */ v(Cs, { size: 13 })
      }
    ),
    e && (l || a || u || s) && /* @__PURE__ */ J("div", { className: "lbdg-no-drag lbdg-chrome", children: [
      l && /* @__PURE__ */ v(
        "button",
        {
          type: "button",
          "aria-label": `edit cell ${t.i}`,
          title: "Edit panel",
          className: "lbdg-btn",
          onClick: () => l(t.i),
          children: /* @__PURE__ */ v(Ms, { size: 13 })
        }
      ),
      a && /* @__PURE__ */ v(
        "button",
        {
          type: "button",
          "aria-label": `duplicate cell ${t.i}`,
          title: "Duplicate widget",
          className: "lbdg-btn",
          onClick: () => a(t.i),
          children: /* @__PURE__ */ v(Es, { size: 13 })
        }
      ),
      u && /* @__PURE__ */ v(
        "button",
        {
          type: "button",
          "aria-label": `export cell ${t.i}`,
          title: "Export widget",
          className: "lbdg-btn",
          onClick: () => u(t.i),
          children: /* @__PURE__ */ v(zs, { size: 13 })
        }
      ),
      s && /* @__PURE__ */ v(
        "button",
        {
          type: "button",
          "aria-label": `remove cell ${t.i}`,
          title: "Remove widget",
          className: "lbdg-btn lbdg-btn--danger",
          onClick: () => s(t.i),
          children: /* @__PURE__ */ v(On, { size: 13 })
        }
      )
    ] }),
    /* @__PURE__ */ v("div", { className: "lbdg-cell-body", children: g ? /* @__PURE__ */ v(g, { cell: t, range: n, scope: o, refreshKey: i, editable: e }) : /* @__PURE__ */ v(Nr, { view: f }) })
  ] });
}
export {
  Hs as DEFAULT_RESIZE_HANDLES,
  ea as DashboardGrid,
  Ns as DashboardStack,
  qn as EXT_WILDCARD,
  Ts as FALLBACK_WIDTH,
  Nn as GRID_COLS,
  kr as GRID_ROW_PX,
  Us as ROW_H,
  Xs as ROW_W,
  Ls as RowHeader,
  Nr as UnknownView,
  Gs as bindingSeries,
  Ys as bindingTags,
  _t as canonicalView,
  Bs as cellFieldConfig,
  Rt as cellLabel,
  As as cellPrimaryTarget,
  sr as cellSources,
  Ne as cellView,
  Ks as createRegistry,
  Fs as emptyFieldConfig,
  Mr as isCollapsed,
  ie as isRow,
  $n as mergeLayout,
  Me as rowMembers,
  Hn as rowOptions,
  $t as rows,
  Wn as timeOverrideBadge,
  Vs as ungroupedCells,
  Lr as visibleCells
};
