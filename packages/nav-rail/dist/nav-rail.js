import { jsx as w, Fragment as Oo, jsxs as Y } from "react/jsx-runtime";
import * as d from "react";
import { forwardRef as qn, createElement as Ft, useState as To, useLayoutEffect as No } from "react";
import * as Ut from "react-dom";
function Qn(e) {
  const t = [], n = /* @__PURE__ */ new Map();
  for (const r of e)
    n.has(r.group) || (n.set(r.group, []), t.push(r.group)), n.get(r.group).push(r);
  return t.map((r) => ({ label: r, items: n.get(r) }));
}
function hn(e, t) {
  if (typeof e == "function")
    return e(t);
  e != null && (e.current = t);
}
function Do(...e) {
  return (t) => {
    let n = !1;
    const r = e.map((o) => {
      const i = hn(o, t);
      return !n && typeof i == "function" && (n = !0), i;
    });
    if (n)
      return () => {
        for (let o = 0; o < r.length; o++) {
          const i = r[o];
          typeof i == "function" ? i() : hn(e[o], null);
        }
      };
  };
}
function ee(...e) {
  return d.useCallback(Do(...e), e);
}
// @__NO_SIDE_EFFECTS__
function Yt(e) {
  const t = d.forwardRef((n, r) => {
    let { children: o, ...i } = n, s = null, a = !1;
    const c = [];
    gn(o) && typeof Qe == "function" && (o = Qe(o._payload)), d.Children.forEach(o, (p) => {
      var m;
      if (Fo(p)) {
        a = !0;
        const g = p;
        let h = "child" in g.props ? g.props.child : g.props.children;
        gn(h) && typeof Qe == "function" && (h = Qe(h._payload)), s = Lo(g, h), c.push((m = s == null ? void 0 : s.props) == null ? void 0 : m.children);
      } else
        c.push(p);
    }), s ? s = d.cloneElement(s, void 0, c) : (
      // A `Slottable` was found but it didn't resolve to a single element (e.g.
      // it wrapped multiple elements, text, or a render-prop `child` that
      // wasn't an element). Don't fall back to treating the `Slottable` wrapper
      // itself as the slot target — throw a descriptive error below instead.
      !a && d.Children.count(o) === 1 && d.isValidElement(o) && (s = o)
    );
    const u = s ? _o(s) : void 0, l = ee(r, u);
    if (!s) {
      if (o || o === 0)
        throw new Error(
          a ? Bo(e) : $o(e)
        );
      return o;
    }
    const f = Io(i, s.props ?? {});
    return s.type !== d.Fragment && (f.ref = r ? l : u), d.cloneElement(s, f);
  });
  return t.displayName = `${e}.Slot`, t;
}
var Jn = /* @__PURE__ */ Yt("Slot"), er = Symbol.for("radix.slottable");
// @__NO_SIDE_EFFECTS__
function Mo(e) {
  const t = (n) => "child" in n ? n.children(n.child) : n.children;
  return t.displayName = `${e}.Slottable`, t.__radixId = er, t;
}
var Lo = (e, t) => {
  if ("child" in e.props) {
    const n = e.props.child;
    return d.isValidElement(n) ? d.cloneElement(n, void 0, e.props.children(n.props.children)) : null;
  }
  return d.isValidElement(t) ? t : null;
};
function Io(e, t) {
  const n = { ...t };
  for (const r in t) {
    const o = e[r], i = t[r];
    /^on[A-Z]/.test(r) ? o && i ? n[r] = (...a) => {
      const c = i(...a);
      return o(...a), c;
    } : o && (n[r] = o) : r === "style" ? n[r] = { ...o, ...i } : r === "className" && (n[r] = [o, i].filter(Boolean).join(" "));
  }
  return { ...e, ...n };
}
function _o(e) {
  var r, o;
  let t = (r = Object.getOwnPropertyDescriptor(e.props, "ref")) == null ? void 0 : r.get, n = t && "isReactWarning" in t && t.isReactWarning;
  return n ? e.ref : (t = (o = Object.getOwnPropertyDescriptor(e, "ref")) == null ? void 0 : o.get, n = t && "isReactWarning" in t && t.isReactWarning, n ? e.props.ref : e.props.ref || e.ref);
}
function Fo(e) {
  return d.isValidElement(e) && typeof e.type == "function" && "__radixId" in e.type && e.type.__radixId === er;
}
var zo = Symbol.for("react.lazy");
function gn(e) {
  return e != null && typeof e == "object" && "$$typeof" in e && e.$$typeof === zo && "_payload" in e && Wo(e._payload);
}
function Wo(e) {
  return typeof e == "object" && e !== null && "then" in e;
}
var $o = (e) => `${e} failed to slot onto its children. Expected a single React element child or \`Slottable\`.`, Bo = (e) => `${e} failed to slot onto its \`Slottable\`. Expected \`Slottable\` to receive a single React element child.`, Qe = d[" use ".trim().toString()];
function tr(e) {
  var t, n, r = "";
  if (typeof e == "string" || typeof e == "number") r += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var o = e.length;
    for (t = 0; t < o; t++) e[t] && (n = tr(e[t])) && (r && (r += " "), r += n);
  } else for (n in e) e[n] && (r && (r += " "), r += n);
  return r;
}
function nr() {
  for (var e, t, n = 0, r = "", o = arguments.length; n < o; n++) (e = arguments[n]) && (t = tr(e)) && (r && (r += " "), r += t);
  return r;
}
const bn = (e) => typeof e == "boolean" ? `${e}` : e === 0 ? "0" : e, vn = nr, rr = (e, t) => (n) => {
  var r;
  if ((t == null ? void 0 : t.variants) == null) return vn(e, n == null ? void 0 : n.class, n == null ? void 0 : n.className);
  const { variants: o, defaultVariants: i } = t, s = Object.keys(o).map((u) => {
    const l = n == null ? void 0 : n[u], f = i == null ? void 0 : i[u];
    if (l === null) return null;
    const p = bn(l) || bn(f);
    return o[u][p];
  }), a = n && Object.entries(n).reduce((u, l) => {
    let [f, p] = l;
    return p === void 0 || (u[f] = p), u;
  }, {}), c = t == null || (r = t.compoundVariants) === null || r === void 0 ? void 0 : r.reduce((u, l) => {
    let { class: f, className: p, ...m } = l;
    return Object.entries(m).every((g) => {
      let [h, b] = g;
      return Array.isArray(b) ? b.includes({
        ...i,
        ...a
      }[h]) : {
        ...i,
        ...a
      }[h] === b;
    }) ? [
      ...u,
      f,
      p
    ] : u;
  }, []);
  return vn(e, s, c, n == null ? void 0 : n.class, n == null ? void 0 : n.className);
};
/**
 * @license lucide-react v0.453.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Vo = (e) => e.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase(), or = (...e) => e.filter((t, n, r) => !!t && r.indexOf(t) === n).join(" ");
/**
 * @license lucide-react v0.453.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
var jo = {
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
 * @license lucide-react v0.453.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Ho = qn(
  ({
    color: e = "currentColor",
    size: t = 24,
    strokeWidth: n = 2,
    absoluteStrokeWidth: r,
    className: o = "",
    children: i,
    iconNode: s,
    ...a
  }, c) => Ft(
    "svg",
    {
      ref: c,
      ...jo,
      width: t,
      height: t,
      stroke: e,
      strokeWidth: r ? Number(n) * 24 / Number(t) : n,
      className: or("lucide", o),
      ...a
    },
    [
      ...s.map(([u, l]) => Ft(u, l)),
      ...Array.isArray(i) ? i : [i]
    ]
  )
);
/**
 * @license lucide-react v0.453.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const ir = (e, t) => {
  const n = qn(
    ({ className: r, ...o }, i) => Ft(Ho, {
      ref: i,
      iconNode: t,
      className: or(`lucide-${Vo(e)}`, r),
      ...o
    })
  );
  return n.displayName = `${e}`, n;
};
/**
 * @license lucide-react v0.453.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Go = ir("PanelLeft", [
  ["rect", { width: "18", height: "18", x: "3", y: "3", rx: "2", key: "afitv7" }],
  ["path", { d: "M9 3v18", key: "fh3hqa" }]
]);
/**
 * @license lucide-react v0.453.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Uo = ir("X", [
  ["path", { d: "M18 6 6 18", key: "1bl5f8" }],
  ["path", { d: "m6 6 12 12", key: "d8bk6v" }]
]), kt = 768;
function Yo() {
  const [e, t] = d.useState(void 0);
  return d.useEffect(() => {
    if (!window.matchMedia) {
      t(window.innerWidth < kt);
      return;
    }
    const n = window.matchMedia(`(max-width: ${kt - 1}px)`), r = () => t(window.innerWidth < kt);
    return n.addEventListener("change", r), r(), () => n.removeEventListener("change", r);
  }, []), !!e;
}
const Xo = (e, t) => {
  const n = new Array(e.length + t.length);
  for (let r = 0; r < e.length; r++)
    n[r] = e[r];
  for (let r = 0; r < t.length; r++)
    n[e.length + r] = t[r];
  return n;
}, Ko = (e, t) => ({
  classGroupId: e,
  validator: t
}), sr = (e = /* @__PURE__ */ new Map(), t = null, n) => ({
  nextPart: e,
  validators: t,
  classGroupId: n
}), ft = "-", yn = [], Zo = "arbitrary..", qo = (e) => {
  const t = Jo(e), {
    conflictingClassGroups: n,
    conflictingClassGroupModifiers: r
  } = e;
  return {
    getClassGroupId: (s) => {
      if (s.startsWith("[") && s.endsWith("]"))
        return Qo(s);
      const a = s.split(ft), c = a[0] === "" && a.length > 1 ? 1 : 0;
      return ar(a, c, t);
    },
    getConflictingClassGroupIds: (s, a) => {
      if (a) {
        const c = r[s], u = n[s];
        return c ? u ? Xo(u, c) : c : u || yn;
      }
      return n[s] || yn;
    }
  };
}, ar = (e, t, n) => {
  if (e.length - t === 0)
    return n.classGroupId;
  const o = e[t], i = n.nextPart.get(o);
  if (i) {
    const u = ar(e, t + 1, i);
    if (u) return u;
  }
  const s = n.validators;
  if (s === null)
    return;
  const a = t === 0 ? e.join(ft) : e.slice(t).join(ft), c = s.length;
  for (let u = 0; u < c; u++) {
    const l = s[u];
    if (l.validator(a))
      return l.classGroupId;
  }
}, Qo = (e) => e.slice(1, -1).indexOf(":") === -1 ? void 0 : (() => {
  const t = e.slice(1, -1), n = t.indexOf(":"), r = t.slice(0, n);
  return r ? Zo + r : void 0;
})(), Jo = (e) => {
  const {
    theme: t,
    classGroups: n
  } = e;
  return ei(n, t);
}, ei = (e, t) => {
  const n = sr();
  for (const r in e) {
    const o = e[r];
    Xt(o, n, r, t);
  }
  return n;
}, Xt = (e, t, n, r) => {
  const o = e.length;
  for (let i = 0; i < o; i++) {
    const s = e[i];
    ti(s, t, n, r);
  }
}, ti = (e, t, n, r) => {
  if (typeof e == "string") {
    ni(e, t, n);
    return;
  }
  if (typeof e == "function") {
    ri(e, t, n, r);
    return;
  }
  oi(e, t, n, r);
}, ni = (e, t, n) => {
  const r = e === "" ? t : cr(t, e);
  r.classGroupId = n;
}, ri = (e, t, n, r) => {
  if (ii(e)) {
    Xt(e(r), t, n, r);
    return;
  }
  t.validators === null && (t.validators = []), t.validators.push(Ko(n, e));
}, oi = (e, t, n, r) => {
  const o = Object.entries(e), i = o.length;
  for (let s = 0; s < i; s++) {
    const [a, c] = o[s];
    Xt(c, cr(t, a), n, r);
  }
}, cr = (e, t) => {
  let n = e;
  const r = t.split(ft), o = r.length;
  for (let i = 0; i < o; i++) {
    const s = r[i];
    let a = n.nextPart.get(s);
    a || (a = sr(), n.nextPart.set(s, a)), n = a;
  }
  return n;
}, ii = (e) => "isThemeGetter" in e && e.isThemeGetter === !0, si = (e) => {
  if (e < 1)
    return {
      get: () => {
      },
      set: () => {
      }
    };
  let t = 0, n = /* @__PURE__ */ Object.create(null), r = /* @__PURE__ */ Object.create(null);
  const o = (i, s) => {
    n[i] = s, t++, t > e && (t = 0, r = n, n = /* @__PURE__ */ Object.create(null));
  };
  return {
    get(i) {
      let s = n[i];
      if (s !== void 0)
        return s;
      if ((s = r[i]) !== void 0)
        return o(i, s), s;
    },
    set(i, s) {
      i in n ? n[i] = s : o(i, s);
    }
  };
}, zt = "!", wn = ":", ai = [], xn = (e, t, n, r, o) => ({
  modifiers: e,
  hasImportantModifier: t,
  baseClassName: n,
  maybePostfixModifierPosition: r,
  isExternal: o
}), ci = (e) => {
  const {
    prefix: t,
    experimentalParseClassName: n
  } = e;
  let r = (o) => {
    const i = [];
    let s = 0, a = 0, c = 0, u;
    const l = o.length;
    for (let h = 0; h < l; h++) {
      const b = o[h];
      if (s === 0 && a === 0) {
        if (b === wn) {
          i.push(o.slice(c, h)), c = h + 1;
          continue;
        }
        if (b === "/") {
          u = h;
          continue;
        }
      }
      b === "[" ? s++ : b === "]" ? s-- : b === "(" ? a++ : b === ")" && a--;
    }
    const f = i.length === 0 ? o : o.slice(c);
    let p = f, m = !1;
    f.endsWith(zt) ? (p = f.slice(0, -1), m = !0) : (
      /**
       * In Tailwind CSS v3 the important modifier was at the start of the base class name. This is still supported for legacy reasons.
       * @see https://github.com/dcastil/tailwind-merge/issues/513#issuecomment-2614029864
       */
      f.startsWith(zt) && (p = f.slice(1), m = !0)
    );
    const g = u && u > c ? u - c : void 0;
    return xn(i, m, p, g);
  };
  if (t) {
    const o = t + wn, i = r;
    r = (s) => s.startsWith(o) ? i(s.slice(o.length)) : xn(ai, !1, s, void 0, !0);
  }
  if (n) {
    const o = r;
    r = (i) => n({
      className: i,
      parseClassName: o
    });
  }
  return r;
}, li = (e) => {
  const t = /* @__PURE__ */ new Map();
  return e.orderSensitiveModifiers.forEach((n, r) => {
    t.set(n, 1e6 + r);
  }), (n) => {
    const r = [];
    let o = [];
    for (let i = 0; i < n.length; i++) {
      const s = n[i], a = s[0] === "[", c = t.has(s);
      a || c ? (o.length > 0 && (o.sort(), r.push(...o), o = []), r.push(s)) : o.push(s);
    }
    return o.length > 0 && (o.sort(), r.push(...o)), r;
  };
}, ui = (e) => ({
  cache: si(e.cacheSize),
  parseClassName: ci(e),
  sortModifiers: li(e),
  postfixLookupClassGroupIds: di(e),
  ...qo(e)
}), di = (e) => {
  const t = /* @__PURE__ */ Object.create(null), n = e.postfixLookupClassGroups;
  if (n)
    for (let r = 0; r < n.length; r++)
      t[n[r]] = !0;
  return t;
}, fi = /\s+/, mi = (e, t) => {
  const {
    parseClassName: n,
    getClassGroupId: r,
    getConflictingClassGroupIds: o,
    sortModifiers: i,
    postfixLookupClassGroupIds: s
  } = t, a = [], c = e.trim().split(fi);
  let u = "";
  for (let l = c.length - 1; l >= 0; l -= 1) {
    const f = c[l], {
      isExternal: p,
      modifiers: m,
      hasImportantModifier: g,
      baseClassName: h,
      maybePostfixModifierPosition: b
    } = n(f);
    if (p) {
      u = f + (u.length > 0 ? " " + u : u);
      continue;
    }
    let y = !!b, v;
    if (y) {
      const O = h.substring(0, b);
      v = r(O);
      const E = v && s[v] ? r(h) : void 0;
      E && E !== v && (v = E, y = !1);
    } else
      v = r(h);
    if (!v) {
      if (!y) {
        u = f + (u.length > 0 ? " " + u : u);
        continue;
      }
      if (v = r(h), !v) {
        u = f + (u.length > 0 ? " " + u : u);
        continue;
      }
      y = !1;
    }
    const x = m.length === 0 ? "" : m.length === 1 ? m[0] : i(m).join(":"), C = g ? x + zt : x, S = C + v;
    if (a.indexOf(S) > -1)
      continue;
    a.push(S);
    const P = o(v, y);
    for (let O = 0; O < P.length; ++O) {
      const E = P[O];
      a.push(C + E);
    }
    u = f + (u.length > 0 ? " " + u : u);
  }
  return u;
}, pi = (...e) => {
  let t = 0, n, r, o = "";
  for (; t < e.length; )
    (n = e[t++]) && (r = lr(n)) && (o && (o += " "), o += r);
  return o;
}, lr = (e) => {
  if (typeof e == "string")
    return e;
  let t, n = "";
  for (let r = 0; r < e.length; r++)
    e[r] && (t = lr(e[r])) && (n && (n += " "), n += t);
  return n;
}, hi = (e, ...t) => {
  let n, r, o, i;
  const s = (c) => {
    const u = t.reduce((l, f) => f(l), e());
    return n = ui(u), r = n.cache.get, o = n.cache.set, i = a, a(c);
  }, a = (c) => {
    const u = r(c);
    if (u)
      return u;
    const l = mi(c, n);
    return o(c, l), l;
  };
  return i = s, (...c) => i(pi(...c));
}, gi = [], H = (e) => {
  const t = (n) => n[e] || gi;
  return t.isThemeGetter = !0, t;
}, ur = /^\[(?:(\w[\w-]*):)?(.+)\]$/i, dr = /^\((?:(\w[\w-]*):)?(.+)\)$/i, bi = /^\d+(?:\.\d+)?\/\d+(?:\.\d+)?$/, vi = /^(\d+(\.\d+)?)?(xs|sm|md|lg|xl)$/, yi = /\d+(%|px|r?em|[sdl]?v([hwib]|min|max)|pt|pc|in|cm|mm|cap|ch|ex|r?lh|cq(w|h|i|b|min|max))|\b(calc|min|max|clamp)\(.+\)|^0$/, wi = /^(rgba?|hsla?|hwb|(ok)?(lab|lch)|color-mix)\(.+\)$/, xi = /^(inset_)?-?((\d+)?\.?(\d+)[a-z]+|0)_-?((\d+)?\.?(\d+)[a-z]+|0)/, Ci = /^(url|image|image-set|cross-fade|element|(repeating-)?(linear|radial|conic)-gradient)\(.+\)$/, he = (e) => bi.test(e), N = (e) => !!e && !Number.isNaN(Number(e)), ne = (e) => !!e && Number.isInteger(Number(e)), At = (e) => e.endsWith("%") && N(e.slice(0, -1)), le = (e) => vi.test(e), fr = () => !0, Ei = (e) => (
  // `colorFunctionRegex` check is necessary because color functions can have percentages in them which which would be incorrectly classified as lengths.
  // For example, `hsl(0 0% 0%)` would be classified as a length without this check.
  // I could also use lookbehind assertion in `lengthUnitRegex` but that isn't supported widely enough.
  yi.test(e) && !wi.test(e)
), Kt = () => !1, Si = (e) => xi.test(e), Ri = (e) => Ci.test(e), ki = (e) => !R(e) && !k(e), Ai = (e) => e.startsWith("@container") && (e[10] === "/" && e[11] !== void 0 || e[11] === "s" && e[16] !== void 0 && e.startsWith("-size/", 10) || e[11] === "n" && e[18] !== void 0 && e.startsWith("-normal/", 10)), Pi = (e) => ye(e, hr, Kt), R = (e) => ur.test(e), Ee = (e) => ye(e, gr, Ei), Cn = (e) => ye(e, _i, N), Oi = (e) => ye(e, vr, fr), Ti = (e) => ye(e, br, Kt), En = (e) => ye(e, mr, Kt), Ni = (e) => ye(e, pr, Ri), Je = (e) => ye(e, yr, Si), k = (e) => dr.test(e), He = (e) => ke(e, gr), Di = (e) => ke(e, br), Sn = (e) => ke(e, mr), Mi = (e) => ke(e, hr), Li = (e) => ke(e, pr), et = (e) => ke(e, yr, !0), Ii = (e) => ke(e, vr, !0), ye = (e, t, n) => {
  const r = ur.exec(e);
  return r ? r[1] ? t(r[1]) : n(r[2]) : !1;
}, ke = (e, t, n = !1) => {
  const r = dr.exec(e);
  return r ? r[1] ? t(r[1]) : n : !1;
}, mr = (e) => e === "position" || e === "percentage", pr = (e) => e === "image" || e === "url", hr = (e) => e === "length" || e === "size" || e === "bg-size", gr = (e) => e === "length", _i = (e) => e === "number", br = (e) => e === "family-name", vr = (e) => e === "number" || e === "weight", yr = (e) => e === "shadow", Fi = () => {
  const e = H("color"), t = H("font"), n = H("text"), r = H("font-weight"), o = H("tracking"), i = H("leading"), s = H("breakpoint"), a = H("container"), c = H("spacing"), u = H("radius"), l = H("shadow"), f = H("inset-shadow"), p = H("text-shadow"), m = H("drop-shadow"), g = H("blur"), h = H("perspective"), b = H("aspect"), y = H("ease"), v = H("animate"), x = () => ["auto", "avoid", "all", "avoid-page", "page", "left", "right", "column"], C = () => [
    "center",
    "top",
    "bottom",
    "left",
    "right",
    "top-left",
    // Deprecated since Tailwind CSS v4.1.0, see https://github.com/tailwindlabs/tailwindcss/pull/17378
    "left-top",
    "top-right",
    // Deprecated since Tailwind CSS v4.1.0, see https://github.com/tailwindlabs/tailwindcss/pull/17378
    "right-top",
    "bottom-right",
    // Deprecated since Tailwind CSS v4.1.0, see https://github.com/tailwindlabs/tailwindcss/pull/17378
    "right-bottom",
    "bottom-left",
    // Deprecated since Tailwind CSS v4.1.0, see https://github.com/tailwindlabs/tailwindcss/pull/17378
    "left-bottom"
  ], S = () => [...C(), k, R], P = () => ["auto", "hidden", "clip", "visible", "scroll"], O = () => ["auto", "contain", "none"], E = () => [k, R, c], M = () => [he, "full", "auto", ...E()], z = () => [ne, "none", "subgrid", k, R], T = () => ["auto", {
    span: ["full", ne, k, R]
  }, ne, k, R], I = () => [ne, "auto", k, R], W = () => ["auto", "min", "max", "fr", k, R], _ = () => ["start", "end", "center", "between", "around", "evenly", "stretch", "baseline", "center-safe", "end-safe"], j = () => ["start", "end", "center", "stretch", "center-safe", "end-safe"], L = () => ["auto", ...E()], F = () => [he, "auto", "full", "dvw", "dvh", "lvw", "lvh", "svw", "svh", "min", "max", "fit", ...E()], D = () => [he, "screen", "full", "dvw", "lvw", "svw", "min", "max", "fit", ...E()], B = () => [he, "screen", "full", "lh", "dvh", "lvh", "svh", "min", "max", "fit", ...E()], A = () => [e, k, R], $e = () => [...C(), Sn, En, {
    position: [k, R]
  }], we = () => ["no-repeat", {
    repeat: ["", "x", "y", "space", "round"]
  }], Ze = () => ["auto", "cover", "contain", Mi, Pi, {
    size: [k, R]
  }], Be = () => [At, He, Ee], G = () => [
    // Deprecated since Tailwind CSS v4.0.0
    "",
    "none",
    "full",
    u,
    k,
    R
  ], U = () => ["", N, He, Ee], Ae = () => ["solid", "dashed", "dotted", "double"], Ve = () => ["normal", "multiply", "screen", "overlay", "darken", "lighten", "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation", "color", "luminosity"], V = () => [N, At, Sn, En], je = () => [
    // Deprecated since Tailwind CSS v4.0.0
    "",
    "none",
    g,
    k,
    R
  ], xe = () => ["none", N, k, R], Ce = () => ["none", N, k, R], Pe = () => [N, k, R], pe = () => [he, "full", ...E()];
  return {
    cacheSize: 500,
    theme: {
      animate: ["spin", "ping", "pulse", "bounce"],
      aspect: ["video"],
      blur: [le],
      breakpoint: [le],
      color: [fr],
      container: [le],
      "drop-shadow": [le],
      ease: ["in", "out", "in-out"],
      font: [ki],
      "font-weight": ["thin", "extralight", "light", "normal", "medium", "semibold", "bold", "extrabold", "black"],
      "inset-shadow": [le],
      leading: ["none", "tight", "snug", "normal", "relaxed", "loose"],
      perspective: ["dramatic", "near", "normal", "midrange", "distant", "none"],
      radius: [le],
      shadow: [le],
      spacing: ["px", N],
      text: [le],
      "text-shadow": [le],
      tracking: ["tighter", "tight", "normal", "wide", "wider", "widest"]
    },
    classGroups: {
      // --------------
      // --- Layout ---
      // --------------
      /**
       * Aspect Ratio
       * @see https://tailwindcss.com/docs/aspect-ratio
       */
      aspect: [{
        aspect: ["auto", "square", he, R, k, b]
      }],
      /**
       * Container
       * @see https://tailwindcss.com/docs/container
       * @deprecated since Tailwind CSS v4.0.0
       */
      container: ["container"],
      /**
       * Container Type
       * @see https://tailwindcss.com/docs/responsive-design#container-queries
       */
      "container-type": [{
        "@container": ["", "normal", "size", k, R]
      }],
      /**
       * Container Name
       * @see https://tailwindcss.com/docs/responsive-design#named-containers
       */
      "container-named": [Ai],
      /**
       * Columns
       * @see https://tailwindcss.com/docs/columns
       */
      columns: [{
        columns: [N, R, k, a]
      }],
      /**
       * Break After
       * @see https://tailwindcss.com/docs/break-after
       */
      "break-after": [{
        "break-after": x()
      }],
      /**
       * Break Before
       * @see https://tailwindcss.com/docs/break-before
       */
      "break-before": [{
        "break-before": x()
      }],
      /**
       * Break Inside
       * @see https://tailwindcss.com/docs/break-inside
       */
      "break-inside": [{
        "break-inside": ["auto", "avoid", "avoid-page", "avoid-column"]
      }],
      /**
       * Box Decoration Break
       * @see https://tailwindcss.com/docs/box-decoration-break
       */
      "box-decoration": [{
        "box-decoration": ["slice", "clone"]
      }],
      /**
       * Box Sizing
       * @see https://tailwindcss.com/docs/box-sizing
       */
      box: [{
        box: ["border", "content"]
      }],
      /**
       * Display
       * @see https://tailwindcss.com/docs/display
       */
      display: ["block", "inline-block", "inline", "flex", "inline-flex", "table", "inline-table", "table-caption", "table-cell", "table-column", "table-column-group", "table-footer-group", "table-header-group", "table-row-group", "table-row", "flow-root", "grid", "inline-grid", "contents", "list-item", "hidden"],
      /**
       * Screen Reader Only
       * @see https://tailwindcss.com/docs/display#screen-reader-only
       */
      sr: ["sr-only", "not-sr-only"],
      /**
       * Floats
       * @see https://tailwindcss.com/docs/float
       */
      float: [{
        float: ["right", "left", "none", "start", "end"]
      }],
      /**
       * Clear
       * @see https://tailwindcss.com/docs/clear
       */
      clear: [{
        clear: ["left", "right", "both", "none", "start", "end"]
      }],
      /**
       * Isolation
       * @see https://tailwindcss.com/docs/isolation
       */
      isolation: ["isolate", "isolation-auto"],
      /**
       * Object Fit
       * @see https://tailwindcss.com/docs/object-fit
       */
      "object-fit": [{
        object: ["contain", "cover", "fill", "none", "scale-down"]
      }],
      /**
       * Object Position
       * @see https://tailwindcss.com/docs/object-position
       */
      "object-position": [{
        object: S()
      }],
      /**
       * Overflow
       * @see https://tailwindcss.com/docs/overflow
       */
      overflow: [{
        overflow: P()
      }],
      /**
       * Overflow X
       * @see https://tailwindcss.com/docs/overflow
       */
      "overflow-x": [{
        "overflow-x": P()
      }],
      /**
       * Overflow Y
       * @see https://tailwindcss.com/docs/overflow
       */
      "overflow-y": [{
        "overflow-y": P()
      }],
      /**
       * Overscroll Behavior
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      overscroll: [{
        overscroll: O()
      }],
      /**
       * Overscroll Behavior X
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      "overscroll-x": [{
        "overscroll-x": O()
      }],
      /**
       * Overscroll Behavior Y
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      "overscroll-y": [{
        "overscroll-y": O()
      }],
      /**
       * Position
       * @see https://tailwindcss.com/docs/position
       */
      position: ["static", "fixed", "absolute", "relative", "sticky"],
      /**
       * Inset
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      inset: [{
        inset: M()
      }],
      /**
       * Inset Inline
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      "inset-x": [{
        "inset-x": M()
      }],
      /**
       * Inset Block
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      "inset-y": [{
        "inset-y": M()
      }],
      /**
       * Inset Inline Start
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       * @todo class group will be renamed to `inset-s` in next major release
       */
      start: [{
        "inset-s": M(),
        /**
         * @deprecated since Tailwind CSS v4.2.0 in favor of `inset-s-*` utilities.
         * @see https://github.com/tailwindlabs/tailwindcss/pull/19613
         */
        start: M()
      }],
      /**
       * Inset Inline End
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       * @todo class group will be renamed to `inset-e` in next major release
       */
      end: [{
        "inset-e": M(),
        /**
         * @deprecated since Tailwind CSS v4.2.0 in favor of `inset-e-*` utilities.
         * @see https://github.com/tailwindlabs/tailwindcss/pull/19613
         */
        end: M()
      }],
      /**
       * Inset Block Start
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      "inset-bs": [{
        "inset-bs": M()
      }],
      /**
       * Inset Block End
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      "inset-be": [{
        "inset-be": M()
      }],
      /**
       * Top
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      top: [{
        top: M()
      }],
      /**
       * Right
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      right: [{
        right: M()
      }],
      /**
       * Bottom
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      bottom: [{
        bottom: M()
      }],
      /**
       * Left
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      left: [{
        left: M()
      }],
      /**
       * Visibility
       * @see https://tailwindcss.com/docs/visibility
       */
      visibility: ["visible", "invisible", "collapse"],
      /**
       * Z-Index
       * @see https://tailwindcss.com/docs/z-index
       */
      z: [{
        z: [ne, "auto", k, R]
      }],
      // ------------------------
      // --- Flexbox and Grid ---
      // ------------------------
      /**
       * Flex Basis
       * @see https://tailwindcss.com/docs/flex-basis
       */
      basis: [{
        basis: [he, "full", "auto", a, ...E()]
      }],
      /**
       * Flex Direction
       * @see https://tailwindcss.com/docs/flex-direction
       */
      "flex-direction": [{
        flex: ["row", "row-reverse", "col", "col-reverse"]
      }],
      /**
       * Flex Wrap
       * @see https://tailwindcss.com/docs/flex-wrap
       */
      "flex-wrap": [{
        flex: ["nowrap", "wrap", "wrap-reverse"]
      }],
      /**
       * Flex
       * @see https://tailwindcss.com/docs/flex
       */
      flex: [{
        flex: [N, he, "auto", "initial", "none", R]
      }],
      /**
       * Flex Grow
       * @see https://tailwindcss.com/docs/flex-grow
       */
      grow: [{
        grow: ["", N, k, R]
      }],
      /**
       * Flex Shrink
       * @see https://tailwindcss.com/docs/flex-shrink
       */
      shrink: [{
        shrink: ["", N, k, R]
      }],
      /**
       * Order
       * @see https://tailwindcss.com/docs/order
       */
      order: [{
        order: [ne, "first", "last", "none", k, R]
      }],
      /**
       * Grid Template Columns
       * @see https://tailwindcss.com/docs/grid-template-columns
       */
      "grid-cols": [{
        "grid-cols": z()
      }],
      /**
       * Grid Column Start / End
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-start-end": [{
        col: T()
      }],
      /**
       * Grid Column Start
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-start": [{
        "col-start": I()
      }],
      /**
       * Grid Column End
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-end": [{
        "col-end": I()
      }],
      /**
       * Grid Template Rows
       * @see https://tailwindcss.com/docs/grid-template-rows
       */
      "grid-rows": [{
        "grid-rows": z()
      }],
      /**
       * Grid Row Start / End
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-start-end": [{
        row: T()
      }],
      /**
       * Grid Row Start
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-start": [{
        "row-start": I()
      }],
      /**
       * Grid Row End
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-end": [{
        "row-end": I()
      }],
      /**
       * Grid Auto Flow
       * @see https://tailwindcss.com/docs/grid-auto-flow
       */
      "grid-flow": [{
        "grid-flow": ["row", "col", "dense", "row-dense", "col-dense"]
      }],
      /**
       * Grid Auto Columns
       * @see https://tailwindcss.com/docs/grid-auto-columns
       */
      "auto-cols": [{
        "auto-cols": W()
      }],
      /**
       * Grid Auto Rows
       * @see https://tailwindcss.com/docs/grid-auto-rows
       */
      "auto-rows": [{
        "auto-rows": W()
      }],
      /**
       * Gap
       * @see https://tailwindcss.com/docs/gap
       */
      gap: [{
        gap: E()
      }],
      /**
       * Gap X
       * @see https://tailwindcss.com/docs/gap
       */
      "gap-x": [{
        "gap-x": E()
      }],
      /**
       * Gap Y
       * @see https://tailwindcss.com/docs/gap
       */
      "gap-y": [{
        "gap-y": E()
      }],
      /**
       * Justify Content
       * @see https://tailwindcss.com/docs/justify-content
       */
      "justify-content": [{
        justify: [..._(), "normal"]
      }],
      /**
       * Justify Items
       * @see https://tailwindcss.com/docs/justify-items
       */
      "justify-items": [{
        "justify-items": [...j(), "normal"]
      }],
      /**
       * Justify Self
       * @see https://tailwindcss.com/docs/justify-self
       */
      "justify-self": [{
        "justify-self": ["auto", ...j()]
      }],
      /**
       * Align Content
       * @see https://tailwindcss.com/docs/align-content
       */
      "align-content": [{
        content: ["normal", ..._()]
      }],
      /**
       * Align Items
       * @see https://tailwindcss.com/docs/align-items
       */
      "align-items": [{
        items: [...j(), {
          baseline: ["", "last"]
        }]
      }],
      /**
       * Align Self
       * @see https://tailwindcss.com/docs/align-self
       */
      "align-self": [{
        self: ["auto", ...j(), {
          baseline: ["", "last"]
        }]
      }],
      /**
       * Place Content
       * @see https://tailwindcss.com/docs/place-content
       */
      "place-content": [{
        "place-content": _()
      }],
      /**
       * Place Items
       * @see https://tailwindcss.com/docs/place-items
       */
      "place-items": [{
        "place-items": [...j(), "baseline"]
      }],
      /**
       * Place Self
       * @see https://tailwindcss.com/docs/place-self
       */
      "place-self": [{
        "place-self": ["auto", ...j()]
      }],
      // Spacing
      /**
       * Padding
       * @see https://tailwindcss.com/docs/padding
       */
      p: [{
        p: E()
      }],
      /**
       * Padding Inline
       * @see https://tailwindcss.com/docs/padding
       */
      px: [{
        px: E()
      }],
      /**
       * Padding Block
       * @see https://tailwindcss.com/docs/padding
       */
      py: [{
        py: E()
      }],
      /**
       * Padding Inline Start
       * @see https://tailwindcss.com/docs/padding
       */
      ps: [{
        ps: E()
      }],
      /**
       * Padding Inline End
       * @see https://tailwindcss.com/docs/padding
       */
      pe: [{
        pe: E()
      }],
      /**
       * Padding Block Start
       * @see https://tailwindcss.com/docs/padding
       */
      pbs: [{
        pbs: E()
      }],
      /**
       * Padding Block End
       * @see https://tailwindcss.com/docs/padding
       */
      pbe: [{
        pbe: E()
      }],
      /**
       * Padding Top
       * @see https://tailwindcss.com/docs/padding
       */
      pt: [{
        pt: E()
      }],
      /**
       * Padding Right
       * @see https://tailwindcss.com/docs/padding
       */
      pr: [{
        pr: E()
      }],
      /**
       * Padding Bottom
       * @see https://tailwindcss.com/docs/padding
       */
      pb: [{
        pb: E()
      }],
      /**
       * Padding Left
       * @see https://tailwindcss.com/docs/padding
       */
      pl: [{
        pl: E()
      }],
      /**
       * Margin
       * @see https://tailwindcss.com/docs/margin
       */
      m: [{
        m: L()
      }],
      /**
       * Margin Inline
       * @see https://tailwindcss.com/docs/margin
       */
      mx: [{
        mx: L()
      }],
      /**
       * Margin Block
       * @see https://tailwindcss.com/docs/margin
       */
      my: [{
        my: L()
      }],
      /**
       * Margin Inline Start
       * @see https://tailwindcss.com/docs/margin
       */
      ms: [{
        ms: L()
      }],
      /**
       * Margin Inline End
       * @see https://tailwindcss.com/docs/margin
       */
      me: [{
        me: L()
      }],
      /**
       * Margin Block Start
       * @see https://tailwindcss.com/docs/margin
       */
      mbs: [{
        mbs: L()
      }],
      /**
       * Margin Block End
       * @see https://tailwindcss.com/docs/margin
       */
      mbe: [{
        mbe: L()
      }],
      /**
       * Margin Top
       * @see https://tailwindcss.com/docs/margin
       */
      mt: [{
        mt: L()
      }],
      /**
       * Margin Right
       * @see https://tailwindcss.com/docs/margin
       */
      mr: [{
        mr: L()
      }],
      /**
       * Margin Bottom
       * @see https://tailwindcss.com/docs/margin
       */
      mb: [{
        mb: L()
      }],
      /**
       * Margin Left
       * @see https://tailwindcss.com/docs/margin
       */
      ml: [{
        ml: L()
      }],
      /**
       * Space Between X
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-x": [{
        "space-x": E()
      }],
      /**
       * Space Between X Reverse
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-x-reverse": ["space-x-reverse"],
      /**
       * Space Between Y
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-y": [{
        "space-y": E()
      }],
      /**
       * Space Between Y Reverse
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-y-reverse": ["space-y-reverse"],
      // --------------
      // --- Sizing ---
      // --------------
      /**
       * Size
       * @see https://tailwindcss.com/docs/width#setting-both-width-and-height
       */
      size: [{
        size: F()
      }],
      /**
       * Inline Size
       * @see https://tailwindcss.com/docs/width
       */
      "inline-size": [{
        inline: ["auto", ...D()]
      }],
      /**
       * Min-Inline Size
       * @see https://tailwindcss.com/docs/min-width
       */
      "min-inline-size": [{
        "min-inline": ["auto", ...D()]
      }],
      /**
       * Max-Inline Size
       * @see https://tailwindcss.com/docs/max-width
       */
      "max-inline-size": [{
        "max-inline": ["none", ...D()]
      }],
      /**
       * Block Size
       * @see https://tailwindcss.com/docs/height
       */
      "block-size": [{
        block: ["auto", ...B()]
      }],
      /**
       * Min-Block Size
       * @see https://tailwindcss.com/docs/min-height
       */
      "min-block-size": [{
        "min-block": ["auto", ...B()]
      }],
      /**
       * Max-Block Size
       * @see https://tailwindcss.com/docs/max-height
       */
      "max-block-size": [{
        "max-block": ["none", ...B()]
      }],
      /**
       * Width
       * @see https://tailwindcss.com/docs/width
       */
      w: [{
        w: [a, "screen", ...F()]
      }],
      /**
       * Min-Width
       * @see https://tailwindcss.com/docs/min-width
       */
      "min-w": [{
        "min-w": [
          a,
          "screen",
          /** Deprecated. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          "none",
          ...F()
        ]
      }],
      /**
       * Max-Width
       * @see https://tailwindcss.com/docs/max-width
       */
      "max-w": [{
        "max-w": [
          a,
          "screen",
          "none",
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          "prose",
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          {
            screen: [s]
          },
          ...F()
        ]
      }],
      /**
       * Height
       * @see https://tailwindcss.com/docs/height
       */
      h: [{
        h: ["screen", "lh", ...F()]
      }],
      /**
       * Min-Height
       * @see https://tailwindcss.com/docs/min-height
       */
      "min-h": [{
        "min-h": ["screen", "lh", "none", ...F()]
      }],
      /**
       * Max-Height
       * @see https://tailwindcss.com/docs/max-height
       */
      "max-h": [{
        "max-h": ["screen", "lh", ...F()]
      }],
      // ------------------
      // --- Typography ---
      // ------------------
      /**
       * Font Size
       * @see https://tailwindcss.com/docs/font-size
       */
      "font-size": [{
        text: ["base", n, He, Ee]
      }],
      /**
       * Font Smoothing
       * @see https://tailwindcss.com/docs/font-smoothing
       */
      "font-smoothing": ["antialiased", "subpixel-antialiased"],
      /**
       * Font Style
       * @see https://tailwindcss.com/docs/font-style
       */
      "font-style": ["italic", "not-italic"],
      /**
       * Font Weight
       * @see https://tailwindcss.com/docs/font-weight
       */
      "font-weight": [{
        font: [r, Ii, Oi]
      }],
      /**
       * Font Stretch
       * @see https://tailwindcss.com/docs/font-stretch
       */
      "font-stretch": [{
        "font-stretch": ["ultra-condensed", "extra-condensed", "condensed", "semi-condensed", "normal", "semi-expanded", "expanded", "extra-expanded", "ultra-expanded", At, R]
      }],
      /**
       * Font Family
       * @see https://tailwindcss.com/docs/font-family
       */
      "font-family": [{
        font: [Di, Ti, t]
      }],
      /**
       * Font Feature Settings
       * @see https://tailwindcss.com/docs/font-feature-settings
       */
      "font-features": [{
        "font-features": [R]
      }],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-normal": ["normal-nums"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-ordinal": ["ordinal"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-slashed-zero": ["slashed-zero"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-figure": ["lining-nums", "oldstyle-nums"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-spacing": ["proportional-nums", "tabular-nums"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-fraction": ["diagonal-fractions", "stacked-fractions"],
      /**
       * Letter Spacing
       * @see https://tailwindcss.com/docs/letter-spacing
       */
      tracking: [{
        tracking: [o, k, R]
      }],
      /**
       * Line Clamp
       * @see https://tailwindcss.com/docs/line-clamp
       */
      "line-clamp": [{
        "line-clamp": [N, "none", k, Cn]
      }],
      /**
       * Line Height
       * @see https://tailwindcss.com/docs/line-height
       */
      leading: [{
        leading: [
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          i,
          ...E()
        ]
      }],
      /**
       * List Style Image
       * @see https://tailwindcss.com/docs/list-style-image
       */
      "list-image": [{
        "list-image": ["none", k, R]
      }],
      /**
       * List Style Position
       * @see https://tailwindcss.com/docs/list-style-position
       */
      "list-style-position": [{
        list: ["inside", "outside"]
      }],
      /**
       * List Style Type
       * @see https://tailwindcss.com/docs/list-style-type
       */
      "list-style-type": [{
        list: ["disc", "decimal", "none", k, R]
      }],
      /**
       * Text Alignment
       * @see https://tailwindcss.com/docs/text-align
       */
      "text-alignment": [{
        text: ["left", "center", "right", "justify", "start", "end"]
      }],
      /**
       * Placeholder Color
       * @deprecated since Tailwind CSS v3.0.0
       * @see https://v3.tailwindcss.com/docs/placeholder-color
       */
      "placeholder-color": [{
        placeholder: A()
      }],
      /**
       * Text Color
       * @see https://tailwindcss.com/docs/text-color
       */
      "text-color": [{
        text: A()
      }],
      /**
       * Text Decoration
       * @see https://tailwindcss.com/docs/text-decoration
       */
      "text-decoration": ["underline", "overline", "line-through", "no-underline"],
      /**
       * Text Decoration Style
       * @see https://tailwindcss.com/docs/text-decoration-style
       */
      "text-decoration-style": [{
        decoration: [...Ae(), "wavy"]
      }],
      /**
       * Text Decoration Thickness
       * @see https://tailwindcss.com/docs/text-decoration-thickness
       */
      "text-decoration-thickness": [{
        decoration: [N, "from-font", "auto", k, Ee]
      }],
      /**
       * Text Decoration Color
       * @see https://tailwindcss.com/docs/text-decoration-color
       */
      "text-decoration-color": [{
        decoration: A()
      }],
      /**
       * Text Underline Offset
       * @see https://tailwindcss.com/docs/text-underline-offset
       */
      "underline-offset": [{
        "underline-offset": [N, "auto", k, R]
      }],
      /**
       * Text Transform
       * @see https://tailwindcss.com/docs/text-transform
       */
      "text-transform": ["uppercase", "lowercase", "capitalize", "normal-case"],
      /**
       * Text Overflow
       * @see https://tailwindcss.com/docs/text-overflow
       */
      "text-overflow": ["truncate", "text-ellipsis", "text-clip"],
      /**
       * Text Wrap
       * @see https://tailwindcss.com/docs/text-wrap
       */
      "text-wrap": [{
        text: ["wrap", "nowrap", "balance", "pretty"]
      }],
      /**
       * Text Indent
       * @see https://tailwindcss.com/docs/text-indent
       */
      indent: [{
        indent: E()
      }],
      /**
       * Tab Size
       * @see https://tailwindcss.com/docs/tab-size
       */
      "tab-size": [{
        tab: [ne, k, R]
      }],
      /**
       * Vertical Alignment
       * @see https://tailwindcss.com/docs/vertical-align
       */
      "vertical-align": [{
        align: ["baseline", "top", "middle", "bottom", "text-top", "text-bottom", "sub", "super", k, R]
      }],
      /**
       * Whitespace
       * @see https://tailwindcss.com/docs/whitespace
       */
      whitespace: [{
        whitespace: ["normal", "nowrap", "pre", "pre-line", "pre-wrap", "break-spaces"]
      }],
      /**
       * Word Break
       * @see https://tailwindcss.com/docs/word-break
       */
      break: [{
        break: ["normal", "words", "all", "keep"]
      }],
      /**
       * Overflow Wrap
       * @see https://tailwindcss.com/docs/overflow-wrap
       */
      wrap: [{
        wrap: ["break-word", "anywhere", "normal"]
      }],
      /**
       * Hyphens
       * @see https://tailwindcss.com/docs/hyphens
       */
      hyphens: [{
        hyphens: ["none", "manual", "auto"]
      }],
      /**
       * Content
       * @see https://tailwindcss.com/docs/content
       */
      content: [{
        content: ["none", k, R]
      }],
      // -------------------
      // --- Backgrounds ---
      // -------------------
      /**
       * Background Attachment
       * @see https://tailwindcss.com/docs/background-attachment
       */
      "bg-attachment": [{
        bg: ["fixed", "local", "scroll"]
      }],
      /**
       * Background Clip
       * @see https://tailwindcss.com/docs/background-clip
       */
      "bg-clip": [{
        "bg-clip": ["border", "padding", "content", "text"]
      }],
      /**
       * Background Origin
       * @see https://tailwindcss.com/docs/background-origin
       */
      "bg-origin": [{
        "bg-origin": ["border", "padding", "content"]
      }],
      /**
       * Background Position
       * @see https://tailwindcss.com/docs/background-position
       */
      "bg-position": [{
        bg: $e()
      }],
      /**
       * Background Repeat
       * @see https://tailwindcss.com/docs/background-repeat
       */
      "bg-repeat": [{
        bg: we()
      }],
      /**
       * Background Size
       * @see https://tailwindcss.com/docs/background-size
       */
      "bg-size": [{
        bg: Ze()
      }],
      /**
       * Background Image
       * @see https://tailwindcss.com/docs/background-image
       */
      "bg-image": [{
        bg: ["none", {
          linear: [{
            to: ["t", "tr", "r", "br", "b", "bl", "l", "tl"]
          }, ne, k, R],
          radial: ["", k, R],
          conic: [ne, k, R]
        }, Li, Ni]
      }],
      /**
       * Background Color
       * @see https://tailwindcss.com/docs/background-color
       */
      "bg-color": [{
        bg: A()
      }],
      /**
       * Gradient Color Stops From Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-from-pos": [{
        from: Be()
      }],
      /**
       * Gradient Color Stops Via Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-via-pos": [{
        via: Be()
      }],
      /**
       * Gradient Color Stops To Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-to-pos": [{
        to: Be()
      }],
      /**
       * Gradient Color Stops From
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-from": [{
        from: A()
      }],
      /**
       * Gradient Color Stops Via
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-via": [{
        via: A()
      }],
      /**
       * Gradient Color Stops To
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-to": [{
        to: A()
      }],
      // ---------------
      // --- Borders ---
      // ---------------
      /**
       * Border Radius
       * @see https://tailwindcss.com/docs/border-radius
       */
      rounded: [{
        rounded: G()
      }],
      /**
       * Border Radius Start
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-s": [{
        "rounded-s": G()
      }],
      /**
       * Border Radius End
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-e": [{
        "rounded-e": G()
      }],
      /**
       * Border Radius Top
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-t": [{
        "rounded-t": G()
      }],
      /**
       * Border Radius Right
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-r": [{
        "rounded-r": G()
      }],
      /**
       * Border Radius Bottom
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-b": [{
        "rounded-b": G()
      }],
      /**
       * Border Radius Left
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-l": [{
        "rounded-l": G()
      }],
      /**
       * Border Radius Start Start
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-ss": [{
        "rounded-ss": G()
      }],
      /**
       * Border Radius Start End
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-se": [{
        "rounded-se": G()
      }],
      /**
       * Border Radius End End
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-ee": [{
        "rounded-ee": G()
      }],
      /**
       * Border Radius End Start
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-es": [{
        "rounded-es": G()
      }],
      /**
       * Border Radius Top Left
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-tl": [{
        "rounded-tl": G()
      }],
      /**
       * Border Radius Top Right
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-tr": [{
        "rounded-tr": G()
      }],
      /**
       * Border Radius Bottom Right
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-br": [{
        "rounded-br": G()
      }],
      /**
       * Border Radius Bottom Left
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-bl": [{
        "rounded-bl": G()
      }],
      /**
       * Border Width
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w": [{
        border: U()
      }],
      /**
       * Border Width Inline
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-x": [{
        "border-x": U()
      }],
      /**
       * Border Width Block
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-y": [{
        "border-y": U()
      }],
      /**
       * Border Width Inline Start
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-s": [{
        "border-s": U()
      }],
      /**
       * Border Width Inline End
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-e": [{
        "border-e": U()
      }],
      /**
       * Border Width Block Start
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-bs": [{
        "border-bs": U()
      }],
      /**
       * Border Width Block End
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-be": [{
        "border-be": U()
      }],
      /**
       * Border Width Top
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-t": [{
        "border-t": U()
      }],
      /**
       * Border Width Right
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-r": [{
        "border-r": U()
      }],
      /**
       * Border Width Bottom
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-b": [{
        "border-b": U()
      }],
      /**
       * Border Width Left
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-l": [{
        "border-l": U()
      }],
      /**
       * Divide Width X
       * @see https://tailwindcss.com/docs/border-width#between-children
       */
      "divide-x": [{
        "divide-x": U()
      }],
      /**
       * Divide Width X Reverse
       * @see https://tailwindcss.com/docs/border-width#between-children
       */
      "divide-x-reverse": ["divide-x-reverse"],
      /**
       * Divide Width Y
       * @see https://tailwindcss.com/docs/border-width#between-children
       */
      "divide-y": [{
        "divide-y": U()
      }],
      /**
       * Divide Width Y Reverse
       * @see https://tailwindcss.com/docs/border-width#between-children
       */
      "divide-y-reverse": ["divide-y-reverse"],
      /**
       * Border Style
       * @see https://tailwindcss.com/docs/border-style
       */
      "border-style": [{
        border: [...Ae(), "hidden", "none"]
      }],
      /**
       * Divide Style
       * @see https://tailwindcss.com/docs/border-style#setting-the-divider-style
       */
      "divide-style": [{
        divide: [...Ae(), "hidden", "none"]
      }],
      /**
       * Border Color
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color": [{
        border: A()
      }],
      /**
       * Border Color Inline
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-x": [{
        "border-x": A()
      }],
      /**
       * Border Color Block
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-y": [{
        "border-y": A()
      }],
      /**
       * Border Color Inline Start
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-s": [{
        "border-s": A()
      }],
      /**
       * Border Color Inline End
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-e": [{
        "border-e": A()
      }],
      /**
       * Border Color Block Start
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-bs": [{
        "border-bs": A()
      }],
      /**
       * Border Color Block End
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-be": [{
        "border-be": A()
      }],
      /**
       * Border Color Top
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-t": [{
        "border-t": A()
      }],
      /**
       * Border Color Right
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-r": [{
        "border-r": A()
      }],
      /**
       * Border Color Bottom
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-b": [{
        "border-b": A()
      }],
      /**
       * Border Color Left
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-l": [{
        "border-l": A()
      }],
      /**
       * Divide Color
       * @see https://tailwindcss.com/docs/divide-color
       */
      "divide-color": [{
        divide: A()
      }],
      /**
       * Outline Style
       * @see https://tailwindcss.com/docs/outline-style
       */
      "outline-style": [{
        outline: [...Ae(), "none", "hidden"]
      }],
      /**
       * Outline Offset
       * @see https://tailwindcss.com/docs/outline-offset
       */
      "outline-offset": [{
        "outline-offset": [N, k, R]
      }],
      /**
       * Outline Width
       * @see https://tailwindcss.com/docs/outline-width
       */
      "outline-w": [{
        outline: ["", N, He, Ee]
      }],
      /**
       * Outline Color
       * @see https://tailwindcss.com/docs/outline-color
       */
      "outline-color": [{
        outline: A()
      }],
      // ---------------
      // --- Effects ---
      // ---------------
      /**
       * Box Shadow
       * @see https://tailwindcss.com/docs/box-shadow
       */
      shadow: [{
        shadow: [
          // Deprecated since Tailwind CSS v4.0.0
          "",
          "none",
          l,
          et,
          Je
        ]
      }],
      /**
       * Box Shadow Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-shadow-color
       */
      "shadow-color": [{
        shadow: A()
      }],
      /**
       * Inset Box Shadow
       * @see https://tailwindcss.com/docs/box-shadow#adding-an-inset-shadow
       */
      "inset-shadow": [{
        "inset-shadow": ["none", f, et, Je]
      }],
      /**
       * Inset Box Shadow Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-inset-shadow-color
       */
      "inset-shadow-color": [{
        "inset-shadow": A()
      }],
      /**
       * Ring Width
       * @see https://tailwindcss.com/docs/box-shadow#adding-a-ring
       */
      "ring-w": [{
        ring: U()
      }],
      /**
       * Ring Width Inset
       * @see https://v3.tailwindcss.com/docs/ring-width#inset-rings
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-w-inset": ["ring-inset"],
      /**
       * Ring Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-ring-color
       */
      "ring-color": [{
        ring: A()
      }],
      /**
       * Ring Offset Width
       * @see https://v3.tailwindcss.com/docs/ring-offset-width
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-offset-w": [{
        "ring-offset": [N, Ee]
      }],
      /**
       * Ring Offset Color
       * @see https://v3.tailwindcss.com/docs/ring-offset-color
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-offset-color": [{
        "ring-offset": A()
      }],
      /**
       * Inset Ring Width
       * @see https://tailwindcss.com/docs/box-shadow#adding-an-inset-ring
       */
      "inset-ring-w": [{
        "inset-ring": U()
      }],
      /**
       * Inset Ring Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-inset-ring-color
       */
      "inset-ring-color": [{
        "inset-ring": A()
      }],
      /**
       * Text Shadow
       * @see https://tailwindcss.com/docs/text-shadow
       */
      "text-shadow": [{
        "text-shadow": ["none", p, et, Je]
      }],
      /**
       * Text Shadow Color
       * @see https://tailwindcss.com/docs/text-shadow#setting-the-shadow-color
       */
      "text-shadow-color": [{
        "text-shadow": A()
      }],
      /**
       * Opacity
       * @see https://tailwindcss.com/docs/opacity
       */
      opacity: [{
        opacity: [N, k, R]
      }],
      /**
       * Mix Blend Mode
       * @see https://tailwindcss.com/docs/mix-blend-mode
       */
      "mix-blend": [{
        "mix-blend": [...Ve(), "plus-darker", "plus-lighter"]
      }],
      /**
       * Background Blend Mode
       * @see https://tailwindcss.com/docs/background-blend-mode
       */
      "bg-blend": [{
        "bg-blend": Ve()
      }],
      /**
       * Mask Clip
       * @see https://tailwindcss.com/docs/mask-clip
       */
      "mask-clip": [{
        "mask-clip": ["border", "padding", "content", "fill", "stroke", "view"]
      }, "mask-no-clip"],
      /**
       * Mask Composite
       * @see https://tailwindcss.com/docs/mask-composite
       */
      "mask-composite": [{
        mask: ["add", "subtract", "intersect", "exclude"]
      }],
      /**
       * Mask Image
       * @see https://tailwindcss.com/docs/mask-image
       */
      "mask-image-linear-pos": [{
        "mask-linear": [N]
      }],
      "mask-image-linear-from-pos": [{
        "mask-linear-from": V()
      }],
      "mask-image-linear-to-pos": [{
        "mask-linear-to": V()
      }],
      "mask-image-linear-from-color": [{
        "mask-linear-from": A()
      }],
      "mask-image-linear-to-color": [{
        "mask-linear-to": A()
      }],
      "mask-image-t-from-pos": [{
        "mask-t-from": V()
      }],
      "mask-image-t-to-pos": [{
        "mask-t-to": V()
      }],
      "mask-image-t-from-color": [{
        "mask-t-from": A()
      }],
      "mask-image-t-to-color": [{
        "mask-t-to": A()
      }],
      "mask-image-r-from-pos": [{
        "mask-r-from": V()
      }],
      "mask-image-r-to-pos": [{
        "mask-r-to": V()
      }],
      "mask-image-r-from-color": [{
        "mask-r-from": A()
      }],
      "mask-image-r-to-color": [{
        "mask-r-to": A()
      }],
      "mask-image-b-from-pos": [{
        "mask-b-from": V()
      }],
      "mask-image-b-to-pos": [{
        "mask-b-to": V()
      }],
      "mask-image-b-from-color": [{
        "mask-b-from": A()
      }],
      "mask-image-b-to-color": [{
        "mask-b-to": A()
      }],
      "mask-image-l-from-pos": [{
        "mask-l-from": V()
      }],
      "mask-image-l-to-pos": [{
        "mask-l-to": V()
      }],
      "mask-image-l-from-color": [{
        "mask-l-from": A()
      }],
      "mask-image-l-to-color": [{
        "mask-l-to": A()
      }],
      "mask-image-x-from-pos": [{
        "mask-x-from": V()
      }],
      "mask-image-x-to-pos": [{
        "mask-x-to": V()
      }],
      "mask-image-x-from-color": [{
        "mask-x-from": A()
      }],
      "mask-image-x-to-color": [{
        "mask-x-to": A()
      }],
      "mask-image-y-from-pos": [{
        "mask-y-from": V()
      }],
      "mask-image-y-to-pos": [{
        "mask-y-to": V()
      }],
      "mask-image-y-from-color": [{
        "mask-y-from": A()
      }],
      "mask-image-y-to-color": [{
        "mask-y-to": A()
      }],
      "mask-image-radial": [{
        "mask-radial": [k, R]
      }],
      "mask-image-radial-from-pos": [{
        "mask-radial-from": V()
      }],
      "mask-image-radial-to-pos": [{
        "mask-radial-to": V()
      }],
      "mask-image-radial-from-color": [{
        "mask-radial-from": A()
      }],
      "mask-image-radial-to-color": [{
        "mask-radial-to": A()
      }],
      "mask-image-radial-shape": [{
        "mask-radial": ["circle", "ellipse"]
      }],
      "mask-image-radial-size": [{
        "mask-radial": [{
          closest: ["side", "corner"],
          farthest: ["side", "corner"]
        }]
      }],
      "mask-image-radial-pos": [{
        "mask-radial-at": C()
      }],
      "mask-image-conic-pos": [{
        "mask-conic": [N]
      }],
      "mask-image-conic-from-pos": [{
        "mask-conic-from": V()
      }],
      "mask-image-conic-to-pos": [{
        "mask-conic-to": V()
      }],
      "mask-image-conic-from-color": [{
        "mask-conic-from": A()
      }],
      "mask-image-conic-to-color": [{
        "mask-conic-to": A()
      }],
      /**
       * Mask Mode
       * @see https://tailwindcss.com/docs/mask-mode
       */
      "mask-mode": [{
        mask: ["alpha", "luminance", "match"]
      }],
      /**
       * Mask Origin
       * @see https://tailwindcss.com/docs/mask-origin
       */
      "mask-origin": [{
        "mask-origin": ["border", "padding", "content", "fill", "stroke", "view"]
      }],
      /**
       * Mask Position
       * @see https://tailwindcss.com/docs/mask-position
       */
      "mask-position": [{
        mask: $e()
      }],
      /**
       * Mask Repeat
       * @see https://tailwindcss.com/docs/mask-repeat
       */
      "mask-repeat": [{
        mask: we()
      }],
      /**
       * Mask Size
       * @see https://tailwindcss.com/docs/mask-size
       */
      "mask-size": [{
        mask: Ze()
      }],
      /**
       * Mask Type
       * @see https://tailwindcss.com/docs/mask-type
       */
      "mask-type": [{
        "mask-type": ["alpha", "luminance"]
      }],
      /**
       * Mask Image
       * @see https://tailwindcss.com/docs/mask-image
       */
      "mask-image": [{
        mask: ["none", k, R]
      }],
      // ---------------
      // --- Filters ---
      // ---------------
      /**
       * Filter
       * @see https://tailwindcss.com/docs/filter
       */
      filter: [{
        filter: [
          // Deprecated since Tailwind CSS v3.0.0
          "",
          "none",
          k,
          R
        ]
      }],
      /**
       * Blur
       * @see https://tailwindcss.com/docs/blur
       */
      blur: [{
        blur: je()
      }],
      /**
       * Brightness
       * @see https://tailwindcss.com/docs/brightness
       */
      brightness: [{
        brightness: [N, k, R]
      }],
      /**
       * Contrast
       * @see https://tailwindcss.com/docs/contrast
       */
      contrast: [{
        contrast: [N, k, R]
      }],
      /**
       * Drop Shadow
       * @see https://tailwindcss.com/docs/drop-shadow
       */
      "drop-shadow": [{
        "drop-shadow": [
          // Deprecated since Tailwind CSS v4.0.0
          "",
          "none",
          m,
          et,
          Je
        ]
      }],
      /**
       * Drop Shadow Color
       * @see https://tailwindcss.com/docs/filter-drop-shadow#setting-the-shadow-color
       */
      "drop-shadow-color": [{
        "drop-shadow": A()
      }],
      /**
       * Grayscale
       * @see https://tailwindcss.com/docs/grayscale
       */
      grayscale: [{
        grayscale: ["", N, k, R]
      }],
      /**
       * Hue Rotate
       * @see https://tailwindcss.com/docs/hue-rotate
       */
      "hue-rotate": [{
        "hue-rotate": [N, k, R]
      }],
      /**
       * Invert
       * @see https://tailwindcss.com/docs/invert
       */
      invert: [{
        invert: ["", N, k, R]
      }],
      /**
       * Saturate
       * @see https://tailwindcss.com/docs/saturate
       */
      saturate: [{
        saturate: [N, k, R]
      }],
      /**
       * Sepia
       * @see https://tailwindcss.com/docs/sepia
       */
      sepia: [{
        sepia: ["", N, k, R]
      }],
      /**
       * Backdrop Filter
       * @see https://tailwindcss.com/docs/backdrop-filter
       */
      "backdrop-filter": [{
        "backdrop-filter": [
          // Deprecated since Tailwind CSS v3.0.0
          "",
          "none",
          k,
          R
        ]
      }],
      /**
       * Backdrop Blur
       * @see https://tailwindcss.com/docs/backdrop-blur
       */
      "backdrop-blur": [{
        "backdrop-blur": je()
      }],
      /**
       * Backdrop Brightness
       * @see https://tailwindcss.com/docs/backdrop-brightness
       */
      "backdrop-brightness": [{
        "backdrop-brightness": [N, k, R]
      }],
      /**
       * Backdrop Contrast
       * @see https://tailwindcss.com/docs/backdrop-contrast
       */
      "backdrop-contrast": [{
        "backdrop-contrast": [N, k, R]
      }],
      /**
       * Backdrop Grayscale
       * @see https://tailwindcss.com/docs/backdrop-grayscale
       */
      "backdrop-grayscale": [{
        "backdrop-grayscale": ["", N, k, R]
      }],
      /**
       * Backdrop Hue Rotate
       * @see https://tailwindcss.com/docs/backdrop-hue-rotate
       */
      "backdrop-hue-rotate": [{
        "backdrop-hue-rotate": [N, k, R]
      }],
      /**
       * Backdrop Invert
       * @see https://tailwindcss.com/docs/backdrop-invert
       */
      "backdrop-invert": [{
        "backdrop-invert": ["", N, k, R]
      }],
      /**
       * Backdrop Opacity
       * @see https://tailwindcss.com/docs/backdrop-opacity
       */
      "backdrop-opacity": [{
        "backdrop-opacity": [N, k, R]
      }],
      /**
       * Backdrop Saturate
       * @see https://tailwindcss.com/docs/backdrop-saturate
       */
      "backdrop-saturate": [{
        "backdrop-saturate": [N, k, R]
      }],
      /**
       * Backdrop Sepia
       * @see https://tailwindcss.com/docs/backdrop-sepia
       */
      "backdrop-sepia": [{
        "backdrop-sepia": ["", N, k, R]
      }],
      // --------------
      // --- Tables ---
      // --------------
      /**
       * Border Collapse
       * @see https://tailwindcss.com/docs/border-collapse
       */
      "border-collapse": [{
        border: ["collapse", "separate"]
      }],
      /**
       * Border Spacing
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing": [{
        "border-spacing": E()
      }],
      /**
       * Border Spacing X
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing-x": [{
        "border-spacing-x": E()
      }],
      /**
       * Border Spacing Y
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing-y": [{
        "border-spacing-y": E()
      }],
      /**
       * Table Layout
       * @see https://tailwindcss.com/docs/table-layout
       */
      "table-layout": [{
        table: ["auto", "fixed"]
      }],
      /**
       * Caption Side
       * @see https://tailwindcss.com/docs/caption-side
       */
      caption: [{
        caption: ["top", "bottom"]
      }],
      // ---------------------------------
      // --- Transitions and Animation ---
      // ---------------------------------
      /**
       * Transition Property
       * @see https://tailwindcss.com/docs/transition-property
       */
      transition: [{
        transition: ["", "all", "colors", "opacity", "shadow", "transform", "none", k, R]
      }],
      /**
       * Transition Behavior
       * @see https://tailwindcss.com/docs/transition-behavior
       */
      "transition-behavior": [{
        transition: ["normal", "discrete"]
      }],
      /**
       * Transition Duration
       * @see https://tailwindcss.com/docs/transition-duration
       */
      duration: [{
        duration: [N, "initial", k, R]
      }],
      /**
       * Transition Timing Function
       * @see https://tailwindcss.com/docs/transition-timing-function
       */
      ease: [{
        ease: ["linear", "initial", y, k, R]
      }],
      /**
       * Transition Delay
       * @see https://tailwindcss.com/docs/transition-delay
       */
      delay: [{
        delay: [N, k, R]
      }],
      /**
       * Animation
       * @see https://tailwindcss.com/docs/animation
       */
      animate: [{
        animate: ["none", v, k, R]
      }],
      // ------------------
      // --- Transforms ---
      // ------------------
      /**
       * Backface Visibility
       * @see https://tailwindcss.com/docs/backface-visibility
       */
      backface: [{
        backface: ["hidden", "visible"]
      }],
      /**
       * Perspective
       * @see https://tailwindcss.com/docs/perspective
       */
      perspective: [{
        perspective: [h, k, R]
      }],
      /**
       * Perspective Origin
       * @see https://tailwindcss.com/docs/perspective-origin
       */
      "perspective-origin": [{
        "perspective-origin": S()
      }],
      /**
       * Rotate
       * @see https://tailwindcss.com/docs/rotate
       */
      rotate: [{
        rotate: xe()
      }],
      /**
       * Rotate X
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-x": [{
        "rotate-x": xe()
      }],
      /**
       * Rotate Y
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-y": [{
        "rotate-y": xe()
      }],
      /**
       * Rotate Z
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-z": [{
        "rotate-z": xe()
      }],
      /**
       * Scale
       * @see https://tailwindcss.com/docs/scale
       */
      scale: [{
        scale: Ce()
      }],
      /**
       * Scale X
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-x": [{
        "scale-x": Ce()
      }],
      /**
       * Scale Y
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-y": [{
        "scale-y": Ce()
      }],
      /**
       * Scale Z
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-z": [{
        "scale-z": Ce()
      }],
      /**
       * Scale 3D
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-3d": ["scale-3d"],
      /**
       * Skew
       * @see https://tailwindcss.com/docs/skew
       */
      skew: [{
        skew: Pe()
      }],
      /**
       * Skew X
       * @see https://tailwindcss.com/docs/skew
       */
      "skew-x": [{
        "skew-x": Pe()
      }],
      /**
       * Skew Y
       * @see https://tailwindcss.com/docs/skew
       */
      "skew-y": [{
        "skew-y": Pe()
      }],
      /**
       * Transform
       * @see https://tailwindcss.com/docs/transform
       */
      transform: [{
        transform: [k, R, "", "none", "gpu", "cpu"]
      }],
      /**
       * Transform Origin
       * @see https://tailwindcss.com/docs/transform-origin
       */
      "transform-origin": [{
        origin: S()
      }],
      /**
       * Transform Style
       * @see https://tailwindcss.com/docs/transform-style
       */
      "transform-style": [{
        transform: ["3d", "flat"]
      }],
      /**
       * Translate
       * @see https://tailwindcss.com/docs/translate
       */
      translate: [{
        translate: pe()
      }],
      /**
       * Translate X
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-x": [{
        "translate-x": pe()
      }],
      /**
       * Translate Y
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-y": [{
        "translate-y": pe()
      }],
      /**
       * Translate Z
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-z": [{
        "translate-z": pe()
      }],
      /**
       * Translate None
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-none": ["translate-none"],
      /**
       * Zoom
       * @see https://tailwindcss.com/docs/zoom
       */
      zoom: [{
        zoom: [ne, k, R]
      }],
      // ---------------------
      // --- Interactivity ---
      // ---------------------
      /**
       * Accent Color
       * @see https://tailwindcss.com/docs/accent-color
       */
      accent: [{
        accent: A()
      }],
      /**
       * Appearance
       * @see https://tailwindcss.com/docs/appearance
       */
      appearance: [{
        appearance: ["none", "auto"]
      }],
      /**
       * Caret Color
       * @see https://tailwindcss.com/docs/just-in-time-mode#caret-color-utilities
       */
      "caret-color": [{
        caret: A()
      }],
      /**
       * Color Scheme
       * @see https://tailwindcss.com/docs/color-scheme
       */
      "color-scheme": [{
        scheme: ["normal", "dark", "light", "light-dark", "only-dark", "only-light"]
      }],
      /**
       * Cursor
       * @see https://tailwindcss.com/docs/cursor
       */
      cursor: [{
        cursor: ["auto", "default", "pointer", "wait", "text", "move", "help", "not-allowed", "none", "context-menu", "progress", "cell", "crosshair", "vertical-text", "alias", "copy", "no-drop", "grab", "grabbing", "all-scroll", "col-resize", "row-resize", "n-resize", "e-resize", "s-resize", "w-resize", "ne-resize", "nw-resize", "se-resize", "sw-resize", "ew-resize", "ns-resize", "nesw-resize", "nwse-resize", "zoom-in", "zoom-out", k, R]
      }],
      /**
       * Field Sizing
       * @see https://tailwindcss.com/docs/field-sizing
       */
      "field-sizing": [{
        "field-sizing": ["fixed", "content"]
      }],
      /**
       * Pointer Events
       * @see https://tailwindcss.com/docs/pointer-events
       */
      "pointer-events": [{
        "pointer-events": ["auto", "none"]
      }],
      /**
       * Resize
       * @see https://tailwindcss.com/docs/resize
       */
      resize: [{
        resize: ["none", "", "y", "x"]
      }],
      /**
       * Scroll Behavior
       * @see https://tailwindcss.com/docs/scroll-behavior
       */
      "scroll-behavior": [{
        scroll: ["auto", "smooth"]
      }],
      /**
       * Scrollbar Thumb Color
       * @see https://tailwindcss.com/docs/scrollbar-color
       */
      "scrollbar-thumb-color": [{
        "scrollbar-thumb": A()
      }],
      /**
       * Scrollbar Track Color
       * @see https://tailwindcss.com/docs/scrollbar-color
       */
      "scrollbar-track-color": [{
        "scrollbar-track": A()
      }],
      /**
       * Scrollbar Gutter
       * @see https://tailwindcss.com/docs/scrollbar-gutter
       */
      "scrollbar-gutter": [{
        "scrollbar-gutter": ["auto", "stable", "both"]
      }],
      /**
       * Scrollbar Width
       * @see https://tailwindcss.com/docs/scrollbar-width
       */
      "scrollbar-w": [{
        scrollbar: ["auto", "thin", "none"]
      }],
      /**
       * Scroll Margin
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-m": [{
        "scroll-m": E()
      }],
      /**
       * Scroll Margin Inline
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mx": [{
        "scroll-mx": E()
      }],
      /**
       * Scroll Margin Block
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-my": [{
        "scroll-my": E()
      }],
      /**
       * Scroll Margin Inline Start
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-ms": [{
        "scroll-ms": E()
      }],
      /**
       * Scroll Margin Inline End
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-me": [{
        "scroll-me": E()
      }],
      /**
       * Scroll Margin Block Start
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mbs": [{
        "scroll-mbs": E()
      }],
      /**
       * Scroll Margin Block End
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mbe": [{
        "scroll-mbe": E()
      }],
      /**
       * Scroll Margin Top
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mt": [{
        "scroll-mt": E()
      }],
      /**
       * Scroll Margin Right
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mr": [{
        "scroll-mr": E()
      }],
      /**
       * Scroll Margin Bottom
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mb": [{
        "scroll-mb": E()
      }],
      /**
       * Scroll Margin Left
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-ml": [{
        "scroll-ml": E()
      }],
      /**
       * Scroll Padding
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-p": [{
        "scroll-p": E()
      }],
      /**
       * Scroll Padding Inline
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-px": [{
        "scroll-px": E()
      }],
      /**
       * Scroll Padding Block
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-py": [{
        "scroll-py": E()
      }],
      /**
       * Scroll Padding Inline Start
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-ps": [{
        "scroll-ps": E()
      }],
      /**
       * Scroll Padding Inline End
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pe": [{
        "scroll-pe": E()
      }],
      /**
       * Scroll Padding Block Start
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pbs": [{
        "scroll-pbs": E()
      }],
      /**
       * Scroll Padding Block End
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pbe": [{
        "scroll-pbe": E()
      }],
      /**
       * Scroll Padding Top
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pt": [{
        "scroll-pt": E()
      }],
      /**
       * Scroll Padding Right
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pr": [{
        "scroll-pr": E()
      }],
      /**
       * Scroll Padding Bottom
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pb": [{
        "scroll-pb": E()
      }],
      /**
       * Scroll Padding Left
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pl": [{
        "scroll-pl": E()
      }],
      /**
       * Scroll Snap Align
       * @see https://tailwindcss.com/docs/scroll-snap-align
       */
      "snap-align": [{
        snap: ["start", "end", "center", "align-none"]
      }],
      /**
       * Scroll Snap Stop
       * @see https://tailwindcss.com/docs/scroll-snap-stop
       */
      "snap-stop": [{
        snap: ["normal", "always"]
      }],
      /**
       * Scroll Snap Type
       * @see https://tailwindcss.com/docs/scroll-snap-type
       */
      "snap-type": [{
        snap: ["none", "x", "y", "both"]
      }],
      /**
       * Scroll Snap Type Strictness
       * @see https://tailwindcss.com/docs/scroll-snap-type
       */
      "snap-strictness": [{
        snap: ["mandatory", "proximity"]
      }],
      /**
       * Touch Action
       * @see https://tailwindcss.com/docs/touch-action
       */
      touch: [{
        touch: ["auto", "none", "manipulation"]
      }],
      /**
       * Touch Action X
       * @see https://tailwindcss.com/docs/touch-action
       */
      "touch-x": [{
        "touch-pan": ["x", "left", "right"]
      }],
      /**
       * Touch Action Y
       * @see https://tailwindcss.com/docs/touch-action
       */
      "touch-y": [{
        "touch-pan": ["y", "up", "down"]
      }],
      /**
       * Touch Action Pinch Zoom
       * @see https://tailwindcss.com/docs/touch-action
       */
      "touch-pz": ["touch-pinch-zoom"],
      /**
       * User Select
       * @see https://tailwindcss.com/docs/user-select
       */
      select: [{
        select: ["none", "text", "all", "auto"]
      }],
      /**
       * Will Change
       * @see https://tailwindcss.com/docs/will-change
       */
      "will-change": [{
        "will-change": ["auto", "scroll", "contents", "transform", k, R]
      }],
      // -----------
      // --- SVG ---
      // -----------
      /**
       * Fill
       * @see https://tailwindcss.com/docs/fill
       */
      fill: [{
        fill: ["none", ...A()]
      }],
      /**
       * Stroke Width
       * @see https://tailwindcss.com/docs/stroke-width
       */
      "stroke-w": [{
        stroke: [N, He, Ee, Cn]
      }],
      /**
       * Stroke
       * @see https://tailwindcss.com/docs/stroke
       */
      stroke: [{
        stroke: ["none", ...A()]
      }],
      // ---------------------
      // --- Accessibility ---
      // ---------------------
      /**
       * Forced Color Adjust
       * @see https://tailwindcss.com/docs/forced-color-adjust
       */
      "forced-color-adjust": [{
        "forced-color-adjust": ["auto", "none"]
      }]
    },
    conflictingClassGroups: {
      "container-named": ["container-type"],
      overflow: ["overflow-x", "overflow-y"],
      overscroll: ["overscroll-x", "overscroll-y"],
      inset: ["inset-x", "inset-y", "inset-bs", "inset-be", "start", "end", "top", "right", "bottom", "left"],
      "inset-x": ["right", "left"],
      "inset-y": ["top", "bottom"],
      flex: ["basis", "grow", "shrink"],
      gap: ["gap-x", "gap-y"],
      p: ["px", "py", "ps", "pe", "pbs", "pbe", "pt", "pr", "pb", "pl"],
      px: ["pr", "pl"],
      py: ["pt", "pb"],
      m: ["mx", "my", "ms", "me", "mbs", "mbe", "mt", "mr", "mb", "ml"],
      mx: ["mr", "ml"],
      my: ["mt", "mb"],
      size: ["w", "h"],
      "font-size": ["leading"],
      "fvn-normal": ["fvn-ordinal", "fvn-slashed-zero", "fvn-figure", "fvn-spacing", "fvn-fraction"],
      "fvn-ordinal": ["fvn-normal"],
      "fvn-slashed-zero": ["fvn-normal"],
      "fvn-figure": ["fvn-normal"],
      "fvn-spacing": ["fvn-normal"],
      "fvn-fraction": ["fvn-normal"],
      "line-clamp": ["display", "overflow"],
      rounded: ["rounded-s", "rounded-e", "rounded-t", "rounded-r", "rounded-b", "rounded-l", "rounded-ss", "rounded-se", "rounded-ee", "rounded-es", "rounded-tl", "rounded-tr", "rounded-br", "rounded-bl"],
      "rounded-s": ["rounded-ss", "rounded-es"],
      "rounded-e": ["rounded-se", "rounded-ee"],
      "rounded-t": ["rounded-tl", "rounded-tr"],
      "rounded-r": ["rounded-tr", "rounded-br"],
      "rounded-b": ["rounded-br", "rounded-bl"],
      "rounded-l": ["rounded-tl", "rounded-bl"],
      "border-spacing": ["border-spacing-x", "border-spacing-y"],
      "border-w": ["border-w-x", "border-w-y", "border-w-s", "border-w-e", "border-w-bs", "border-w-be", "border-w-t", "border-w-r", "border-w-b", "border-w-l"],
      "border-w-x": ["border-w-r", "border-w-l"],
      "border-w-y": ["border-w-t", "border-w-b"],
      "border-color": ["border-color-x", "border-color-y", "border-color-s", "border-color-e", "border-color-bs", "border-color-be", "border-color-t", "border-color-r", "border-color-b", "border-color-l"],
      "border-color-x": ["border-color-r", "border-color-l"],
      "border-color-y": ["border-color-t", "border-color-b"],
      translate: ["translate-x", "translate-y", "translate-none"],
      "translate-none": ["translate", "translate-x", "translate-y", "translate-z"],
      "scroll-m": ["scroll-mx", "scroll-my", "scroll-ms", "scroll-me", "scroll-mbs", "scroll-mbe", "scroll-mt", "scroll-mr", "scroll-mb", "scroll-ml"],
      "scroll-mx": ["scroll-mr", "scroll-ml"],
      "scroll-my": ["scroll-mt", "scroll-mb"],
      "scroll-p": ["scroll-px", "scroll-py", "scroll-ps", "scroll-pe", "scroll-pbs", "scroll-pbe", "scroll-pt", "scroll-pr", "scroll-pb", "scroll-pl"],
      "scroll-px": ["scroll-pr", "scroll-pl"],
      "scroll-py": ["scroll-pt", "scroll-pb"],
      touch: ["touch-x", "touch-y", "touch-pz"],
      "touch-x": ["touch"],
      "touch-y": ["touch"],
      "touch-pz": ["touch"]
    },
    conflictingClassGroupModifiers: {
      "font-size": ["leading"]
    },
    postfixLookupClassGroups: ["container-type"],
    orderSensitiveModifiers: ["*", "**", "after", "backdrop", "before", "details-content", "file", "first-letter", "first-line", "marker", "placeholder", "selection"]
  };
}, zi = /* @__PURE__ */ hi(Fi);
function $(...e) {
  return zi(nr(e));
}
const Wi = rr(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors disabled:pointer-events-none disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-nr-accent/25 [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "border border-nr-accent/20 bg-nr-accent/10 text-nr-accent hover:bg-nr-accent/20",
        ghost: "hover:bg-nr-bg hover:text-nr-fg"
      },
      size: {
        default: "h-9 px-3 py-2",
        icon: "h-9 w-9"
      }
    },
    defaultVariants: {
      variant: "default",
      size: "default"
    }
  }
), $i = d.forwardRef(function({ className: t, variant: n, size: r, asChild: o = !1, ...i }, s) {
  return /* @__PURE__ */ w(o ? Jn : "button", { ref: s, className: $(Wi({ variant: n, size: r, className: t })), ...i });
});
function X(e, t, { checkForDefaultPrevented: n = !0 } = {}) {
  return function(o) {
    if (e == null || e(o), n === !1 || !o.defaultPrevented)
      return t == null ? void 0 : t(o);
  };
}
function Zt(e, t = []) {
  let n = [];
  function r(i, s) {
    const a = d.createContext(s);
    a.displayName = i + "Context";
    const c = n.length;
    n = [...n, s];
    const u = (f) => {
      var y;
      const { scope: p, children: m, ...g } = f, h = ((y = p == null ? void 0 : p[e]) == null ? void 0 : y[c]) || a, b = d.useMemo(() => g, Object.values(g));
      return /* @__PURE__ */ w(h.Provider, { value: b, children: m });
    };
    u.displayName = i + "Provider";
    function l(f, p) {
      var h;
      const m = ((h = p == null ? void 0 : p[e]) == null ? void 0 : h[c]) || a, g = d.useContext(m);
      if (g) return g;
      if (s !== void 0) return s;
      throw new Error(`\`${f}\` must be used within \`${i}\``);
    }
    return [u, l];
  }
  const o = () => {
    const i = n.map((s) => d.createContext(s));
    return function(a) {
      const c = (a == null ? void 0 : a[e]) || i;
      return d.useMemo(
        () => ({ [`__scope${e}`]: { ...a, [e]: c } }),
        [a, c]
      );
    };
  };
  return o.scopeName = e, [r, Bi(o, ...t)];
}
function Bi(...e) {
  const t = e[0];
  if (e.length === 1) return t;
  const n = () => {
    const r = e.map((o) => ({
      useScope: o(),
      scopeName: o.scopeName
    }));
    return function(i) {
      const s = r.reduce((a, { useScope: c, scopeName: u }) => {
        const f = c(i)[`__scope${u}`];
        return { ...a, ...f };
      }, {});
      return d.useMemo(() => ({ [`__scope${t.scopeName}`]: s }), [s]);
    };
  };
  return n.scopeName = t.scopeName, n;
}
var se = globalThis != null && globalThis.document ? d.useLayoutEffect : () => {
}, Vi = d[" useId ".trim().toString()] || (() => {
}), ji = 0;
function ct(e) {
  const [t, n] = d.useState(Vi());
  return se(() => {
    n((r) => r ?? String(ji++));
  }, [e]), e || (t ? `radix-${t}` : "");
}
var Rn = d[" useEffectEvent ".trim().toString()], kn = d[" useInsertionEffect ".trim().toString()];
function Hi(e) {
  if (typeof Rn == "function")
    return Rn(e);
  const t = d.useRef(() => {
    throw new Error("Cannot call an event handler while rendering.");
  });
  return typeof kn == "function" ? kn(() => {
    t.current = e;
  }) : se(() => {
    t.current = e;
  }), d.useMemo(() => (...n) => {
    var r;
    return (r = t.current) == null ? void 0 : r.call(t, ...n);
  }, []);
}
var Gi = d[" useInsertionEffect ".trim().toString()] || se;
function wr({
  prop: e,
  defaultProp: t,
  onChange: n = () => {
  },
  caller: r
}) {
  const [o, i, s] = Ui({
    defaultProp: t,
    onChange: n
  }), a = e !== void 0, c = a ? e : o;
  {
    const l = d.useRef(e !== void 0);
    d.useEffect(() => {
      const f = l.current;
      f !== a && console.warn(
        `${r} is changing from ${f ? "controlled" : "uncontrolled"} to ${a ? "controlled" : "uncontrolled"}. Components should not switch from controlled to uncontrolled (or vice versa). Decide between using a controlled or uncontrolled value for the lifetime of the component.`
      ), l.current = a;
    }, [a, r]);
  }
  const u = d.useCallback(
    (l) => {
      var f;
      if (a) {
        const p = Yi(l) ? l(e) : l;
        p !== e && ((f = s.current) == null || f.call(s, p));
      } else
        i(l);
    },
    [a, e, i, s]
  );
  return [c, u];
}
function Ui({
  defaultProp: e,
  onChange: t
}) {
  const [n, r] = d.useState(e), o = d.useRef(n), i = d.useRef(t);
  return Gi(() => {
    i.current = t;
  }, [t]), d.useEffect(() => {
    var s;
    o.current !== n && ((s = i.current) == null || s.call(i, n), o.current = n);
  }, [n, o]), [n, r, i];
}
function Yi(e) {
  return typeof e == "function";
}
var Xi = [
  "a",
  "button",
  "div",
  "form",
  "h2",
  "h3",
  "img",
  "input",
  "label",
  "li",
  "nav",
  "ol",
  "p",
  "select",
  "span",
  "svg",
  "ul"
], K = Xi.reduce((e, t) => {
  const n = /* @__PURE__ */ Yt(`Primitive.${t}`), r = d.forwardRef((o, i) => {
    const { asChild: s, ...a } = o, c = s ? n : t;
    return typeof window < "u" && (window[Symbol.for("radix-ui")] = !0), /* @__PURE__ */ w(c, { ...a, ref: i });
  });
  return r.displayName = `Primitive.${t}`, { ...e, [t]: r };
}, {});
function Ki(e, t) {
  e && Ut.flushSync(() => e.dispatchEvent(t));
}
function Ge(e) {
  const t = d.useRef(e);
  return d.useEffect(() => {
    t.current = e;
  }), d.useMemo(() => (...n) => {
    var r;
    return (r = t.current) == null ? void 0 : r.call(t, ...n);
  }, []);
}
var Zi = "DismissableLayer", Wt = "dismissableLayer.update", qi = "dismissableLayer.pointerDownOutside", Qi = "dismissableLayer.focusOutside", An, qt = d.createContext({
  layers: /* @__PURE__ */ new Set(),
  layersWithOutsidePointerEventsDisabled: /* @__PURE__ */ new Set(),
  branches: /* @__PURE__ */ new Set(),
  // Outside elements that belong to a layer's own dismiss affordance (eg, a
  // dialog overlay). Pressing them should dismiss the layer regardless of
  // whether or not they stop propagation.
  //
  // See https://github.com/radix-ui/primitives/issues/3346
  dismissableSurfaces: /* @__PURE__ */ new Set()
}), Qt = d.forwardRef(
  (e, t) => {
    const {
      disableOutsidePointerEvents: n = !1,
      deferPointerDownOutside: r = !1,
      onEscapeKeyDown: o,
      onPointerDownOutside: i,
      onFocusOutside: s,
      onInteractOutside: a,
      onDismiss: c,
      ...u
    } = e, l = d.useContext(qt), [f, p] = d.useState(null), m = (f == null ? void 0 : f.ownerDocument) ?? (globalThis == null ? void 0 : globalThis.document), [, g] = d.useState({}), h = ee(t, p), b = Array.from(l.layers), [y] = [...l.layersWithOutsidePointerEventsDisabled].slice(-1), v = b.indexOf(y), x = f ? b.indexOf(f) : -1, C = l.layersWithOutsidePointerEventsDisabled.size > 0, S = x >= v, P = d.useRef(!1), O = ns(
      (T) => {
        const I = T.target;
        if (!(I instanceof Node))
          return;
        const W = [...l.branches].some(
          (_) => _.contains(I)
        );
        !S || W || (i == null || i(T), a == null || a(T), T.defaultPrevented || c == null || c());
      },
      {
        ownerDocument: m,
        deferPointerDownOutside: r,
        isDeferredPointerDownOutsideRef: P,
        dismissableSurfaces: l.dismissableSurfaces
      }
    ), E = rs((T) => {
      if (r && P.current)
        return;
      const I = T.target;
      [...l.branches].some((_) => _.contains(I)) || (s == null || s(T), a == null || a(T), T.defaultPrevented || c == null || c());
    }, m), M = f ? x === b.length - 1 : !1, z = Hi((T) => {
      T.key === "Escape" && (o == null || o(T), !T.defaultPrevented && c && (T.preventDefault(), c()));
    });
    return d.useEffect(() => {
      if (M)
        return m.addEventListener("keydown", z, { capture: !0 }), () => m.removeEventListener("keydown", z, { capture: !0 });
    }, [m, M]), d.useEffect(() => {
      if (f)
        return n && (l.layersWithOutsidePointerEventsDisabled.size === 0 && (An = m.body.style.pointerEvents, m.body.style.pointerEvents = "none"), l.layersWithOutsidePointerEventsDisabled.add(f)), l.layers.add(f), Pn(), () => {
          n && (l.layersWithOutsidePointerEventsDisabled.delete(f), l.layersWithOutsidePointerEventsDisabled.size === 0 && (m.body.style.pointerEvents = An));
        };
    }, [f, m, n, l]), d.useEffect(() => () => {
      f && (l.layers.delete(f), l.layersWithOutsidePointerEventsDisabled.delete(f), Pn());
    }, [f, l]), d.useEffect(() => {
      const T = () => g({});
      return document.addEventListener(Wt, T), () => document.removeEventListener(Wt, T);
    }, []), /* @__PURE__ */ w(
      K.div,
      {
        ...u,
        ref: h,
        style: {
          pointerEvents: C ? S ? "auto" : "none" : void 0,
          ...e.style
        },
        onFocusCapture: X(e.onFocusCapture, E.onFocusCapture),
        onBlurCapture: X(e.onBlurCapture, E.onBlurCapture),
        onPointerDownCapture: X(
          e.onPointerDownCapture,
          O.onPointerDownCapture
        )
      }
    );
  }
);
Qt.displayName = Zi;
var Ji = "DismissableLayerBranch", es = d.forwardRef((e, t) => {
  const n = d.useContext(qt), r = d.useRef(null), o = ee(t, r);
  return d.useEffect(() => {
    const i = r.current;
    if (i)
      return n.branches.add(i), () => {
        n.branches.delete(i);
      };
  }, [n.branches]), /* @__PURE__ */ w(K.div, { ...e, ref: o });
});
es.displayName = Ji;
function ts() {
  const e = d.useContext(qt), [t, n] = d.useState(null);
  return d.useEffect(() => {
    if (t)
      return e.dismissableSurfaces.add(t), () => {
        e.dismissableSurfaces.delete(t);
      };
  }, [t, e.dismissableSurfaces]), n;
}
function ns(e, t) {
  const {
    ownerDocument: n = globalThis == null ? void 0 : globalThis.document,
    deferPointerDownOutside: r = !1,
    isDeferredPointerDownOutsideRef: o,
    dismissableSurfaces: i
  } = t, s = Ge(e), a = d.useRef(!1), c = d.useRef(!1), u = d.useRef(/* @__PURE__ */ new Map()), l = d.useRef(() => {
  });
  return d.useEffect(() => {
    function f() {
      c.current = !1, o.current = !1, u.current.clear();
    }
    function p() {
      return Array.from(u.current.values()).some(Boolean);
    }
    function m(v) {
      if (!c.current)
        return;
      const x = v.target;
      x instanceof Node && [...i].some((S) => S.contains(x)) || u.current.set(v.type, !0), v.type === "click" && window.setTimeout(() => {
        c.current && l.current();
      }, 0);
    }
    function g(v) {
      c.current && u.current.set(v.type, !1);
    }
    const h = (v) => {
      if (v.target && !a.current) {
        let x = function() {
          n.removeEventListener("click", l.current);
          const S = p();
          f(), S || xr(
            qi,
            s,
            C,
            { discrete: !0 }
          );
        };
        const C = { originalEvent: v };
        c.current = !0, o.current = r && v.button === 0, u.current.clear(), !r || v.button !== 0 ? x() : (n.removeEventListener("click", l.current), l.current = x, n.addEventListener("click", l.current, { once: !0 }));
      } else
        n.removeEventListener("click", l.current), f();
      a.current = !1;
    }, b = [
      "pointerup",
      "mousedown",
      "mouseup",
      "touchstart",
      "touchend",
      "click"
    ];
    for (const v of b)
      n.addEventListener(v, m, !0), n.addEventListener(v, g);
    const y = window.setTimeout(() => {
      n.addEventListener("pointerdown", h);
    }, 0);
    return () => {
      window.clearTimeout(y), n.removeEventListener("pointerdown", h), n.removeEventListener("click", l.current);
      for (const v of b)
        n.removeEventListener(v, m, !0), n.removeEventListener(v, g);
    };
  }, [
    n,
    s,
    r,
    o,
    i
  ]), {
    // ensures we check React component tree (not just DOM tree)
    onPointerDownCapture: () => a.current = !0
  };
}
function rs(e, t = globalThis == null ? void 0 : globalThis.document) {
  const n = Ge(e), r = d.useRef(!1);
  return d.useEffect(() => {
    const o = (i) => {
      i.target && !r.current && xr(Qi, n, { originalEvent: i }, {
        discrete: !1
      });
    };
    return t.addEventListener("focusin", o), () => t.removeEventListener("focusin", o);
  }, [t, n]), {
    onFocusCapture: () => r.current = !0,
    onBlurCapture: () => r.current = !1
  };
}
function Pn() {
  const e = new CustomEvent(Wt);
  document.dispatchEvent(e);
}
function xr(e, t, n, { discrete: r }) {
  const o = n.originalEvent.target, i = new CustomEvent(e, { bubbles: !1, cancelable: !0, detail: n });
  t && o.addEventListener(e, t, { once: !0 }), r ? Ki(o, i) : o.dispatchEvent(i);
}
var Pt = "focusScope.autoFocusOnMount", Ot = "focusScope.autoFocusOnUnmount", On = { bubbles: !1, cancelable: !0 }, os = "FocusScope", Cr = d.forwardRef((e, t) => {
  const {
    loop: n = !1,
    trapped: r = !1,
    onMountAutoFocus: o,
    onUnmountAutoFocus: i,
    ...s
  } = e, [a, c] = d.useState(null), u = Ge(o), l = Ge(i), f = d.useRef(null), p = ee(t, c), m = d.useRef({
    paused: !1,
    pause() {
      this.paused = !0;
    },
    resume() {
      this.paused = !1;
    }
  }).current;
  d.useEffect(() => {
    if (r) {
      let h = function(x) {
        if (m.paused || !a) return;
        const C = x.target;
        a.contains(C) ? f.current = C : ge(f.current, { select: !0 });
      }, b = function(x) {
        if (m.paused || !a) return;
        const C = x.relatedTarget;
        C !== null && (a.contains(C) || ge(f.current, { select: !0 }));
      }, y = function(x) {
        if (document.activeElement === document.body)
          for (const S of x)
            S.removedNodes.length > 0 && ge(a);
      };
      document.addEventListener("focusin", h), document.addEventListener("focusout", b);
      const v = new MutationObserver(y);
      return a && v.observe(a, { childList: !0, subtree: !0 }), () => {
        document.removeEventListener("focusin", h), document.removeEventListener("focusout", b), v.disconnect();
      };
    }
  }, [r, a, m.paused]), d.useEffect(() => {
    if (a) {
      Nn.add(m);
      const h = document.activeElement;
      if (!a.contains(h)) {
        const y = new CustomEvent(Pt, On);
        a.addEventListener(Pt, u), a.dispatchEvent(y), y.defaultPrevented || (is(us(Er(a)), { select: !0 }), document.activeElement === h && ge(a));
      }
      return () => {
        a.removeEventListener(Pt, u), setTimeout(() => {
          const y = new CustomEvent(Ot, On);
          a.addEventListener(Ot, l), a.dispatchEvent(y), y.defaultPrevented || ge(h ?? document.body, { select: !0 }), a.removeEventListener(Ot, l), Nn.remove(m);
        }, 0);
      };
    }
  }, [a, u, l, m]);
  const g = d.useCallback(
    (h) => {
      if (!n && !r || m.paused) return;
      const b = h.key === "Tab" && !h.altKey && !h.ctrlKey && !h.metaKey, y = document.activeElement;
      if (b && y) {
        const v = h.currentTarget, [x, C] = ss(v);
        x && C ? !h.shiftKey && y === C ? (h.preventDefault(), n && ge(x, { select: !0 })) : h.shiftKey && y === x && (h.preventDefault(), n && ge(C, { select: !0 })) : y === v && h.preventDefault();
      }
    },
    [n, r, m.paused]
  );
  return /* @__PURE__ */ w(K.div, { tabIndex: -1, ...s, ref: p, onKeyDown: g });
});
Cr.displayName = os;
function is(e, { select: t = !1 } = {}) {
  const n = document.activeElement;
  for (const r of e)
    if (ge(r, { select: t }), document.activeElement !== n) return;
}
function ss(e) {
  const t = Er(e), n = Tn(t, e), r = Tn(t.reverse(), e);
  return [n, r];
}
function Er(e) {
  const t = [], n = document.createTreeWalker(e, NodeFilter.SHOW_ELEMENT, {
    acceptNode: (r) => {
      const o = r.tagName === "INPUT" && r.type === "hidden";
      return r.disabled || r.hidden || o ? NodeFilter.FILTER_SKIP : r.tabIndex >= 0 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;
    }
  });
  for (; n.nextNode(); ) t.push(n.currentNode);
  return t;
}
function Tn(e, t) {
  for (const n of e)
    if (!as(n, { upTo: t })) return n;
}
function as(e, { upTo: t }) {
  if (getComputedStyle(e).visibility === "hidden") return !0;
  for (; e; ) {
    if (t !== void 0 && e === t) return !1;
    if (getComputedStyle(e).display === "none") return !0;
    e = e.parentElement;
  }
  return !1;
}
function cs(e) {
  return e instanceof HTMLInputElement && "select" in e;
}
function ge(e, { select: t = !1 } = {}) {
  if (e && e.focus) {
    const n = document.activeElement;
    e.focus({ preventScroll: !0 }), e !== n && cs(e) && t && e.select();
  }
}
var Nn = ls();
function ls() {
  let e = [];
  return {
    add(t) {
      const n = e[0];
      t !== n && (n == null || n.pause()), e = Dn(e, t), e.unshift(t);
    },
    remove(t) {
      var n;
      e = Dn(e, t), (n = e[0]) == null || n.resume();
    }
  };
}
function Dn(e, t) {
  const n = [...e], r = n.indexOf(t);
  return r !== -1 && n.splice(r, 1), n;
}
function us(e) {
  return e.filter((t) => t.tagName !== "A");
}
var ds = "Portal", Jt = d.forwardRef((e, t) => {
  var a;
  const { container: n, ...r } = e, [o, i] = d.useState(!1);
  se(() => i(!0), []);
  const s = n || o && ((a = globalThis == null ? void 0 : globalThis.document) == null ? void 0 : a.body);
  return s ? Ut.createPortal(/* @__PURE__ */ w(K.div, { ...r, ref: t }), s) : null;
});
Jt.displayName = ds;
function fs(e, t) {
  return d.useReducer((n, r) => t[n][r] ?? n, e);
}
var Fe = (e) => {
  const { present: t, children: n } = e, r = ms(t), o = typeof n == "function" ? n({ present: r.isPresent }) : d.Children.only(n), i = ps(r.ref, hs(o));
  return typeof n == "function" || r.isPresent ? d.cloneElement(o, { ref: i }) : null;
};
Fe.displayName = "Presence";
function ms(e) {
  const [t, n] = d.useState(), r = d.useRef(null), o = d.useRef(e), i = d.useRef("none"), s = e ? "mounted" : "unmounted", [a, c] = fs(s, {
    mounted: {
      UNMOUNT: "unmounted",
      ANIMATION_OUT: "unmountSuspended"
    },
    unmountSuspended: {
      MOUNT: "mounted",
      ANIMATION_END: "unmounted"
    },
    unmounted: {
      MOUNT: "mounted"
    }
  });
  return d.useEffect(() => {
    const u = tt(r.current);
    i.current = a === "mounted" ? u : "none";
  }, [a]), se(() => {
    const u = r.current, l = o.current;
    if (l !== e) {
      const p = i.current, m = tt(u);
      e ? c("MOUNT") : m === "none" || (u == null ? void 0 : u.display) === "none" ? c("UNMOUNT") : c(l && p !== m ? "ANIMATION_OUT" : "UNMOUNT"), o.current = e;
    }
  }, [e, c]), se(() => {
    if (t) {
      let u;
      const l = t.ownerDocument.defaultView ?? window, f = (m) => {
        const h = tt(r.current).includes(CSS.escape(m.animationName));
        if (m.target === t && h && (c("ANIMATION_END"), !o.current)) {
          const b = t.style.animationFillMode;
          t.style.animationFillMode = "forwards", u = l.setTimeout(() => {
            t.style.animationFillMode === "forwards" && (t.style.animationFillMode = b);
          });
        }
      }, p = (m) => {
        m.target === t && (i.current = tt(r.current));
      };
      return t.addEventListener("animationstart", p), t.addEventListener("animationcancel", f), t.addEventListener("animationend", f), () => {
        l.clearTimeout(u), t.removeEventListener("animationstart", p), t.removeEventListener("animationcancel", f), t.removeEventListener("animationend", f);
      };
    } else
      c("ANIMATION_END");
  }, [t, c]), {
    isPresent: ["mounted", "unmountSuspended"].includes(a),
    ref: d.useCallback((u) => {
      r.current = u ? getComputedStyle(u) : null, n(u);
    }, [])
  };
}
function Mn(e, t) {
  if (typeof e == "function")
    return e(t);
  e != null && (e.current = t);
}
function ps(...e) {
  const t = d.useRef(e);
  return t.current = e, d.useCallback((n) => {
    const r = t.current;
    let o = !1;
    const i = r.map((s) => {
      const a = Mn(s, n);
      return !o && typeof a == "function" && (o = !0), a;
    });
    if (o)
      return () => {
        for (let s = 0; s < i.length; s++) {
          const a = i[s];
          typeof a == "function" ? a() : Mn(r[s], null);
        }
      };
  }, []);
}
function tt(e) {
  return (e == null ? void 0 : e.animationName) || "none";
}
function hs(e) {
  var r, o;
  let t = (r = Object.getOwnPropertyDescriptor(e.props, "ref")) == null ? void 0 : r.get, n = t && "isReactWarning" in t && t.isReactWarning;
  return n ? e.ref : (t = (o = Object.getOwnPropertyDescriptor(e, "ref")) == null ? void 0 : o.get, n = t && "isReactWarning" in t && t.isReactWarning, n ? e.props.ref : e.props.ref || e.ref);
}
var nt = 0, re = null;
function gs() {
  d.useEffect(() => {
    re || (re = { start: Ln(), end: Ln() });
    const { start: e, end: t } = re;
    return document.body.firstElementChild !== e && document.body.insertAdjacentElement("afterbegin", e), document.body.lastElementChild !== t && document.body.insertAdjacentElement("beforeend", t), nt++, () => {
      nt === 1 && (re == null || re.start.remove(), re == null || re.end.remove(), re = null), nt = Math.max(0, nt - 1);
    };
  }, []);
}
function Ln() {
  const e = document.createElement("span");
  return e.setAttribute("data-radix-focus-guard", ""), e.tabIndex = 0, e.style.outline = "none", e.style.opacity = "0", e.style.position = "fixed", e.style.pointerEvents = "none", e;
}
var oe = function() {
  return oe = Object.assign || function(t) {
    for (var n, r = 1, o = arguments.length; r < o; r++) {
      n = arguments[r];
      for (var i in n) Object.prototype.hasOwnProperty.call(n, i) && (t[i] = n[i]);
    }
    return t;
  }, oe.apply(this, arguments);
};
function Sr(e, t) {
  var n = {};
  for (var r in e) Object.prototype.hasOwnProperty.call(e, r) && t.indexOf(r) < 0 && (n[r] = e[r]);
  if (e != null && typeof Object.getOwnPropertySymbols == "function")
    for (var o = 0, r = Object.getOwnPropertySymbols(e); o < r.length; o++)
      t.indexOf(r[o]) < 0 && Object.prototype.propertyIsEnumerable.call(e, r[o]) && (n[r[o]] = e[r[o]]);
  return n;
}
function bs(e, t, n) {
  if (n || arguments.length === 2) for (var r = 0, o = t.length, i; r < o; r++)
    (i || !(r in t)) && (i || (i = Array.prototype.slice.call(t, 0, r)), i[r] = t[r]);
  return e.concat(i || Array.prototype.slice.call(t));
}
var lt = "right-scroll-bar-position", ut = "width-before-scroll-bar", vs = "with-scroll-bars-hidden", ys = "--removed-body-scroll-bar-size";
function Tt(e, t) {
  return typeof e == "function" ? e(t) : e && (e.current = t), e;
}
function ws(e, t) {
  var n = To(function() {
    return {
      // value
      value: e,
      // last callback
      callback: t,
      // "memoized" public interface
      facade: {
        get current() {
          return n.value;
        },
        set current(r) {
          var o = n.value;
          o !== r && (n.value = r, n.callback(r, o));
        }
      }
    };
  })[0];
  return n.callback = t, n.facade;
}
var xs = typeof window < "u" ? d.useLayoutEffect : d.useEffect, In = /* @__PURE__ */ new WeakMap();
function Cs(e, t) {
  var n = ws(null, function(r) {
    return e.forEach(function(o) {
      return Tt(o, r);
    });
  });
  return xs(function() {
    var r = In.get(n);
    if (r) {
      var o = new Set(r), i = new Set(e), s = n.current;
      o.forEach(function(a) {
        i.has(a) || Tt(a, null);
      }), i.forEach(function(a) {
        o.has(a) || Tt(a, s);
      });
    }
    In.set(n, e);
  }, [e]), n;
}
function Es(e) {
  return e;
}
function Ss(e, t) {
  t === void 0 && (t = Es);
  var n = [], r = !1, o = {
    read: function() {
      if (r)
        throw new Error("Sidecar: could not `read` from an `assigned` medium. `read` could be used only with `useMedium`.");
      return n.length ? n[n.length - 1] : e;
    },
    useMedium: function(i) {
      var s = t(i, r);
      return n.push(s), function() {
        n = n.filter(function(a) {
          return a !== s;
        });
      };
    },
    assignSyncMedium: function(i) {
      for (r = !0; n.length; ) {
        var s = n;
        n = [], s.forEach(i);
      }
      n = {
        push: function(a) {
          return i(a);
        },
        filter: function() {
          return n;
        }
      };
    },
    assignMedium: function(i) {
      r = !0;
      var s = [];
      if (n.length) {
        var a = n;
        n = [], a.forEach(i), s = n;
      }
      var c = function() {
        var l = s;
        s = [], l.forEach(i);
      }, u = function() {
        return Promise.resolve().then(c);
      };
      u(), n = {
        push: function(l) {
          s.push(l), u();
        },
        filter: function(l) {
          return s = s.filter(l), n;
        }
      };
    }
  };
  return o;
}
function Rs(e) {
  e === void 0 && (e = {});
  var t = Ss(null);
  return t.options = oe({ async: !0, ssr: !1 }, e), t;
}
var Rr = function(e) {
  var t = e.sideCar, n = Sr(e, ["sideCar"]);
  if (!t)
    throw new Error("Sidecar: please provide `sideCar` property to import the right car");
  var r = t.read();
  if (!r)
    throw new Error("Sidecar medium not found");
  return d.createElement(r, oe({}, n));
};
Rr.isSideCarExport = !0;
function ks(e, t) {
  return e.useMedium(t), Rr;
}
var kr = Rs(), Nt = function() {
}, vt = d.forwardRef(function(e, t) {
  var n = d.useRef(null), r = d.useState({
    onScrollCapture: Nt,
    onWheelCapture: Nt,
    onTouchMoveCapture: Nt
  }), o = r[0], i = r[1], s = e.forwardProps, a = e.children, c = e.className, u = e.removeScrollBar, l = e.enabled, f = e.shards, p = e.sideCar, m = e.noRelative, g = e.noIsolation, h = e.inert, b = e.allowPinchZoom, y = e.as, v = y === void 0 ? "div" : y, x = e.gapMode, C = Sr(e, ["forwardProps", "children", "className", "removeScrollBar", "enabled", "shards", "sideCar", "noRelative", "noIsolation", "inert", "allowPinchZoom", "as", "gapMode"]), S = p, P = Cs([n, t]), O = oe(oe({}, C), o);
  return d.createElement(
    d.Fragment,
    null,
    l && d.createElement(S, { sideCar: kr, removeScrollBar: u, shards: f, noRelative: m, noIsolation: g, inert: h, setCallbacks: i, allowPinchZoom: !!b, lockRef: n, gapMode: x }),
    s ? d.cloneElement(d.Children.only(a), oe(oe({}, O), { ref: P })) : d.createElement(v, oe({}, O, { className: c, ref: P }), a)
  );
});
vt.defaultProps = {
  enabled: !0,
  removeScrollBar: !0,
  inert: !1
};
vt.classNames = {
  fullWidth: ut,
  zeroRight: lt
};
var As = function() {
  if (typeof __webpack_nonce__ < "u")
    return __webpack_nonce__;
};
function Ps() {
  if (!document)
    return null;
  var e = document.createElement("style");
  e.type = "text/css";
  var t = As();
  return t && e.setAttribute("nonce", t), e;
}
function Os(e, t) {
  e.styleSheet ? e.styleSheet.cssText = t : e.appendChild(document.createTextNode(t));
}
function Ts(e) {
  var t = document.head || document.getElementsByTagName("head")[0];
  t.appendChild(e);
}
var Ns = function() {
  var e = 0, t = null;
  return {
    add: function(n) {
      e == 0 && (t = Ps()) && (Os(t, n), Ts(t)), e++;
    },
    remove: function() {
      e--, !e && t && (t.parentNode && t.parentNode.removeChild(t), t = null);
    }
  };
}, Ds = function() {
  var e = Ns();
  return function(t, n) {
    d.useEffect(function() {
      return e.add(t), function() {
        e.remove();
      };
    }, [t && n]);
  };
}, Ar = function() {
  var e = Ds(), t = function(n) {
    var r = n.styles, o = n.dynamic;
    return e(r, o), null;
  };
  return t;
}, Ms = {
  left: 0,
  top: 0,
  right: 0,
  gap: 0
}, Dt = function(e) {
  return parseInt(e || "", 10) || 0;
}, Ls = function(e) {
  var t = window.getComputedStyle(document.body), n = t[e === "padding" ? "paddingLeft" : "marginLeft"], r = t[e === "padding" ? "paddingTop" : "marginTop"], o = t[e === "padding" ? "paddingRight" : "marginRight"];
  return [Dt(n), Dt(r), Dt(o)];
}, Is = function(e) {
  if (e === void 0 && (e = "margin"), typeof window > "u")
    return Ms;
  var t = Ls(e), n = document.documentElement.clientWidth, r = window.innerWidth;
  return {
    left: t[0],
    top: t[1],
    right: t[2],
    gap: Math.max(0, r - n + t[2] - t[0])
  };
}, _s = Ar(), De = "data-scroll-locked", Fs = function(e, t, n, r) {
  var o = e.left, i = e.top, s = e.right, a = e.gap;
  return n === void 0 && (n = "margin"), `
  .`.concat(vs, ` {
   overflow: hidden `).concat(r, `;
   padding-right: `).concat(a, "px ").concat(r, `;
  }
  body[`).concat(De, `] {
    overflow: hidden `).concat(r, `;
    overscroll-behavior: contain;
    `).concat([
    t && "position: relative ".concat(r, ";"),
    n === "margin" && `
    padding-left: `.concat(o, `px;
    padding-top: `).concat(i, `px;
    padding-right: `).concat(s, `px;
    margin-left:0;
    margin-top:0;
    margin-right: `).concat(a, "px ").concat(r, `;
    `),
    n === "padding" && "padding-right: ".concat(a, "px ").concat(r, ";")
  ].filter(Boolean).join(""), `
  }
  
  .`).concat(lt, ` {
    right: `).concat(a, "px ").concat(r, `;
  }
  
  .`).concat(ut, ` {
    margin-right: `).concat(a, "px ").concat(r, `;
  }
  
  .`).concat(lt, " .").concat(lt, ` {
    right: 0 `).concat(r, `;
  }
  
  .`).concat(ut, " .").concat(ut, ` {
    margin-right: 0 `).concat(r, `;
  }
  
  body[`).concat(De, `] {
    `).concat(ys, ": ").concat(a, `px;
  }
`);
}, _n = function() {
  var e = parseInt(document.body.getAttribute(De) || "0", 10);
  return isFinite(e) ? e : 0;
}, zs = function() {
  d.useEffect(function() {
    return document.body.setAttribute(De, (_n() + 1).toString()), function() {
      var e = _n() - 1;
      e <= 0 ? document.body.removeAttribute(De) : document.body.setAttribute(De, e.toString());
    };
  }, []);
}, Ws = function(e) {
  var t = e.noRelative, n = e.noImportant, r = e.gapMode, o = r === void 0 ? "margin" : r;
  zs();
  var i = d.useMemo(function() {
    return Is(o);
  }, [o]);
  return d.createElement(_s, { styles: Fs(i, !t, o, n ? "" : "!important") });
}, $t = !1;
if (typeof window < "u")
  try {
    var rt = Object.defineProperty({}, "passive", {
      get: function() {
        return $t = !0, !0;
      }
    });
    window.addEventListener("test", rt, rt), window.removeEventListener("test", rt, rt);
  } catch {
    $t = !1;
  }
var Oe = $t ? { passive: !1 } : !1, $s = function(e) {
  return e.tagName === "TEXTAREA";
}, Pr = function(e, t) {
  if (!(e instanceof Element))
    return !1;
  var n = window.getComputedStyle(e);
  return (
    // not-not-scrollable
    n[t] !== "hidden" && // contains scroll inside self
    !(n.overflowY === n.overflowX && !$s(e) && n[t] === "visible")
  );
}, Bs = function(e) {
  return Pr(e, "overflowY");
}, Vs = function(e) {
  return Pr(e, "overflowX");
}, Fn = function(e, t) {
  var n = t.ownerDocument, r = t;
  do {
    typeof ShadowRoot < "u" && r instanceof ShadowRoot && (r = r.host);
    var o = Or(e, r);
    if (o) {
      var i = Tr(e, r), s = i[1], a = i[2];
      if (s > a)
        return !0;
    }
    r = r.parentNode;
  } while (r && r !== n.body);
  return !1;
}, js = function(e) {
  var t = e.scrollTop, n = e.scrollHeight, r = e.clientHeight;
  return [
    t,
    n,
    r
  ];
}, Hs = function(e) {
  var t = e.scrollLeft, n = e.scrollWidth, r = e.clientWidth;
  return [
    t,
    n,
    r
  ];
}, Or = function(e, t) {
  return e === "v" ? Bs(t) : Vs(t);
}, Tr = function(e, t) {
  return e === "v" ? js(t) : Hs(t);
}, Gs = function(e, t) {
  return e === "h" && t === "rtl" ? -1 : 1;
}, Us = function(e, t, n, r, o) {
  var i = Gs(e, window.getComputedStyle(t).direction), s = i * r, a = n.target, c = t.contains(a), u = !1, l = s > 0, f = 0, p = 0;
  do {
    if (!a)
      break;
    var m = Tr(e, a), g = m[0], h = m[1], b = m[2], y = h - b - i * g;
    (g || y) && Or(e, a) && (f += y, p += g);
    var v = a.parentNode;
    a = v && v.nodeType === Node.DOCUMENT_FRAGMENT_NODE ? v.host : v;
  } while (
    // portaled content
    !c && a !== document.body || // self content
    c && (t.contains(a) || t === a)
  );
  return (l && Math.abs(f) < 1 || !l && Math.abs(p) < 1) && (u = !0), u;
}, ot = function(e) {
  return "changedTouches" in e ? [e.changedTouches[0].clientX, e.changedTouches[0].clientY] : [0, 0];
}, zn = function(e) {
  return [e.deltaX, e.deltaY];
}, Wn = function(e) {
  return e && "current" in e ? e.current : e;
}, Ys = function(e, t) {
  return e[0] === t[0] && e[1] === t[1];
}, Xs = function(e) {
  return `
  .block-interactivity-`.concat(e, ` {pointer-events: none;}
  .allow-interactivity-`).concat(e, ` {pointer-events: all;}
`);
}, Ks = 0, Te = [];
function Zs(e) {
  var t = d.useRef([]), n = d.useRef([0, 0]), r = d.useRef(), o = d.useState(Ks++)[0], i = d.useState(Ar)[0], s = d.useRef(e);
  d.useEffect(function() {
    s.current = e;
  }, [e]), d.useEffect(function() {
    if (e.inert) {
      document.body.classList.add("block-interactivity-".concat(o));
      var h = bs([e.lockRef.current], (e.shards || []).map(Wn), !0).filter(Boolean);
      return h.forEach(function(b) {
        return b.classList.add("allow-interactivity-".concat(o));
      }), function() {
        document.body.classList.remove("block-interactivity-".concat(o)), h.forEach(function(b) {
          return b.classList.remove("allow-interactivity-".concat(o));
        });
      };
    }
  }, [e.inert, e.lockRef.current, e.shards]);
  var a = d.useCallback(function(h, b) {
    if ("touches" in h && h.touches.length === 2 || h.type === "wheel" && h.ctrlKey)
      return !s.current.allowPinchZoom;
    var y = ot(h), v = n.current, x = "deltaX" in h ? h.deltaX : v[0] - y[0], C = "deltaY" in h ? h.deltaY : v[1] - y[1], S, P = h.target, O = Math.abs(x) > Math.abs(C) ? "h" : "v";
    if ("touches" in h && O === "h" && P.type === "range")
      return !1;
    var E = window.getSelection(), M = E && E.anchorNode, z = M ? M === P || M.contains(P) : !1;
    if (z)
      return !1;
    var T = Fn(O, P);
    if (!T)
      return !0;
    if (T ? S = O : (S = O === "v" ? "h" : "v", T = Fn(O, P)), !T)
      return !1;
    if (!r.current && "changedTouches" in h && (x || C) && (r.current = S), !S)
      return !0;
    var I = r.current || S;
    return Us(I, b, h, I === "h" ? x : C);
  }, []), c = d.useCallback(function(h) {
    var b = h;
    if (!(!Te.length || Te[Te.length - 1] !== i)) {
      var y = "deltaY" in b ? zn(b) : ot(b), v = t.current.filter(function(S) {
        return S.name === b.type && (S.target === b.target || b.target === S.shadowParent) && Ys(S.delta, y);
      })[0];
      if (v && v.should) {
        b.cancelable && b.preventDefault();
        return;
      }
      if (!v) {
        var x = (s.current.shards || []).map(Wn).filter(Boolean).filter(function(S) {
          return S.contains(b.target);
        }), C = x.length > 0 ? a(b, x[0]) : !s.current.noIsolation;
        C && b.cancelable && b.preventDefault();
      }
    }
  }, []), u = d.useCallback(function(h, b, y, v) {
    var x = { name: h, delta: b, target: y, should: v, shadowParent: qs(y) };
    t.current.push(x), setTimeout(function() {
      t.current = t.current.filter(function(C) {
        return C !== x;
      });
    }, 1);
  }, []), l = d.useCallback(function(h) {
    n.current = ot(h), r.current = void 0;
  }, []), f = d.useCallback(function(h) {
    u(h.type, zn(h), h.target, a(h, e.lockRef.current));
  }, []), p = d.useCallback(function(h) {
    u(h.type, ot(h), h.target, a(h, e.lockRef.current));
  }, []);
  d.useEffect(function() {
    return Te.push(i), e.setCallbacks({
      onScrollCapture: f,
      onWheelCapture: f,
      onTouchMoveCapture: p
    }), document.addEventListener("wheel", c, Oe), document.addEventListener("touchmove", c, Oe), document.addEventListener("touchstart", l, Oe), function() {
      Te = Te.filter(function(h) {
        return h !== i;
      }), document.removeEventListener("wheel", c, Oe), document.removeEventListener("touchmove", c, Oe), document.removeEventListener("touchstart", l, Oe);
    };
  }, []);
  var m = e.removeScrollBar, g = e.inert;
  return d.createElement(
    d.Fragment,
    null,
    g ? d.createElement(i, { styles: Xs(o) }) : null,
    m ? d.createElement(Ws, { noRelative: e.noRelative, gapMode: e.gapMode }) : null
  );
}
function qs(e) {
  for (var t = null; e !== null; )
    e instanceof ShadowRoot && (t = e.host, e = e.host), e = e.parentNode;
  return t;
}
const Qs = ks(kr, Zs);
var Nr = d.forwardRef(function(e, t) {
  return d.createElement(vt, oe({}, e, { ref: t, sideCar: Qs }));
});
Nr.classNames = vt.classNames;
var Js = function(e) {
  if (typeof document > "u")
    return null;
  var t = Array.isArray(e) ? e[0] : e;
  return t.ownerDocument.body;
}, Ne = /* @__PURE__ */ new WeakMap(), it = /* @__PURE__ */ new WeakMap(), st = {}, Mt = 0, Dr = function(e) {
  return e && (e.host || Dr(e.parentNode));
}, ea = function(e, t) {
  return t.map(function(n) {
    if (e.contains(n))
      return n;
    var r = Dr(n);
    return r && e.contains(r) ? r : (console.error("aria-hidden", n, "in not contained inside", e, ". Doing nothing"), null);
  }).filter(function(n) {
    return !!n;
  });
}, ta = function(e, t, n, r) {
  var o = ea(t, Array.isArray(e) ? e : [e]);
  st[n] || (st[n] = /* @__PURE__ */ new WeakMap());
  var i = st[n], s = [], a = /* @__PURE__ */ new Set(), c = new Set(o), u = function(f) {
    !f || a.has(f) || (a.add(f), u(f.parentNode));
  };
  o.forEach(u);
  var l = function(f) {
    !f || c.has(f) || Array.prototype.forEach.call(f.children, function(p) {
      if (a.has(p))
        l(p);
      else
        try {
          var m = p.getAttribute(r), g = m !== null && m !== "false", h = (Ne.get(p) || 0) + 1, b = (i.get(p) || 0) + 1;
          Ne.set(p, h), i.set(p, b), s.push(p), h === 1 && g && it.set(p, !0), b === 1 && p.setAttribute(n, "true"), g || p.setAttribute(r, "true");
        } catch (y) {
          console.error("aria-hidden: cannot operate on ", p, y);
        }
    });
  };
  return l(t), a.clear(), Mt++, function() {
    s.forEach(function(f) {
      var p = Ne.get(f) - 1, m = i.get(f) - 1;
      Ne.set(f, p), i.set(f, m), p || (it.has(f) || f.removeAttribute(r), it.delete(f)), m || f.removeAttribute(n);
    }), Mt--, Mt || (Ne = /* @__PURE__ */ new WeakMap(), Ne = /* @__PURE__ */ new WeakMap(), it = /* @__PURE__ */ new WeakMap(), st = {});
  };
}, na = function(e, t, n) {
  n === void 0 && (n = "data-aria-hidden");
  var r = Array.from(Array.isArray(e) ? e : [e]), o = Js(e);
  return o ? (r.push.apply(r, Array.from(o.querySelectorAll("[aria-live], script"))), ta(r, o, n, "aria-hidden")) : function() {
    return null;
  };
}, yt = "Dialog", [Mr] = Zt(yt), [ra, te] = Mr(yt), Lr = (e) => {
  const {
    __scopeDialog: t,
    children: n,
    open: r,
    defaultOpen: o,
    onOpenChange: i,
    modal: s = !0
  } = e, a = d.useRef(null), c = d.useRef(null), [u, l] = wr({
    prop: r,
    defaultProp: o ?? !1,
    onChange: i,
    caller: yt
  });
  return /* @__PURE__ */ w(
    ra,
    {
      scope: t,
      triggerRef: a,
      contentRef: c,
      contentId: ct(),
      titleId: ct(),
      descriptionId: ct(),
      open: u,
      onOpenChange: l,
      onOpenToggle: d.useCallback(() => l((f) => !f), [l]),
      modal: s,
      children: n
    }
  );
};
Lr.displayName = yt;
var Ir = "DialogTrigger", oa = d.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = te(Ir, n), i = ee(t, o.triggerRef);
    return /* @__PURE__ */ w(
      K.button,
      {
        type: "button",
        "aria-haspopup": "dialog",
        "aria-expanded": o.open,
        "aria-controls": o.open ? o.contentId : void 0,
        "data-state": tn(o.open),
        ...r,
        ref: i,
        onClick: X(e.onClick, o.onOpenToggle)
      }
    );
  }
);
oa.displayName = Ir;
var en = "DialogPortal", [ia, _r] = Mr(en, {
  forceMount: void 0
}), Fr = (e) => {
  const { __scopeDialog: t, forceMount: n, children: r, container: o } = e, i = te(en, t);
  return /* @__PURE__ */ w(ia, { scope: t, forceMount: n, children: d.Children.map(r, (s) => /* @__PURE__ */ w(Fe, { present: n || i.open, children: /* @__PURE__ */ w(Jt, { asChild: !0, container: o, children: s }) })) });
};
Fr.displayName = en;
var mt = "DialogOverlay", zr = d.forwardRef(
  (e, t) => {
    const n = _r(mt, e.__scopeDialog), { forceMount: r = n.forceMount, ...o } = e, i = te(mt, e.__scopeDialog);
    return i.modal ? /* @__PURE__ */ w(Fe, { present: r || i.open, children: /* @__PURE__ */ w(aa, { ...o, ref: t }) }) : null;
  }
);
zr.displayName = mt;
var sa = /* @__PURE__ */ Yt("DialogOverlay.RemoveScroll"), aa = d.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = te(mt, n), i = ts(), s = ee(t, i);
    return (
      // Make sure `Content` is scrollable even when it doesn't live inside `RemoveScroll`
      // ie. when `Overlay` and `Content` are siblings
      /* @__PURE__ */ w(Nr, { as: sa, allowPinchZoom: !0, shards: [o.contentRef], children: /* @__PURE__ */ w(
        K.div,
        {
          "data-state": tn(o.open),
          ...r,
          ref: s,
          style: { pointerEvents: "auto", ...r.style }
        }
      ) })
    );
  }
), Le = "DialogContent", Wr = d.forwardRef(
  (e, t) => {
    const n = _r(Le, e.__scopeDialog), { forceMount: r = n.forceMount, ...o } = e, i = te(Le, e.__scopeDialog);
    return /* @__PURE__ */ w(Fe, { present: r || i.open, children: i.modal ? /* @__PURE__ */ w(ca, { ...o, ref: t }) : /* @__PURE__ */ w(la, { ...o, ref: t }) });
  }
);
Wr.displayName = Le;
var ca = d.forwardRef(
  (e, t) => {
    const n = te(Le, e.__scopeDialog), r = d.useRef(null), o = ee(t, n.contentRef, r);
    return d.useEffect(() => {
      const i = r.current;
      if (i) return na(i);
    }, []), /* @__PURE__ */ w(
      $r,
      {
        ...e,
        ref: o,
        trapFocus: n.open,
        disableOutsidePointerEvents: n.open,
        onCloseAutoFocus: X(e.onCloseAutoFocus, (i) => {
          var s;
          i.preventDefault(), (s = n.triggerRef.current) == null || s.focus();
        }),
        onPointerDownOutside: X(e.onPointerDownOutside, (i) => {
          const s = i.detail.originalEvent, a = s.button === 0 && s.ctrlKey === !0;
          (s.button === 2 || a) && i.preventDefault();
        }),
        onFocusOutside: X(
          e.onFocusOutside,
          (i) => i.preventDefault()
        )
      }
    );
  }
), la = d.forwardRef(
  (e, t) => {
    const n = te(Le, e.__scopeDialog), r = d.useRef(!1), o = d.useRef(!1);
    return /* @__PURE__ */ w(
      $r,
      {
        ...e,
        ref: t,
        trapFocus: !1,
        disableOutsidePointerEvents: !1,
        onCloseAutoFocus: (i) => {
          var s, a;
          (s = e.onCloseAutoFocus) == null || s.call(e, i), i.defaultPrevented || (r.current || (a = n.triggerRef.current) == null || a.focus(), i.preventDefault()), r.current = !1, o.current = !1;
        },
        onInteractOutside: (i) => {
          var c, u;
          (c = e.onInteractOutside) == null || c.call(e, i), i.defaultPrevented || (r.current = !0, i.detail.originalEvent.type === "pointerdown" && (o.current = !0));
          const s = i.target;
          ((u = n.triggerRef.current) == null ? void 0 : u.contains(s)) && i.preventDefault(), i.detail.originalEvent.type === "focusin" && o.current && i.preventDefault();
        }
      }
    );
  }
), $r = d.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, trapFocus: r, onOpenAutoFocus: o, onCloseAutoFocus: i, ...s } = e, a = te(Le, n);
    return gs(), /* @__PURE__ */ w(Oo, { children: /* @__PURE__ */ w(
      Cr,
      {
        asChild: !0,
        loop: !0,
        trapped: r,
        onMountAutoFocus: o,
        onUnmountAutoFocus: i,
        children: /* @__PURE__ */ w(
          Qt,
          {
            role: "dialog",
            id: a.contentId,
            "aria-describedby": a.descriptionId,
            "aria-labelledby": a.titleId,
            "data-state": tn(a.open),
            ...s,
            ref: t,
            deferPointerDownOutside: !0,
            onDismiss: () => a.onOpenChange(!1)
          }
        )
      }
    ) });
  }
), Br = "DialogTitle", Vr = d.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = te(Br, n);
    return /* @__PURE__ */ w(K.h2, { id: o.titleId, ...r, ref: t });
  }
);
Vr.displayName = Br;
var jr = "DialogDescription", Hr = d.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = te(jr, n);
    return /* @__PURE__ */ w(K.p, { id: o.descriptionId, ...r, ref: t });
  }
);
Hr.displayName = jr;
var Gr = "DialogClose", Ur = d.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = te(Gr, n);
    return /* @__PURE__ */ w(
      K.button,
      {
        type: "button",
        ...r,
        ref: t,
        onClick: X(e.onClick, () => o.onOpenChange(!1))
      }
    );
  }
);
Ur.displayName = Gr;
function tn(e) {
  return e ? "open" : "closed";
}
function ua({ ...e }) {
  return /* @__PURE__ */ w(Lr, { ...e });
}
function da({ ...e }) {
  return /* @__PURE__ */ w(Fr, { ...e });
}
const fa = d.forwardRef(function({ className: t, ...n }, r) {
  return /* @__PURE__ */ w(
    zr,
    {
      ref: r,
      className: $("fixed inset-0 z-50 bg-black/50 animate-in fade-in-0", t),
      ...n
    }
  );
}), ma = d.forwardRef(function({ className: t, children: n, side: r = "right", ...o }, i) {
  return /* @__PURE__ */ Y(da, { children: [
    /* @__PURE__ */ w(fa, {}),
    /* @__PURE__ */ Y(
      Wr,
      {
        ref: i,
        className: $(
          "fixed z-50 flex flex-col gap-4 bg-nr-bg text-nr-fg shadow-lg transition ease-in-out animate-in",
          r === "right" && "inset-y-0 right-0 h-full w-3/4 border-l border-nr-border sm:max-w-sm",
          r === "left" && "inset-y-0 left-0 h-full w-3/4 border-r border-nr-border sm:max-w-sm",
          r === "top" && "inset-x-0 top-0 h-auto border-b border-nr-border",
          r === "bottom" && "inset-x-0 bottom-0 h-auto border-t border-nr-border",
          t
        ),
        ...o,
        children: [
          n,
          /* @__PURE__ */ Y(Ur, { className: "absolute right-4 top-4 rounded-sm opacity-70 transition-opacity hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-nr-accent/25", children: [
            /* @__PURE__ */ w(Uo, { className: "h-4 w-4" }),
            /* @__PURE__ */ w("span", { className: "sr-only", children: "Close" })
          ] })
        ]
      }
    )
  ] });
});
function pa({ className: e, ...t }) {
  return /* @__PURE__ */ w("div", { className: $("flex flex-col gap-1.5 p-4", e), ...t });
}
const ha = d.forwardRef(function({ className: t, ...n }, r) {
  return /* @__PURE__ */ w(Vr, { ref: r, className: $("font-semibold text-nr-fg", t), ...n });
}), ga = d.forwardRef(function({ className: t, ...n }, r) {
  return /* @__PURE__ */ w(Hr, { ref: r, className: $("text-sm text-nr-muted", t), ...n });
}), ba = ["top", "right", "bottom", "left"], be = Math.min, Z = Math.max, pt = Math.round, at = Math.floor, ae = (e) => ({
  x: e,
  y: e
}), va = {
  left: "right",
  right: "left",
  bottom: "top",
  top: "bottom"
};
function Bt(e, t, n) {
  return Z(e, be(t, n));
}
function ue(e, t) {
  return typeof e == "function" ? e(t) : e;
}
function de(e) {
  return e.split("-")[0];
}
function ze(e) {
  return e.split("-")[1];
}
function nn(e) {
  return e === "x" ? "y" : "x";
}
function rn(e) {
  return e === "y" ? "height" : "width";
}
function ie(e) {
  const t = e[0];
  return t === "t" || t === "b" ? "y" : "x";
}
function on(e) {
  return nn(ie(e));
}
function ya(e, t, n) {
  n === void 0 && (n = !1);
  const r = ze(e), o = on(e), i = rn(o);
  let s = o === "x" ? r === (n ? "end" : "start") ? "right" : "left" : r === "start" ? "bottom" : "top";
  return t.reference[i] > t.floating[i] && (s = ht(s)), [s, ht(s)];
}
function wa(e) {
  const t = ht(e);
  return [Vt(e), t, Vt(t)];
}
function Vt(e) {
  return e.includes("start") ? e.replace("start", "end") : e.replace("end", "start");
}
const $n = ["left", "right"], Bn = ["right", "left"], xa = ["top", "bottom"], Ca = ["bottom", "top"];
function Ea(e, t, n) {
  switch (e) {
    case "top":
    case "bottom":
      return n ? t ? Bn : $n : t ? $n : Bn;
    case "left":
    case "right":
      return t ? xa : Ca;
    default:
      return [];
  }
}
function Sa(e, t, n, r) {
  const o = ze(e);
  let i = Ea(de(e), n === "start", r);
  return o && (i = i.map((s) => s + "-" + o), t && (i = i.concat(i.map(Vt)))), i;
}
function ht(e) {
  const t = de(e);
  return va[t] + e.slice(t.length);
}
function Ra(e) {
  return {
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    ...e
  };
}
function Yr(e) {
  return typeof e != "number" ? Ra(e) : {
    top: e,
    right: e,
    bottom: e,
    left: e
  };
}
function gt(e) {
  const {
    x: t,
    y: n,
    width: r,
    height: o
  } = e;
  return {
    width: r,
    height: o,
    top: n,
    left: t,
    right: t + r,
    bottom: n + o,
    x: t,
    y: n
  };
}
function Vn(e, t, n) {
  let {
    reference: r,
    floating: o
  } = e;
  const i = ie(t), s = on(t), a = rn(s), c = de(t), u = i === "y", l = r.x + r.width / 2 - o.width / 2, f = r.y + r.height / 2 - o.height / 2, p = r[a] / 2 - o[a] / 2;
  let m;
  switch (c) {
    case "top":
      m = {
        x: l,
        y: r.y - o.height
      };
      break;
    case "bottom":
      m = {
        x: l,
        y: r.y + r.height
      };
      break;
    case "right":
      m = {
        x: r.x + r.width,
        y: f
      };
      break;
    case "left":
      m = {
        x: r.x - o.width,
        y: f
      };
      break;
    default:
      m = {
        x: r.x,
        y: r.y
      };
  }
  switch (ze(t)) {
    case "start":
      m[s] -= p * (n && u ? -1 : 1);
      break;
    case "end":
      m[s] += p * (n && u ? -1 : 1);
      break;
  }
  return m;
}
async function ka(e, t) {
  var n;
  t === void 0 && (t = {});
  const {
    x: r,
    y: o,
    platform: i,
    rects: s,
    elements: a,
    strategy: c
  } = e, {
    boundary: u = "clippingAncestors",
    rootBoundary: l = "viewport",
    elementContext: f = "floating",
    altBoundary: p = !1,
    padding: m = 0
  } = ue(t, e), g = Yr(m), b = a[p ? f === "floating" ? "reference" : "floating" : f], y = gt(await i.getClippingRect({
    element: (n = await (i.isElement == null ? void 0 : i.isElement(b))) == null || n ? b : b.contextElement || await (i.getDocumentElement == null ? void 0 : i.getDocumentElement(a.floating)),
    boundary: u,
    rootBoundary: l,
    strategy: c
  })), v = f === "floating" ? {
    x: r,
    y: o,
    width: s.floating.width,
    height: s.floating.height
  } : s.reference, x = await (i.getOffsetParent == null ? void 0 : i.getOffsetParent(a.floating)), C = await (i.isElement == null ? void 0 : i.isElement(x)) ? await (i.getScale == null ? void 0 : i.getScale(x)) || {
    x: 1,
    y: 1
  } : {
    x: 1,
    y: 1
  }, S = gt(i.convertOffsetParentRelativeRectToViewportRelativeRect ? await i.convertOffsetParentRelativeRectToViewportRelativeRect({
    elements: a,
    rect: v,
    offsetParent: x,
    strategy: c
  }) : v);
  return {
    top: (y.top - S.top + g.top) / C.y,
    bottom: (S.bottom - y.bottom + g.bottom) / C.y,
    left: (y.left - S.left + g.left) / C.x,
    right: (S.right - y.right + g.right) / C.x
  };
}
const Aa = 50, Pa = async (e, t, n) => {
  const {
    placement: r = "bottom",
    strategy: o = "absolute",
    middleware: i = [],
    platform: s
  } = n, a = s.detectOverflow ? s : {
    ...s,
    detectOverflow: ka
  }, c = await (s.isRTL == null ? void 0 : s.isRTL(t));
  let u = await s.getElementRects({
    reference: e,
    floating: t,
    strategy: o
  }), {
    x: l,
    y: f
  } = Vn(u, r, c), p = r, m = 0;
  const g = {};
  for (let h = 0; h < i.length; h++) {
    const b = i[h];
    if (!b)
      continue;
    const {
      name: y,
      fn: v
    } = b, {
      x,
      y: C,
      data: S,
      reset: P
    } = await v({
      x: l,
      y: f,
      initialPlacement: r,
      placement: p,
      strategy: o,
      middlewareData: g,
      rects: u,
      platform: a,
      elements: {
        reference: e,
        floating: t
      }
    });
    l = x ?? l, f = C ?? f, g[y] = {
      ...g[y],
      ...S
    }, P && m < Aa && (m++, typeof P == "object" && (P.placement && (p = P.placement), P.rects && (u = P.rects === !0 ? await s.getElementRects({
      reference: e,
      floating: t,
      strategy: o
    }) : P.rects), {
      x: l,
      y: f
    } = Vn(u, p, c)), h = -1);
  }
  return {
    x: l,
    y: f,
    placement: p,
    strategy: o,
    middlewareData: g
  };
}, Oa = (e) => ({
  name: "arrow",
  options: e,
  async fn(t) {
    const {
      x: n,
      y: r,
      placement: o,
      rects: i,
      platform: s,
      elements: a,
      middlewareData: c
    } = t, {
      element: u,
      padding: l = 0
    } = ue(e, t) || {};
    if (u == null)
      return {};
    const f = Yr(l), p = {
      x: n,
      y: r
    }, m = on(o), g = rn(m), h = await s.getDimensions(u), b = m === "y", y = b ? "top" : "left", v = b ? "bottom" : "right", x = b ? "clientHeight" : "clientWidth", C = i.reference[g] + i.reference[m] - p[m] - i.floating[g], S = p[m] - i.reference[m], P = await (s.getOffsetParent == null ? void 0 : s.getOffsetParent(u));
    let O = P ? P[x] : 0;
    (!O || !await (s.isElement == null ? void 0 : s.isElement(P))) && (O = a.floating[x] || i.floating[g]);
    const E = C / 2 - S / 2, M = O / 2 - h[g] / 2 - 1, z = be(f[y], M), T = be(f[v], M), I = z, W = O - h[g] - T, _ = O / 2 - h[g] / 2 + E, j = Bt(I, _, W), L = !c.arrow && ze(o) != null && _ !== j && i.reference[g] / 2 - (_ < I ? z : T) - h[g] / 2 < 0, F = L ? _ < I ? _ - I : _ - W : 0;
    return {
      [m]: p[m] + F,
      data: {
        [m]: j,
        centerOffset: _ - j - F,
        ...L && {
          alignmentOffset: F
        }
      },
      reset: L
    };
  }
}), Ta = function(e) {
  return e === void 0 && (e = {}), {
    name: "flip",
    options: e,
    async fn(t) {
      var n, r;
      const {
        placement: o,
        middlewareData: i,
        rects: s,
        initialPlacement: a,
        platform: c,
        elements: u
      } = t, {
        mainAxis: l = !0,
        crossAxis: f = !0,
        fallbackPlacements: p,
        fallbackStrategy: m = "bestFit",
        fallbackAxisSideDirection: g = "none",
        flipAlignment: h = !0,
        ...b
      } = ue(e, t);
      if ((n = i.arrow) != null && n.alignmentOffset)
        return {};
      const y = de(o), v = ie(a), x = de(a) === a, C = await (c.isRTL == null ? void 0 : c.isRTL(u.floating)), S = p || (x || !h ? [ht(a)] : wa(a)), P = g !== "none";
      !p && P && S.push(...Sa(a, h, g, C));
      const O = [a, ...S], E = await c.detectOverflow(t, b), M = [];
      let z = ((r = i.flip) == null ? void 0 : r.overflows) || [];
      if (l && M.push(E[y]), f) {
        const _ = ya(o, s, C);
        M.push(E[_[0]], E[_[1]]);
      }
      if (z = [...z, {
        placement: o,
        overflows: M
      }], !M.every((_) => _ <= 0)) {
        var T, I;
        const _ = (((T = i.flip) == null ? void 0 : T.index) || 0) + 1, j = O[_];
        if (j && (!(f === "alignment" ? v !== ie(j) : !1) || // We leave the current main axis only if every placement on that axis
        // overflows the main axis.
        z.every((D) => ie(D.placement) === v ? D.overflows[0] > 0 : !0)))
          return {
            data: {
              index: _,
              overflows: z
            },
            reset: {
              placement: j
            }
          };
        let L = (I = z.filter((F) => F.overflows[0] <= 0).sort((F, D) => F.overflows[1] - D.overflows[1])[0]) == null ? void 0 : I.placement;
        if (!L)
          switch (m) {
            case "bestFit": {
              var W;
              const F = (W = z.filter((D) => {
                if (P) {
                  const B = ie(D.placement);
                  return B === v || // Create a bias to the `y` side axis due to horizontal
                  // reading directions favoring greater width.
                  B === "y";
                }
                return !0;
              }).map((D) => [D.placement, D.overflows.filter((B) => B > 0).reduce((B, A) => B + A, 0)]).sort((D, B) => D[1] - B[1])[0]) == null ? void 0 : W[0];
              F && (L = F);
              break;
            }
            case "initialPlacement":
              L = a;
              break;
          }
        if (o !== L)
          return {
            reset: {
              placement: L
            }
          };
      }
      return {};
    }
  };
};
function jn(e, t) {
  return {
    top: e.top - t.height,
    right: e.right - t.width,
    bottom: e.bottom - t.height,
    left: e.left - t.width
  };
}
function Hn(e) {
  return ba.some((t) => e[t] >= 0);
}
const Na = function(e) {
  return e === void 0 && (e = {}), {
    name: "hide",
    options: e,
    async fn(t) {
      const {
        rects: n,
        platform: r
      } = t, {
        strategy: o = "referenceHidden",
        ...i
      } = ue(e, t);
      switch (o) {
        case "referenceHidden": {
          const s = await r.detectOverflow(t, {
            ...i,
            elementContext: "reference"
          }), a = jn(s, n.reference);
          return {
            data: {
              referenceHiddenOffsets: a,
              referenceHidden: Hn(a)
            }
          };
        }
        case "escaped": {
          const s = await r.detectOverflow(t, {
            ...i,
            altBoundary: !0
          }), a = jn(s, n.floating);
          return {
            data: {
              escapedOffsets: a,
              escaped: Hn(a)
            }
          };
        }
        default:
          return {};
      }
    }
  };
}, Xr = /* @__PURE__ */ new Set(["left", "top"]);
async function Da(e, t) {
  const {
    placement: n,
    platform: r,
    elements: o
  } = e, i = await (r.isRTL == null ? void 0 : r.isRTL(o.floating)), s = de(n), a = ze(n), c = ie(n) === "y", u = Xr.has(s) ? -1 : 1, l = i && c ? -1 : 1, f = ue(t, e);
  let {
    mainAxis: p,
    crossAxis: m,
    alignmentAxis: g
  } = typeof f == "number" ? {
    mainAxis: f,
    crossAxis: 0,
    alignmentAxis: null
  } : {
    mainAxis: f.mainAxis || 0,
    crossAxis: f.crossAxis || 0,
    alignmentAxis: f.alignmentAxis
  };
  return a && typeof g == "number" && (m = a === "end" ? g * -1 : g), c ? {
    x: m * l,
    y: p * u
  } : {
    x: p * u,
    y: m * l
  };
}
const Ma = function(e) {
  return e === void 0 && (e = 0), {
    name: "offset",
    options: e,
    async fn(t) {
      var n, r;
      const {
        x: o,
        y: i,
        placement: s,
        middlewareData: a
      } = t, c = await Da(t, e);
      return s === ((n = a.offset) == null ? void 0 : n.placement) && (r = a.arrow) != null && r.alignmentOffset ? {} : {
        x: o + c.x,
        y: i + c.y,
        data: {
          ...c,
          placement: s
        }
      };
    }
  };
}, La = function(e) {
  return e === void 0 && (e = {}), {
    name: "shift",
    options: e,
    async fn(t) {
      const {
        x: n,
        y: r,
        placement: o,
        platform: i
      } = t, {
        mainAxis: s = !0,
        crossAxis: a = !1,
        limiter: c = {
          fn: (y) => {
            let {
              x: v,
              y: x
            } = y;
            return {
              x: v,
              y: x
            };
          }
        },
        ...u
      } = ue(e, t), l = {
        x: n,
        y: r
      }, f = await i.detectOverflow(t, u), p = ie(de(o)), m = nn(p);
      let g = l[m], h = l[p];
      if (s) {
        const y = m === "y" ? "top" : "left", v = m === "y" ? "bottom" : "right", x = g + f[y], C = g - f[v];
        g = Bt(x, g, C);
      }
      if (a) {
        const y = p === "y" ? "top" : "left", v = p === "y" ? "bottom" : "right", x = h + f[y], C = h - f[v];
        h = Bt(x, h, C);
      }
      const b = c.fn({
        ...t,
        [m]: g,
        [p]: h
      });
      return {
        ...b,
        data: {
          x: b.x - n,
          y: b.y - r,
          enabled: {
            [m]: s,
            [p]: a
          }
        }
      };
    }
  };
}, Ia = function(e) {
  return e === void 0 && (e = {}), {
    options: e,
    fn(t) {
      const {
        x: n,
        y: r,
        placement: o,
        rects: i,
        middlewareData: s
      } = t, {
        offset: a = 0,
        mainAxis: c = !0,
        crossAxis: u = !0
      } = ue(e, t), l = {
        x: n,
        y: r
      }, f = ie(o), p = nn(f);
      let m = l[p], g = l[f];
      const h = ue(a, t), b = typeof h == "number" ? {
        mainAxis: h,
        crossAxis: 0
      } : {
        mainAxis: 0,
        crossAxis: 0,
        ...h
      };
      if (c) {
        const x = p === "y" ? "height" : "width", C = i.reference[p] - i.floating[x] + b.mainAxis, S = i.reference[p] + i.reference[x] - b.mainAxis;
        m < C ? m = C : m > S && (m = S);
      }
      if (u) {
        var y, v;
        const x = p === "y" ? "width" : "height", C = Xr.has(de(o)), S = i.reference[f] - i.floating[x] + (C && ((y = s.offset) == null ? void 0 : y[f]) || 0) + (C ? 0 : b.crossAxis), P = i.reference[f] + i.reference[x] + (C ? 0 : ((v = s.offset) == null ? void 0 : v[f]) || 0) - (C ? b.crossAxis : 0);
        g < S ? g = S : g > P && (g = P);
      }
      return {
        [p]: m,
        [f]: g
      };
    }
  };
}, _a = function(e) {
  return e === void 0 && (e = {}), {
    name: "size",
    options: e,
    async fn(t) {
      var n, r;
      const {
        placement: o,
        rects: i,
        platform: s,
        elements: a
      } = t, {
        apply: c = () => {
        },
        ...u
      } = ue(e, t), l = await s.detectOverflow(t, u), f = de(o), p = ze(o), m = ie(o) === "y", {
        width: g,
        height: h
      } = i.floating;
      let b, y;
      f === "top" || f === "bottom" ? (b = f, y = p === (await (s.isRTL == null ? void 0 : s.isRTL(a.floating)) ? "start" : "end") ? "left" : "right") : (y = f, b = p === "end" ? "top" : "bottom");
      const v = h - l.top - l.bottom, x = g - l.left - l.right, C = be(h - l[b], v), S = be(g - l[y], x), P = !t.middlewareData.shift;
      let O = C, E = S;
      if ((n = t.middlewareData.shift) != null && n.enabled.x && (E = x), (r = t.middlewareData.shift) != null && r.enabled.y && (O = v), P && !p) {
        const z = Z(l.left, 0), T = Z(l.right, 0), I = Z(l.top, 0), W = Z(l.bottom, 0);
        m ? E = g - 2 * (z !== 0 || T !== 0 ? z + T : Z(l.left, l.right)) : O = h - 2 * (I !== 0 || W !== 0 ? I + W : Z(l.top, l.bottom));
      }
      await c({
        ...t,
        availableWidth: E,
        availableHeight: O
      });
      const M = await s.getDimensions(a.floating);
      return g !== M.width || h !== M.height ? {
        reset: {
          rects: !0
        }
      } : {};
    }
  };
};
function wt() {
  return typeof window < "u";
}
function We(e) {
  return Kr(e) ? (e.nodeName || "").toLowerCase() : "#document";
}
function q(e) {
  var t;
  return (e == null || (t = e.ownerDocument) == null ? void 0 : t.defaultView) || window;
}
function ce(e) {
  var t;
  return (t = (Kr(e) ? e.ownerDocument : e.document) || window.document) == null ? void 0 : t.documentElement;
}
function Kr(e) {
  return wt() ? e instanceof Node || e instanceof q(e).Node : !1;
}
function Q(e) {
  return wt() ? e instanceof Element || e instanceof q(e).Element : !1;
}
function fe(e) {
  return wt() ? e instanceof HTMLElement || e instanceof q(e).HTMLElement : !1;
}
function Gn(e) {
  return !wt() || typeof ShadowRoot > "u" ? !1 : e instanceof ShadowRoot || e instanceof q(e).ShadowRoot;
}
function Xe(e) {
  const {
    overflow: t,
    overflowX: n,
    overflowY: r,
    display: o
  } = J(e);
  return /auto|scroll|overlay|hidden|clip/.test(t + r + n) && o !== "inline" && o !== "contents";
}
function Fa(e) {
  return /^(table|td|th)$/.test(We(e));
}
function xt(e) {
  try {
    if (e.matches(":popover-open"))
      return !0;
  } catch {
  }
  try {
    return e.matches(":modal");
  } catch {
    return !1;
  }
}
const za = /transform|translate|scale|rotate|perspective|filter/, Wa = /paint|layout|strict|content/, Se = (e) => !!e && e !== "none";
let Lt;
function sn(e) {
  const t = Q(e) ? J(e) : e;
  return Se(t.transform) || Se(t.translate) || Se(t.scale) || Se(t.rotate) || Se(t.perspective) || !an() && (Se(t.backdropFilter) || Se(t.filter)) || za.test(t.willChange || "") || Wa.test(t.contain || "");
}
function $a(e) {
  let t = ve(e);
  for (; fe(t) && !Ie(t); ) {
    if (sn(t))
      return t;
    if (xt(t))
      return null;
    t = ve(t);
  }
  return null;
}
function an() {
  return Lt == null && (Lt = typeof CSS < "u" && CSS.supports && CSS.supports("-webkit-backdrop-filter", "none")), Lt;
}
function Ie(e) {
  return /^(html|body|#document)$/.test(We(e));
}
function J(e) {
  return q(e).getComputedStyle(e);
}
function Ct(e) {
  return Q(e) ? {
    scrollLeft: e.scrollLeft,
    scrollTop: e.scrollTop
  } : {
    scrollLeft: e.scrollX,
    scrollTop: e.scrollY
  };
}
function ve(e) {
  if (We(e) === "html")
    return e;
  const t = (
    // Step into the shadow DOM of the parent of a slotted node.
    e.assignedSlot || // DOM Element detected.
    e.parentNode || // ShadowRoot detected.
    Gn(e) && e.host || // Fallback.
    ce(e)
  );
  return Gn(t) ? t.host : t;
}
function Zr(e) {
  const t = ve(e);
  return Ie(t) ? e.ownerDocument ? e.ownerDocument.body : e.body : fe(t) && Xe(t) ? t : Zr(t);
}
function Ue(e, t, n) {
  var r;
  t === void 0 && (t = []), n === void 0 && (n = !0);
  const o = Zr(e), i = o === ((r = e.ownerDocument) == null ? void 0 : r.body), s = q(o);
  if (i) {
    const a = jt(s);
    return t.concat(s, s.visualViewport || [], Xe(o) ? o : [], a && n ? Ue(a) : []);
  } else
    return t.concat(o, Ue(o, [], n));
}
function jt(e) {
  return e.parent && Object.getPrototypeOf(e.parent) ? e.frameElement : null;
}
function qr(e) {
  const t = J(e);
  let n = parseFloat(t.width) || 0, r = parseFloat(t.height) || 0;
  const o = fe(e), i = o ? e.offsetWidth : n, s = o ? e.offsetHeight : r, a = pt(n) !== i || pt(r) !== s;
  return a && (n = i, r = s), {
    width: n,
    height: r,
    $: a
  };
}
function cn(e) {
  return Q(e) ? e : e.contextElement;
}
function Me(e) {
  const t = cn(e);
  if (!fe(t))
    return ae(1);
  const n = t.getBoundingClientRect(), {
    width: r,
    height: o,
    $: i
  } = qr(t);
  let s = (i ? pt(n.width) : n.width) / r, a = (i ? pt(n.height) : n.height) / o;
  return (!s || !Number.isFinite(s)) && (s = 1), (!a || !Number.isFinite(a)) && (a = 1), {
    x: s,
    y: a
  };
}
const Ba = /* @__PURE__ */ ae(0);
function Qr(e) {
  const t = q(e);
  return !an() || !t.visualViewport ? Ba : {
    x: t.visualViewport.offsetLeft,
    y: t.visualViewport.offsetTop
  };
}
function Va(e, t, n) {
  return t === void 0 && (t = !1), !n || t && n !== q(e) ? !1 : t;
}
function Re(e, t, n, r) {
  t === void 0 && (t = !1), n === void 0 && (n = !1);
  const o = e.getBoundingClientRect(), i = cn(e);
  let s = ae(1);
  t && (r ? Q(r) && (s = Me(r)) : s = Me(e));
  const a = Va(i, n, r) ? Qr(i) : ae(0);
  let c = (o.left + a.x) / s.x, u = (o.top + a.y) / s.y, l = o.width / s.x, f = o.height / s.y;
  if (i) {
    const p = q(i), m = r && Q(r) ? q(r) : r;
    let g = p, h = jt(g);
    for (; h && r && m !== g; ) {
      const b = Me(h), y = h.getBoundingClientRect(), v = J(h), x = y.left + (h.clientLeft + parseFloat(v.paddingLeft)) * b.x, C = y.top + (h.clientTop + parseFloat(v.paddingTop)) * b.y;
      c *= b.x, u *= b.y, l *= b.x, f *= b.y, c += x, u += C, g = q(h), h = jt(g);
    }
  }
  return gt({
    width: l,
    height: f,
    x: c,
    y: u
  });
}
function Et(e, t) {
  const n = Ct(e).scrollLeft;
  return t ? t.left + n : Re(ce(e)).left + n;
}
function Jr(e, t) {
  const n = e.getBoundingClientRect(), r = n.left + t.scrollLeft - Et(e, n), o = n.top + t.scrollTop;
  return {
    x: r,
    y: o
  };
}
function ja(e) {
  let {
    elements: t,
    rect: n,
    offsetParent: r,
    strategy: o
  } = e;
  const i = o === "fixed", s = ce(r), a = t ? xt(t.floating) : !1;
  if (r === s || a && i)
    return n;
  let c = {
    scrollLeft: 0,
    scrollTop: 0
  }, u = ae(1);
  const l = ae(0), f = fe(r);
  if ((f || !f && !i) && ((We(r) !== "body" || Xe(s)) && (c = Ct(r)), f)) {
    const m = Re(r);
    u = Me(r), l.x = m.x + r.clientLeft, l.y = m.y + r.clientTop;
  }
  const p = s && !f && !i ? Jr(s, c) : ae(0);
  return {
    width: n.width * u.x,
    height: n.height * u.y,
    x: n.x * u.x - c.scrollLeft * u.x + l.x + p.x,
    y: n.y * u.y - c.scrollTop * u.y + l.y + p.y
  };
}
function Ha(e) {
  return Array.from(e.getClientRects());
}
function Ga(e) {
  const t = ce(e), n = Ct(e), r = e.ownerDocument.body, o = Z(t.scrollWidth, t.clientWidth, r.scrollWidth, r.clientWidth), i = Z(t.scrollHeight, t.clientHeight, r.scrollHeight, r.clientHeight);
  let s = -n.scrollLeft + Et(e);
  const a = -n.scrollTop;
  return J(r).direction === "rtl" && (s += Z(t.clientWidth, r.clientWidth) - o), {
    width: o,
    height: i,
    x: s,
    y: a
  };
}
const Un = 25;
function Ua(e, t) {
  const n = q(e), r = ce(e), o = n.visualViewport;
  let i = r.clientWidth, s = r.clientHeight, a = 0, c = 0;
  if (o) {
    i = o.width, s = o.height;
    const l = an();
    (!l || l && t === "fixed") && (a = o.offsetLeft, c = o.offsetTop);
  }
  const u = Et(r);
  if (u <= 0) {
    const l = r.ownerDocument, f = l.body, p = getComputedStyle(f), m = l.compatMode === "CSS1Compat" && parseFloat(p.marginLeft) + parseFloat(p.marginRight) || 0, g = Math.abs(r.clientWidth - f.clientWidth - m);
    g <= Un && (i -= g);
  } else u <= Un && (i += u);
  return {
    width: i,
    height: s,
    x: a,
    y: c
  };
}
function Ya(e, t) {
  const n = Re(e, !0, t === "fixed"), r = n.top + e.clientTop, o = n.left + e.clientLeft, i = fe(e) ? Me(e) : ae(1), s = e.clientWidth * i.x, a = e.clientHeight * i.y, c = o * i.x, u = r * i.y;
  return {
    width: s,
    height: a,
    x: c,
    y: u
  };
}
function Yn(e, t, n) {
  let r;
  if (t === "viewport")
    r = Ua(e, n);
  else if (t === "document")
    r = Ga(ce(e));
  else if (Q(t))
    r = Ya(t, n);
  else {
    const o = Qr(e);
    r = {
      x: t.x - o.x,
      y: t.y - o.y,
      width: t.width,
      height: t.height
    };
  }
  return gt(r);
}
function eo(e, t) {
  const n = ve(e);
  return n === t || !Q(n) || Ie(n) ? !1 : J(n).position === "fixed" || eo(n, t);
}
function Xa(e, t) {
  const n = t.get(e);
  if (n)
    return n;
  let r = Ue(e, [], !1).filter((a) => Q(a) && We(a) !== "body"), o = null;
  const i = J(e).position === "fixed";
  let s = i ? ve(e) : e;
  for (; Q(s) && !Ie(s); ) {
    const a = J(s), c = sn(s);
    !c && a.position === "fixed" && (o = null), (i ? !c && !o : !c && a.position === "static" && !!o && (o.position === "absolute" || o.position === "fixed") || Xe(s) && !c && eo(e, s)) ? r = r.filter((l) => l !== s) : o = a, s = ve(s);
  }
  return t.set(e, r), r;
}
function Ka(e) {
  let {
    element: t,
    boundary: n,
    rootBoundary: r,
    strategy: o
  } = e;
  const s = [...n === "clippingAncestors" ? xt(t) ? [] : Xa(t, this._c) : [].concat(n), r], a = Yn(t, s[0], o);
  let c = a.top, u = a.right, l = a.bottom, f = a.left;
  for (let p = 1; p < s.length; p++) {
    const m = Yn(t, s[p], o);
    c = Z(m.top, c), u = be(m.right, u), l = be(m.bottom, l), f = Z(m.left, f);
  }
  return {
    width: u - f,
    height: l - c,
    x: f,
    y: c
  };
}
function Za(e) {
  const {
    width: t,
    height: n
  } = qr(e);
  return {
    width: t,
    height: n
  };
}
function qa(e, t, n) {
  const r = fe(t), o = ce(t), i = n === "fixed", s = Re(e, !0, i, t);
  let a = {
    scrollLeft: 0,
    scrollTop: 0
  };
  const c = ae(0);
  function u() {
    c.x = Et(o);
  }
  if (r || !r && !i)
    if ((We(t) !== "body" || Xe(o)) && (a = Ct(t)), r) {
      const m = Re(t, !0, i, t);
      c.x = m.x + t.clientLeft, c.y = m.y + t.clientTop;
    } else o && u();
  i && !r && o && u();
  const l = o && !r && !i ? Jr(o, a) : ae(0), f = s.left + a.scrollLeft - c.x - l.x, p = s.top + a.scrollTop - c.y - l.y;
  return {
    x: f,
    y: p,
    width: s.width,
    height: s.height
  };
}
function It(e) {
  return J(e).position === "static";
}
function Xn(e, t) {
  if (!fe(e) || J(e).position === "fixed")
    return null;
  if (t)
    return t(e);
  let n = e.offsetParent;
  return ce(e) === n && (n = n.ownerDocument.body), n;
}
function to(e, t) {
  const n = q(e);
  if (xt(e))
    return n;
  if (!fe(e)) {
    let o = ve(e);
    for (; o && !Ie(o); ) {
      if (Q(o) && !It(o))
        return o;
      o = ve(o);
    }
    return n;
  }
  let r = Xn(e, t);
  for (; r && Fa(r) && It(r); )
    r = Xn(r, t);
  return r && Ie(r) && It(r) && !sn(r) ? n : r || $a(e) || n;
}
const Qa = async function(e) {
  const t = this.getOffsetParent || to, n = this.getDimensions, r = await n(e.floating);
  return {
    reference: qa(e.reference, await t(e.floating), e.strategy),
    floating: {
      x: 0,
      y: 0,
      width: r.width,
      height: r.height
    }
  };
};
function Ja(e) {
  return J(e).direction === "rtl";
}
const ec = {
  convertOffsetParentRelativeRectToViewportRelativeRect: ja,
  getDocumentElement: ce,
  getClippingRect: Ka,
  getOffsetParent: to,
  getElementRects: Qa,
  getClientRects: Ha,
  getDimensions: Za,
  getScale: Me,
  isElement: Q,
  isRTL: Ja
};
function no(e, t) {
  return e.x === t.x && e.y === t.y && e.width === t.width && e.height === t.height;
}
function tc(e, t) {
  let n = null, r;
  const o = ce(e);
  function i() {
    var a;
    clearTimeout(r), (a = n) == null || a.disconnect(), n = null;
  }
  function s(a, c) {
    a === void 0 && (a = !1), c === void 0 && (c = 1), i();
    const u = e.getBoundingClientRect(), {
      left: l,
      top: f,
      width: p,
      height: m
    } = u;
    if (a || t(), !p || !m)
      return;
    const g = at(f), h = at(o.clientWidth - (l + p)), b = at(o.clientHeight - (f + m)), y = at(l), x = {
      rootMargin: -g + "px " + -h + "px " + -b + "px " + -y + "px",
      threshold: Z(0, be(1, c)) || 1
    };
    let C = !0;
    function S(P) {
      const O = P[0].intersectionRatio;
      if (O !== c) {
        if (!C)
          return s();
        O ? s(!1, O) : r = setTimeout(() => {
          s(!1, 1e-7);
        }, 1e3);
      }
      O === 1 && !no(u, e.getBoundingClientRect()) && s(), C = !1;
    }
    try {
      n = new IntersectionObserver(S, {
        ...x,
        // Handle <iframe>s
        root: o.ownerDocument
      });
    } catch {
      n = new IntersectionObserver(S, x);
    }
    n.observe(e);
  }
  return s(!0), i;
}
function nc(e, t, n, r) {
  r === void 0 && (r = {});
  const {
    ancestorScroll: o = !0,
    ancestorResize: i = !0,
    elementResize: s = typeof ResizeObserver == "function",
    layoutShift: a = typeof IntersectionObserver == "function",
    animationFrame: c = !1
  } = r, u = cn(e), l = o || i ? [...u ? Ue(u) : [], ...t ? Ue(t) : []] : [];
  l.forEach((y) => {
    o && y.addEventListener("scroll", n, {
      passive: !0
    }), i && y.addEventListener("resize", n);
  });
  const f = u && a ? tc(u, n) : null;
  let p = -1, m = null;
  s && (m = new ResizeObserver((y) => {
    let [v] = y;
    v && v.target === u && m && t && (m.unobserve(t), cancelAnimationFrame(p), p = requestAnimationFrame(() => {
      var x;
      (x = m) == null || x.observe(t);
    })), n();
  }), u && !c && m.observe(u), t && m.observe(t));
  let g, h = c ? Re(e) : null;
  c && b();
  function b() {
    const y = Re(e);
    h && !no(h, y) && n(), h = y, g = requestAnimationFrame(b);
  }
  return n(), () => {
    var y;
    l.forEach((v) => {
      o && v.removeEventListener("scroll", n), i && v.removeEventListener("resize", n);
    }), f == null || f(), (y = m) == null || y.disconnect(), m = null, c && cancelAnimationFrame(g);
  };
}
const rc = Ma, oc = La, ic = Ta, sc = _a, ac = Na, Kn = Oa, cc = Ia, lc = (e, t, n) => {
  const r = /* @__PURE__ */ new Map(), o = {
    platform: ec,
    ...n
  }, i = {
    ...o.platform,
    _c: r
  };
  return Pa(e, t, {
    ...o,
    platform: i
  });
};
var uc = typeof document < "u", dc = function() {
}, dt = uc ? No : dc;
function bt(e, t) {
  if (e === t)
    return !0;
  if (typeof e != typeof t)
    return !1;
  if (typeof e == "function" && e.toString() === t.toString())
    return !0;
  let n, r, o;
  if (e && t && typeof e == "object") {
    if (Array.isArray(e)) {
      if (n = e.length, n !== t.length) return !1;
      for (r = n; r-- !== 0; )
        if (!bt(e[r], t[r]))
          return !1;
      return !0;
    }
    if (o = Object.keys(e), n = o.length, n !== Object.keys(t).length)
      return !1;
    for (r = n; r-- !== 0; )
      if (!{}.hasOwnProperty.call(t, o[r]))
        return !1;
    for (r = n; r-- !== 0; ) {
      const i = o[r];
      if (!(i === "_owner" && e.$$typeof) && !bt(e[i], t[i]))
        return !1;
    }
    return !0;
  }
  return e !== e && t !== t;
}
function ro(e) {
  return typeof window > "u" ? 1 : (e.ownerDocument.defaultView || window).devicePixelRatio || 1;
}
function Zn(e, t) {
  const n = ro(e);
  return Math.round(t * n) / n;
}
function _t(e) {
  const t = d.useRef(e);
  return dt(() => {
    t.current = e;
  }), t;
}
function fc(e) {
  e === void 0 && (e = {});
  const {
    placement: t = "bottom",
    strategy: n = "absolute",
    middleware: r = [],
    platform: o,
    elements: {
      reference: i,
      floating: s
    } = {},
    transform: a = !0,
    whileElementsMounted: c,
    open: u
  } = e, [l, f] = d.useState({
    x: 0,
    y: 0,
    strategy: n,
    placement: t,
    middlewareData: {},
    isPositioned: !1
  }), [p, m] = d.useState(r);
  bt(p, r) || m(r);
  const [g, h] = d.useState(null), [b, y] = d.useState(null), v = d.useCallback((D) => {
    D !== P.current && (P.current = D, h(D));
  }, []), x = d.useCallback((D) => {
    D !== O.current && (O.current = D, y(D));
  }, []), C = i || g, S = s || b, P = d.useRef(null), O = d.useRef(null), E = d.useRef(l), M = c != null, z = _t(c), T = _t(o), I = _t(u), W = d.useCallback(() => {
    if (!P.current || !O.current)
      return;
    const D = {
      placement: t,
      strategy: n,
      middleware: p
    };
    T.current && (D.platform = T.current), lc(P.current, O.current, D).then((B) => {
      const A = {
        ...B,
        // The floating element's position may be recomputed while it's closed
        // but still mounted (such as when transitioning out). To ensure
        // `isPositioned` will be `false` initially on the next open, avoid
        // setting it to `true` when `open === false` (must be specified).
        isPositioned: I.current !== !1
      };
      _.current && !bt(E.current, A) && (E.current = A, Ut.flushSync(() => {
        f(A);
      }));
    });
  }, [p, t, n, T, I]);
  dt(() => {
    u === !1 && E.current.isPositioned && (E.current.isPositioned = !1, f((D) => ({
      ...D,
      isPositioned: !1
    })));
  }, [u]);
  const _ = d.useRef(!1);
  dt(() => (_.current = !0, () => {
    _.current = !1;
  }), []), dt(() => {
    if (C && (P.current = C), S && (O.current = S), C && S) {
      if (z.current)
        return z.current(C, S, W);
      W();
    }
  }, [C, S, W, z, M]);
  const j = d.useMemo(() => ({
    reference: P,
    floating: O,
    setReference: v,
    setFloating: x
  }), [v, x]), L = d.useMemo(() => ({
    reference: C,
    floating: S
  }), [C, S]), F = d.useMemo(() => {
    const D = {
      position: n,
      left: 0,
      top: 0
    };
    if (!L.floating)
      return D;
    const B = Zn(L.floating, l.x), A = Zn(L.floating, l.y);
    return a ? {
      ...D,
      transform: "translate(" + B + "px, " + A + "px)",
      ...ro(L.floating) >= 1.5 && {
        willChange: "transform"
      }
    } : {
      position: n,
      left: B,
      top: A
    };
  }, [n, a, L.floating, l.x, l.y]);
  return d.useMemo(() => ({
    ...l,
    update: W,
    refs: j,
    elements: L,
    floatingStyles: F
  }), [l, W, j, L, F]);
}
const mc = (e) => {
  function t(n) {
    return {}.hasOwnProperty.call(n, "current");
  }
  return {
    name: "arrow",
    options: e,
    fn(n) {
      const {
        element: r,
        padding: o
      } = typeof e == "function" ? e(n) : e;
      return r && t(r) ? r.current != null ? Kn({
        element: r.current,
        padding: o
      }).fn(n) : {} : r ? Kn({
        element: r,
        padding: o
      }).fn(n) : {};
    }
  };
}, pc = (e, t) => {
  const n = rc(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, hc = (e, t) => {
  const n = oc(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, gc = (e, t) => ({
  fn: cc(e).fn,
  options: [e, t]
}), bc = (e, t) => {
  const n = ic(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, vc = (e, t) => {
  const n = sc(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, yc = (e, t) => {
  const n = ac(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, wc = (e, t) => {
  const n = mc(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
};
var xc = "Arrow", oo = d.forwardRef((e, t) => {
  const { children: n, width: r = 10, height: o = 5, ...i } = e;
  return /* @__PURE__ */ w(
    K.svg,
    {
      ...i,
      ref: t,
      width: r,
      height: o,
      viewBox: "0 0 30 10",
      preserveAspectRatio: "none",
      children: e.asChild ? n : /* @__PURE__ */ w("polygon", { points: "0,0 30,0 15,10" })
    }
  );
});
oo.displayName = xc;
var Cc = oo;
function Ec(e) {
  const [t, n] = d.useState(void 0);
  return se(() => {
    if (e) {
      n({ width: e.offsetWidth, height: e.offsetHeight });
      const r = new ResizeObserver((o) => {
        if (!Array.isArray(o) || !o.length)
          return;
        const i = o[0];
        let s, a;
        if ("borderBoxSize" in i) {
          const c = i.borderBoxSize, u = Array.isArray(c) ? c[0] : c;
          s = u.inlineSize, a = u.blockSize;
        } else
          s = e.offsetWidth, a = e.offsetHeight;
        n({ width: s, height: a });
      });
      return r.observe(e, { box: "border-box" }), () => r.unobserve(e);
    } else
      n(void 0);
  }, [e]), t;
}
var ln = "Popper", [io, so] = Zt(ln), [Sc, ao] = io(ln), co = (e) => {
  const { __scopePopper: t, children: n } = e, [r, o] = d.useState(null), [i, s] = d.useState(void 0);
  return /* @__PURE__ */ w(
    Sc,
    {
      scope: t,
      anchor: r,
      onAnchorChange: o,
      placementState: i,
      setPlacementState: s,
      children: n
    }
  );
};
co.displayName = ln;
var lo = "PopperAnchor", uo = d.forwardRef(
  (e, t) => {
    const { __scopePopper: n, virtualRef: r, ...o } = e, i = ao(lo, n), s = d.useRef(null), a = i.onAnchorChange, c = d.useCallback(
      (g) => {
        s.current = g, g && a(g);
      },
      [a]
    ), u = ee(t, c), l = d.useRef(null);
    d.useEffect(() => {
      if (!r)
        return;
      const g = l.current;
      l.current = r.current, g !== l.current && a(l.current);
    });
    const f = i.placementState && dn(i.placementState), p = f == null ? void 0 : f[0], m = f == null ? void 0 : f[1];
    return r ? null : /* @__PURE__ */ w(
      K.div,
      {
        "data-radix-popper-side": p,
        "data-radix-popper-align": m,
        ...o,
        ref: u
      }
    );
  }
);
uo.displayName = lo;
var un = "PopperContent", [Rc, kc] = io(un), fo = d.forwardRef(
  (e, t) => {
    var Ve, V, je, xe, Ce, Pe;
    const {
      __scopePopper: n,
      side: r = "bottom",
      sideOffset: o = 0,
      align: i = "center",
      alignOffset: s = 0,
      arrowPadding: a = 0,
      avoidCollisions: c = !0,
      collisionBoundary: u = [],
      collisionPadding: l = 0,
      sticky: f = "partial",
      hideWhenDetached: p = !1,
      updatePositionStrategy: m = "optimized",
      onPlaced: g,
      ...h
    } = e, b = ao(un, n), [y, v] = d.useState(null), x = ee(t, v), [C, S] = d.useState(null), P = Ec(C), O = (P == null ? void 0 : P.width) ?? 0, E = (P == null ? void 0 : P.height) ?? 0, M = r + (i !== "center" ? "-" + i : ""), z = typeof l == "number" ? l : { top: 0, right: 0, bottom: 0, left: 0, ...l }, T = Array.isArray(u) ? u : [u], I = T.length > 0, W = {
      padding: z,
      boundary: T.filter(Pc),
      // with `strategy: 'fixed'`, this is the only way to get it to respect boundaries
      altBoundary: I
    }, { refs: _, floatingStyles: j, placement: L, isPositioned: F, middlewareData: D } = fc({
      // default to `fixed` strategy so users don't have to pick and we also avoid focus scroll issues
      strategy: "fixed",
      placement: M,
      whileElementsMounted: (...pe) => nc(...pe, {
        animationFrame: m === "always"
      }),
      elements: {
        reference: b.anchor
      },
      middleware: [
        pc({ mainAxis: o + E, alignmentAxis: s }),
        c && hc({
          mainAxis: !0,
          crossAxis: !1,
          limiter: f === "partial" ? gc() : void 0,
          ...W
        }),
        c && bc({ ...W }),
        vc({
          ...W,
          apply: ({ elements: pe, rects: pn, availableWidth: Ro, availableHeight: ko }) => {
            const { width: Ao, height: Po } = pn.reference, qe = pe.floating.style;
            qe.setProperty("--radix-popper-available-width", `${Ro}px`), qe.setProperty("--radix-popper-available-height", `${ko}px`), qe.setProperty("--radix-popper-anchor-width", `${Ao}px`), qe.setProperty("--radix-popper-anchor-height", `${Po}px`);
          }
        }),
        C && wc({ element: C, padding: a }),
        Oc({ arrowWidth: O, arrowHeight: E }),
        p && yc({
          strategy: "referenceHidden",
          ...W,
          // `hide` detects whether the anchor (reference) is clipped, so when
          // no explicit `collisionBoundary` is set we fall back to Floating
          // UI's default clipping ancestors (e.g. a scrollable menu). This
          // lets an occluded submenu hide once its anchor scrolls out of view
          // (#3237). The collision/size middlewares deliberately keep the
          // viewport-based default to avoid clamping content rendered inside
          // transformed or overflow-clipping portal containers.
          boundary: I ? W.boundary : void 0
        })
      ]
    }), B = b.setPlacementState;
    se(() => (B(L), () => {
      B(void 0);
    }), [L, B]);
    const [A, $e] = dn(L), we = Ge(g);
    se(() => {
      F && (we == null || we());
    }, [F, we]);
    const Ze = (Ve = D.arrow) == null ? void 0 : Ve.x, Be = (V = D.arrow) == null ? void 0 : V.y, G = ((je = D.arrow) == null ? void 0 : je.centerOffset) !== 0, [U, Ae] = d.useState();
    return se(() => {
      y && Ae(window.getComputedStyle(y).zIndex);
    }, [y]), /* @__PURE__ */ w(
      "div",
      {
        ref: _.setFloating,
        "data-radix-popper-content-wrapper": "",
        style: {
          ...j,
          transform: F ? j.transform : "translate(0, -200%)",
          // keep off the page when measuring
          minWidth: "max-content",
          zIndex: U,
          "--radix-popper-transform-origin": [
            (xe = D.transformOrigin) == null ? void 0 : xe.x,
            (Ce = D.transformOrigin) == null ? void 0 : Ce.y
          ].join(" "),
          // hide the content if using the hide middleware and should be hidden
          // set visibility to hidden and disable pointer events so the UI behaves
          // as if the PopperContent isn't there at all
          ...((Pe = D.hide) == null ? void 0 : Pe.referenceHidden) && {
            visibility: "hidden",
            pointerEvents: "none"
          }
        },
        dir: e.dir,
        children: /* @__PURE__ */ w(
          Rc,
          {
            scope: n,
            placedSide: A,
            placedAlign: $e,
            onArrowChange: S,
            arrowX: Ze,
            arrowY: Be,
            shouldHideArrow: G,
            children: /* @__PURE__ */ w(
              K.div,
              {
                "data-side": A,
                "data-align": $e,
                ...h,
                ref: x,
                style: {
                  ...h.style,
                  // if the PopperContent hasn't been placed yet (not all measurements done)
                  // we prevent animations so that users's animation don't kick in too early referring wrong sides
                  animation: F ? void 0 : "none"
                }
              }
            )
          }
        )
      }
    );
  }
);
fo.displayName = un;
var mo = "PopperArrow", Ac = {
  top: "bottom",
  right: "left",
  bottom: "top",
  left: "right"
}, po = d.forwardRef(function(t, n) {
  const { __scopePopper: r, ...o } = t, i = kc(mo, r), s = Ac[i.placedSide];
  return (
    // we have to use an extra wrapper because `ResizeObserver` (used by `useSize`)
    // doesn't report size as we'd expect on SVG elements.
    // it reports their bounding box which is effectively the largest path inside the SVG.
    /* @__PURE__ */ w(
      "span",
      {
        ref: i.onArrowChange,
        style: {
          position: "absolute",
          left: i.arrowX,
          top: i.arrowY,
          [s]: 0,
          transformOrigin: {
            top: "",
            right: "0 0",
            bottom: "center 0",
            left: "100% 0"
          }[i.placedSide],
          transform: {
            top: "translateY(100%)",
            right: "translateY(50%) rotate(90deg) translateX(-50%)",
            bottom: "rotate(180deg)",
            left: "translateY(50%) rotate(-90deg) translateX(50%)"
          }[i.placedSide],
          visibility: i.shouldHideArrow ? "hidden" : void 0
        },
        children: /* @__PURE__ */ w(
          Cc,
          {
            ...o,
            ref: n,
            style: {
              ...o.style,
              // ensures the element can be measured correctly (mostly for if SVG)
              display: "block"
            }
          }
        )
      }
    )
  );
});
po.displayName = mo;
function Pc(e) {
  return e !== null;
}
var Oc = (e) => ({
  name: "transformOrigin",
  options: e,
  fn(t) {
    var b, y, v;
    const { placement: n, rects: r, middlewareData: o } = t, s = ((b = o.arrow) == null ? void 0 : b.centerOffset) !== 0, a = s ? 0 : e.arrowWidth, c = s ? 0 : e.arrowHeight, [u, l] = dn(n), f = { start: "0%", center: "50%", end: "100%" }[l], p = (((y = o.arrow) == null ? void 0 : y.x) ?? 0) + a / 2, m = (((v = o.arrow) == null ? void 0 : v.y) ?? 0) + c / 2;
    let g = "", h = "";
    return u === "bottom" ? (g = s ? f : `${p}px`, h = `${-c}px`) : u === "top" ? (g = s ? f : `${p}px`, h = `${r.floating.height + c}px`) : u === "right" ? (g = `${-c}px`, h = s ? f : `${m}px`) : u === "left" && (g = `${r.floating.width + c}px`, h = s ? f : `${m}px`), { data: { x: g, y: h } };
  }
});
function dn(e) {
  const [t, n = "center"] = e.split("-");
  return [t, n];
}
var Tc = co, Nc = uo, Dc = fo, Mc = po, Lc = Object.freeze({
  // See: https://github.com/twbs/bootstrap/blob/main/scss/mixins/_visually-hidden.scss
  position: "absolute",
  border: 0,
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: "hidden",
  clip: "rect(0, 0, 0, 0)",
  whiteSpace: "nowrap",
  wordWrap: "normal"
}), Ic = "VisuallyHidden", ho = d.forwardRef(
  (e, t) => /* @__PURE__ */ w(
    K.span,
    {
      ...e,
      ref: t,
      style: { ...Lc, ...e.style }
    }
  )
);
ho.displayName = Ic;
var _c = ho, [St] = Zt("Tooltip", [
  so
]), Rt = so(), go = "TooltipProvider", Fc = 700, Ht = "tooltip.open", [zc, fn] = St(go), bo = (e) => {
  const {
    __scopeTooltip: t,
    delayDuration: n = Fc,
    skipDelayDuration: r = 300,
    disableHoverableContent: o = !1,
    children: i
  } = e, s = d.useRef(!0), a = d.useRef(!1), c = d.useRef(0);
  return d.useEffect(() => {
    const u = c.current;
    return () => window.clearTimeout(u);
  }, []), /* @__PURE__ */ w(
    zc,
    {
      scope: t,
      isOpenDelayedRef: s,
      delayDuration: n,
      onOpen: d.useCallback(() => {
        r <= 0 || (window.clearTimeout(c.current), s.current = !1);
      }, [r]),
      onClose: d.useCallback(() => {
        r <= 0 || (window.clearTimeout(c.current), c.current = window.setTimeout(
          () => s.current = !0,
          r
        ));
      }, [r]),
      isPointerInTransitRef: a,
      onPointerInTransitChange: d.useCallback((u) => {
        a.current = u;
      }, []),
      disableHoverableContent: o,
      children: i
    }
  );
};
bo.displayName = go;
var Ye = "Tooltip", [Wc, Ke] = St(Ye), vo = (e) => {
  const {
    __scopeTooltip: t,
    children: n,
    open: r,
    defaultOpen: o,
    onOpenChange: i,
    disableHoverableContent: s,
    delayDuration: a
  } = e, c = fn(Ye, e.__scopeTooltip), u = Rt(t), [l, f] = d.useState(null), p = ct(), m = d.useRef(0), g = s ?? c.disableHoverableContent, h = a ?? c.delayDuration, b = d.useRef(!1), [y, v] = wr({
    prop: r,
    defaultProp: o ?? !1,
    onChange: (O) => {
      O ? (c.onOpen(), document.dispatchEvent(new CustomEvent(Ht))) : c.onClose(), i == null || i(O);
    },
    caller: Ye
  }), x = d.useMemo(() => y ? b.current ? "delayed-open" : "instant-open" : "closed", [y]), C = d.useCallback(() => {
    window.clearTimeout(m.current), m.current = 0, b.current = !1, v(!0);
  }, [v]), S = d.useCallback(() => {
    window.clearTimeout(m.current), m.current = 0, v(!1);
  }, [v]), P = d.useCallback(() => {
    window.clearTimeout(m.current), m.current = window.setTimeout(() => {
      b.current = !0, v(!0), m.current = 0;
    }, h);
  }, [h, v]);
  return d.useEffect(() => () => {
    m.current && (window.clearTimeout(m.current), m.current = 0);
  }, []), /* @__PURE__ */ w(Tc, { ...u, children: /* @__PURE__ */ w(
    Wc,
    {
      scope: t,
      contentId: p,
      open: y,
      stateAttribute: x,
      trigger: l,
      onTriggerChange: f,
      onTriggerEnter: d.useCallback(() => {
        c.isOpenDelayedRef.current ? P() : C();
      }, [c.isOpenDelayedRef, P, C]),
      onTriggerLeave: d.useCallback(() => {
        g ? S() : (window.clearTimeout(m.current), m.current = 0);
      }, [S, g]),
      onOpen: C,
      onClose: S,
      disableHoverableContent: g,
      children: n
    }
  ) });
};
vo.displayName = Ye;
var Gt = "TooltipTrigger", yo = d.forwardRef(
  (e, t) => {
    const { __scopeTooltip: n, ...r } = e, o = Ke(Gt, n), i = fn(Gt, n), s = Rt(n), a = d.useRef(null), c = ee(t, a, o.onTriggerChange), u = d.useRef(!1), l = d.useRef(!1), f = d.useCallback(() => u.current = !1, []);
    return d.useEffect(() => () => document.removeEventListener("pointerup", f), [f]), /* @__PURE__ */ w(Nc, { asChild: !0, ...s, children: /* @__PURE__ */ w(
      K.button,
      {
        "aria-describedby": o.open ? o.contentId : void 0,
        "data-state": o.stateAttribute,
        ...r,
        ref: c,
        onPointerMove: X(e.onPointerMove, (p) => {
          p.pointerType !== "touch" && !l.current && !i.isPointerInTransitRef.current && (o.onTriggerEnter(), l.current = !0);
        }),
        onPointerLeave: X(e.onPointerLeave, () => {
          o.onTriggerLeave(), l.current = !1;
        }),
        onPointerDown: X(e.onPointerDown, () => {
          o.open && o.onClose(), u.current = !0, document.addEventListener("pointerup", f, { once: !0 });
        }),
        onFocus: X(e.onFocus, () => {
          u.current || o.onOpen();
        }),
        onBlur: X(e.onBlur, o.onClose),
        onClick: X(e.onClick, o.onClose)
      }
    ) });
  }
);
yo.displayName = Gt;
var mn = "TooltipPortal", [$c, Bc] = St(mn, {
  forceMount: void 0
}), wo = (e) => {
  const { __scopeTooltip: t, forceMount: n, children: r, container: o } = e, i = Ke(mn, t);
  return /* @__PURE__ */ w($c, { scope: t, forceMount: n, children: /* @__PURE__ */ w(Fe, { present: n || i.open, children: /* @__PURE__ */ w(Jt, { asChild: !0, container: o, children: r }) }) });
};
wo.displayName = mn;
var _e = "TooltipContent", xo = d.forwardRef(
  (e, t) => {
    const n = Bc(_e, e.__scopeTooltip), { forceMount: r = n.forceMount, side: o = "top", ...i } = e, s = Ke(_e, e.__scopeTooltip);
    return /* @__PURE__ */ w(Fe, { present: r || s.open, children: s.disableHoverableContent ? /* @__PURE__ */ w(Co, { side: o, ...i, ref: t }) : /* @__PURE__ */ w(Vc, { side: o, ...i, ref: t }) });
  }
), Vc = d.forwardRef((e, t) => {
  const n = Ke(_e, e.__scopeTooltip), r = fn(_e, e.__scopeTooltip), o = d.useRef(null), i = ee(t, o), [s, a] = d.useState(null), { trigger: c, onClose: u } = n, l = o.current, { onPointerInTransitChange: f } = r, p = d.useCallback(() => {
    a(null), f(!1);
  }, [f]), m = d.useCallback(
    (g, h) => {
      const b = g.currentTarget, y = { x: g.clientX, y: g.clientY }, v = Yc(y, b.getBoundingClientRect()), x = Xc(y, v), C = Kc(h.getBoundingClientRect()), S = qc([...x, ...C]);
      a(S), f(!0);
    },
    [f]
  );
  return d.useEffect(() => () => p(), [p]), d.useEffect(() => {
    if (c && l) {
      const g = (b) => m(b, l), h = (b) => m(b, c);
      return c.addEventListener("pointerleave", g), l.addEventListener("pointerleave", h), () => {
        c.removeEventListener("pointerleave", g), l.removeEventListener("pointerleave", h);
      };
    }
  }, [c, l, m, p]), d.useEffect(() => {
    if (s) {
      const g = (h) => {
        const b = h.target, y = { x: h.clientX, y: h.clientY }, v = (c == null ? void 0 : c.contains(b)) || (l == null ? void 0 : l.contains(b)), x = !Zc(y, s);
        v ? p() : x && (p(), u());
      };
      return document.addEventListener("pointermove", g), () => document.removeEventListener("pointermove", g);
    }
  }, [c, l, s, u, p]), /* @__PURE__ */ w(Co, { ...e, ref: i });
}), [jc, Hc] = St(Ye, { isInside: !1 }), Gc = /* @__PURE__ */ Mo("TooltipContent"), Co = d.forwardRef(
  (e, t) => {
    const {
      __scopeTooltip: n,
      children: r,
      "aria-label": o,
      onEscapeKeyDown: i,
      onPointerDownOutside: s,
      ...a
    } = e, c = Ke(_e, n), u = Rt(n), { onClose: l } = c;
    return d.useEffect(() => (document.addEventListener(Ht, l), () => document.removeEventListener(Ht, l)), [l]), d.useEffect(() => {
      if (c.trigger) {
        const f = (p) => {
          p.target instanceof Node && p.target.contains(c.trigger) && l();
        };
        return window.addEventListener("scroll", f, { capture: !0 }), () => window.removeEventListener("scroll", f, { capture: !0 });
      }
    }, [c.trigger, l]), /* @__PURE__ */ w(
      Qt,
      {
        asChild: !0,
        disableOutsidePointerEvents: !1,
        onEscapeKeyDown: i,
        onPointerDownOutside: s,
        onFocusOutside: (f) => f.preventDefault(),
        onDismiss: l,
        children: /* @__PURE__ */ Y(
          Dc,
          {
            "data-state": c.stateAttribute,
            ...u,
            ...a,
            ref: t,
            style: {
              ...a.style,
              "--radix-tooltip-content-transform-origin": "var(--radix-popper-transform-origin)",
              "--radix-tooltip-content-available-width": "var(--radix-popper-available-width)",
              "--radix-tooltip-content-available-height": "var(--radix-popper-available-height)",
              "--radix-tooltip-trigger-width": "var(--radix-popper-anchor-width)",
              "--radix-tooltip-trigger-height": "var(--radix-popper-anchor-height)"
            },
            children: [
              /* @__PURE__ */ w(Gc, { children: r }),
              /* @__PURE__ */ w(jc, { scope: n, isInside: !0, children: /* @__PURE__ */ w(_c, { id: c.contentId, role: "tooltip", children: o || r }) })
            ]
          }
        )
      }
    );
  }
);
xo.displayName = _e;
var Eo = "TooltipArrow", Uc = d.forwardRef(
  (e, t) => {
    const { __scopeTooltip: n, ...r } = e, o = Rt(n);
    return Hc(
      Eo,
      n
    ).isInside ? null : /* @__PURE__ */ w(Mc, { ...o, ...r, ref: t });
  }
);
Uc.displayName = Eo;
function Yc(e, t) {
  const n = Math.abs(t.top - e.y), r = Math.abs(t.bottom - e.y), o = Math.abs(t.right - e.x), i = Math.abs(t.left - e.x);
  switch (Math.min(n, r, o, i)) {
    case i:
      return "left";
    case o:
      return "right";
    case n:
      return "top";
    case r:
      return "bottom";
    default:
      throw new Error("unreachable");
  }
}
function Xc(e, t, n = 5) {
  const r = [];
  switch (t) {
    case "top":
      r.push(
        { x: e.x - n, y: e.y + n },
        { x: e.x + n, y: e.y + n }
      );
      break;
    case "bottom":
      r.push(
        { x: e.x - n, y: e.y - n },
        { x: e.x + n, y: e.y - n }
      );
      break;
    case "left":
      r.push(
        { x: e.x + n, y: e.y - n },
        { x: e.x + n, y: e.y + n }
      );
      break;
    case "right":
      r.push(
        { x: e.x - n, y: e.y - n },
        { x: e.x - n, y: e.y + n }
      );
      break;
  }
  return r;
}
function Kc(e) {
  const { top: t, right: n, bottom: r, left: o } = e;
  return [
    { x: o, y: t },
    { x: n, y: t },
    { x: n, y: r },
    { x: o, y: r }
  ];
}
function Zc(e, t) {
  const { x: n, y: r } = e;
  let o = !1;
  for (let i = 0, s = t.length - 1; i < t.length; s = i++) {
    const a = t[i], c = t[s], u = a.x, l = a.y, f = c.x, p = c.y;
    l > r != p > r && n < (f - u) * (r - l) / (p - l) + u && (o = !o);
  }
  return o;
}
function qc(e) {
  const t = e.slice();
  return t.sort((n, r) => n.x < r.x ? -1 : n.x > r.x ? 1 : n.y < r.y ? -1 : n.y > r.y ? 1 : 0), Qc(t);
}
function Qc(e) {
  if (e.length <= 1) return e.slice();
  const t = [];
  for (let r = 0; r < e.length; r++) {
    const o = e[r];
    for (; t.length >= 2; ) {
      const i = t[t.length - 1], s = t[t.length - 2];
      if ((i.x - s.x) * (o.y - s.y) >= (i.y - s.y) * (o.x - s.x)) t.pop();
      else break;
    }
    t.push(o);
  }
  t.pop();
  const n = [];
  for (let r = e.length - 1; r >= 0; r--) {
    const o = e[r];
    for (; n.length >= 2; ) {
      const i = n[n.length - 1], s = n[n.length - 2];
      if ((i.x - s.x) * (o.y - s.y) >= (i.y - s.y) * (o.x - s.x)) n.pop();
      else break;
    }
    n.push(o);
  }
  return n.pop(), t.length === 1 && n.length === 1 && t[0].x === n[0].x && t[0].y === n[0].y ? t : t.concat(n);
}
var Jc = bo, el = vo, tl = yo, nl = wo, rl = xo;
function ol({
  delayDuration: e = 0,
  ...t
}) {
  return /* @__PURE__ */ w(Jc, { delayDuration: e, ...t });
}
function il({ ...e }) {
  return /* @__PURE__ */ w(el, { ...e });
}
function sl({ ...e }) {
  return /* @__PURE__ */ w(tl, { ...e });
}
function al({
  className: e,
  sideOffset: t = 6,
  ...n
}) {
  return /* @__PURE__ */ w(nl, { children: /* @__PURE__ */ w(
    rl,
    {
      sideOffset: t,
      className: $(
        "z-50 overflow-hidden rounded-md border border-nr-border bg-nr-panel px-2.5 py-1.5 text-xs text-nr-fg shadow-md animate-in fade-in-0 zoom-in-95",
        e
      ),
      ...n
    }
  ) });
}
const cl = "nav_rail_state", ll = 60 * 60 * 24 * 7, ul = "16rem", dl = "18rem", fl = "3.5rem", ml = "b", So = d.createContext(null);
function me() {
  const e = d.useContext(So);
  if (!e)
    throw new Error("useSidebar must be used within a SidebarProvider.");
  return e;
}
function pl({
  defaultOpen: e = !0,
  open: t,
  onOpenChange: n,
  className: r,
  style: o,
  children: i,
  ...s
}) {
  const a = Yo(), [c, u] = d.useState(!1), [l, f] = d.useState(e), p = t ?? l, m = d.useCallback(
    (y) => {
      const v = typeof y == "function" ? y(p) : y;
      n ? n(v) : f(v), document.cookie = `${cl}=${v}; path=/; max-age=${ll}`;
    },
    [p, n]
  ), g = d.useCallback(() => a ? u((y) => !y) : m((y) => !y), [a, m]);
  d.useEffect(() => {
    const y = (v) => {
      v.key === ml && (v.metaKey || v.ctrlKey) && (v.preventDefault(), g());
    };
    return window.addEventListener("keydown", y), () => window.removeEventListener("keydown", y);
  }, [g]);
  const h = p ? "expanded" : "collapsed", b = d.useMemo(
    () => ({
      state: h,
      open: p,
      setOpen: m,
      isMobile: a,
      openMobile: c,
      setOpenMobile: u,
      toggleSidebar: g
    }),
    [h, p, m, a, c, g]
  );
  return /* @__PURE__ */ w(So.Provider, { value: b, children: /* @__PURE__ */ w(ol, { delayDuration: 0, children: /* @__PURE__ */ w(
    "div",
    {
      "data-slot": "sidebar-wrapper",
      style: {
        "--sidebar-width": ul,
        "--sidebar-width-icon": fl,
        ...o
      },
      className: $("group/sidebar-wrapper flex h-full min-h-0 w-full", r),
      ...s,
      children: i
    }
  ) }) });
}
function hl({
  side: e = "left",
  variant: t = "sidebar",
  collapsible: n = "offcanvas",
  className: r,
  children: o,
  ...i
}) {
  const { isMobile: s, state: a, openMobile: c, setOpenMobile: u } = me(), l = a === "collapsed" && n !== "none", f = t === "floating" || t === "inset";
  if (n === "none")
    return /* @__PURE__ */ w("div", { className: $("flex h-full w-[var(--sidebar-width)] flex-col bg-nr-panel text-nr-fg", r), ...i, children: o });
  if (s)
    return /* @__PURE__ */ w(ua, { open: c, onOpenChange: u, ...i, children: /* @__PURE__ */ Y(
      ma,
      {
        "data-sidebar": "sidebar",
        "data-mobile": "true",
        className: "w-[var(--sidebar-width)] bg-nr-panel p-0 text-nr-fg [&>button]:hidden",
        style: { "--sidebar-width": dl },
        side: e,
        children: [
          /* @__PURE__ */ Y(pa, { className: "sr-only", children: [
            /* @__PURE__ */ w(ha, { children: "Sidebar" }),
            /* @__PURE__ */ w(ga, { children: "Displays the mobile sidebar." })
          ] }),
          /* @__PURE__ */ w("div", { className: "flex h-full w-full flex-col", children: o })
        ]
      }
    ) });
  const p = "w-[var(--sidebar-width)]", m = f ? "w-[calc(var(--sidebar-width-icon)+1rem)]" : "w-[var(--sidebar-width-icon)]";
  return /* @__PURE__ */ Y(
    "div",
    {
      className: "group peer hidden text-nr-fg md:block",
      "data-state": a,
      "data-collapsible": l ? n : "",
      "data-variant": t,
      "data-side": e,
      "data-slot": "sidebar",
      children: [
        /* @__PURE__ */ w(
          "div",
          {
            "data-slot": "sidebar-gap",
            className: $(
              "relative h-full bg-transparent transition-[width] duration-200 ease-linear",
              l && n === "offcanvas" ? "w-0" : l ? m : p
            )
          }
        ),
        /* @__PURE__ */ w(
          "div",
          {
            "data-slot": "sidebar-container",
            className: $(
              "fixed inset-y-0 z-10 hidden h-full transition-[left,right,width] duration-200 ease-linear md:flex",
              e === "left" ? "left-0" : "right-0",
              l && n === "offcanvas" && e === "left" && "-left-[var(--sidebar-width)]",
              l && n === "offcanvas" && e === "right" && "-right-[var(--sidebar-width)]",
              l && n === "icon" ? m : p,
              f && "p-2",
              !f && "border-r border-nr-border",
              r
            ),
            ...i,
            children: /* @__PURE__ */ w(
              "div",
              {
                "data-sidebar": "sidebar",
                "data-slot": "sidebar-inner",
                className: $(
                  "flex h-full w-full flex-col bg-nr-panel text-nr-fg",
                  f && "rounded-lg border border-nr-border shadow-sm"
                ),
                children: o
              }
            )
          }
        )
      ]
    }
  );
}
function gl({
  className: e,
  onClick: t,
  ...n
}) {
  const { toggleSidebar: r } = me();
  return /* @__PURE__ */ Y(
    $i,
    {
      "data-sidebar": "trigger",
      variant: "ghost",
      size: "icon",
      className: $("h-8 w-8 text-nr-muted hover:bg-nr-bg hover:text-nr-fg", e),
      onClick: (o) => {
        t == null || t(o), r();
      },
      ...n,
      children: [
        /* @__PURE__ */ w(Go, { className: "h-4 w-4" }),
        /* @__PURE__ */ w("span", { className: "sr-only", children: "Toggle Sidebar" })
      ]
    }
  );
}
function bl({ className: e, ...t }) {
  const { toggleSidebar: n } = me();
  return /* @__PURE__ */ w(
    "button",
    {
      "data-sidebar": "rail",
      "aria-label": "Toggle Sidebar",
      tabIndex: -1,
      onClick: n,
      title: "Toggle Sidebar",
      className: $(
        "absolute inset-y-0 -right-3 z-20 hidden w-4 transition-all after:absolute after:inset-y-0 after:left-1/2 after:w-0.5 hover:after:bg-nr-border sm:flex",
        e
      ),
      ...t
    }
  );
}
function vl({ className: e, ...t }) {
  const { state: n } = me();
  return /* @__PURE__ */ w(
    "div",
    {
      "data-sidebar": "header",
      className: $("flex flex-col gap-2 p-2", n === "collapsed" && "items-center px-0", e),
      ...t
    }
  );
}
function yl({ className: e, ...t }) {
  const { state: n } = me();
  return /* @__PURE__ */ w(
    "div",
    {
      "data-sidebar": "footer",
      className: $("flex flex-col gap-2 p-2", n === "collapsed" && "items-center px-0", e),
      ...t
    }
  );
}
function wl({ className: e, ...t }) {
  return /* @__PURE__ */ w(
    "div",
    {
      "data-sidebar": "content",
      className: $(
        "flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto overflow-x-hidden [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden",
        e
      ),
      ...t
    }
  );
}
function xl({ className: e, ...t }) {
  const { state: n } = me();
  return /* @__PURE__ */ w(
    "div",
    {
      "data-sidebar": "group",
      className: $("relative flex w-full min-w-0 flex-col p-2", n === "collapsed" && "items-center px-0", e),
      ...t
    }
  );
}
function Cl({ className: e, ...t }) {
  const { state: n } = me();
  return /* @__PURE__ */ w(
    "div",
    {
      "data-sidebar": "group-label",
      className: $(
        "flex h-8 shrink-0 items-center rounded-md px-2 text-xs font-medium text-nr-muted transition-[margin,opacity] duration-200",
        n === "collapsed" && "-mt-8 opacity-0",
        e
      ),
      ...t
    }
  );
}
function El({ className: e, ...t }) {
  return /* @__PURE__ */ w("div", { "data-sidebar": "group-content", className: $("w-full text-sm", e), ...t });
}
function Sl({ className: e, ...t }) {
  const { state: n } = me();
  return /* @__PURE__ */ w(
    "ul",
    {
      "data-sidebar": "menu",
      className: $("flex w-full min-w-0 flex-col gap-1", n === "collapsed" && "items-center", e),
      ...t
    }
  );
}
function Rl({ className: e, ...t }) {
  return /* @__PURE__ */ w("li", { "data-sidebar": "menu-item", className: $("group/menu-item relative", e), ...t });
}
const kl = rr(
  "peer/menu-button flex w-full items-center gap-2 overflow-hidden rounded-md p-2 text-left text-sm text-nr-muted outline-none ring-nr-accent transition-[width,height,padding,color,background-color] hover:bg-nr-bg hover:text-nr-fg focus-visible:ring-2 active:bg-nr-accent/10 active:text-nr-fg disabled:pointer-events-none disabled:opacity-50 data-[active=true]:bg-nr-bg data-[active=true]:font-medium data-[active=true]:text-nr-fg [&>span:last-child]:truncate [&>svg]:h-4 [&>svg]:w-4 [&>svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "hover:bg-nr-bg hover:text-nr-fg",
        outline: "bg-nr-bg shadow-[0_0_0_1px_hsl(var(--nr-border))] hover:bg-nr-bg hover:text-nr-fg"
      },
      size: {
        default: "h-8 text-sm",
        sm: "h-7 text-xs",
        lg: "h-12 text-sm"
      }
    },
    defaultVariants: {
      variant: "default",
      size: "default"
    }
  }
);
function Al({
  asChild: e = !1,
  isActive: t = !1,
  variant: n = "default",
  size: r = "default",
  tooltip: o,
  className: i,
  ...s
}) {
  const a = e ? Jn : "button", { isMobile: c, state: u } = me(), l = /* @__PURE__ */ w(
    a,
    {
      "data-sidebar": "menu-button",
      "data-size": r,
      "data-active": t,
      className: $(
        kl({ variant: n, size: r }),
        u === "collapsed" && "mx-auto h-8 w-8 p-2 [&>span]:sr-only",
        r === "lg" && u === "collapsed" && "mx-auto h-8 w-8 p-0",
        i
      ),
      ...s
    }
  );
  return !o || u !== "collapsed" || c ? l : /* @__PURE__ */ Y(il, { children: [
    /* @__PURE__ */ w(sl, { asChild: !0, children: l }),
    /* @__PURE__ */ w(al, { side: "right", align: "center", ...typeof o == "string" ? { children: o } : o })
  ] });
}
function Tl({
  items: e,
  active: t,
  onSelect: n,
  header: r,
  footer: o,
  defaultCollapsed: i = !1,
  className: s
}) {
  const a = Qn(e);
  return /* @__PURE__ */ w(pl, { defaultOpen: !i, className: `nav-rail ${s ?? ""}`, children: /* @__PURE__ */ Y(hl, { collapsible: "icon", variant: "sidebar", children: [
    /* @__PURE__ */ Y(vl, { children: [
      r,
      /* @__PURE__ */ w("div", { className: "flex items-center justify-end px-1 group-data-[collapsible=icon]:justify-center", children: /* @__PURE__ */ w(gl, { "aria-label": "Toggle sidebar", title: "Toggle sidebar" }) })
    ] }),
    /* @__PURE__ */ w(wl, { children: a.map((c, u) => /* @__PURE__ */ Y(xl, { children: [
      c.label && /* @__PURE__ */ w(Cl, { children: c.label }),
      /* @__PURE__ */ w(El, { children: /* @__PURE__ */ w(Sl, { children: c.items.map((l) => {
        const f = t === l.id, p = l.icon;
        return /* @__PURE__ */ w(Rl, { children: /* @__PURE__ */ Y(
          Al,
          {
            "aria-label": l.label,
            "aria-current": f ? "page" : void 0,
            isActive: f,
            tooltip: l.label,
            onClick: () => n(l.id),
            children: [
              p && /* @__PURE__ */ w(p, {}),
              /* @__PURE__ */ w("span", { children: l.label })
            ]
          }
        ) }, l.id);
      }) }) })
    ] }, c.label ?? `__default-${u}`)) }),
    o && /* @__PURE__ */ w(yl, { children: o }),
    /* @__PURE__ */ w(bl, {})
  ] }) });
}
function Nl({
  items: e,
  active: t,
  onSelect: n,
  badge: r,
  className: o,
  "aria-label": i = "section navigation"
}) {
  const s = Qn(e);
  return /* @__PURE__ */ w(
    "nav",
    {
      "aria-label": i,
      className: $("nav-rail flex min-w-0 flex-col gap-2 text-nr-fg", o),
      children: s.map((a, c) => /* @__PURE__ */ Y("div", { className: "flex flex-col gap-1", children: [
        a.label && /* @__PURE__ */ w("div", { className: "px-2 text-xs font-medium text-nr-muted", children: a.label }),
        a.items.map((u) => {
          const l = t === u.id, f = u.icon, p = r == null ? void 0 : r(u.id);
          return /* @__PURE__ */ Y(
            "button",
            {
              type: "button",
              role: "tab",
              "aria-label": u.label,
              "aria-current": l ? "page" : void 0,
              "aria-selected": l,
              onClick: () => n(u.id),
              className: $(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none ring-nr-accent transition-colors focus-visible:ring-2",
                "[&>svg]:h-4 [&>svg]:w-4 [&>svg]:shrink-0",
                l ? "bg-nr-bg font-medium text-nr-fg" : "text-nr-muted hover:bg-nr-bg hover:text-nr-fg"
              ),
              children: [
                f && /* @__PURE__ */ w(f, {}),
                /* @__PURE__ */ w("span", { className: "min-w-0 flex-1 truncate", children: u.label }),
                p ? /* @__PURE__ */ w("span", { className: "rounded-full bg-nr-accent/15 px-1.5 text-[10px] text-nr-accent", children: p }) : null
              ]
            },
            u.id
          );
        })
      ] }, a.label ?? `__default-${c}`))
    }
  );
}
export {
  Nl as NavMenu,
  Tl as NavRail
};
