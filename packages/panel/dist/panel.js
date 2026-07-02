import { jsx as C, Fragment as bo, jsxs as Q } from "react/jsx-runtime";
import * as l from "react";
import { useState as Ht, useCallback as pt, useRef as Da, forwardRef as vo, createElement as Dn, useLayoutEffect as Ta } from "react";
import * as vt from "react-dom";
function ze(e, t, { checkForDefaultPrevented: n = !0 } = {}) {
  return function(o) {
    if (e == null || e(o), n === !1 || !o.defaultPrevented)
      return t == null ? void 0 : t(o);
  };
}
function ur(e, t) {
  if (typeof e == "function")
    return e(t);
  e != null && (e.current = t);
}
function Ma(...e) {
  return (t) => {
    let n = !1;
    const r = e.map((o) => {
      const i = ur(o, t);
      return !n && typeof i == "function" && (n = !0), i;
    });
    if (n)
      return () => {
        for (let o = 0; o < r.length; o++) {
          const i = r[o];
          typeof i == "function" ? i() : ur(e[o], null);
        }
      };
  };
}
function Ye(...e) {
  return l.useCallback(Ma(...e), e);
}
function La(e, t = []) {
  let n = [];
  function r(i, a) {
    const s = l.createContext(a);
    s.displayName = i + "Context";
    const u = n.length;
    n = [...n, a];
    const c = (f) => {
      var w;
      const { scope: m, children: h, ...b } = f, p = ((w = m == null ? void 0 : m[e]) == null ? void 0 : w[u]) || s, g = l.useMemo(() => b, Object.values(b));
      return /* @__PURE__ */ C(p.Provider, { value: g, children: h });
    };
    c.displayName = i + "Provider";
    function d(f, m) {
      var p;
      const h = ((p = m == null ? void 0 : m[e]) == null ? void 0 : p[u]) || s, b = l.useContext(h);
      if (b) return b;
      if (a !== void 0) return a;
      throw new Error(`\`${f}\` must be used within \`${i}\``);
    }
    return [c, d];
  }
  const o = () => {
    const i = n.map((a) => l.createContext(a));
    return function(s) {
      const u = (s == null ? void 0 : s[e]) || i;
      return l.useMemo(
        () => ({ [`__scope${e}`]: { ...s, [e]: u } }),
        [s, u]
      );
    };
  };
  return o.scopeName = e, [r, Ia(o, ...t)];
}
function Ia(...e) {
  const t = e[0];
  if (e.length === 1) return t;
  const n = () => {
    const r = e.map((o) => ({
      useScope: o(),
      scopeName: o.scopeName
    }));
    return function(i) {
      const a = r.reduce((s, { useScope: u, scopeName: c }) => {
        const f = u(i)[`__scope${c}`];
        return { ...s, ...f };
      }, {});
      return l.useMemo(() => ({ [`__scope${t.scopeName}`]: a }), [a]);
    };
  };
  return n.scopeName = t.scopeName, n;
}
var at = globalThis != null && globalThis.document ? l.useLayoutEffect : () => {
}, za = l[" useId ".trim().toString()] || (() => {
}), _a = 0;
function pn(e) {
  const [t, n] = l.useState(za());
  return at(() => {
    n((r) => r ?? String(_a++));
  }, [e]), e || (t ? `radix-${t}` : "");
}
var dr = l[" useEffectEvent ".trim().toString()], fr = l[" useInsertionEffect ".trim().toString()];
function Fa(e) {
  if (typeof dr == "function")
    return dr(e);
  const t = l.useRef(() => {
    throw new Error("Cannot call an event handler while rendering.");
  });
  return typeof fr == "function" ? fr(() => {
    t.current = e;
  }) : at(() => {
    t.current = e;
  }), l.useMemo(() => (...n) => {
    var r;
    return (r = t.current) == null ? void 0 : r.call(t, ...n);
  }, []);
}
var Wa = l[" useInsertionEffect ".trim().toString()] || at;
function ja({
  prop: e,
  defaultProp: t,
  onChange: n = () => {
  },
  caller: r
}) {
  const [o, i, a] = Ba({
    defaultProp: t,
    onChange: n
  }), s = e !== void 0, u = s ? e : o;
  {
    const d = l.useRef(e !== void 0);
    l.useEffect(() => {
      const f = d.current;
      f !== s && console.warn(
        `${r} is changing from ${f ? "controlled" : "uncontrolled"} to ${s ? "controlled" : "uncontrolled"}. Components should not switch from controlled to uncontrolled (or vice versa). Decide between using a controlled or uncontrolled value for the lifetime of the component.`
      ), d.current = s;
    }, [s, r]);
  }
  const c = l.useCallback(
    (d) => {
      var f;
      if (s) {
        const m = $a(d) ? d(e) : d;
        m !== e && ((f = a.current) == null || f.call(a, m));
      } else
        i(d);
    },
    [s, e, i, a]
  );
  return [u, c];
}
function Ba({
  defaultProp: e,
  onChange: t
}) {
  const [n, r] = l.useState(e), o = l.useRef(n), i = l.useRef(t);
  return Wa(() => {
    i.current = t;
  }, [t]), l.useEffect(() => {
    var a;
    o.current !== n && ((a = i.current) == null || a.call(i, n), o.current = n);
  }, [n, o]), [n, r, i];
}
function $a(e) {
  return typeof e == "function";
}
// @__NO_SIDE_EFFECTS__
function yo(e) {
  const t = l.forwardRef((n, r) => {
    let { children: o, ...i } = n, a = null, s = !1;
    const u = [];
    pr(o) && typeof kt == "function" && (o = kt(o._payload)), l.Children.forEach(o, (m) => {
      var h;
      if (Xa(m)) {
        s = !0;
        const b = m;
        let p = "child" in b.props ? b.props.child : b.props.children;
        pr(p) && typeof kt == "function" && (p = kt(p._payload)), a = Ua(b, p), u.push((h = a == null ? void 0 : a.props) == null ? void 0 : h.children);
      } else
        u.push(m);
    }), a ? a = l.cloneElement(a, void 0, u) : (
      // A `Slottable` was found but it didn't resolve to a single element (e.g.
      // it wrapped multiple elements, text, or a render-prop `child` that
      // wasn't an element). Don't fall back to treating the `Slottable` wrapper
      // itself as the slot target — throw a descriptive error below instead.
      !s && l.Children.count(o) === 1 && l.isValidElement(o) && (a = o)
    );
    const c = a ? Ha(a) : void 0, d = Ye(r, c);
    if (!a) {
      if (o || o === 0)
        throw new Error(
          s ? qa(e) : Za(e)
        );
      return o;
    }
    const f = Va(i, a.props ?? {});
    return a.type !== l.Fragment && (f.ref = r ? d : c), l.cloneElement(a, f);
  });
  return t.displayName = `${e}.Slot`, t;
}
var Ga = Symbol.for("radix.slottable"), Ua = (e, t) => {
  if ("child" in e.props) {
    const n = e.props.child;
    return l.isValidElement(n) ? l.cloneElement(n, void 0, e.props.children(n.props.children)) : null;
  }
  return l.isValidElement(t) ? t : null;
};
function Va(e, t) {
  const n = { ...t };
  for (const r in t) {
    const o = e[r], i = t[r];
    /^on[A-Z]/.test(r) ? o && i ? n[r] = (...s) => {
      const u = i(...s);
      return o(...s), u;
    } : o && (n[r] = o) : r === "style" ? n[r] = { ...o, ...i } : r === "className" && (n[r] = [o, i].filter(Boolean).join(" "));
  }
  return { ...e, ...n };
}
function Ha(e) {
  var r, o;
  let t = (r = Object.getOwnPropertyDescriptor(e.props, "ref")) == null ? void 0 : r.get, n = t && "isReactWarning" in t && t.isReactWarning;
  return n ? e.ref : (t = (o = Object.getOwnPropertyDescriptor(e, "ref")) == null ? void 0 : o.get, n = t && "isReactWarning" in t && t.isReactWarning, n ? e.props.ref : e.props.ref || e.ref);
}
function Xa(e) {
  return l.isValidElement(e) && typeof e.type == "function" && "__radixId" in e.type && e.type.__radixId === Ga;
}
var Ya = Symbol.for("react.lazy");
function pr(e) {
  return e != null && typeof e == "object" && "$$typeof" in e && e.$$typeof === Ya && "_payload" in e && Ka(e._payload);
}
function Ka(e) {
  return typeof e == "object" && e !== null && "then" in e;
}
var Za = (e) => `${e} failed to slot onto its children. Expected a single React element child or \`Slottable\`.`, qa = (e) => `${e} failed to slot onto its \`Slottable\`. Expected \`Slottable\` to receive a single React element child.`, kt = l[" use ".trim().toString()], Qa = [
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
], Ne = Qa.reduce((e, t) => {
  const n = /* @__PURE__ */ yo(`Primitive.${t}`), r = l.forwardRef((o, i) => {
    const { asChild: a, ...s } = o, u = a ? n : t;
    return typeof window < "u" && (window[Symbol.for("radix-ui")] = !0), /* @__PURE__ */ C(u, { ...s, ref: i });
  });
  return r.displayName = `Primitive.${t}`, { ...e, [t]: r };
}, {});
function Ja(e, t) {
  e && vt.flushSync(() => e.dispatchEvent(t));
}
function Xt(e) {
  const t = l.useRef(e);
  return l.useEffect(() => {
    t.current = e;
  }), l.useMemo(() => (...n) => {
    var r;
    return (r = t.current) == null ? void 0 : r.call(t, ...n);
  }, []);
}
var es = "DismissableLayer", Tn = "dismissableLayer.update", ts = "dismissableLayer.pointerDownOutside", ns = "dismissableLayer.focusOutside", mr, $n = l.createContext({
  layers: /* @__PURE__ */ new Set(),
  layersWithOutsidePointerEventsDisabled: /* @__PURE__ */ new Set(),
  branches: /* @__PURE__ */ new Set(),
  // Outside elements that belong to a layer's own dismiss affordance (eg, a
  // dialog overlay). Pressing them should dismiss the layer regardless of
  // whether or not they stop propagation.
  //
  // See https://github.com/radix-ui/primitives/issues/3346
  dismissableSurfaces: /* @__PURE__ */ new Set()
}), wo = l.forwardRef(
  (e, t) => {
    const {
      disableOutsidePointerEvents: n = !1,
      deferPointerDownOutside: r = !1,
      onEscapeKeyDown: o,
      onPointerDownOutside: i,
      onFocusOutside: a,
      onInteractOutside: s,
      onDismiss: u,
      ...c
    } = e, d = l.useContext($n), [f, m] = l.useState(null), h = (f == null ? void 0 : f.ownerDocument) ?? (globalThis == null ? void 0 : globalThis.document), [, b] = l.useState({}), p = Ye(t, m), g = Array.from(d.layers), [w] = [...d.layersWithOutsidePointerEventsDisabled].slice(-1), v = g.indexOf(w), x = f ? g.indexOf(f) : -1, E = d.layersWithOutsidePointerEventsDisabled.size > 0, S = x >= v, R = l.useRef(!1), T = as(
      (O) => {
        const L = O.target;
        if (!(L instanceof Node))
          return;
        const V = [...d.branches].some(
          (j) => j.contains(L)
        );
        !S || V || (i == null || i(O), s == null || s(O), O.defaultPrevented || u == null || u());
      },
      {
        ownerDocument: h,
        deferPointerDownOutside: r,
        isDeferredPointerDownOutsideRef: R,
        dismissableSurfaces: d.dismissableSurfaces
      }
    ), y = ss((O) => {
      if (r && R.current)
        return;
      const L = O.target;
      [...d.branches].some((j) => j.contains(L)) || (a == null || a(O), s == null || s(O), O.defaultPrevented || u == null || u());
    }, h), M = f ? x === g.length - 1 : !1, W = Fa((O) => {
      O.key === "Escape" && (o == null || o(O), !O.defaultPrevented && u && (O.preventDefault(), u()));
    });
    return l.useEffect(() => {
      if (M)
        return h.addEventListener("keydown", W, { capture: !0 }), () => h.removeEventListener("keydown", W, { capture: !0 });
    }, [h, M]), l.useEffect(() => {
      if (f)
        return n && (d.layersWithOutsidePointerEventsDisabled.size === 0 && (mr = h.body.style.pointerEvents, h.body.style.pointerEvents = "none"), d.layersWithOutsidePointerEventsDisabled.add(f)), d.layers.add(f), hr(), () => {
          n && (d.layersWithOutsidePointerEventsDisabled.delete(f), d.layersWithOutsidePointerEventsDisabled.size === 0 && (h.body.style.pointerEvents = mr));
        };
    }, [f, h, n, d]), l.useEffect(() => () => {
      f && (d.layers.delete(f), d.layersWithOutsidePointerEventsDisabled.delete(f), hr());
    }, [f, d]), l.useEffect(() => {
      const O = () => b({});
      return document.addEventListener(Tn, O), () => document.removeEventListener(Tn, O);
    }, []), /* @__PURE__ */ C(
      Ne.div,
      {
        ...c,
        ref: p,
        style: {
          pointerEvents: E ? S ? "auto" : "none" : void 0,
          ...e.style
        },
        onFocusCapture: ze(e.onFocusCapture, y.onFocusCapture),
        onBlurCapture: ze(e.onBlurCapture, y.onBlurCapture),
        onPointerDownCapture: ze(
          e.onPointerDownCapture,
          T.onPointerDownCapture
        )
      }
    );
  }
);
wo.displayName = es;
var rs = "DismissableLayerBranch", os = l.forwardRef((e, t) => {
  const n = l.useContext($n), r = l.useRef(null), o = Ye(t, r);
  return l.useEffect(() => {
    const i = r.current;
    if (i)
      return n.branches.add(i), () => {
        n.branches.delete(i);
      };
  }, [n.branches]), /* @__PURE__ */ C(Ne.div, { ...e, ref: o });
});
os.displayName = rs;
function is() {
  const e = l.useContext($n), [t, n] = l.useState(null);
  return l.useEffect(() => {
    if (t)
      return e.dismissableSurfaces.add(t), () => {
        e.dismissableSurfaces.delete(t);
      };
  }, [t, e.dismissableSurfaces]), n;
}
function as(e, t) {
  const {
    ownerDocument: n = globalThis == null ? void 0 : globalThis.document,
    deferPointerDownOutside: r = !1,
    isDeferredPointerDownOutsideRef: o,
    dismissableSurfaces: i
  } = t, a = Xt(e), s = l.useRef(!1), u = l.useRef(!1), c = l.useRef(/* @__PURE__ */ new Map()), d = l.useRef(() => {
  });
  return l.useEffect(() => {
    function f() {
      u.current = !1, o.current = !1, c.current.clear();
    }
    function m() {
      return Array.from(c.current.values()).some(Boolean);
    }
    function h(v) {
      if (!u.current)
        return;
      const x = v.target;
      x instanceof Node && [...i].some((S) => S.contains(x)) || c.current.set(v.type, !0), v.type === "click" && window.setTimeout(() => {
        u.current && d.current();
      }, 0);
    }
    function b(v) {
      u.current && c.current.set(v.type, !1);
    }
    const p = (v) => {
      if (v.target && !s.current) {
        let x = function() {
          n.removeEventListener("click", d.current);
          const S = m();
          f(), S || xo(
            ts,
            a,
            E,
            { discrete: !0 }
          );
        };
        const E = { originalEvent: v };
        u.current = !0, o.current = r && v.button === 0, c.current.clear(), !r || v.button !== 0 ? x() : (n.removeEventListener("click", d.current), d.current = x, n.addEventListener("click", d.current, { once: !0 }));
      } else
        n.removeEventListener("click", d.current), f();
      s.current = !1;
    }, g = [
      "pointerup",
      "mousedown",
      "mouseup",
      "touchstart",
      "touchend",
      "click"
    ];
    for (const v of g)
      n.addEventListener(v, h, !0), n.addEventListener(v, b);
    const w = window.setTimeout(() => {
      n.addEventListener("pointerdown", p);
    }, 0);
    return () => {
      window.clearTimeout(w), n.removeEventListener("pointerdown", p), n.removeEventListener("click", d.current);
      for (const v of g)
        n.removeEventListener(v, h, !0), n.removeEventListener(v, b);
    };
  }, [
    n,
    a,
    r,
    o,
    i
  ]), {
    // ensures we check React component tree (not just DOM tree)
    onPointerDownCapture: () => s.current = !0
  };
}
function ss(e, t = globalThis == null ? void 0 : globalThis.document) {
  const n = Xt(e), r = l.useRef(!1);
  return l.useEffect(() => {
    const o = (i) => {
      i.target && !r.current && xo(ns, n, { originalEvent: i }, {
        discrete: !1
      });
    };
    return t.addEventListener("focusin", o), () => t.removeEventListener("focusin", o);
  }, [t, n]), {
    onFocusCapture: () => r.current = !0,
    onBlurCapture: () => r.current = !1
  };
}
function hr() {
  const e = new CustomEvent(Tn);
  document.dispatchEvent(e);
}
function xo(e, t, n, { discrete: r }) {
  const o = n.originalEvent.target, i = new CustomEvent(e, { bubbles: !1, cancelable: !0, detail: n });
  t && o.addEventListener(e, t, { once: !0 }), r ? Ja(o, i) : o.dispatchEvent(i);
}
var mn = "focusScope.autoFocusOnMount", hn = "focusScope.autoFocusOnUnmount", gr = { bubbles: !1, cancelable: !0 }, ls = "FocusScope", ko = l.forwardRef((e, t) => {
  const {
    loop: n = !1,
    trapped: r = !1,
    onMountAutoFocus: o,
    onUnmountAutoFocus: i,
    ...a
  } = e, [s, u] = l.useState(null), c = Xt(o), d = Xt(i), f = l.useRef(null), m = Ye(t, u), h = l.useRef({
    paused: !1,
    pause() {
      this.paused = !0;
    },
    resume() {
      this.paused = !1;
    }
  }).current;
  l.useEffect(() => {
    if (r) {
      let p = function(x) {
        if (h.paused || !s) return;
        const E = x.target;
        s.contains(E) ? f.current = E : Le(f.current, { select: !0 });
      }, g = function(x) {
        if (h.paused || !s) return;
        const E = x.relatedTarget;
        E !== null && (s.contains(E) || Le(f.current, { select: !0 }));
      }, w = function(x) {
        if (document.activeElement === document.body)
          for (const S of x)
            S.removedNodes.length > 0 && Le(s);
      };
      document.addEventListener("focusin", p), document.addEventListener("focusout", g);
      const v = new MutationObserver(w);
      return s && v.observe(s, { childList: !0, subtree: !0 }), () => {
        document.removeEventListener("focusin", p), document.removeEventListener("focusout", g), v.disconnect();
      };
    }
  }, [r, s, h.paused]), l.useEffect(() => {
    if (s) {
      vr.add(h);
      const p = document.activeElement;
      if (!s.contains(p)) {
        const w = new CustomEvent(mn, gr);
        s.addEventListener(mn, c), s.dispatchEvent(w), w.defaultPrevented || (cs(ms(Eo(s)), { select: !0 }), document.activeElement === p && Le(s));
      }
      return () => {
        s.removeEventListener(mn, c), setTimeout(() => {
          const w = new CustomEvent(hn, gr);
          s.addEventListener(hn, d), s.dispatchEvent(w), w.defaultPrevented || Le(p ?? document.body, { select: !0 }), s.removeEventListener(hn, d), vr.remove(h);
        }, 0);
      };
    }
  }, [s, c, d, h]);
  const b = l.useCallback(
    (p) => {
      if (!n && !r || h.paused) return;
      const g = p.key === "Tab" && !p.altKey && !p.ctrlKey && !p.metaKey, w = document.activeElement;
      if (g && w) {
        const v = p.currentTarget, [x, E] = us(v);
        x && E ? !p.shiftKey && w === E ? (p.preventDefault(), n && Le(x, { select: !0 })) : p.shiftKey && w === x && (p.preventDefault(), n && Le(E, { select: !0 })) : w === v && p.preventDefault();
      }
    },
    [n, r, h.paused]
  );
  return /* @__PURE__ */ C(Ne.div, { tabIndex: -1, ...a, ref: m, onKeyDown: b });
});
ko.displayName = ls;
function cs(e, { select: t = !1 } = {}) {
  const n = document.activeElement;
  for (const r of e)
    if (Le(r, { select: t }), document.activeElement !== n) return;
}
function us(e) {
  const t = Eo(e), n = br(t, e), r = br(t.reverse(), e);
  return [n, r];
}
function Eo(e) {
  const t = [], n = document.createTreeWalker(e, NodeFilter.SHOW_ELEMENT, {
    acceptNode: (r) => {
      const o = r.tagName === "INPUT" && r.type === "hidden";
      return r.disabled || r.hidden || o ? NodeFilter.FILTER_SKIP : r.tabIndex >= 0 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;
    }
  });
  for (; n.nextNode(); ) t.push(n.currentNode);
  return t;
}
function br(e, t) {
  for (const n of e)
    if (!ds(n, { upTo: t })) return n;
}
function ds(e, { upTo: t }) {
  if (getComputedStyle(e).visibility === "hidden") return !0;
  for (; e; ) {
    if (t !== void 0 && e === t) return !1;
    if (getComputedStyle(e).display === "none") return !0;
    e = e.parentElement;
  }
  return !1;
}
function fs(e) {
  return e instanceof HTMLInputElement && "select" in e;
}
function Le(e, { select: t = !1 } = {}) {
  if (e && e.focus) {
    const n = document.activeElement;
    e.focus({ preventScroll: !0 }), e !== n && fs(e) && t && e.select();
  }
}
var vr = ps();
function ps() {
  let e = [];
  return {
    add(t) {
      const n = e[0];
      t !== n && (n == null || n.pause()), e = yr(e, t), e.unshift(t);
    },
    remove(t) {
      var n;
      e = yr(e, t), (n = e[0]) == null || n.resume();
    }
  };
}
function yr(e, t) {
  const n = [...e], r = n.indexOf(t);
  return r !== -1 && n.splice(r, 1), n;
}
function ms(e) {
  return e.filter((t) => t.tagName !== "A");
}
var hs = "Portal", Co = l.forwardRef((e, t) => {
  var s;
  const { container: n, ...r } = e, [o, i] = l.useState(!1);
  at(() => i(!0), []);
  const a = n || o && ((s = globalThis == null ? void 0 : globalThis.document) == null ? void 0 : s.body);
  return a ? vt.createPortal(/* @__PURE__ */ C(Ne.div, { ...r, ref: t }), a) : null;
});
Co.displayName = hs;
function gs(e, t) {
  return l.useReducer((n, r) => t[n][r] ?? n, e);
}
var nn = (e) => {
  const { present: t, children: n } = e, r = bs(t), o = typeof n == "function" ? n({ present: r.isPresent }) : l.Children.only(n), i = vs(r.ref, ys(o));
  return typeof n == "function" || r.isPresent ? l.cloneElement(o, { ref: i }) : null;
};
nn.displayName = "Presence";
function bs(e) {
  const [t, n] = l.useState(), r = l.useRef(null), o = l.useRef(e), i = l.useRef("none"), a = e ? "mounted" : "unmounted", [s, u] = gs(a, {
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
  return l.useEffect(() => {
    const c = Et(r.current);
    i.current = s === "mounted" ? c : "none";
  }, [s]), at(() => {
    const c = r.current, d = o.current;
    if (d !== e) {
      const m = i.current, h = Et(c);
      e ? u("MOUNT") : h === "none" || (c == null ? void 0 : c.display) === "none" ? u("UNMOUNT") : u(d && m !== h ? "ANIMATION_OUT" : "UNMOUNT"), o.current = e;
    }
  }, [e, u]), at(() => {
    if (t) {
      let c;
      const d = t.ownerDocument.defaultView ?? window, f = (h) => {
        const p = Et(r.current).includes(CSS.escape(h.animationName));
        if (h.target === t && p && (u("ANIMATION_END"), !o.current)) {
          const g = t.style.animationFillMode;
          t.style.animationFillMode = "forwards", c = d.setTimeout(() => {
            t.style.animationFillMode === "forwards" && (t.style.animationFillMode = g);
          });
        }
      }, m = (h) => {
        h.target === t && (i.current = Et(r.current));
      };
      return t.addEventListener("animationstart", m), t.addEventListener("animationcancel", f), t.addEventListener("animationend", f), () => {
        d.clearTimeout(c), t.removeEventListener("animationstart", m), t.removeEventListener("animationcancel", f), t.removeEventListener("animationend", f);
      };
    } else
      u("ANIMATION_END");
  }, [t, u]), {
    isPresent: ["mounted", "unmountSuspended"].includes(s),
    ref: l.useCallback((c) => {
      r.current = c ? getComputedStyle(c) : null, n(c);
    }, [])
  };
}
function wr(e, t) {
  if (typeof e == "function")
    return e(t);
  e != null && (e.current = t);
}
function vs(...e) {
  const t = l.useRef(e);
  return t.current = e, l.useCallback((n) => {
    const r = t.current;
    let o = !1;
    const i = r.map((a) => {
      const s = wr(a, n);
      return !o && typeof s == "function" && (o = !0), s;
    });
    if (o)
      return () => {
        for (let a = 0; a < i.length; a++) {
          const s = i[a];
          typeof s == "function" ? s() : wr(r[a], null);
        }
      };
  }, []);
}
function Et(e) {
  return (e == null ? void 0 : e.animationName) || "none";
}
function ys(e) {
  var r, o;
  let t = (r = Object.getOwnPropertyDescriptor(e.props, "ref")) == null ? void 0 : r.get, n = t && "isReactWarning" in t && t.isReactWarning;
  return n ? e.ref : (t = (o = Object.getOwnPropertyDescriptor(e, "ref")) == null ? void 0 : o.get, n = t && "isReactWarning" in t && t.isReactWarning, n ? e.props.ref : e.props.ref || e.ref);
}
var Ct = 0, pe = null;
function ws() {
  l.useEffect(() => {
    pe || (pe = { start: xr(), end: xr() });
    const { start: e, end: t } = pe;
    return document.body.firstElementChild !== e && document.body.insertAdjacentElement("afterbegin", e), document.body.lastElementChild !== t && document.body.insertAdjacentElement("beforeend", t), Ct++, () => {
      Ct === 1 && (pe == null || pe.start.remove(), pe == null || pe.end.remove(), pe = null), Ct = Math.max(0, Ct - 1);
    };
  }, []);
}
function xr() {
  const e = document.createElement("span");
  return e.setAttribute("data-radix-focus-guard", ""), e.tabIndex = 0, e.style.outline = "none", e.style.opacity = "0", e.style.position = "fixed", e.style.pointerEvents = "none", e;
}
var ge = function() {
  return ge = Object.assign || function(t) {
    for (var n, r = 1, o = arguments.length; r < o; r++) {
      n = arguments[r];
      for (var i in n) Object.prototype.hasOwnProperty.call(n, i) && (t[i] = n[i]);
    }
    return t;
  }, ge.apply(this, arguments);
};
function So(e, t) {
  var n = {};
  for (var r in e) Object.prototype.hasOwnProperty.call(e, r) && t.indexOf(r) < 0 && (n[r] = e[r]);
  if (e != null && typeof Object.getOwnPropertySymbols == "function")
    for (var o = 0, r = Object.getOwnPropertySymbols(e); o < r.length; o++)
      t.indexOf(r[o]) < 0 && Object.prototype.propertyIsEnumerable.call(e, r[o]) && (n[r[o]] = e[r[o]]);
  return n;
}
function xs(e, t, n) {
  if (n || arguments.length === 2) for (var r = 0, o = t.length, i; r < o; r++)
    (i || !(r in t)) && (i || (i = Array.prototype.slice.call(t, 0, r)), i[r] = t[r]);
  return e.concat(i || Array.prototype.slice.call(t));
}
var Bt = "right-scroll-bar-position", $t = "width-before-scroll-bar", ks = "with-scroll-bars-hidden", Es = "--removed-body-scroll-bar-size";
function gn(e, t) {
  return typeof e == "function" ? e(t) : e && (e.current = t), e;
}
function Cs(e, t) {
  var n = Ht(function() {
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
var Ss = typeof window < "u" ? l.useLayoutEffect : l.useEffect, kr = /* @__PURE__ */ new WeakMap();
function Rs(e, t) {
  var n = Cs(null, function(r) {
    return e.forEach(function(o) {
      return gn(o, r);
    });
  });
  return Ss(function() {
    var r = kr.get(n);
    if (r) {
      var o = new Set(r), i = new Set(e), a = n.current;
      o.forEach(function(s) {
        i.has(s) || gn(s, null);
      }), i.forEach(function(s) {
        o.has(s) || gn(s, a);
      });
    }
    kr.set(n, e);
  }, [e]), n;
}
function Ps(e) {
  return e;
}
function Ns(e, t) {
  t === void 0 && (t = Ps);
  var n = [], r = !1, o = {
    read: function() {
      if (r)
        throw new Error("Sidecar: could not `read` from an `assigned` medium. `read` could be used only with `useMedium`.");
      return n.length ? n[n.length - 1] : e;
    },
    useMedium: function(i) {
      var a = t(i, r);
      return n.push(a), function() {
        n = n.filter(function(s) {
          return s !== a;
        });
      };
    },
    assignSyncMedium: function(i) {
      for (r = !0; n.length; ) {
        var a = n;
        n = [], a.forEach(i);
      }
      n = {
        push: function(s) {
          return i(s);
        },
        filter: function() {
          return n;
        }
      };
    },
    assignMedium: function(i) {
      r = !0;
      var a = [];
      if (n.length) {
        var s = n;
        n = [], s.forEach(i), a = n;
      }
      var u = function() {
        var d = a;
        a = [], d.forEach(i);
      }, c = function() {
        return Promise.resolve().then(u);
      };
      c(), n = {
        push: function(d) {
          a.push(d), c();
        },
        filter: function(d) {
          return a = a.filter(d), n;
        }
      };
    }
  };
  return o;
}
function Os(e) {
  e === void 0 && (e = {});
  var t = Ns(null);
  return t.options = ge({ async: !0, ssr: !1 }, e), t;
}
var Ro = function(e) {
  var t = e.sideCar, n = So(e, ["sideCar"]);
  if (!t)
    throw new Error("Sidecar: please provide `sideCar` property to import the right car");
  var r = t.read();
  if (!r)
    throw new Error("Sidecar medium not found");
  return l.createElement(r, ge({}, n));
};
Ro.isSideCarExport = !0;
function As(e, t) {
  return e.useMedium(t), Ro;
}
var Po = Os(), bn = function() {
}, rn = l.forwardRef(function(e, t) {
  var n = l.useRef(null), r = l.useState({
    onScrollCapture: bn,
    onWheelCapture: bn,
    onTouchMoveCapture: bn
  }), o = r[0], i = r[1], a = e.forwardProps, s = e.children, u = e.className, c = e.removeScrollBar, d = e.enabled, f = e.shards, m = e.sideCar, h = e.noRelative, b = e.noIsolation, p = e.inert, g = e.allowPinchZoom, w = e.as, v = w === void 0 ? "div" : w, x = e.gapMode, E = So(e, ["forwardProps", "children", "className", "removeScrollBar", "enabled", "shards", "sideCar", "noRelative", "noIsolation", "inert", "allowPinchZoom", "as", "gapMode"]), S = m, R = Rs([n, t]), T = ge(ge({}, E), o);
  return l.createElement(
    l.Fragment,
    null,
    d && l.createElement(S, { sideCar: Po, removeScrollBar: c, shards: f, noRelative: h, noIsolation: b, inert: p, setCallbacks: i, allowPinchZoom: !!g, lockRef: n, gapMode: x }),
    a ? l.cloneElement(l.Children.only(s), ge(ge({}, T), { ref: R })) : l.createElement(v, ge({}, T, { className: u, ref: R }), s)
  );
});
rn.defaultProps = {
  enabled: !0,
  removeScrollBar: !0,
  inert: !1
};
rn.classNames = {
  fullWidth: $t,
  zeroRight: Bt
};
var Ds = function() {
  if (typeof __webpack_nonce__ < "u")
    return __webpack_nonce__;
};
function Ts() {
  if (!document)
    return null;
  var e = document.createElement("style");
  e.type = "text/css";
  var t = Ds();
  return t && e.setAttribute("nonce", t), e;
}
function Ms(e, t) {
  e.styleSheet ? e.styleSheet.cssText = t : e.appendChild(document.createTextNode(t));
}
function Ls(e) {
  var t = document.head || document.getElementsByTagName("head")[0];
  t.appendChild(e);
}
var Is = function() {
  var e = 0, t = null;
  return {
    add: function(n) {
      e == 0 && (t = Ts()) && (Ms(t, n), Ls(t)), e++;
    },
    remove: function() {
      e--, !e && t && (t.parentNode && t.parentNode.removeChild(t), t = null);
    }
  };
}, zs = function() {
  var e = Is();
  return function(t, n) {
    l.useEffect(function() {
      return e.add(t), function() {
        e.remove();
      };
    }, [t && n]);
  };
}, No = function() {
  var e = zs(), t = function(n) {
    var r = n.styles, o = n.dynamic;
    return e(r, o), null;
  };
  return t;
}, _s = {
  left: 0,
  top: 0,
  right: 0,
  gap: 0
}, vn = function(e) {
  return parseInt(e || "", 10) || 0;
}, Fs = function(e) {
  var t = window.getComputedStyle(document.body), n = t[e === "padding" ? "paddingLeft" : "marginLeft"], r = t[e === "padding" ? "paddingTop" : "marginTop"], o = t[e === "padding" ? "paddingRight" : "marginRight"];
  return [vn(n), vn(r), vn(o)];
}, Ws = function(e) {
  if (e === void 0 && (e = "margin"), typeof window > "u")
    return _s;
  var t = Fs(e), n = document.documentElement.clientWidth, r = window.innerWidth;
  return {
    left: t[0],
    top: t[1],
    right: t[2],
    gap: Math.max(0, r - n + t[2] - t[0])
  };
}, js = No(), rt = "data-scroll-locked", Bs = function(e, t, n, r) {
  var o = e.left, i = e.top, a = e.right, s = e.gap;
  return n === void 0 && (n = "margin"), `
  .`.concat(ks, ` {
   overflow: hidden `).concat(r, `;
   padding-right: `).concat(s, "px ").concat(r, `;
  }
  body[`).concat(rt, `] {
    overflow: hidden `).concat(r, `;
    overscroll-behavior: contain;
    `).concat([
    t && "position: relative ".concat(r, ";"),
    n === "margin" && `
    padding-left: `.concat(o, `px;
    padding-top: `).concat(i, `px;
    padding-right: `).concat(a, `px;
    margin-left:0;
    margin-top:0;
    margin-right: `).concat(s, "px ").concat(r, `;
    `),
    n === "padding" && "padding-right: ".concat(s, "px ").concat(r, ";")
  ].filter(Boolean).join(""), `
  }
  
  .`).concat(Bt, ` {
    right: `).concat(s, "px ").concat(r, `;
  }
  
  .`).concat($t, ` {
    margin-right: `).concat(s, "px ").concat(r, `;
  }
  
  .`).concat(Bt, " .").concat(Bt, ` {
    right: 0 `).concat(r, `;
  }
  
  .`).concat($t, " .").concat($t, ` {
    margin-right: 0 `).concat(r, `;
  }
  
  body[`).concat(rt, `] {
    `).concat(Es, ": ").concat(s, `px;
  }
`);
}, Er = function() {
  var e = parseInt(document.body.getAttribute(rt) || "0", 10);
  return isFinite(e) ? e : 0;
}, $s = function() {
  l.useEffect(function() {
    return document.body.setAttribute(rt, (Er() + 1).toString()), function() {
      var e = Er() - 1;
      e <= 0 ? document.body.removeAttribute(rt) : document.body.setAttribute(rt, e.toString());
    };
  }, []);
}, Gs = function(e) {
  var t = e.noRelative, n = e.noImportant, r = e.gapMode, o = r === void 0 ? "margin" : r;
  $s();
  var i = l.useMemo(function() {
    return Ws(o);
  }, [o]);
  return l.createElement(js, { styles: Bs(i, !t, o, n ? "" : "!important") });
}, Mn = !1;
if (typeof window < "u")
  try {
    var St = Object.defineProperty({}, "passive", {
      get: function() {
        return Mn = !0, !0;
      }
    });
    window.addEventListener("test", St, St), window.removeEventListener("test", St, St);
  } catch {
    Mn = !1;
  }
var qe = Mn ? { passive: !1 } : !1, Us = function(e) {
  return e.tagName === "TEXTAREA";
}, Oo = function(e, t) {
  if (!(e instanceof Element))
    return !1;
  var n = window.getComputedStyle(e);
  return (
    // not-not-scrollable
    n[t] !== "hidden" && // contains scroll inside self
    !(n.overflowY === n.overflowX && !Us(e) && n[t] === "visible")
  );
}, Vs = function(e) {
  return Oo(e, "overflowY");
}, Hs = function(e) {
  return Oo(e, "overflowX");
}, Cr = function(e, t) {
  var n = t.ownerDocument, r = t;
  do {
    typeof ShadowRoot < "u" && r instanceof ShadowRoot && (r = r.host);
    var o = Ao(e, r);
    if (o) {
      var i = Do(e, r), a = i[1], s = i[2];
      if (a > s)
        return !0;
    }
    r = r.parentNode;
  } while (r && r !== n.body);
  return !1;
}, Xs = function(e) {
  var t = e.scrollTop, n = e.scrollHeight, r = e.clientHeight;
  return [
    t,
    n,
    r
  ];
}, Ys = function(e) {
  var t = e.scrollLeft, n = e.scrollWidth, r = e.clientWidth;
  return [
    t,
    n,
    r
  ];
}, Ao = function(e, t) {
  return e === "v" ? Vs(t) : Hs(t);
}, Do = function(e, t) {
  return e === "v" ? Xs(t) : Ys(t);
}, Ks = function(e, t) {
  return e === "h" && t === "rtl" ? -1 : 1;
}, Zs = function(e, t, n, r, o) {
  var i = Ks(e, window.getComputedStyle(t).direction), a = i * r, s = n.target, u = t.contains(s), c = !1, d = a > 0, f = 0, m = 0;
  do {
    if (!s)
      break;
    var h = Do(e, s), b = h[0], p = h[1], g = h[2], w = p - g - i * b;
    (b || w) && Ao(e, s) && (f += w, m += b);
    var v = s.parentNode;
    s = v && v.nodeType === Node.DOCUMENT_FRAGMENT_NODE ? v.host : v;
  } while (
    // portaled content
    !u && s !== document.body || // self content
    u && (t.contains(s) || t === s)
  );
  return (d && Math.abs(f) < 1 || !d && Math.abs(m) < 1) && (c = !0), c;
}, Rt = function(e) {
  return "changedTouches" in e ? [e.changedTouches[0].clientX, e.changedTouches[0].clientY] : [0, 0];
}, Sr = function(e) {
  return [e.deltaX, e.deltaY];
}, Rr = function(e) {
  return e && "current" in e ? e.current : e;
}, qs = function(e, t) {
  return e[0] === t[0] && e[1] === t[1];
}, Qs = function(e) {
  return `
  .block-interactivity-`.concat(e, ` {pointer-events: none;}
  .allow-interactivity-`).concat(e, ` {pointer-events: all;}
`);
}, Js = 0, Qe = [];
function el(e) {
  var t = l.useRef([]), n = l.useRef([0, 0]), r = l.useRef(), o = l.useState(Js++)[0], i = l.useState(No)[0], a = l.useRef(e);
  l.useEffect(function() {
    a.current = e;
  }, [e]), l.useEffect(function() {
    if (e.inert) {
      document.body.classList.add("block-interactivity-".concat(o));
      var p = xs([e.lockRef.current], (e.shards || []).map(Rr), !0).filter(Boolean);
      return p.forEach(function(g) {
        return g.classList.add("allow-interactivity-".concat(o));
      }), function() {
        document.body.classList.remove("block-interactivity-".concat(o)), p.forEach(function(g) {
          return g.classList.remove("allow-interactivity-".concat(o));
        });
      };
    }
  }, [e.inert, e.lockRef.current, e.shards]);
  var s = l.useCallback(function(p, g) {
    if ("touches" in p && p.touches.length === 2 || p.type === "wheel" && p.ctrlKey)
      return !a.current.allowPinchZoom;
    var w = Rt(p), v = n.current, x = "deltaX" in p ? p.deltaX : v[0] - w[0], E = "deltaY" in p ? p.deltaY : v[1] - w[1], S, R = p.target, T = Math.abs(x) > Math.abs(E) ? "h" : "v";
    if ("touches" in p && T === "h" && R.type === "range")
      return !1;
    var y = window.getSelection(), M = y && y.anchorNode, W = M ? M === R || M.contains(R) : !1;
    if (W)
      return !1;
    var O = Cr(T, R);
    if (!O)
      return !0;
    if (O ? S = T : (S = T === "v" ? "h" : "v", O = Cr(T, R)), !O)
      return !1;
    if (!r.current && "changedTouches" in p && (x || E) && (r.current = S), !S)
      return !0;
    var L = r.current || S;
    return Zs(L, g, p, L === "h" ? x : E);
  }, []), u = l.useCallback(function(p) {
    var g = p;
    if (!(!Qe.length || Qe[Qe.length - 1] !== i)) {
      var w = "deltaY" in g ? Sr(g) : Rt(g), v = t.current.filter(function(S) {
        return S.name === g.type && (S.target === g.target || g.target === S.shadowParent) && qs(S.delta, w);
      })[0];
      if (v && v.should) {
        g.cancelable && g.preventDefault();
        return;
      }
      if (!v) {
        var x = (a.current.shards || []).map(Rr).filter(Boolean).filter(function(S) {
          return S.contains(g.target);
        }), E = x.length > 0 ? s(g, x[0]) : !a.current.noIsolation;
        E && g.cancelable && g.preventDefault();
      }
    }
  }, []), c = l.useCallback(function(p, g, w, v) {
    var x = { name: p, delta: g, target: w, should: v, shadowParent: tl(w) };
    t.current.push(x), setTimeout(function() {
      t.current = t.current.filter(function(E) {
        return E !== x;
      });
    }, 1);
  }, []), d = l.useCallback(function(p) {
    n.current = Rt(p), r.current = void 0;
  }, []), f = l.useCallback(function(p) {
    c(p.type, Sr(p), p.target, s(p, e.lockRef.current));
  }, []), m = l.useCallback(function(p) {
    c(p.type, Rt(p), p.target, s(p, e.lockRef.current));
  }, []);
  l.useEffect(function() {
    return Qe.push(i), e.setCallbacks({
      onScrollCapture: f,
      onWheelCapture: f,
      onTouchMoveCapture: m
    }), document.addEventListener("wheel", u, qe), document.addEventListener("touchmove", u, qe), document.addEventListener("touchstart", d, qe), function() {
      Qe = Qe.filter(function(p) {
        return p !== i;
      }), document.removeEventListener("wheel", u, qe), document.removeEventListener("touchmove", u, qe), document.removeEventListener("touchstart", d, qe);
    };
  }, []);
  var h = e.removeScrollBar, b = e.inert;
  return l.createElement(
    l.Fragment,
    null,
    b ? l.createElement(i, { styles: Qs(o) }) : null,
    h ? l.createElement(Gs, { noRelative: e.noRelative, gapMode: e.gapMode }) : null
  );
}
function tl(e) {
  for (var t = null; e !== null; )
    e instanceof ShadowRoot && (t = e.host, e = e.host), e = e.parentNode;
  return t;
}
const nl = As(Po, el);
var To = l.forwardRef(function(e, t) {
  return l.createElement(rn, ge({}, e, { ref: t, sideCar: nl }));
});
To.classNames = rn.classNames;
var rl = function(e) {
  if (typeof document > "u")
    return null;
  var t = Array.isArray(e) ? e[0] : e;
  return t.ownerDocument.body;
}, Je = /* @__PURE__ */ new WeakMap(), Pt = /* @__PURE__ */ new WeakMap(), Nt = {}, yn = 0, Mo = function(e) {
  return e && (e.host || Mo(e.parentNode));
}, ol = function(e, t) {
  return t.map(function(n) {
    if (e.contains(n))
      return n;
    var r = Mo(n);
    return r && e.contains(r) ? r : (console.error("aria-hidden", n, "in not contained inside", e, ". Doing nothing"), null);
  }).filter(function(n) {
    return !!n;
  });
}, il = function(e, t, n, r) {
  var o = ol(t, Array.isArray(e) ? e : [e]);
  Nt[n] || (Nt[n] = /* @__PURE__ */ new WeakMap());
  var i = Nt[n], a = [], s = /* @__PURE__ */ new Set(), u = new Set(o), c = function(f) {
    !f || s.has(f) || (s.add(f), c(f.parentNode));
  };
  o.forEach(c);
  var d = function(f) {
    !f || u.has(f) || Array.prototype.forEach.call(f.children, function(m) {
      if (s.has(m))
        d(m);
      else
        try {
          var h = m.getAttribute(r), b = h !== null && h !== "false", p = (Je.get(m) || 0) + 1, g = (i.get(m) || 0) + 1;
          Je.set(m, p), i.set(m, g), a.push(m), p === 1 && b && Pt.set(m, !0), g === 1 && m.setAttribute(n, "true"), b || m.setAttribute(r, "true");
        } catch (w) {
          console.error("aria-hidden: cannot operate on ", m, w);
        }
    });
  };
  return d(t), s.clear(), yn++, function() {
    a.forEach(function(f) {
      var m = Je.get(f) - 1, h = i.get(f) - 1;
      Je.set(f, m), i.set(f, h), m || (Pt.has(f) || f.removeAttribute(r), Pt.delete(f)), h || f.removeAttribute(n);
    }), yn--, yn || (Je = /* @__PURE__ */ new WeakMap(), Je = /* @__PURE__ */ new WeakMap(), Pt = /* @__PURE__ */ new WeakMap(), Nt = {});
  };
}, al = function(e, t, n) {
  n === void 0 && (n = "data-aria-hidden");
  var r = Array.from(Array.isArray(e) ? e : [e]), o = rl(e);
  return o ? (r.push.apply(r, Array.from(o.querySelectorAll("[aria-live], script"))), il(r, o, n, "aria-hidden")) : function() {
    return null;
  };
}, on = "Dialog", [Lo] = La(on), [sl, ce] = Lo(on), Io = (e) => {
  const {
    __scopeDialog: t,
    children: n,
    open: r,
    defaultOpen: o,
    onOpenChange: i,
    modal: a = !0
  } = e, s = l.useRef(null), u = l.useRef(null), [c, d] = ja({
    prop: r,
    defaultProp: o ?? !1,
    onChange: i,
    caller: on
  });
  return /* @__PURE__ */ C(
    sl,
    {
      scope: t,
      triggerRef: s,
      contentRef: u,
      contentId: pn(),
      titleId: pn(),
      descriptionId: pn(),
      open: c,
      onOpenChange: d,
      onOpenToggle: l.useCallback(() => d((f) => !f), [d]),
      modal: a,
      children: n
    }
  );
};
Io.displayName = on;
var zo = "DialogTrigger", ll = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = ce(zo, n), i = Ye(t, o.triggerRef);
    return /* @__PURE__ */ C(
      Ne.button,
      {
        type: "button",
        "aria-haspopup": "dialog",
        "aria-expanded": o.open,
        "aria-controls": o.open ? o.contentId : void 0,
        "data-state": Un(o.open),
        ...r,
        ref: i,
        onClick: ze(e.onClick, o.onOpenToggle)
      }
    );
  }
);
ll.displayName = zo;
var Gn = "DialogPortal", [cl, _o] = Lo(Gn, {
  forceMount: void 0
}), Fo = (e) => {
  const { __scopeDialog: t, forceMount: n, children: r, container: o } = e, i = ce(Gn, t);
  return /* @__PURE__ */ C(cl, { scope: t, forceMount: n, children: l.Children.map(r, (a) => /* @__PURE__ */ C(nn, { present: n || i.open, children: /* @__PURE__ */ C(Co, { asChild: !0, container: o, children: a }) })) });
};
Fo.displayName = Gn;
var Yt = "DialogOverlay", Wo = l.forwardRef(
  (e, t) => {
    const n = _o(Yt, e.__scopeDialog), { forceMount: r = n.forceMount, ...o } = e, i = ce(Yt, e.__scopeDialog);
    return i.modal ? /* @__PURE__ */ C(nn, { present: r || i.open, children: /* @__PURE__ */ C(dl, { ...o, ref: t }) }) : null;
  }
);
Wo.displayName = Yt;
var ul = /* @__PURE__ */ yo("DialogOverlay.RemoveScroll"), dl = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = ce(Yt, n), i = is(), a = Ye(t, i);
    return (
      // Make sure `Content` is scrollable even when it doesn't live inside `RemoveScroll`
      // ie. when `Overlay` and `Content` are siblings
      /* @__PURE__ */ C(To, { as: ul, allowPinchZoom: !0, shards: [o.contentRef], children: /* @__PURE__ */ C(
        Ne.div,
        {
          "data-state": Un(o.open),
          ...r,
          ref: a,
          style: { pointerEvents: "auto", ...r.style }
        }
      ) })
    );
  }
), st = "DialogContent", jo = l.forwardRef(
  (e, t) => {
    const n = _o(st, e.__scopeDialog), { forceMount: r = n.forceMount, ...o } = e, i = ce(st, e.__scopeDialog);
    return /* @__PURE__ */ C(nn, { present: r || i.open, children: i.modal ? /* @__PURE__ */ C(fl, { ...o, ref: t }) : /* @__PURE__ */ C(pl, { ...o, ref: t }) });
  }
);
jo.displayName = st;
var fl = l.forwardRef(
  (e, t) => {
    const n = ce(st, e.__scopeDialog), r = l.useRef(null), o = Ye(t, n.contentRef, r);
    return l.useEffect(() => {
      const i = r.current;
      if (i) return al(i);
    }, []), /* @__PURE__ */ C(
      Bo,
      {
        ...e,
        ref: o,
        trapFocus: n.open,
        disableOutsidePointerEvents: n.open,
        onCloseAutoFocus: ze(e.onCloseAutoFocus, (i) => {
          var a;
          i.preventDefault(), (a = n.triggerRef.current) == null || a.focus();
        }),
        onPointerDownOutside: ze(e.onPointerDownOutside, (i) => {
          const a = i.detail.originalEvent, s = a.button === 0 && a.ctrlKey === !0;
          (a.button === 2 || s) && i.preventDefault();
        }),
        onFocusOutside: ze(
          e.onFocusOutside,
          (i) => i.preventDefault()
        )
      }
    );
  }
), pl = l.forwardRef(
  (e, t) => {
    const n = ce(st, e.__scopeDialog), r = l.useRef(!1), o = l.useRef(!1);
    return /* @__PURE__ */ C(
      Bo,
      {
        ...e,
        ref: t,
        trapFocus: !1,
        disableOutsidePointerEvents: !1,
        onCloseAutoFocus: (i) => {
          var a, s;
          (a = e.onCloseAutoFocus) == null || a.call(e, i), i.defaultPrevented || (r.current || (s = n.triggerRef.current) == null || s.focus(), i.preventDefault()), r.current = !1, o.current = !1;
        },
        onInteractOutside: (i) => {
          var u, c;
          (u = e.onInteractOutside) == null || u.call(e, i), i.defaultPrevented || (r.current = !0, i.detail.originalEvent.type === "pointerdown" && (o.current = !0));
          const a = i.target;
          ((c = n.triggerRef.current) == null ? void 0 : c.contains(a)) && i.preventDefault(), i.detail.originalEvent.type === "focusin" && o.current && i.preventDefault();
        }
      }
    );
  }
), Bo = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, trapFocus: r, onOpenAutoFocus: o, onCloseAutoFocus: i, ...a } = e, s = ce(st, n);
    return ws(), /* @__PURE__ */ C(bo, { children: /* @__PURE__ */ C(
      ko,
      {
        asChild: !0,
        loop: !0,
        trapped: r,
        onMountAutoFocus: o,
        onUnmountAutoFocus: i,
        children: /* @__PURE__ */ C(
          wo,
          {
            role: "dialog",
            id: s.contentId,
            "aria-describedby": s.descriptionId,
            "aria-labelledby": s.titleId,
            "data-state": Un(s.open),
            ...a,
            ref: t,
            deferPointerDownOutside: !0,
            onDismiss: () => s.onOpenChange(!1)
          }
        )
      }
    ) });
  }
), $o = "DialogTitle", Go = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = ce($o, n);
    return /* @__PURE__ */ C(Ne.h2, { id: o.titleId, ...r, ref: t });
  }
);
Go.displayName = $o;
var Uo = "DialogDescription", Vo = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = ce(Uo, n);
    return /* @__PURE__ */ C(Ne.p, { id: o.descriptionId, ...r, ref: t });
  }
);
Vo.displayName = Uo;
var Ho = "DialogClose", ml = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = ce(Ho, n);
    return /* @__PURE__ */ C(
      Ne.button,
      {
        type: "button",
        ...r,
        ref: t,
        onClick: ze(e.onClick, () => o.onOpenChange(!1))
      }
    );
  }
);
ml.displayName = Ho;
function Un(e) {
  return e ? "open" : "closed";
}
function Xo(e) {
  var t, n, r = "";
  if (typeof e == "string" || typeof e == "number") r += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var o = e.length;
    for (t = 0; t < o; t++) e[t] && (n = Xo(e[t])) && (r && (r += " "), r += n);
  } else for (n in e) e[n] && (r && (r += " "), r += n);
  return r;
}
function hl() {
  for (var e, t, n = 0, r = "", o = arguments.length; n < o; n++) (e = arguments[n]) && (t = Xo(e)) && (r && (r += " "), r += t);
  return r;
}
const gl = (e, t) => {
  const n = new Array(e.length + t.length);
  for (let r = 0; r < e.length; r++)
    n[r] = e[r];
  for (let r = 0; r < t.length; r++)
    n[e.length + r] = t[r];
  return n;
}, bl = (e, t) => ({
  classGroupId: e,
  validator: t
}), Yo = (e = /* @__PURE__ */ new Map(), t = null, n) => ({
  nextPart: e,
  validators: t,
  classGroupId: n
}), Kt = "-", Pr = [], vl = "arbitrary..", yl = (e) => {
  const t = xl(e), {
    conflictingClassGroups: n,
    conflictingClassGroupModifiers: r
  } = e;
  return {
    getClassGroupId: (a) => {
      if (a.startsWith("[") && a.endsWith("]"))
        return wl(a);
      const s = a.split(Kt), u = s[0] === "" && s.length > 1 ? 1 : 0;
      return Ko(s, u, t);
    },
    getConflictingClassGroupIds: (a, s) => {
      if (s) {
        const u = r[a], c = n[a];
        return u ? c ? gl(c, u) : u : c || Pr;
      }
      return n[a] || Pr;
    }
  };
}, Ko = (e, t, n) => {
  if (e.length - t === 0)
    return n.classGroupId;
  const o = e[t], i = n.nextPart.get(o);
  if (i) {
    const c = Ko(e, t + 1, i);
    if (c) return c;
  }
  const a = n.validators;
  if (a === null)
    return;
  const s = t === 0 ? e.join(Kt) : e.slice(t).join(Kt), u = a.length;
  for (let c = 0; c < u; c++) {
    const d = a[c];
    if (d.validator(s))
      return d.classGroupId;
  }
}, wl = (e) => e.slice(1, -1).indexOf(":") === -1 ? void 0 : (() => {
  const t = e.slice(1, -1), n = t.indexOf(":"), r = t.slice(0, n);
  return r ? vl + r : void 0;
})(), xl = (e) => {
  const {
    theme: t,
    classGroups: n
  } = e;
  return kl(n, t);
}, kl = (e, t) => {
  const n = Yo();
  for (const r in e) {
    const o = e[r];
    Vn(o, n, r, t);
  }
  return n;
}, Vn = (e, t, n, r) => {
  const o = e.length;
  for (let i = 0; i < o; i++) {
    const a = e[i];
    El(a, t, n, r);
  }
}, El = (e, t, n, r) => {
  if (typeof e == "string") {
    Cl(e, t, n);
    return;
  }
  if (typeof e == "function") {
    Sl(e, t, n, r);
    return;
  }
  Rl(e, t, n, r);
}, Cl = (e, t, n) => {
  const r = e === "" ? t : Zo(t, e);
  r.classGroupId = n;
}, Sl = (e, t, n, r) => {
  if (Pl(e)) {
    Vn(e(r), t, n, r);
    return;
  }
  t.validators === null && (t.validators = []), t.validators.push(bl(n, e));
}, Rl = (e, t, n, r) => {
  const o = Object.entries(e), i = o.length;
  for (let a = 0; a < i; a++) {
    const [s, u] = o[a];
    Vn(u, Zo(t, s), n, r);
  }
}, Zo = (e, t) => {
  let n = e;
  const r = t.split(Kt), o = r.length;
  for (let i = 0; i < o; i++) {
    const a = r[i];
    let s = n.nextPart.get(a);
    s || (s = Yo(), n.nextPart.set(a, s)), n = s;
  }
  return n;
}, Pl = (e) => "isThemeGetter" in e && e.isThemeGetter === !0, Nl = (e) => {
  if (e < 1)
    return {
      get: () => {
      },
      set: () => {
      }
    };
  let t = 0, n = /* @__PURE__ */ Object.create(null), r = /* @__PURE__ */ Object.create(null);
  const o = (i, a) => {
    n[i] = a, t++, t > e && (t = 0, r = n, n = /* @__PURE__ */ Object.create(null));
  };
  return {
    get(i) {
      let a = n[i];
      if (a !== void 0)
        return a;
      if ((a = r[i]) !== void 0)
        return o(i, a), a;
    },
    set(i, a) {
      i in n ? n[i] = a : o(i, a);
    }
  };
}, Ln = "!", Nr = ":", Ol = [], Or = (e, t, n, r, o) => ({
  modifiers: e,
  hasImportantModifier: t,
  baseClassName: n,
  maybePostfixModifierPosition: r,
  isExternal: o
}), Al = (e) => {
  const {
    prefix: t,
    experimentalParseClassName: n
  } = e;
  let r = (o) => {
    const i = [];
    let a = 0, s = 0, u = 0, c;
    const d = o.length;
    for (let p = 0; p < d; p++) {
      const g = o[p];
      if (a === 0 && s === 0) {
        if (g === Nr) {
          i.push(o.slice(u, p)), u = p + 1;
          continue;
        }
        if (g === "/") {
          c = p;
          continue;
        }
      }
      g === "[" ? a++ : g === "]" ? a-- : g === "(" ? s++ : g === ")" && s--;
    }
    const f = i.length === 0 ? o : o.slice(u);
    let m = f, h = !1;
    f.endsWith(Ln) ? (m = f.slice(0, -1), h = !0) : (
      /**
       * In Tailwind CSS v3 the important modifier was at the start of the base class name. This is still supported for legacy reasons.
       * @see https://github.com/dcastil/tailwind-merge/issues/513#issuecomment-2614029864
       */
      f.startsWith(Ln) && (m = f.slice(1), h = !0)
    );
    const b = c && c > u ? c - u : void 0;
    return Or(i, h, m, b);
  };
  if (t) {
    const o = t + Nr, i = r;
    r = (a) => a.startsWith(o) ? i(a.slice(o.length)) : Or(Ol, !1, a, void 0, !0);
  }
  if (n) {
    const o = r;
    r = (i) => n({
      className: i,
      parseClassName: o
    });
  }
  return r;
}, Dl = (e) => {
  const t = /* @__PURE__ */ new Map();
  return e.orderSensitiveModifiers.forEach((n, r) => {
    t.set(n, 1e6 + r);
  }), (n) => {
    const r = [];
    let o = [];
    for (let i = 0; i < n.length; i++) {
      const a = n[i], s = a[0] === "[", u = t.has(a);
      s || u ? (o.length > 0 && (o.sort(), r.push(...o), o = []), r.push(a)) : o.push(a);
    }
    return o.length > 0 && (o.sort(), r.push(...o)), r;
  };
}, Tl = (e) => ({
  cache: Nl(e.cacheSize),
  parseClassName: Al(e),
  sortModifiers: Dl(e),
  postfixLookupClassGroupIds: Ml(e),
  ...yl(e)
}), Ml = (e) => {
  const t = /* @__PURE__ */ Object.create(null), n = e.postfixLookupClassGroups;
  if (n)
    for (let r = 0; r < n.length; r++)
      t[n[r]] = !0;
  return t;
}, Ll = /\s+/, Il = (e, t) => {
  const {
    parseClassName: n,
    getClassGroupId: r,
    getConflictingClassGroupIds: o,
    sortModifiers: i,
    postfixLookupClassGroupIds: a
  } = t, s = [], u = e.trim().split(Ll);
  let c = "";
  for (let d = u.length - 1; d >= 0; d -= 1) {
    const f = u[d], {
      isExternal: m,
      modifiers: h,
      hasImportantModifier: b,
      baseClassName: p,
      maybePostfixModifierPosition: g
    } = n(f);
    if (m) {
      c = f + (c.length > 0 ? " " + c : c);
      continue;
    }
    let w = !!g, v;
    if (w) {
      const T = p.substring(0, g);
      v = r(T);
      const y = v && a[v] ? r(p) : void 0;
      y && y !== v && (v = y, w = !1);
    } else
      v = r(p);
    if (!v) {
      if (!w) {
        c = f + (c.length > 0 ? " " + c : c);
        continue;
      }
      if (v = r(p), !v) {
        c = f + (c.length > 0 ? " " + c : c);
        continue;
      }
      w = !1;
    }
    const x = h.length === 0 ? "" : h.length === 1 ? h[0] : i(h).join(":"), E = b ? x + Ln : x, S = E + v;
    if (s.indexOf(S) > -1)
      continue;
    s.push(S);
    const R = o(v, w);
    for (let T = 0; T < R.length; ++T) {
      const y = R[T];
      s.push(E + y);
    }
    c = f + (c.length > 0 ? " " + c : c);
  }
  return c;
}, zl = (...e) => {
  let t = 0, n, r, o = "";
  for (; t < e.length; )
    (n = e[t++]) && (r = qo(n)) && (o && (o += " "), o += r);
  return o;
}, qo = (e) => {
  if (typeof e == "string")
    return e;
  let t, n = "";
  for (let r = 0; r < e.length; r++)
    e[r] && (t = qo(e[r])) && (n && (n += " "), n += t);
  return n;
}, _l = (e, ...t) => {
  let n, r, o, i;
  const a = (u) => {
    const c = t.reduce((d, f) => f(d), e());
    return n = Tl(c), r = n.cache.get, o = n.cache.set, i = s, s(u);
  }, s = (u) => {
    const c = r(u);
    if (c)
      return c;
    const d = Il(u, n);
    return o(u, d), d;
  };
  return i = a, (...u) => i(zl(...u));
}, Fl = [], Y = (e) => {
  const t = (n) => n[e] || Fl;
  return t.isThemeGetter = !0, t;
}, Qo = /^\[(?:(\w[\w-]*):)?(.+)\]$/i, Jo = /^\((?:(\w[\w-]*):)?(.+)\)$/i, Wl = /^\d+(?:\.\d+)?\/\d+(?:\.\d+)?$/, jl = /^(\d+(\.\d+)?)?(xs|sm|md|lg|xl)$/, Bl = /\d+(%|px|r?em|[sdl]?v([hwib]|min|max)|pt|pc|in|cm|mm|cap|ch|ex|r?lh|cq(w|h|i|b|min|max))|\b(calc|min|max|clamp)\(.+\)|^0$/, $l = /^(rgba?|hsla?|hwb|(ok)?(lab|lch)|color-mix)\(.+\)$/, Gl = /^(inset_)?-?((\d+)?\.?(\d+)[a-z]+|0)_-?((\d+)?\.?(\d+)[a-z]+|0)/, Ul = /^(url|image|image-set|cross-fade|element|(repeating-)?(linear|radial|conic)-gradient)\(.+\)$/, De = (e) => Wl.test(e), _ = (e) => !!e && !Number.isNaN(Number(e)), me = (e) => !!e && Number.isInteger(Number(e)), wn = (e) => e.endsWith("%") && _(e.slice(0, -1)), Ce = (e) => jl.test(e), ei = () => !0, Vl = (e) => (
  // `colorFunctionRegex` check is necessary because color functions can have percentages in them which which would be incorrectly classified as lengths.
  // For example, `hsl(0 0% 0%)` would be classified as a length without this check.
  // I could also use lookbehind assertion in `lengthUnitRegex` but that isn't supported widely enough.
  Bl.test(e) && !$l.test(e)
), Hn = () => !1, Hl = (e) => Gl.test(e), Xl = (e) => Ul.test(e), Yl = (e) => !P(e) && !A(e), Kl = (e) => e.startsWith("@container") && (e[10] === "/" && e[11] !== void 0 || e[11] === "s" && e[16] !== void 0 && e.startsWith("-size/", 10) || e[11] === "n" && e[18] !== void 0 && e.startsWith("-normal/", 10)), Zl = (e) => je(e, ri, Hn), P = (e) => Qo.test(e), Ge = (e) => je(e, oi, Vl), Ar = (e) => je(e, oc, _), ql = (e) => je(e, ai, ei), Ql = (e) => je(e, ii, Hn), Dr = (e) => je(e, ti, Hn), Jl = (e) => je(e, ni, Xl), Ot = (e) => je(e, si, Hl), A = (e) => Jo.test(e), mt = (e) => Ke(e, oi), ec = (e) => Ke(e, ii), Tr = (e) => Ke(e, ti), tc = (e) => Ke(e, ri), nc = (e) => Ke(e, ni), At = (e) => Ke(e, si, !0), rc = (e) => Ke(e, ai, !0), je = (e, t, n) => {
  const r = Qo.exec(e);
  return r ? r[1] ? t(r[1]) : n(r[2]) : !1;
}, Ke = (e, t, n = !1) => {
  const r = Jo.exec(e);
  return r ? r[1] ? t(r[1]) : n : !1;
}, ti = (e) => e === "position" || e === "percentage", ni = (e) => e === "image" || e === "url", ri = (e) => e === "length" || e === "size" || e === "bg-size", oi = (e) => e === "length", oc = (e) => e === "number", ii = (e) => e === "family-name", ai = (e) => e === "number" || e === "weight", si = (e) => e === "shadow", ic = () => {
  const e = Y("color"), t = Y("font"), n = Y("text"), r = Y("font-weight"), o = Y("tracking"), i = Y("leading"), a = Y("breakpoint"), s = Y("container"), u = Y("spacing"), c = Y("radius"), d = Y("shadow"), f = Y("inset-shadow"), m = Y("text-shadow"), h = Y("drop-shadow"), b = Y("blur"), p = Y("perspective"), g = Y("aspect"), w = Y("ease"), v = Y("animate"), x = () => ["auto", "avoid", "all", "avoid-page", "page", "left", "right", "column"], E = () => [
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
  ], S = () => [...E(), A, P], R = () => ["auto", "hidden", "clip", "visible", "scroll"], T = () => ["auto", "contain", "none"], y = () => [A, P, u], M = () => [De, "full", "auto", ...y()], W = () => [me, "none", "subgrid", A, P], O = () => ["auto", {
    span: ["full", me, A, P]
  }, me, A, P], L = () => [me, "auto", A, P], V = () => ["auto", "min", "max", "fr", A, P], j = () => ["start", "end", "center", "between", "around", "evenly", "stretch", "baseline", "center-safe", "end-safe"], X = () => ["start", "end", "center", "stretch", "center-safe", "end-safe"], I = () => ["auto", ...y()], B = () => [De, "auto", "full", "dvw", "dvh", "lvw", "lvh", "svw", "svh", "min", "max", "fit", ...y()], $ = () => [De, "screen", "full", "dvw", "lvw", "svw", "min", "max", "fit", ...y()], H = () => [De, "screen", "full", "lh", "dvh", "lvh", "svh", "min", "max", "fit", ...y()], k = () => [e, A, P], Ae = () => [...E(), Tr, Dr, {
    position: [A, P]
  }], xe = () => ["no-repeat", {
    repeat: ["", "x", "y", "space", "round"]
  }], fe = () => ["auto", "cover", "contain", tc, Zl, {
    size: [A, P]
  }], Z = () => [wn, mt, Ge], G = () => [
    // Deprecated since Tailwind CSS v4.0.0
    "",
    "none",
    "full",
    c,
    A,
    P
  ], U = () => ["", _, mt, Ge], re = () => ["solid", "dashed", "dotted", "double"], ke = () => ["normal", "multiply", "screen", "overlay", "darken", "lighten", "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation", "color", "luminosity"], z = () => [_, wn, Tr, Dr], $e = () => [
    // Deprecated since Tailwind CSS v4.0.0
    "",
    "none",
    b,
    A,
    P
  ], oe = () => ["none", _, A, P], ie = () => ["none", _, A, P], Ee = () => [_, A, P], ee = () => [De, "full", ...y()];
  return {
    cacheSize: 500,
    theme: {
      animate: ["spin", "ping", "pulse", "bounce"],
      aspect: ["video"],
      blur: [Ce],
      breakpoint: [Ce],
      color: [ei],
      container: [Ce],
      "drop-shadow": [Ce],
      ease: ["in", "out", "in-out"],
      font: [Yl],
      "font-weight": ["thin", "extralight", "light", "normal", "medium", "semibold", "bold", "extrabold", "black"],
      "inset-shadow": [Ce],
      leading: ["none", "tight", "snug", "normal", "relaxed", "loose"],
      perspective: ["dramatic", "near", "normal", "midrange", "distant", "none"],
      radius: [Ce],
      shadow: [Ce],
      spacing: ["px", _],
      text: [Ce],
      "text-shadow": [Ce],
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
        aspect: ["auto", "square", De, P, A, g]
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
        "@container": ["", "normal", "size", A, P]
      }],
      /**
       * Container Name
       * @see https://tailwindcss.com/docs/responsive-design#named-containers
       */
      "container-named": [Kl],
      /**
       * Columns
       * @see https://tailwindcss.com/docs/columns
       */
      columns: [{
        columns: [_, P, A, s]
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
        overflow: R()
      }],
      /**
       * Overflow X
       * @see https://tailwindcss.com/docs/overflow
       */
      "overflow-x": [{
        "overflow-x": R()
      }],
      /**
       * Overflow Y
       * @see https://tailwindcss.com/docs/overflow
       */
      "overflow-y": [{
        "overflow-y": R()
      }],
      /**
       * Overscroll Behavior
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      overscroll: [{
        overscroll: T()
      }],
      /**
       * Overscroll Behavior X
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      "overscroll-x": [{
        "overscroll-x": T()
      }],
      /**
       * Overscroll Behavior Y
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      "overscroll-y": [{
        "overscroll-y": T()
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
        z: [me, "auto", A, P]
      }],
      // ------------------------
      // --- Flexbox and Grid ---
      // ------------------------
      /**
       * Flex Basis
       * @see https://tailwindcss.com/docs/flex-basis
       */
      basis: [{
        basis: [De, "full", "auto", s, ...y()]
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
        flex: [_, De, "auto", "initial", "none", P]
      }],
      /**
       * Flex Grow
       * @see https://tailwindcss.com/docs/flex-grow
       */
      grow: [{
        grow: ["", _, A, P]
      }],
      /**
       * Flex Shrink
       * @see https://tailwindcss.com/docs/flex-shrink
       */
      shrink: [{
        shrink: ["", _, A, P]
      }],
      /**
       * Order
       * @see https://tailwindcss.com/docs/order
       */
      order: [{
        order: [me, "first", "last", "none", A, P]
      }],
      /**
       * Grid Template Columns
       * @see https://tailwindcss.com/docs/grid-template-columns
       */
      "grid-cols": [{
        "grid-cols": W()
      }],
      /**
       * Grid Column Start / End
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-start-end": [{
        col: O()
      }],
      /**
       * Grid Column Start
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-start": [{
        "col-start": L()
      }],
      /**
       * Grid Column End
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-end": [{
        "col-end": L()
      }],
      /**
       * Grid Template Rows
       * @see https://tailwindcss.com/docs/grid-template-rows
       */
      "grid-rows": [{
        "grid-rows": W()
      }],
      /**
       * Grid Row Start / End
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-start-end": [{
        row: O()
      }],
      /**
       * Grid Row Start
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-start": [{
        "row-start": L()
      }],
      /**
       * Grid Row End
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-end": [{
        "row-end": L()
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
        "auto-cols": V()
      }],
      /**
       * Grid Auto Rows
       * @see https://tailwindcss.com/docs/grid-auto-rows
       */
      "auto-rows": [{
        "auto-rows": V()
      }],
      /**
       * Gap
       * @see https://tailwindcss.com/docs/gap
       */
      gap: [{
        gap: y()
      }],
      /**
       * Gap X
       * @see https://tailwindcss.com/docs/gap
       */
      "gap-x": [{
        "gap-x": y()
      }],
      /**
       * Gap Y
       * @see https://tailwindcss.com/docs/gap
       */
      "gap-y": [{
        "gap-y": y()
      }],
      /**
       * Justify Content
       * @see https://tailwindcss.com/docs/justify-content
       */
      "justify-content": [{
        justify: [...j(), "normal"]
      }],
      /**
       * Justify Items
       * @see https://tailwindcss.com/docs/justify-items
       */
      "justify-items": [{
        "justify-items": [...X(), "normal"]
      }],
      /**
       * Justify Self
       * @see https://tailwindcss.com/docs/justify-self
       */
      "justify-self": [{
        "justify-self": ["auto", ...X()]
      }],
      /**
       * Align Content
       * @see https://tailwindcss.com/docs/align-content
       */
      "align-content": [{
        content: ["normal", ...j()]
      }],
      /**
       * Align Items
       * @see https://tailwindcss.com/docs/align-items
       */
      "align-items": [{
        items: [...X(), {
          baseline: ["", "last"]
        }]
      }],
      /**
       * Align Self
       * @see https://tailwindcss.com/docs/align-self
       */
      "align-self": [{
        self: ["auto", ...X(), {
          baseline: ["", "last"]
        }]
      }],
      /**
       * Place Content
       * @see https://tailwindcss.com/docs/place-content
       */
      "place-content": [{
        "place-content": j()
      }],
      /**
       * Place Items
       * @see https://tailwindcss.com/docs/place-items
       */
      "place-items": [{
        "place-items": [...X(), "baseline"]
      }],
      /**
       * Place Self
       * @see https://tailwindcss.com/docs/place-self
       */
      "place-self": [{
        "place-self": ["auto", ...X()]
      }],
      // Spacing
      /**
       * Padding
       * @see https://tailwindcss.com/docs/padding
       */
      p: [{
        p: y()
      }],
      /**
       * Padding Inline
       * @see https://tailwindcss.com/docs/padding
       */
      px: [{
        px: y()
      }],
      /**
       * Padding Block
       * @see https://tailwindcss.com/docs/padding
       */
      py: [{
        py: y()
      }],
      /**
       * Padding Inline Start
       * @see https://tailwindcss.com/docs/padding
       */
      ps: [{
        ps: y()
      }],
      /**
       * Padding Inline End
       * @see https://tailwindcss.com/docs/padding
       */
      pe: [{
        pe: y()
      }],
      /**
       * Padding Block Start
       * @see https://tailwindcss.com/docs/padding
       */
      pbs: [{
        pbs: y()
      }],
      /**
       * Padding Block End
       * @see https://tailwindcss.com/docs/padding
       */
      pbe: [{
        pbe: y()
      }],
      /**
       * Padding Top
       * @see https://tailwindcss.com/docs/padding
       */
      pt: [{
        pt: y()
      }],
      /**
       * Padding Right
       * @see https://tailwindcss.com/docs/padding
       */
      pr: [{
        pr: y()
      }],
      /**
       * Padding Bottom
       * @see https://tailwindcss.com/docs/padding
       */
      pb: [{
        pb: y()
      }],
      /**
       * Padding Left
       * @see https://tailwindcss.com/docs/padding
       */
      pl: [{
        pl: y()
      }],
      /**
       * Margin
       * @see https://tailwindcss.com/docs/margin
       */
      m: [{
        m: I()
      }],
      /**
       * Margin Inline
       * @see https://tailwindcss.com/docs/margin
       */
      mx: [{
        mx: I()
      }],
      /**
       * Margin Block
       * @see https://tailwindcss.com/docs/margin
       */
      my: [{
        my: I()
      }],
      /**
       * Margin Inline Start
       * @see https://tailwindcss.com/docs/margin
       */
      ms: [{
        ms: I()
      }],
      /**
       * Margin Inline End
       * @see https://tailwindcss.com/docs/margin
       */
      me: [{
        me: I()
      }],
      /**
       * Margin Block Start
       * @see https://tailwindcss.com/docs/margin
       */
      mbs: [{
        mbs: I()
      }],
      /**
       * Margin Block End
       * @see https://tailwindcss.com/docs/margin
       */
      mbe: [{
        mbe: I()
      }],
      /**
       * Margin Top
       * @see https://tailwindcss.com/docs/margin
       */
      mt: [{
        mt: I()
      }],
      /**
       * Margin Right
       * @see https://tailwindcss.com/docs/margin
       */
      mr: [{
        mr: I()
      }],
      /**
       * Margin Bottom
       * @see https://tailwindcss.com/docs/margin
       */
      mb: [{
        mb: I()
      }],
      /**
       * Margin Left
       * @see https://tailwindcss.com/docs/margin
       */
      ml: [{
        ml: I()
      }],
      /**
       * Space Between X
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-x": [{
        "space-x": y()
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
        "space-y": y()
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
        size: B()
      }],
      /**
       * Inline Size
       * @see https://tailwindcss.com/docs/width
       */
      "inline-size": [{
        inline: ["auto", ...$()]
      }],
      /**
       * Min-Inline Size
       * @see https://tailwindcss.com/docs/min-width
       */
      "min-inline-size": [{
        "min-inline": ["auto", ...$()]
      }],
      /**
       * Max-Inline Size
       * @see https://tailwindcss.com/docs/max-width
       */
      "max-inline-size": [{
        "max-inline": ["none", ...$()]
      }],
      /**
       * Block Size
       * @see https://tailwindcss.com/docs/height
       */
      "block-size": [{
        block: ["auto", ...H()]
      }],
      /**
       * Min-Block Size
       * @see https://tailwindcss.com/docs/min-height
       */
      "min-block-size": [{
        "min-block": ["auto", ...H()]
      }],
      /**
       * Max-Block Size
       * @see https://tailwindcss.com/docs/max-height
       */
      "max-block-size": [{
        "max-block": ["none", ...H()]
      }],
      /**
       * Width
       * @see https://tailwindcss.com/docs/width
       */
      w: [{
        w: [s, "screen", ...B()]
      }],
      /**
       * Min-Width
       * @see https://tailwindcss.com/docs/min-width
       */
      "min-w": [{
        "min-w": [
          s,
          "screen",
          /** Deprecated. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          "none",
          ...B()
        ]
      }],
      /**
       * Max-Width
       * @see https://tailwindcss.com/docs/max-width
       */
      "max-w": [{
        "max-w": [
          s,
          "screen",
          "none",
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          "prose",
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          {
            screen: [a]
          },
          ...B()
        ]
      }],
      /**
       * Height
       * @see https://tailwindcss.com/docs/height
       */
      h: [{
        h: ["screen", "lh", ...B()]
      }],
      /**
       * Min-Height
       * @see https://tailwindcss.com/docs/min-height
       */
      "min-h": [{
        "min-h": ["screen", "lh", "none", ...B()]
      }],
      /**
       * Max-Height
       * @see https://tailwindcss.com/docs/max-height
       */
      "max-h": [{
        "max-h": ["screen", "lh", ...B()]
      }],
      // ------------------
      // --- Typography ---
      // ------------------
      /**
       * Font Size
       * @see https://tailwindcss.com/docs/font-size
       */
      "font-size": [{
        text: ["base", n, mt, Ge]
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
        font: [r, rc, ql]
      }],
      /**
       * Font Stretch
       * @see https://tailwindcss.com/docs/font-stretch
       */
      "font-stretch": [{
        "font-stretch": ["ultra-condensed", "extra-condensed", "condensed", "semi-condensed", "normal", "semi-expanded", "expanded", "extra-expanded", "ultra-expanded", wn, P]
      }],
      /**
       * Font Family
       * @see https://tailwindcss.com/docs/font-family
       */
      "font-family": [{
        font: [ec, Ql, t]
      }],
      /**
       * Font Feature Settings
       * @see https://tailwindcss.com/docs/font-feature-settings
       */
      "font-features": [{
        "font-features": [P]
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
        tracking: [o, A, P]
      }],
      /**
       * Line Clamp
       * @see https://tailwindcss.com/docs/line-clamp
       */
      "line-clamp": [{
        "line-clamp": [_, "none", A, Ar]
      }],
      /**
       * Line Height
       * @see https://tailwindcss.com/docs/line-height
       */
      leading: [{
        leading: [
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          i,
          ...y()
        ]
      }],
      /**
       * List Style Image
       * @see https://tailwindcss.com/docs/list-style-image
       */
      "list-image": [{
        "list-image": ["none", A, P]
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
        list: ["disc", "decimal", "none", A, P]
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
        placeholder: k()
      }],
      /**
       * Text Color
       * @see https://tailwindcss.com/docs/text-color
       */
      "text-color": [{
        text: k()
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
        decoration: [...re(), "wavy"]
      }],
      /**
       * Text Decoration Thickness
       * @see https://tailwindcss.com/docs/text-decoration-thickness
       */
      "text-decoration-thickness": [{
        decoration: [_, "from-font", "auto", A, Ge]
      }],
      /**
       * Text Decoration Color
       * @see https://tailwindcss.com/docs/text-decoration-color
       */
      "text-decoration-color": [{
        decoration: k()
      }],
      /**
       * Text Underline Offset
       * @see https://tailwindcss.com/docs/text-underline-offset
       */
      "underline-offset": [{
        "underline-offset": [_, "auto", A, P]
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
        indent: y()
      }],
      /**
       * Tab Size
       * @see https://tailwindcss.com/docs/tab-size
       */
      "tab-size": [{
        tab: [me, A, P]
      }],
      /**
       * Vertical Alignment
       * @see https://tailwindcss.com/docs/vertical-align
       */
      "vertical-align": [{
        align: ["baseline", "top", "middle", "bottom", "text-top", "text-bottom", "sub", "super", A, P]
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
        content: ["none", A, P]
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
        bg: Ae()
      }],
      /**
       * Background Repeat
       * @see https://tailwindcss.com/docs/background-repeat
       */
      "bg-repeat": [{
        bg: xe()
      }],
      /**
       * Background Size
       * @see https://tailwindcss.com/docs/background-size
       */
      "bg-size": [{
        bg: fe()
      }],
      /**
       * Background Image
       * @see https://tailwindcss.com/docs/background-image
       */
      "bg-image": [{
        bg: ["none", {
          linear: [{
            to: ["t", "tr", "r", "br", "b", "bl", "l", "tl"]
          }, me, A, P],
          radial: ["", A, P],
          conic: [me, A, P]
        }, nc, Jl]
      }],
      /**
       * Background Color
       * @see https://tailwindcss.com/docs/background-color
       */
      "bg-color": [{
        bg: k()
      }],
      /**
       * Gradient Color Stops From Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-from-pos": [{
        from: Z()
      }],
      /**
       * Gradient Color Stops Via Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-via-pos": [{
        via: Z()
      }],
      /**
       * Gradient Color Stops To Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-to-pos": [{
        to: Z()
      }],
      /**
       * Gradient Color Stops From
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-from": [{
        from: k()
      }],
      /**
       * Gradient Color Stops Via
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-via": [{
        via: k()
      }],
      /**
       * Gradient Color Stops To
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-to": [{
        to: k()
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
        border: [...re(), "hidden", "none"]
      }],
      /**
       * Divide Style
       * @see https://tailwindcss.com/docs/border-style#setting-the-divider-style
       */
      "divide-style": [{
        divide: [...re(), "hidden", "none"]
      }],
      /**
       * Border Color
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color": [{
        border: k()
      }],
      /**
       * Border Color Inline
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-x": [{
        "border-x": k()
      }],
      /**
       * Border Color Block
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-y": [{
        "border-y": k()
      }],
      /**
       * Border Color Inline Start
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-s": [{
        "border-s": k()
      }],
      /**
       * Border Color Inline End
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-e": [{
        "border-e": k()
      }],
      /**
       * Border Color Block Start
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-bs": [{
        "border-bs": k()
      }],
      /**
       * Border Color Block End
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-be": [{
        "border-be": k()
      }],
      /**
       * Border Color Top
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-t": [{
        "border-t": k()
      }],
      /**
       * Border Color Right
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-r": [{
        "border-r": k()
      }],
      /**
       * Border Color Bottom
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-b": [{
        "border-b": k()
      }],
      /**
       * Border Color Left
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-l": [{
        "border-l": k()
      }],
      /**
       * Divide Color
       * @see https://tailwindcss.com/docs/divide-color
       */
      "divide-color": [{
        divide: k()
      }],
      /**
       * Outline Style
       * @see https://tailwindcss.com/docs/outline-style
       */
      "outline-style": [{
        outline: [...re(), "none", "hidden"]
      }],
      /**
       * Outline Offset
       * @see https://tailwindcss.com/docs/outline-offset
       */
      "outline-offset": [{
        "outline-offset": [_, A, P]
      }],
      /**
       * Outline Width
       * @see https://tailwindcss.com/docs/outline-width
       */
      "outline-w": [{
        outline: ["", _, mt, Ge]
      }],
      /**
       * Outline Color
       * @see https://tailwindcss.com/docs/outline-color
       */
      "outline-color": [{
        outline: k()
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
          d,
          At,
          Ot
        ]
      }],
      /**
       * Box Shadow Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-shadow-color
       */
      "shadow-color": [{
        shadow: k()
      }],
      /**
       * Inset Box Shadow
       * @see https://tailwindcss.com/docs/box-shadow#adding-an-inset-shadow
       */
      "inset-shadow": [{
        "inset-shadow": ["none", f, At, Ot]
      }],
      /**
       * Inset Box Shadow Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-inset-shadow-color
       */
      "inset-shadow-color": [{
        "inset-shadow": k()
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
        ring: k()
      }],
      /**
       * Ring Offset Width
       * @see https://v3.tailwindcss.com/docs/ring-offset-width
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-offset-w": [{
        "ring-offset": [_, Ge]
      }],
      /**
       * Ring Offset Color
       * @see https://v3.tailwindcss.com/docs/ring-offset-color
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-offset-color": [{
        "ring-offset": k()
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
        "inset-ring": k()
      }],
      /**
       * Text Shadow
       * @see https://tailwindcss.com/docs/text-shadow
       */
      "text-shadow": [{
        "text-shadow": ["none", m, At, Ot]
      }],
      /**
       * Text Shadow Color
       * @see https://tailwindcss.com/docs/text-shadow#setting-the-shadow-color
       */
      "text-shadow-color": [{
        "text-shadow": k()
      }],
      /**
       * Opacity
       * @see https://tailwindcss.com/docs/opacity
       */
      opacity: [{
        opacity: [_, A, P]
      }],
      /**
       * Mix Blend Mode
       * @see https://tailwindcss.com/docs/mix-blend-mode
       */
      "mix-blend": [{
        "mix-blend": [...ke(), "plus-darker", "plus-lighter"]
      }],
      /**
       * Background Blend Mode
       * @see https://tailwindcss.com/docs/background-blend-mode
       */
      "bg-blend": [{
        "bg-blend": ke()
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
        "mask-linear": [_]
      }],
      "mask-image-linear-from-pos": [{
        "mask-linear-from": z()
      }],
      "mask-image-linear-to-pos": [{
        "mask-linear-to": z()
      }],
      "mask-image-linear-from-color": [{
        "mask-linear-from": k()
      }],
      "mask-image-linear-to-color": [{
        "mask-linear-to": k()
      }],
      "mask-image-t-from-pos": [{
        "mask-t-from": z()
      }],
      "mask-image-t-to-pos": [{
        "mask-t-to": z()
      }],
      "mask-image-t-from-color": [{
        "mask-t-from": k()
      }],
      "mask-image-t-to-color": [{
        "mask-t-to": k()
      }],
      "mask-image-r-from-pos": [{
        "mask-r-from": z()
      }],
      "mask-image-r-to-pos": [{
        "mask-r-to": z()
      }],
      "mask-image-r-from-color": [{
        "mask-r-from": k()
      }],
      "mask-image-r-to-color": [{
        "mask-r-to": k()
      }],
      "mask-image-b-from-pos": [{
        "mask-b-from": z()
      }],
      "mask-image-b-to-pos": [{
        "mask-b-to": z()
      }],
      "mask-image-b-from-color": [{
        "mask-b-from": k()
      }],
      "mask-image-b-to-color": [{
        "mask-b-to": k()
      }],
      "mask-image-l-from-pos": [{
        "mask-l-from": z()
      }],
      "mask-image-l-to-pos": [{
        "mask-l-to": z()
      }],
      "mask-image-l-from-color": [{
        "mask-l-from": k()
      }],
      "mask-image-l-to-color": [{
        "mask-l-to": k()
      }],
      "mask-image-x-from-pos": [{
        "mask-x-from": z()
      }],
      "mask-image-x-to-pos": [{
        "mask-x-to": z()
      }],
      "mask-image-x-from-color": [{
        "mask-x-from": k()
      }],
      "mask-image-x-to-color": [{
        "mask-x-to": k()
      }],
      "mask-image-y-from-pos": [{
        "mask-y-from": z()
      }],
      "mask-image-y-to-pos": [{
        "mask-y-to": z()
      }],
      "mask-image-y-from-color": [{
        "mask-y-from": k()
      }],
      "mask-image-y-to-color": [{
        "mask-y-to": k()
      }],
      "mask-image-radial": [{
        "mask-radial": [A, P]
      }],
      "mask-image-radial-from-pos": [{
        "mask-radial-from": z()
      }],
      "mask-image-radial-to-pos": [{
        "mask-radial-to": z()
      }],
      "mask-image-radial-from-color": [{
        "mask-radial-from": k()
      }],
      "mask-image-radial-to-color": [{
        "mask-radial-to": k()
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
        "mask-radial-at": E()
      }],
      "mask-image-conic-pos": [{
        "mask-conic": [_]
      }],
      "mask-image-conic-from-pos": [{
        "mask-conic-from": z()
      }],
      "mask-image-conic-to-pos": [{
        "mask-conic-to": z()
      }],
      "mask-image-conic-from-color": [{
        "mask-conic-from": k()
      }],
      "mask-image-conic-to-color": [{
        "mask-conic-to": k()
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
        mask: Ae()
      }],
      /**
       * Mask Repeat
       * @see https://tailwindcss.com/docs/mask-repeat
       */
      "mask-repeat": [{
        mask: xe()
      }],
      /**
       * Mask Size
       * @see https://tailwindcss.com/docs/mask-size
       */
      "mask-size": [{
        mask: fe()
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
        mask: ["none", A, P]
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
          A,
          P
        ]
      }],
      /**
       * Blur
       * @see https://tailwindcss.com/docs/blur
       */
      blur: [{
        blur: $e()
      }],
      /**
       * Brightness
       * @see https://tailwindcss.com/docs/brightness
       */
      brightness: [{
        brightness: [_, A, P]
      }],
      /**
       * Contrast
       * @see https://tailwindcss.com/docs/contrast
       */
      contrast: [{
        contrast: [_, A, P]
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
          h,
          At,
          Ot
        ]
      }],
      /**
       * Drop Shadow Color
       * @see https://tailwindcss.com/docs/filter-drop-shadow#setting-the-shadow-color
       */
      "drop-shadow-color": [{
        "drop-shadow": k()
      }],
      /**
       * Grayscale
       * @see https://tailwindcss.com/docs/grayscale
       */
      grayscale: [{
        grayscale: ["", _, A, P]
      }],
      /**
       * Hue Rotate
       * @see https://tailwindcss.com/docs/hue-rotate
       */
      "hue-rotate": [{
        "hue-rotate": [_, A, P]
      }],
      /**
       * Invert
       * @see https://tailwindcss.com/docs/invert
       */
      invert: [{
        invert: ["", _, A, P]
      }],
      /**
       * Saturate
       * @see https://tailwindcss.com/docs/saturate
       */
      saturate: [{
        saturate: [_, A, P]
      }],
      /**
       * Sepia
       * @see https://tailwindcss.com/docs/sepia
       */
      sepia: [{
        sepia: ["", _, A, P]
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
          A,
          P
        ]
      }],
      /**
       * Backdrop Blur
       * @see https://tailwindcss.com/docs/backdrop-blur
       */
      "backdrop-blur": [{
        "backdrop-blur": $e()
      }],
      /**
       * Backdrop Brightness
       * @see https://tailwindcss.com/docs/backdrop-brightness
       */
      "backdrop-brightness": [{
        "backdrop-brightness": [_, A, P]
      }],
      /**
       * Backdrop Contrast
       * @see https://tailwindcss.com/docs/backdrop-contrast
       */
      "backdrop-contrast": [{
        "backdrop-contrast": [_, A, P]
      }],
      /**
       * Backdrop Grayscale
       * @see https://tailwindcss.com/docs/backdrop-grayscale
       */
      "backdrop-grayscale": [{
        "backdrop-grayscale": ["", _, A, P]
      }],
      /**
       * Backdrop Hue Rotate
       * @see https://tailwindcss.com/docs/backdrop-hue-rotate
       */
      "backdrop-hue-rotate": [{
        "backdrop-hue-rotate": [_, A, P]
      }],
      /**
       * Backdrop Invert
       * @see https://tailwindcss.com/docs/backdrop-invert
       */
      "backdrop-invert": [{
        "backdrop-invert": ["", _, A, P]
      }],
      /**
       * Backdrop Opacity
       * @see https://tailwindcss.com/docs/backdrop-opacity
       */
      "backdrop-opacity": [{
        "backdrop-opacity": [_, A, P]
      }],
      /**
       * Backdrop Saturate
       * @see https://tailwindcss.com/docs/backdrop-saturate
       */
      "backdrop-saturate": [{
        "backdrop-saturate": [_, A, P]
      }],
      /**
       * Backdrop Sepia
       * @see https://tailwindcss.com/docs/backdrop-sepia
       */
      "backdrop-sepia": [{
        "backdrop-sepia": ["", _, A, P]
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
        "border-spacing": y()
      }],
      /**
       * Border Spacing X
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing-x": [{
        "border-spacing-x": y()
      }],
      /**
       * Border Spacing Y
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing-y": [{
        "border-spacing-y": y()
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
        transition: ["", "all", "colors", "opacity", "shadow", "transform", "none", A, P]
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
        duration: [_, "initial", A, P]
      }],
      /**
       * Transition Timing Function
       * @see https://tailwindcss.com/docs/transition-timing-function
       */
      ease: [{
        ease: ["linear", "initial", w, A, P]
      }],
      /**
       * Transition Delay
       * @see https://tailwindcss.com/docs/transition-delay
       */
      delay: [{
        delay: [_, A, P]
      }],
      /**
       * Animation
       * @see https://tailwindcss.com/docs/animation
       */
      animate: [{
        animate: ["none", v, A, P]
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
        perspective: [p, A, P]
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
        rotate: oe()
      }],
      /**
       * Rotate X
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-x": [{
        "rotate-x": oe()
      }],
      /**
       * Rotate Y
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-y": [{
        "rotate-y": oe()
      }],
      /**
       * Rotate Z
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-z": [{
        "rotate-z": oe()
      }],
      /**
       * Scale
       * @see https://tailwindcss.com/docs/scale
       */
      scale: [{
        scale: ie()
      }],
      /**
       * Scale X
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-x": [{
        "scale-x": ie()
      }],
      /**
       * Scale Y
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-y": [{
        "scale-y": ie()
      }],
      /**
       * Scale Z
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-z": [{
        "scale-z": ie()
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
        skew: Ee()
      }],
      /**
       * Skew X
       * @see https://tailwindcss.com/docs/skew
       */
      "skew-x": [{
        "skew-x": Ee()
      }],
      /**
       * Skew Y
       * @see https://tailwindcss.com/docs/skew
       */
      "skew-y": [{
        "skew-y": Ee()
      }],
      /**
       * Transform
       * @see https://tailwindcss.com/docs/transform
       */
      transform: [{
        transform: [A, P, "", "none", "gpu", "cpu"]
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
        translate: ee()
      }],
      /**
       * Translate X
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-x": [{
        "translate-x": ee()
      }],
      /**
       * Translate Y
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-y": [{
        "translate-y": ee()
      }],
      /**
       * Translate Z
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-z": [{
        "translate-z": ee()
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
        zoom: [me, A, P]
      }],
      // ---------------------
      // --- Interactivity ---
      // ---------------------
      /**
       * Accent Color
       * @see https://tailwindcss.com/docs/accent-color
       */
      accent: [{
        accent: k()
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
        caret: k()
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
        cursor: ["auto", "default", "pointer", "wait", "text", "move", "help", "not-allowed", "none", "context-menu", "progress", "cell", "crosshair", "vertical-text", "alias", "copy", "no-drop", "grab", "grabbing", "all-scroll", "col-resize", "row-resize", "n-resize", "e-resize", "s-resize", "w-resize", "ne-resize", "nw-resize", "se-resize", "sw-resize", "ew-resize", "ns-resize", "nesw-resize", "nwse-resize", "zoom-in", "zoom-out", A, P]
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
        "scrollbar-thumb": k()
      }],
      /**
       * Scrollbar Track Color
       * @see https://tailwindcss.com/docs/scrollbar-color
       */
      "scrollbar-track-color": [{
        "scrollbar-track": k()
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
        "scroll-m": y()
      }],
      /**
       * Scroll Margin Inline
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mx": [{
        "scroll-mx": y()
      }],
      /**
       * Scroll Margin Block
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-my": [{
        "scroll-my": y()
      }],
      /**
       * Scroll Margin Inline Start
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-ms": [{
        "scroll-ms": y()
      }],
      /**
       * Scroll Margin Inline End
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-me": [{
        "scroll-me": y()
      }],
      /**
       * Scroll Margin Block Start
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mbs": [{
        "scroll-mbs": y()
      }],
      /**
       * Scroll Margin Block End
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mbe": [{
        "scroll-mbe": y()
      }],
      /**
       * Scroll Margin Top
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mt": [{
        "scroll-mt": y()
      }],
      /**
       * Scroll Margin Right
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mr": [{
        "scroll-mr": y()
      }],
      /**
       * Scroll Margin Bottom
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mb": [{
        "scroll-mb": y()
      }],
      /**
       * Scroll Margin Left
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-ml": [{
        "scroll-ml": y()
      }],
      /**
       * Scroll Padding
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-p": [{
        "scroll-p": y()
      }],
      /**
       * Scroll Padding Inline
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-px": [{
        "scroll-px": y()
      }],
      /**
       * Scroll Padding Block
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-py": [{
        "scroll-py": y()
      }],
      /**
       * Scroll Padding Inline Start
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-ps": [{
        "scroll-ps": y()
      }],
      /**
       * Scroll Padding Inline End
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pe": [{
        "scroll-pe": y()
      }],
      /**
       * Scroll Padding Block Start
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pbs": [{
        "scroll-pbs": y()
      }],
      /**
       * Scroll Padding Block End
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pbe": [{
        "scroll-pbe": y()
      }],
      /**
       * Scroll Padding Top
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pt": [{
        "scroll-pt": y()
      }],
      /**
       * Scroll Padding Right
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pr": [{
        "scroll-pr": y()
      }],
      /**
       * Scroll Padding Bottom
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pb": [{
        "scroll-pb": y()
      }],
      /**
       * Scroll Padding Left
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pl": [{
        "scroll-pl": y()
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
        "will-change": ["auto", "scroll", "contents", "transform", A, P]
      }],
      // -----------
      // --- SVG ---
      // -----------
      /**
       * Fill
       * @see https://tailwindcss.com/docs/fill
       */
      fill: [{
        fill: ["none", ...k()]
      }],
      /**
       * Stroke Width
       * @see https://tailwindcss.com/docs/stroke-width
       */
      "stroke-w": [{
        stroke: [_, mt, Ge, Ar]
      }],
      /**
       * Stroke
       * @see https://tailwindcss.com/docs/stroke
       */
      stroke: [{
        stroke: ["none", ...k()]
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
}, ac = /* @__PURE__ */ _l(ic);
function ae(...e) {
  return ac(hl(e));
}
function sc({ ...e }) {
  return /* @__PURE__ */ C(Io, { ...e });
}
function lc({ ...e }) {
  return /* @__PURE__ */ C(Fo, { ...e });
}
const cc = l.forwardRef(function({ className: t, ...n }, r) {
  return /* @__PURE__ */ C(
    Wo,
    {
      ref: r,
      className: ae("fixed inset-0 z-50 bg-black/50", t),
      ...n
    }
  );
}), uc = l.forwardRef(function({ className: t, children: n, ...r }, o) {
  return /* @__PURE__ */ Q(lc, { children: [
    /* @__PURE__ */ C(cc, {}),
    /* @__PURE__ */ C(
      jo,
      {
        ref: o,
        className: ae(
          "lb-panel fixed inset-y-0 right-0 z-50 flex h-full max-w-[95vw] flex-col border-l border-lbp-border bg-lbp-panel font-sans text-lbp-fg shadow-2xl outline-none",
          t
        ),
        ...r,
        children: n
      }
    )
  ] });
}), dc = l.forwardRef(function({ className: t, ...n }, r) {
  return /* @__PURE__ */ C(Go, { ref: r, className: ae("text-base font-semibold text-lbp-fg", t), ...n });
}), fc = l.forwardRef(function({ className: t, ...n }, r) {
  return /* @__PURE__ */ C(Vo, { ref: r, className: ae("text-xs text-lbp-muted", t), ...n });
});
function pc({ resizable: e, className: t, "aria-label": n = "resize panel" }) {
  return /* @__PURE__ */ C(
    "div",
    {
      role: "separator",
      "aria-orientation": "vertical",
      "aria-label": n,
      tabIndex: 0,
      ...e.handleProps,
      className: ae(
        "group absolute left-0 top-0 z-10 h-full w-1.5 -translate-x-1/2 cursor-col-resize touch-none select-none",
        "outline-none",
        t
      ),
      children: /* @__PURE__ */ C(
        "div",
        {
          className: ae(
            "mx-auto h-full w-px bg-lbp-border transition-colors",
            "group-hover:w-0.5 group-hover:bg-lbp-accent group-focus-visible:w-0.5 group-focus-visible:bg-lbp-accent",
            e.dragging && "w-0.5 bg-lbp-accent"
          )
        }
      )
    }
  );
}
function mc({ initial: e, min: t, max: n, step: r = 24 }) {
  const o = pt((b) => Math.min(n, Math.max(t, b)), [t, n]), [i, a] = Ht(() => o(e)), [s, u] = Ht(!1), c = Da(null), d = pt(
    (b) => {
      c.current = { x: b.clientX, w: i }, u(!0), b.currentTarget.setPointerCapture(b.pointerId), b.preventDefault();
    },
    [i]
  ), f = pt(
    (b) => {
      if (!c.current) return;
      const p = c.current.x - b.clientX;
      a(o(c.current.w + p));
    },
    [o]
  ), m = pt((b) => {
    c.current = null, u(!1), b.currentTarget.hasPointerCapture(b.pointerId) && b.currentTarget.releasePointerCapture(b.pointerId);
  }, []), h = pt(
    (b) => {
      b.key === "ArrowLeft" ? (a((p) => o(p + r)), b.preventDefault()) : b.key === "ArrowRight" && (a((p) => o(p - r)), b.preventDefault());
    },
    [o, r]
  );
  return { width: i, dragging: s, handleProps: { onPointerDown: d, onPointerMove: f, onPointerUp: m, onKeyDown: h } };
}
function Rp({
  open: e,
  onOpenChange: t,
  title: n,
  description: r,
  headerAside: o,
  footer: i,
  "aria-label": a,
  initialWidth: s = 720,
  minWidth: u = 360,
  maxWidth: c = 1200,
  className: d,
  children: f
}) {
  const m = mc({ initial: s, min: u, max: c });
  return /* @__PURE__ */ C(sc, { open: e, onOpenChange: t, children: /* @__PURE__ */ Q(
    uc,
    {
      "aria-label": a,
      style: { width: m.width },
      className: ae(m.dragging && "select-none", d),
      children: [
        /* @__PURE__ */ C(pc, { resizable: m }),
        /* @__PURE__ */ Q("header", { className: "flex items-start justify-between gap-3 border-b border-lbp-border bg-lbp-secondary px-4 py-3", children: [
          /* @__PURE__ */ Q("div", { className: "min-w-0", children: [
            /* @__PURE__ */ C(dc, { children: n }),
            r ? /* @__PURE__ */ C(fc, { className: "mt-0.5", children: r }) : null
          ] }),
          o ? /* @__PURE__ */ C("div", { className: "shrink-0", children: o }) : null
        ] }),
        /* @__PURE__ */ C("div", { className: "min-h-0 flex-1 overflow-auto", children: f }),
        i ? /* @__PURE__ */ C("footer", { className: "flex items-center justify-end gap-2 border-t border-lbp-border bg-lbp-secondary px-4 py-3", children: i }) : null
      ]
    }
  ) });
}
function Pp({ title: e, aside: t, className: n, children: r }) {
  return /* @__PURE__ */ Q("section", { className: ae("mb-4 last:mb-0", n), children: [
    /* @__PURE__ */ Q("div", { className: "mb-1.5 flex items-center justify-between gap-2", children: [
      /* @__PURE__ */ C("div", { className: "text-[10px] font-semibold uppercase tracking-wide text-lbp-muted", children: e }),
      t
    ] }),
    r
  ] });
}
function Np({ columns: e, rows: t, empty: n = "—", className: r }) {
  return t.length === 0 ? /* @__PURE__ */ C("div", { className: "py-1 font-mono text-[11px] text-lbp-muted", children: n }) : /* @__PURE__ */ Q("table", { className: ae("w-full border-collapse font-mono text-[11px] tabular-nums", r), children: [
    /* @__PURE__ */ C("thead", { children: /* @__PURE__ */ C("tr", { className: "text-left text-lbp-muted", children: e.map((o) => /* @__PURE__ */ C("th", { className: "px-0 pb-1 pr-2 font-medium", children: o.header ?? o.key }, o.key)) }) }),
    /* @__PURE__ */ C("tbody", { children: t.map((o) => /* @__PURE__ */ C("tr", { className: "border-t border-lbp-border align-top", children: e.map((i) => {
      const a = o.cells[i.key], s = i.ellipsize && typeof a == "string" ? a : void 0;
      return /* @__PURE__ */ C(
        "td",
        {
          title: s,
          style: i.maxWidth ? { maxWidth: i.maxWidth } : void 0,
          className: ae(
            "py-[3px] pr-2 pt-[3px]",
            i.ellipsize && "overflow-hidden text-ellipsis whitespace-nowrap",
            o.tone === "warn" && "text-lbp-amber",
            i.className
          ),
          children: a ?? "—"
        },
        i.key
      );
    }) }, o.id)) })
  ] });
}
function Op({ k: e, v: t, keyWidth: n = 80, className: r }) {
  return /* @__PURE__ */ Q("div", { className: ae("flex gap-2 py-[2px] font-mono text-[11px]", r), children: [
    /* @__PURE__ */ C("span", { style: { width: n }, className: "shrink-0 text-lbp-muted", children: e }),
    /* @__PURE__ */ C("span", { className: "min-w-0 break-words text-lbp-fg", children: t })
  ] });
}
function hc(e) {
  const t = [], n = /* @__PURE__ */ new Map();
  for (const r of e)
    n.has(r.group) || (n.set(r.group, []), t.push(r.group)), n.get(r.group).push(r);
  return t.map((r) => ({ label: r, items: n.get(r) }));
}
function Mr(e, t) {
  if (typeof e == "function")
    return e(t);
  e != null && (e.current = t);
}
function gc(...e) {
  return (t) => {
    let n = !1;
    const r = e.map((o) => {
      const i = Mr(o, t);
      return !n && typeof i == "function" && (n = !0), i;
    });
    if (n)
      return () => {
        for (let o = 0; o < r.length; o++) {
          const i = r[o];
          typeof i == "function" ? i() : Mr(e[o], null);
        }
      };
  };
}
function ue(...e) {
  return l.useCallback(gc(...e), e);
}
// @__NO_SIDE_EFFECTS__
function Xn(e) {
  const t = l.forwardRef((n, r) => {
    let { children: o, ...i } = n, a = null, s = !1;
    const u = [];
    Lr(o) && typeof Dt == "function" && (o = Dt(o._payload)), l.Children.forEach(o, (m) => {
      var h;
      if (kc(m)) {
        s = !0;
        const b = m;
        let p = "child" in b.props ? b.props.child : b.props.children;
        Lr(p) && typeof Dt == "function" && (p = Dt(p._payload)), a = yc(b, p), u.push((h = a == null ? void 0 : a.props) == null ? void 0 : h.children);
      } else
        u.push(m);
    }), a ? a = l.cloneElement(a, void 0, u) : (
      // A `Slottable` was found but it didn't resolve to a single element (e.g.
      // it wrapped multiple elements, text, or a render-prop `child` that
      // wasn't an element). Don't fall back to treating the `Slottable` wrapper
      // itself as the slot target — throw a descriptive error below instead.
      !s && l.Children.count(o) === 1 && l.isValidElement(o) && (a = o)
    );
    const c = a ? xc(a) : void 0, d = ue(r, c);
    if (!a) {
      if (o || o === 0)
        throw new Error(
          s ? Rc(e) : Sc(e)
        );
      return o;
    }
    const f = wc(i, a.props ?? {});
    return a.type !== l.Fragment && (f.ref = r ? d : c), l.cloneElement(a, f);
  });
  return t.displayName = `${e}.Slot`, t;
}
var bc = /* @__PURE__ */ Xn("Slot"), li = Symbol.for("radix.slottable");
// @__NO_SIDE_EFFECTS__
function vc(e) {
  const t = (n) => "child" in n ? n.children(n.child) : n.children;
  return t.displayName = `${e}.Slottable`, t.__radixId = li, t;
}
var yc = (e, t) => {
  if ("child" in e.props) {
    const n = e.props.child;
    return l.isValidElement(n) ? l.cloneElement(n, void 0, e.props.children(n.props.children)) : null;
  }
  return l.isValidElement(t) ? t : null;
};
function wc(e, t) {
  const n = { ...t };
  for (const r in t) {
    const o = e[r], i = t[r];
    /^on[A-Z]/.test(r) ? o && i ? n[r] = (...a) => {
      const s = i(...a);
      return o(...a), s;
    } : o && (n[r] = o) : r === "style" ? n[r] = { ...o, ...i } : r === "className" && (n[r] = [o, i].filter(Boolean).join(" "));
  }
  return { ...e, ...n };
}
function xc(e) {
  var t, n;
  let r = (t = Object.getOwnPropertyDescriptor(e.props, "ref")) == null ? void 0 : t.get, o = r && "isReactWarning" in r && r.isReactWarning;
  return o ? e.ref : (r = (n = Object.getOwnPropertyDescriptor(e, "ref")) == null ? void 0 : n.get, o = r && "isReactWarning" in r && r.isReactWarning, o ? e.props.ref : e.props.ref || e.ref);
}
function kc(e) {
  return l.isValidElement(e) && typeof e.type == "function" && "__radixId" in e.type && e.type.__radixId === li;
}
var Ec = Symbol.for("react.lazy");
function Lr(e) {
  return e != null && typeof e == "object" && "$$typeof" in e && e.$$typeof === Ec && "_payload" in e && Cc(e._payload);
}
function Cc(e) {
  return typeof e == "object" && e !== null && "then" in e;
}
var Sc = (e) => `${e} failed to slot onto its children. Expected a single React element child or \`Slottable\`.`, Rc = (e) => `${e} failed to slot onto its \`Slottable\`. Expected \`Slottable\` to receive a single React element child.`, Dt = l[" use ".trim().toString()];
function ci(e) {
  var t, n, r = "";
  if (typeof e == "string" || typeof e == "number") r += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var o = e.length;
    for (t = 0; t < o; t++) e[t] && (n = ci(e[t])) && (r && (r += " "), r += n);
  } else for (n in e) e[n] && (r && (r += " "), r += n);
  return r;
}
function ui() {
  for (var e, t, n = 0, r = "", o = arguments.length; n < o; n++) (e = arguments[n]) && (t = ci(e)) && (r && (r += " "), r += t);
  return r;
}
const Ir = (e) => typeof e == "boolean" ? `${e}` : e === 0 ? "0" : e, zr = ui, Pc = (e, t) => (n) => {
  var r;
  if ((t == null ? void 0 : t.variants) == null) return zr(e, n == null ? void 0 : n.class, n == null ? void 0 : n.className);
  const { variants: o, defaultVariants: i } = t, a = Object.keys(o).map((c) => {
    const d = n == null ? void 0 : n[c], f = i == null ? void 0 : i[c];
    if (d === null) return null;
    const m = Ir(d) || Ir(f);
    return o[c][m];
  }), s = n && Object.entries(n).reduce((c, d) => {
    let [f, m] = d;
    return m === void 0 || (c[f] = m), c;
  }, {}), u = t == null || (r = t.compoundVariants) === null || r === void 0 ? void 0 : r.reduce((c, d) => {
    let { class: f, className: m, ...h } = d;
    return Object.entries(h).every((b) => {
      let [p, g] = b;
      return Array.isArray(g) ? g.includes({
        ...i,
        ...s
      }[p]) : {
        ...i,
        ...s
      }[p] === g;
    }) ? [
      ...c,
      f,
      m
    ] : c;
  }, []);
  return zr(e, a, u, n == null ? void 0 : n.class, n == null ? void 0 : n.className);
};
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Nc = (e) => e.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase(), di = (...e) => e.filter((t, n, r) => !!t && t.trim() !== "" && r.indexOf(t) === n).join(" ").trim();
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
var Oc = {
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
const Ac = vo(
  ({
    color: e = "currentColor",
    size: t = 24,
    strokeWidth: n = 2,
    absoluteStrokeWidth: r,
    className: o = "",
    children: i,
    iconNode: a,
    ...s
  }, u) => Dn(
    "svg",
    {
      ref: u,
      ...Oc,
      width: t,
      height: t,
      stroke: e,
      strokeWidth: r ? Number(n) * 24 / Number(t) : n,
      className: di("lucide", o),
      ...s
    },
    [
      ...a.map(([c, d]) => Dn(c, d)),
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
const fi = (e, t) => {
  const n = vo(
    ({ className: r, ...o }, i) => Dn(Ac, {
      ref: i,
      iconNode: t,
      className: di(`lucide-${Nc(e)}`, r),
      ...o
    })
  );
  return n.displayName = `${e}`, n;
};
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
fi("PanelLeft", [
  ["rect", { width: "18", height: "18", x: "3", y: "3", rx: "2", key: "afitv7" }],
  ["path", { d: "M9 3v18", key: "fh3hqa" }]
]);
/**
 * @license lucide-react v0.460.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */
const Dc = fi("X", [
  ["path", { d: "M18 6 6 18", key: "1bl5f8" }],
  ["path", { d: "m6 6 12 12", key: "d8bk6v" }]
]), Tc = (e, t) => {
  const n = new Array(e.length + t.length);
  for (let r = 0; r < e.length; r++)
    n[r] = e[r];
  for (let r = 0; r < t.length; r++)
    n[e.length + r] = t[r];
  return n;
}, Mc = (e, t) => ({
  classGroupId: e,
  validator: t
}), pi = (e = /* @__PURE__ */ new Map(), t = null, n) => ({
  nextPart: e,
  validators: t,
  classGroupId: n
}), Zt = "-", _r = [], Lc = "arbitrary..", Ic = (e) => {
  const t = _c(e), {
    conflictingClassGroups: n,
    conflictingClassGroupModifiers: r
  } = e;
  return {
    getClassGroupId: (o) => {
      if (o.startsWith("[") && o.endsWith("]"))
        return zc(o);
      const i = o.split(Zt), a = i[0] === "" && i.length > 1 ? 1 : 0;
      return mi(i, a, t);
    },
    getConflictingClassGroupIds: (o, i) => {
      if (i) {
        const a = r[o], s = n[o];
        return a ? s ? Tc(s, a) : a : s || _r;
      }
      return n[o] || _r;
    }
  };
}, mi = (e, t, n) => {
  if (e.length - t === 0)
    return n.classGroupId;
  const r = e[t], o = n.nextPart.get(r);
  if (o) {
    const u = mi(e, t + 1, o);
    if (u) return u;
  }
  const i = n.validators;
  if (i === null)
    return;
  const a = t === 0 ? e.join(Zt) : e.slice(t).join(Zt), s = i.length;
  for (let u = 0; u < s; u++) {
    const c = i[u];
    if (c.validator(a))
      return c.classGroupId;
  }
}, zc = (e) => e.slice(1, -1).indexOf(":") === -1 ? void 0 : (() => {
  const t = e.slice(1, -1), n = t.indexOf(":"), r = t.slice(0, n);
  return r ? Lc + r : void 0;
})(), _c = (e) => {
  const {
    theme: t,
    classGroups: n
  } = e;
  return Fc(n, t);
}, Fc = (e, t) => {
  const n = pi();
  for (const r in e) {
    const o = e[r];
    Yn(o, n, r, t);
  }
  return n;
}, Yn = (e, t, n, r) => {
  const o = e.length;
  for (let i = 0; i < o; i++) {
    const a = e[i];
    Wc(a, t, n, r);
  }
}, Wc = (e, t, n, r) => {
  if (typeof e == "string") {
    jc(e, t, n);
    return;
  }
  if (typeof e == "function") {
    Bc(e, t, n, r);
    return;
  }
  $c(e, t, n, r);
}, jc = (e, t, n) => {
  const r = e === "" ? t : hi(t, e);
  r.classGroupId = n;
}, Bc = (e, t, n, r) => {
  if (Gc(e)) {
    Yn(e(r), t, n, r);
    return;
  }
  t.validators === null && (t.validators = []), t.validators.push(Mc(n, e));
}, $c = (e, t, n, r) => {
  const o = Object.entries(e), i = o.length;
  for (let a = 0; a < i; a++) {
    const [s, u] = o[a];
    Yn(u, hi(t, s), n, r);
  }
}, hi = (e, t) => {
  let n = e;
  const r = t.split(Zt), o = r.length;
  for (let i = 0; i < o; i++) {
    const a = r[i];
    let s = n.nextPart.get(a);
    s || (s = pi(), n.nextPart.set(a, s)), n = s;
  }
  return n;
}, Gc = (e) => "isThemeGetter" in e && e.isThemeGetter === !0, Uc = (e) => {
  if (e < 1)
    return {
      get: () => {
      },
      set: () => {
      }
    };
  let t = 0, n = /* @__PURE__ */ Object.create(null), r = /* @__PURE__ */ Object.create(null);
  const o = (i, a) => {
    n[i] = a, t++, t > e && (t = 0, r = n, n = /* @__PURE__ */ Object.create(null));
  };
  return {
    get(i) {
      let a = n[i];
      if (a !== void 0)
        return a;
      if ((a = r[i]) !== void 0)
        return o(i, a), a;
    },
    set(i, a) {
      i in n ? n[i] = a : o(i, a);
    }
  };
}, In = "!", Fr = ":", Vc = [], Wr = (e, t, n, r, o) => ({
  modifiers: e,
  hasImportantModifier: t,
  baseClassName: n,
  maybePostfixModifierPosition: r,
  isExternal: o
}), Hc = (e) => {
  const {
    prefix: t,
    experimentalParseClassName: n
  } = e;
  let r = (o) => {
    const i = [];
    let a = 0, s = 0, u = 0, c;
    const d = o.length;
    for (let p = 0; p < d; p++) {
      const g = o[p];
      if (a === 0 && s === 0) {
        if (g === Fr) {
          i.push(o.slice(u, p)), u = p + 1;
          continue;
        }
        if (g === "/") {
          c = p;
          continue;
        }
      }
      g === "[" ? a++ : g === "]" ? a-- : g === "(" ? s++ : g === ")" && s--;
    }
    const f = i.length === 0 ? o : o.slice(u);
    let m = f, h = !1;
    f.endsWith(In) ? (m = f.slice(0, -1), h = !0) : (
      /**
       * In Tailwind CSS v3 the important modifier was at the start of the base class name. This is still supported for legacy reasons.
       * @see https://github.com/dcastil/tailwind-merge/issues/513#issuecomment-2614029864
       */
      f.startsWith(In) && (m = f.slice(1), h = !0)
    );
    const b = c && c > u ? c - u : void 0;
    return Wr(i, h, m, b);
  };
  if (t) {
    const o = t + Fr, i = r;
    r = (a) => a.startsWith(o) ? i(a.slice(o.length)) : Wr(Vc, !1, a, void 0, !0);
  }
  if (n) {
    const o = r;
    r = (i) => n({
      className: i,
      parseClassName: o
    });
  }
  return r;
}, Xc = (e) => {
  const t = /* @__PURE__ */ new Map();
  return e.orderSensitiveModifiers.forEach((n, r) => {
    t.set(n, 1e6 + r);
  }), (n) => {
    const r = [];
    let o = [];
    for (let i = 0; i < n.length; i++) {
      const a = n[i], s = a[0] === "[", u = t.has(a);
      s || u ? (o.length > 0 && (o.sort(), r.push(...o), o = []), r.push(a)) : o.push(a);
    }
    return o.length > 0 && (o.sort(), r.push(...o)), r;
  };
}, Yc = (e) => ({
  cache: Uc(e.cacheSize),
  parseClassName: Hc(e),
  sortModifiers: Xc(e),
  postfixLookupClassGroupIds: Kc(e),
  ...Ic(e)
}), Kc = (e) => {
  const t = /* @__PURE__ */ Object.create(null), n = e.postfixLookupClassGroups;
  if (n)
    for (let r = 0; r < n.length; r++)
      t[n[r]] = !0;
  return t;
}, Zc = /\s+/, qc = (e, t) => {
  const {
    parseClassName: n,
    getClassGroupId: r,
    getConflictingClassGroupIds: o,
    sortModifiers: i,
    postfixLookupClassGroupIds: a
  } = t, s = [], u = e.trim().split(Zc);
  let c = "";
  for (let d = u.length - 1; d >= 0; d -= 1) {
    const f = u[d], {
      isExternal: m,
      modifiers: h,
      hasImportantModifier: b,
      baseClassName: p,
      maybePostfixModifierPosition: g
    } = n(f);
    if (m) {
      c = f + (c.length > 0 ? " " + c : c);
      continue;
    }
    let w = !!g, v;
    if (w) {
      const T = p.substring(0, g);
      v = r(T);
      const y = v && a[v] ? r(p) : void 0;
      y && y !== v && (v = y, w = !1);
    } else
      v = r(p);
    if (!v) {
      if (!w) {
        c = f + (c.length > 0 ? " " + c : c);
        continue;
      }
      if (v = r(p), !v) {
        c = f + (c.length > 0 ? " " + c : c);
        continue;
      }
      w = !1;
    }
    const x = h.length === 0 ? "" : h.length === 1 ? h[0] : i(h).join(":"), E = b ? x + In : x, S = E + v;
    if (s.indexOf(S) > -1)
      continue;
    s.push(S);
    const R = o(v, w);
    for (let T = 0; T < R.length; ++T) {
      const y = R[T];
      s.push(E + y);
    }
    c = f + (c.length > 0 ? " " + c : c);
  }
  return c;
}, Qc = (...e) => {
  let t = 0, n, r, o = "";
  for (; t < e.length; )
    (n = e[t++]) && (r = gi(n)) && (o && (o += " "), o += r);
  return o;
}, gi = (e) => {
  if (typeof e == "string")
    return e;
  let t, n = "";
  for (let r = 0; r < e.length; r++)
    e[r] && (t = gi(e[r])) && (n && (n += " "), n += t);
  return n;
}, Jc = (e, ...t) => {
  let n, r, o, i;
  const a = (u) => {
    const c = t.reduce((d, f) => f(d), e());
    return n = Yc(c), r = n.cache.get, o = n.cache.set, i = s, s(u);
  }, s = (u) => {
    const c = r(u);
    if (c)
      return c;
    const d = qc(u, n);
    return o(u, d), d;
  };
  return i = a, (...u) => i(Qc(...u));
}, eu = [], K = (e) => {
  const t = (n) => n[e] || eu;
  return t.isThemeGetter = !0, t;
}, bi = /^\[(?:(\w[\w-]*):)?(.+)\]$/i, vi = /^\((?:(\w[\w-]*):)?(.+)\)$/i, tu = /^\d+(?:\.\d+)?\/\d+(?:\.\d+)?$/, nu = /^(\d+(\.\d+)?)?(xs|sm|md|lg|xl)$/, ru = /\d+(%|px|r?em|[sdl]?v([hwib]|min|max)|pt|pc|in|cm|mm|cap|ch|ex|r?lh|cq(w|h|i|b|min|max))|\b(calc|min|max|clamp)\(.+\)|^0$/, ou = /^(rgba?|hsla?|hwb|(ok)?(lab|lch)|color-mix)\(.+\)$/, iu = /^(inset_)?-?((\d+)?\.?(\d+)[a-z]+|0)_-?((\d+)?\.?(\d+)[a-z]+|0)/, au = /^(url|image|image-set|cross-fade|element|(repeating-)?(linear|radial|conic)-gradient)\(.+\)$/, Te = (e) => tu.test(e), F = (e) => !!e && !Number.isNaN(Number(e)), he = (e) => !!e && Number.isInteger(Number(e)), xn = (e) => e.endsWith("%") && F(e.slice(0, -1)), Se = (e) => nu.test(e), yi = () => !0, su = (e) => (
  // `colorFunctionRegex` check is necessary because color functions can have percentages in them which which would be incorrectly classified as lengths.
  // For example, `hsl(0 0% 0%)` would be classified as a length without this check.
  // I could also use lookbehind assertion in `lengthUnitRegex` but that isn't supported widely enough.
  ru.test(e) && !ou.test(e)
), Kn = () => !1, lu = (e) => iu.test(e), cu = (e) => au.test(e), uu = (e) => !N(e) && !D(e), du = (e) => e.startsWith("@container") && (e[10] === "/" && e[11] !== void 0 || e[11] === "s" && e[16] !== void 0 && e.startsWith("-size/", 10) || e[11] === "n" && e[18] !== void 0 && e.startsWith("-normal/", 10)), fu = (e) => Be(e, ki, Kn), N = (e) => bi.test(e), Ue = (e) => Be(e, Ei, su), jr = (e) => Be(e, wu, F), pu = (e) => Be(e, Si, yi), mu = (e) => Be(e, Ci, Kn), Br = (e) => Be(e, wi, Kn), hu = (e) => Be(e, xi, cu), Tt = (e) => Be(e, Ri, lu), D = (e) => vi.test(e), ht = (e) => Ze(e, Ei), gu = (e) => Ze(e, Ci), $r = (e) => Ze(e, wi), bu = (e) => Ze(e, ki), vu = (e) => Ze(e, xi), Mt = (e) => Ze(e, Ri, !0), yu = (e) => Ze(e, Si, !0), Be = (e, t, n) => {
  const r = bi.exec(e);
  return r ? r[1] ? t(r[1]) : n(r[2]) : !1;
}, Ze = (e, t, n = !1) => {
  const r = vi.exec(e);
  return r ? r[1] ? t(r[1]) : n : !1;
}, wi = (e) => e === "position" || e === "percentage", xi = (e) => e === "image" || e === "url", ki = (e) => e === "length" || e === "size" || e === "bg-size", Ei = (e) => e === "length", wu = (e) => e === "number", Ci = (e) => e === "family-name", Si = (e) => e === "number" || e === "weight", Ri = (e) => e === "shadow", xu = () => {
  const e = K("color"), t = K("font"), n = K("text"), r = K("font-weight"), o = K("tracking"), i = K("leading"), a = K("breakpoint"), s = K("container"), u = K("spacing"), c = K("radius"), d = K("shadow"), f = K("inset-shadow"), m = K("text-shadow"), h = K("drop-shadow"), b = K("blur"), p = K("perspective"), g = K("aspect"), w = K("ease"), v = K("animate"), x = () => ["auto", "avoid", "all", "avoid-page", "page", "left", "right", "column"], E = () => [
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
  ], S = () => [...E(), D, N], R = () => ["auto", "hidden", "clip", "visible", "scroll"], T = () => ["auto", "contain", "none"], y = () => [D, N, u], M = () => [Te, "full", "auto", ...y()], W = () => [he, "none", "subgrid", D, N], O = () => ["auto", {
    span: ["full", he, D, N]
  }, he, D, N], L = () => [he, "auto", D, N], V = () => ["auto", "min", "max", "fr", D, N], j = () => ["start", "end", "center", "between", "around", "evenly", "stretch", "baseline", "center-safe", "end-safe"], X = () => ["start", "end", "center", "stretch", "center-safe", "end-safe"], I = () => ["auto", ...y()], B = () => [Te, "auto", "full", "dvw", "dvh", "lvw", "lvh", "svw", "svh", "min", "max", "fit", ...y()], $ = () => [Te, "screen", "full", "dvw", "lvw", "svw", "min", "max", "fit", ...y()], H = () => [Te, "screen", "full", "lh", "dvh", "lvh", "svh", "min", "max", "fit", ...y()], k = () => [e, D, N], Ae = () => [...E(), $r, Br, {
    position: [D, N]
  }], xe = () => ["no-repeat", {
    repeat: ["", "x", "y", "space", "round"]
  }], fe = () => ["auto", "cover", "contain", bu, fu, {
    size: [D, N]
  }], Z = () => [xn, ht, Ue], G = () => [
    // Deprecated since Tailwind CSS v4.0.0
    "",
    "none",
    "full",
    c,
    D,
    N
  ], U = () => ["", F, ht, Ue], re = () => ["solid", "dashed", "dotted", "double"], ke = () => ["normal", "multiply", "screen", "overlay", "darken", "lighten", "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation", "color", "luminosity"], z = () => [F, xn, $r, Br], $e = () => [
    // Deprecated since Tailwind CSS v4.0.0
    "",
    "none",
    b,
    D,
    N
  ], oe = () => ["none", F, D, N], ie = () => ["none", F, D, N], Ee = () => [F, D, N], ee = () => [Te, "full", ...y()];
  return {
    cacheSize: 500,
    theme: {
      animate: ["spin", "ping", "pulse", "bounce"],
      aspect: ["video"],
      blur: [Se],
      breakpoint: [Se],
      color: [yi],
      container: [Se],
      "drop-shadow": [Se],
      ease: ["in", "out", "in-out"],
      font: [uu],
      "font-weight": ["thin", "extralight", "light", "normal", "medium", "semibold", "bold", "extrabold", "black"],
      "inset-shadow": [Se],
      leading: ["none", "tight", "snug", "normal", "relaxed", "loose"],
      perspective: ["dramatic", "near", "normal", "midrange", "distant", "none"],
      radius: [Se],
      shadow: [Se],
      spacing: ["px", F],
      text: [Se],
      "text-shadow": [Se],
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
        aspect: ["auto", "square", Te, N, D, g]
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
        "@container": ["", "normal", "size", D, N]
      }],
      /**
       * Container Name
       * @see https://tailwindcss.com/docs/responsive-design#named-containers
       */
      "container-named": [du],
      /**
       * Columns
       * @see https://tailwindcss.com/docs/columns
       */
      columns: [{
        columns: [F, N, D, s]
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
        overflow: R()
      }],
      /**
       * Overflow X
       * @see https://tailwindcss.com/docs/overflow
       */
      "overflow-x": [{
        "overflow-x": R()
      }],
      /**
       * Overflow Y
       * @see https://tailwindcss.com/docs/overflow
       */
      "overflow-y": [{
        "overflow-y": R()
      }],
      /**
       * Overscroll Behavior
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      overscroll: [{
        overscroll: T()
      }],
      /**
       * Overscroll Behavior X
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      "overscroll-x": [{
        "overscroll-x": T()
      }],
      /**
       * Overscroll Behavior Y
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      "overscroll-y": [{
        "overscroll-y": T()
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
        z: [he, "auto", D, N]
      }],
      // ------------------------
      // --- Flexbox and Grid ---
      // ------------------------
      /**
       * Flex Basis
       * @see https://tailwindcss.com/docs/flex-basis
       */
      basis: [{
        basis: [Te, "full", "auto", s, ...y()]
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
        flex: [F, Te, "auto", "initial", "none", N]
      }],
      /**
       * Flex Grow
       * @see https://tailwindcss.com/docs/flex-grow
       */
      grow: [{
        grow: ["", F, D, N]
      }],
      /**
       * Flex Shrink
       * @see https://tailwindcss.com/docs/flex-shrink
       */
      shrink: [{
        shrink: ["", F, D, N]
      }],
      /**
       * Order
       * @see https://tailwindcss.com/docs/order
       */
      order: [{
        order: [he, "first", "last", "none", D, N]
      }],
      /**
       * Grid Template Columns
       * @see https://tailwindcss.com/docs/grid-template-columns
       */
      "grid-cols": [{
        "grid-cols": W()
      }],
      /**
       * Grid Column Start / End
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-start-end": [{
        col: O()
      }],
      /**
       * Grid Column Start
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-start": [{
        "col-start": L()
      }],
      /**
       * Grid Column End
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-end": [{
        "col-end": L()
      }],
      /**
       * Grid Template Rows
       * @see https://tailwindcss.com/docs/grid-template-rows
       */
      "grid-rows": [{
        "grid-rows": W()
      }],
      /**
       * Grid Row Start / End
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-start-end": [{
        row: O()
      }],
      /**
       * Grid Row Start
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-start": [{
        "row-start": L()
      }],
      /**
       * Grid Row End
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-end": [{
        "row-end": L()
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
        "auto-cols": V()
      }],
      /**
       * Grid Auto Rows
       * @see https://tailwindcss.com/docs/grid-auto-rows
       */
      "auto-rows": [{
        "auto-rows": V()
      }],
      /**
       * Gap
       * @see https://tailwindcss.com/docs/gap
       */
      gap: [{
        gap: y()
      }],
      /**
       * Gap X
       * @see https://tailwindcss.com/docs/gap
       */
      "gap-x": [{
        "gap-x": y()
      }],
      /**
       * Gap Y
       * @see https://tailwindcss.com/docs/gap
       */
      "gap-y": [{
        "gap-y": y()
      }],
      /**
       * Justify Content
       * @see https://tailwindcss.com/docs/justify-content
       */
      "justify-content": [{
        justify: [...j(), "normal"]
      }],
      /**
       * Justify Items
       * @see https://tailwindcss.com/docs/justify-items
       */
      "justify-items": [{
        "justify-items": [...X(), "normal"]
      }],
      /**
       * Justify Self
       * @see https://tailwindcss.com/docs/justify-self
       */
      "justify-self": [{
        "justify-self": ["auto", ...X()]
      }],
      /**
       * Align Content
       * @see https://tailwindcss.com/docs/align-content
       */
      "align-content": [{
        content: ["normal", ...j()]
      }],
      /**
       * Align Items
       * @see https://tailwindcss.com/docs/align-items
       */
      "align-items": [{
        items: [...X(), {
          baseline: ["", "last"]
        }]
      }],
      /**
       * Align Self
       * @see https://tailwindcss.com/docs/align-self
       */
      "align-self": [{
        self: ["auto", ...X(), {
          baseline: ["", "last"]
        }]
      }],
      /**
       * Place Content
       * @see https://tailwindcss.com/docs/place-content
       */
      "place-content": [{
        "place-content": j()
      }],
      /**
       * Place Items
       * @see https://tailwindcss.com/docs/place-items
       */
      "place-items": [{
        "place-items": [...X(), "baseline"]
      }],
      /**
       * Place Self
       * @see https://tailwindcss.com/docs/place-self
       */
      "place-self": [{
        "place-self": ["auto", ...X()]
      }],
      // Spacing
      /**
       * Padding
       * @see https://tailwindcss.com/docs/padding
       */
      p: [{
        p: y()
      }],
      /**
       * Padding Inline
       * @see https://tailwindcss.com/docs/padding
       */
      px: [{
        px: y()
      }],
      /**
       * Padding Block
       * @see https://tailwindcss.com/docs/padding
       */
      py: [{
        py: y()
      }],
      /**
       * Padding Inline Start
       * @see https://tailwindcss.com/docs/padding
       */
      ps: [{
        ps: y()
      }],
      /**
       * Padding Inline End
       * @see https://tailwindcss.com/docs/padding
       */
      pe: [{
        pe: y()
      }],
      /**
       * Padding Block Start
       * @see https://tailwindcss.com/docs/padding
       */
      pbs: [{
        pbs: y()
      }],
      /**
       * Padding Block End
       * @see https://tailwindcss.com/docs/padding
       */
      pbe: [{
        pbe: y()
      }],
      /**
       * Padding Top
       * @see https://tailwindcss.com/docs/padding
       */
      pt: [{
        pt: y()
      }],
      /**
       * Padding Right
       * @see https://tailwindcss.com/docs/padding
       */
      pr: [{
        pr: y()
      }],
      /**
       * Padding Bottom
       * @see https://tailwindcss.com/docs/padding
       */
      pb: [{
        pb: y()
      }],
      /**
       * Padding Left
       * @see https://tailwindcss.com/docs/padding
       */
      pl: [{
        pl: y()
      }],
      /**
       * Margin
       * @see https://tailwindcss.com/docs/margin
       */
      m: [{
        m: I()
      }],
      /**
       * Margin Inline
       * @see https://tailwindcss.com/docs/margin
       */
      mx: [{
        mx: I()
      }],
      /**
       * Margin Block
       * @see https://tailwindcss.com/docs/margin
       */
      my: [{
        my: I()
      }],
      /**
       * Margin Inline Start
       * @see https://tailwindcss.com/docs/margin
       */
      ms: [{
        ms: I()
      }],
      /**
       * Margin Inline End
       * @see https://tailwindcss.com/docs/margin
       */
      me: [{
        me: I()
      }],
      /**
       * Margin Block Start
       * @see https://tailwindcss.com/docs/margin
       */
      mbs: [{
        mbs: I()
      }],
      /**
       * Margin Block End
       * @see https://tailwindcss.com/docs/margin
       */
      mbe: [{
        mbe: I()
      }],
      /**
       * Margin Top
       * @see https://tailwindcss.com/docs/margin
       */
      mt: [{
        mt: I()
      }],
      /**
       * Margin Right
       * @see https://tailwindcss.com/docs/margin
       */
      mr: [{
        mr: I()
      }],
      /**
       * Margin Bottom
       * @see https://tailwindcss.com/docs/margin
       */
      mb: [{
        mb: I()
      }],
      /**
       * Margin Left
       * @see https://tailwindcss.com/docs/margin
       */
      ml: [{
        ml: I()
      }],
      /**
       * Space Between X
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-x": [{
        "space-x": y()
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
        "space-y": y()
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
        size: B()
      }],
      /**
       * Inline Size
       * @see https://tailwindcss.com/docs/width
       */
      "inline-size": [{
        inline: ["auto", ...$()]
      }],
      /**
       * Min-Inline Size
       * @see https://tailwindcss.com/docs/min-width
       */
      "min-inline-size": [{
        "min-inline": ["auto", ...$()]
      }],
      /**
       * Max-Inline Size
       * @see https://tailwindcss.com/docs/max-width
       */
      "max-inline-size": [{
        "max-inline": ["none", ...$()]
      }],
      /**
       * Block Size
       * @see https://tailwindcss.com/docs/height
       */
      "block-size": [{
        block: ["auto", ...H()]
      }],
      /**
       * Min-Block Size
       * @see https://tailwindcss.com/docs/min-height
       */
      "min-block-size": [{
        "min-block": ["auto", ...H()]
      }],
      /**
       * Max-Block Size
       * @see https://tailwindcss.com/docs/max-height
       */
      "max-block-size": [{
        "max-block": ["none", ...H()]
      }],
      /**
       * Width
       * @see https://tailwindcss.com/docs/width
       */
      w: [{
        w: [s, "screen", ...B()]
      }],
      /**
       * Min-Width
       * @see https://tailwindcss.com/docs/min-width
       */
      "min-w": [{
        "min-w": [
          s,
          "screen",
          /** Deprecated. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          "none",
          ...B()
        ]
      }],
      /**
       * Max-Width
       * @see https://tailwindcss.com/docs/max-width
       */
      "max-w": [{
        "max-w": [
          s,
          "screen",
          "none",
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          "prose",
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          {
            screen: [a]
          },
          ...B()
        ]
      }],
      /**
       * Height
       * @see https://tailwindcss.com/docs/height
       */
      h: [{
        h: ["screen", "lh", ...B()]
      }],
      /**
       * Min-Height
       * @see https://tailwindcss.com/docs/min-height
       */
      "min-h": [{
        "min-h": ["screen", "lh", "none", ...B()]
      }],
      /**
       * Max-Height
       * @see https://tailwindcss.com/docs/max-height
       */
      "max-h": [{
        "max-h": ["screen", "lh", ...B()]
      }],
      // ------------------
      // --- Typography ---
      // ------------------
      /**
       * Font Size
       * @see https://tailwindcss.com/docs/font-size
       */
      "font-size": [{
        text: ["base", n, ht, Ue]
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
        font: [r, yu, pu]
      }],
      /**
       * Font Stretch
       * @see https://tailwindcss.com/docs/font-stretch
       */
      "font-stretch": [{
        "font-stretch": ["ultra-condensed", "extra-condensed", "condensed", "semi-condensed", "normal", "semi-expanded", "expanded", "extra-expanded", "ultra-expanded", xn, N]
      }],
      /**
       * Font Family
       * @see https://tailwindcss.com/docs/font-family
       */
      "font-family": [{
        font: [gu, mu, t]
      }],
      /**
       * Font Feature Settings
       * @see https://tailwindcss.com/docs/font-feature-settings
       */
      "font-features": [{
        "font-features": [N]
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
        tracking: [o, D, N]
      }],
      /**
       * Line Clamp
       * @see https://tailwindcss.com/docs/line-clamp
       */
      "line-clamp": [{
        "line-clamp": [F, "none", D, jr]
      }],
      /**
       * Line Height
       * @see https://tailwindcss.com/docs/line-height
       */
      leading: [{
        leading: [
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          i,
          ...y()
        ]
      }],
      /**
       * List Style Image
       * @see https://tailwindcss.com/docs/list-style-image
       */
      "list-image": [{
        "list-image": ["none", D, N]
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
        list: ["disc", "decimal", "none", D, N]
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
        placeholder: k()
      }],
      /**
       * Text Color
       * @see https://tailwindcss.com/docs/text-color
       */
      "text-color": [{
        text: k()
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
        decoration: [...re(), "wavy"]
      }],
      /**
       * Text Decoration Thickness
       * @see https://tailwindcss.com/docs/text-decoration-thickness
       */
      "text-decoration-thickness": [{
        decoration: [F, "from-font", "auto", D, Ue]
      }],
      /**
       * Text Decoration Color
       * @see https://tailwindcss.com/docs/text-decoration-color
       */
      "text-decoration-color": [{
        decoration: k()
      }],
      /**
       * Text Underline Offset
       * @see https://tailwindcss.com/docs/text-underline-offset
       */
      "underline-offset": [{
        "underline-offset": [F, "auto", D, N]
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
        indent: y()
      }],
      /**
       * Tab Size
       * @see https://tailwindcss.com/docs/tab-size
       */
      "tab-size": [{
        tab: [he, D, N]
      }],
      /**
       * Vertical Alignment
       * @see https://tailwindcss.com/docs/vertical-align
       */
      "vertical-align": [{
        align: ["baseline", "top", "middle", "bottom", "text-top", "text-bottom", "sub", "super", D, N]
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
        content: ["none", D, N]
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
        bg: Ae()
      }],
      /**
       * Background Repeat
       * @see https://tailwindcss.com/docs/background-repeat
       */
      "bg-repeat": [{
        bg: xe()
      }],
      /**
       * Background Size
       * @see https://tailwindcss.com/docs/background-size
       */
      "bg-size": [{
        bg: fe()
      }],
      /**
       * Background Image
       * @see https://tailwindcss.com/docs/background-image
       */
      "bg-image": [{
        bg: ["none", {
          linear: [{
            to: ["t", "tr", "r", "br", "b", "bl", "l", "tl"]
          }, he, D, N],
          radial: ["", D, N],
          conic: [he, D, N]
        }, vu, hu]
      }],
      /**
       * Background Color
       * @see https://tailwindcss.com/docs/background-color
       */
      "bg-color": [{
        bg: k()
      }],
      /**
       * Gradient Color Stops From Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-from-pos": [{
        from: Z()
      }],
      /**
       * Gradient Color Stops Via Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-via-pos": [{
        via: Z()
      }],
      /**
       * Gradient Color Stops To Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-to-pos": [{
        to: Z()
      }],
      /**
       * Gradient Color Stops From
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-from": [{
        from: k()
      }],
      /**
       * Gradient Color Stops Via
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-via": [{
        via: k()
      }],
      /**
       * Gradient Color Stops To
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-to": [{
        to: k()
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
        border: [...re(), "hidden", "none"]
      }],
      /**
       * Divide Style
       * @see https://tailwindcss.com/docs/border-style#setting-the-divider-style
       */
      "divide-style": [{
        divide: [...re(), "hidden", "none"]
      }],
      /**
       * Border Color
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color": [{
        border: k()
      }],
      /**
       * Border Color Inline
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-x": [{
        "border-x": k()
      }],
      /**
       * Border Color Block
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-y": [{
        "border-y": k()
      }],
      /**
       * Border Color Inline Start
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-s": [{
        "border-s": k()
      }],
      /**
       * Border Color Inline End
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-e": [{
        "border-e": k()
      }],
      /**
       * Border Color Block Start
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-bs": [{
        "border-bs": k()
      }],
      /**
       * Border Color Block End
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-be": [{
        "border-be": k()
      }],
      /**
       * Border Color Top
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-t": [{
        "border-t": k()
      }],
      /**
       * Border Color Right
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-r": [{
        "border-r": k()
      }],
      /**
       * Border Color Bottom
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-b": [{
        "border-b": k()
      }],
      /**
       * Border Color Left
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-l": [{
        "border-l": k()
      }],
      /**
       * Divide Color
       * @see https://tailwindcss.com/docs/divide-color
       */
      "divide-color": [{
        divide: k()
      }],
      /**
       * Outline Style
       * @see https://tailwindcss.com/docs/outline-style
       */
      "outline-style": [{
        outline: [...re(), "none", "hidden"]
      }],
      /**
       * Outline Offset
       * @see https://tailwindcss.com/docs/outline-offset
       */
      "outline-offset": [{
        "outline-offset": [F, D, N]
      }],
      /**
       * Outline Width
       * @see https://tailwindcss.com/docs/outline-width
       */
      "outline-w": [{
        outline: ["", F, ht, Ue]
      }],
      /**
       * Outline Color
       * @see https://tailwindcss.com/docs/outline-color
       */
      "outline-color": [{
        outline: k()
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
          d,
          Mt,
          Tt
        ]
      }],
      /**
       * Box Shadow Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-shadow-color
       */
      "shadow-color": [{
        shadow: k()
      }],
      /**
       * Inset Box Shadow
       * @see https://tailwindcss.com/docs/box-shadow#adding-an-inset-shadow
       */
      "inset-shadow": [{
        "inset-shadow": ["none", f, Mt, Tt]
      }],
      /**
       * Inset Box Shadow Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-inset-shadow-color
       */
      "inset-shadow-color": [{
        "inset-shadow": k()
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
        ring: k()
      }],
      /**
       * Ring Offset Width
       * @see https://v3.tailwindcss.com/docs/ring-offset-width
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-offset-w": [{
        "ring-offset": [F, Ue]
      }],
      /**
       * Ring Offset Color
       * @see https://v3.tailwindcss.com/docs/ring-offset-color
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-offset-color": [{
        "ring-offset": k()
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
        "inset-ring": k()
      }],
      /**
       * Text Shadow
       * @see https://tailwindcss.com/docs/text-shadow
       */
      "text-shadow": [{
        "text-shadow": ["none", m, Mt, Tt]
      }],
      /**
       * Text Shadow Color
       * @see https://tailwindcss.com/docs/text-shadow#setting-the-shadow-color
       */
      "text-shadow-color": [{
        "text-shadow": k()
      }],
      /**
       * Opacity
       * @see https://tailwindcss.com/docs/opacity
       */
      opacity: [{
        opacity: [F, D, N]
      }],
      /**
       * Mix Blend Mode
       * @see https://tailwindcss.com/docs/mix-blend-mode
       */
      "mix-blend": [{
        "mix-blend": [...ke(), "plus-darker", "plus-lighter"]
      }],
      /**
       * Background Blend Mode
       * @see https://tailwindcss.com/docs/background-blend-mode
       */
      "bg-blend": [{
        "bg-blend": ke()
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
        "mask-linear": [F]
      }],
      "mask-image-linear-from-pos": [{
        "mask-linear-from": z()
      }],
      "mask-image-linear-to-pos": [{
        "mask-linear-to": z()
      }],
      "mask-image-linear-from-color": [{
        "mask-linear-from": k()
      }],
      "mask-image-linear-to-color": [{
        "mask-linear-to": k()
      }],
      "mask-image-t-from-pos": [{
        "mask-t-from": z()
      }],
      "mask-image-t-to-pos": [{
        "mask-t-to": z()
      }],
      "mask-image-t-from-color": [{
        "mask-t-from": k()
      }],
      "mask-image-t-to-color": [{
        "mask-t-to": k()
      }],
      "mask-image-r-from-pos": [{
        "mask-r-from": z()
      }],
      "mask-image-r-to-pos": [{
        "mask-r-to": z()
      }],
      "mask-image-r-from-color": [{
        "mask-r-from": k()
      }],
      "mask-image-r-to-color": [{
        "mask-r-to": k()
      }],
      "mask-image-b-from-pos": [{
        "mask-b-from": z()
      }],
      "mask-image-b-to-pos": [{
        "mask-b-to": z()
      }],
      "mask-image-b-from-color": [{
        "mask-b-from": k()
      }],
      "mask-image-b-to-color": [{
        "mask-b-to": k()
      }],
      "mask-image-l-from-pos": [{
        "mask-l-from": z()
      }],
      "mask-image-l-to-pos": [{
        "mask-l-to": z()
      }],
      "mask-image-l-from-color": [{
        "mask-l-from": k()
      }],
      "mask-image-l-to-color": [{
        "mask-l-to": k()
      }],
      "mask-image-x-from-pos": [{
        "mask-x-from": z()
      }],
      "mask-image-x-to-pos": [{
        "mask-x-to": z()
      }],
      "mask-image-x-from-color": [{
        "mask-x-from": k()
      }],
      "mask-image-x-to-color": [{
        "mask-x-to": k()
      }],
      "mask-image-y-from-pos": [{
        "mask-y-from": z()
      }],
      "mask-image-y-to-pos": [{
        "mask-y-to": z()
      }],
      "mask-image-y-from-color": [{
        "mask-y-from": k()
      }],
      "mask-image-y-to-color": [{
        "mask-y-to": k()
      }],
      "mask-image-radial": [{
        "mask-radial": [D, N]
      }],
      "mask-image-radial-from-pos": [{
        "mask-radial-from": z()
      }],
      "mask-image-radial-to-pos": [{
        "mask-radial-to": z()
      }],
      "mask-image-radial-from-color": [{
        "mask-radial-from": k()
      }],
      "mask-image-radial-to-color": [{
        "mask-radial-to": k()
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
        "mask-radial-at": E()
      }],
      "mask-image-conic-pos": [{
        "mask-conic": [F]
      }],
      "mask-image-conic-from-pos": [{
        "mask-conic-from": z()
      }],
      "mask-image-conic-to-pos": [{
        "mask-conic-to": z()
      }],
      "mask-image-conic-from-color": [{
        "mask-conic-from": k()
      }],
      "mask-image-conic-to-color": [{
        "mask-conic-to": k()
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
        mask: Ae()
      }],
      /**
       * Mask Repeat
       * @see https://tailwindcss.com/docs/mask-repeat
       */
      "mask-repeat": [{
        mask: xe()
      }],
      /**
       * Mask Size
       * @see https://tailwindcss.com/docs/mask-size
       */
      "mask-size": [{
        mask: fe()
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
        mask: ["none", D, N]
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
          D,
          N
        ]
      }],
      /**
       * Blur
       * @see https://tailwindcss.com/docs/blur
       */
      blur: [{
        blur: $e()
      }],
      /**
       * Brightness
       * @see https://tailwindcss.com/docs/brightness
       */
      brightness: [{
        brightness: [F, D, N]
      }],
      /**
       * Contrast
       * @see https://tailwindcss.com/docs/contrast
       */
      contrast: [{
        contrast: [F, D, N]
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
          h,
          Mt,
          Tt
        ]
      }],
      /**
       * Drop Shadow Color
       * @see https://tailwindcss.com/docs/filter-drop-shadow#setting-the-shadow-color
       */
      "drop-shadow-color": [{
        "drop-shadow": k()
      }],
      /**
       * Grayscale
       * @see https://tailwindcss.com/docs/grayscale
       */
      grayscale: [{
        grayscale: ["", F, D, N]
      }],
      /**
       * Hue Rotate
       * @see https://tailwindcss.com/docs/hue-rotate
       */
      "hue-rotate": [{
        "hue-rotate": [F, D, N]
      }],
      /**
       * Invert
       * @see https://tailwindcss.com/docs/invert
       */
      invert: [{
        invert: ["", F, D, N]
      }],
      /**
       * Saturate
       * @see https://tailwindcss.com/docs/saturate
       */
      saturate: [{
        saturate: [F, D, N]
      }],
      /**
       * Sepia
       * @see https://tailwindcss.com/docs/sepia
       */
      sepia: [{
        sepia: ["", F, D, N]
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
          D,
          N
        ]
      }],
      /**
       * Backdrop Blur
       * @see https://tailwindcss.com/docs/backdrop-blur
       */
      "backdrop-blur": [{
        "backdrop-blur": $e()
      }],
      /**
       * Backdrop Brightness
       * @see https://tailwindcss.com/docs/backdrop-brightness
       */
      "backdrop-brightness": [{
        "backdrop-brightness": [F, D, N]
      }],
      /**
       * Backdrop Contrast
       * @see https://tailwindcss.com/docs/backdrop-contrast
       */
      "backdrop-contrast": [{
        "backdrop-contrast": [F, D, N]
      }],
      /**
       * Backdrop Grayscale
       * @see https://tailwindcss.com/docs/backdrop-grayscale
       */
      "backdrop-grayscale": [{
        "backdrop-grayscale": ["", F, D, N]
      }],
      /**
       * Backdrop Hue Rotate
       * @see https://tailwindcss.com/docs/backdrop-hue-rotate
       */
      "backdrop-hue-rotate": [{
        "backdrop-hue-rotate": [F, D, N]
      }],
      /**
       * Backdrop Invert
       * @see https://tailwindcss.com/docs/backdrop-invert
       */
      "backdrop-invert": [{
        "backdrop-invert": ["", F, D, N]
      }],
      /**
       * Backdrop Opacity
       * @see https://tailwindcss.com/docs/backdrop-opacity
       */
      "backdrop-opacity": [{
        "backdrop-opacity": [F, D, N]
      }],
      /**
       * Backdrop Saturate
       * @see https://tailwindcss.com/docs/backdrop-saturate
       */
      "backdrop-saturate": [{
        "backdrop-saturate": [F, D, N]
      }],
      /**
       * Backdrop Sepia
       * @see https://tailwindcss.com/docs/backdrop-sepia
       */
      "backdrop-sepia": [{
        "backdrop-sepia": ["", F, D, N]
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
        "border-spacing": y()
      }],
      /**
       * Border Spacing X
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing-x": [{
        "border-spacing-x": y()
      }],
      /**
       * Border Spacing Y
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing-y": [{
        "border-spacing-y": y()
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
        transition: ["", "all", "colors", "opacity", "shadow", "transform", "none", D, N]
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
        duration: [F, "initial", D, N]
      }],
      /**
       * Transition Timing Function
       * @see https://tailwindcss.com/docs/transition-timing-function
       */
      ease: [{
        ease: ["linear", "initial", w, D, N]
      }],
      /**
       * Transition Delay
       * @see https://tailwindcss.com/docs/transition-delay
       */
      delay: [{
        delay: [F, D, N]
      }],
      /**
       * Animation
       * @see https://tailwindcss.com/docs/animation
       */
      animate: [{
        animate: ["none", v, D, N]
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
        perspective: [p, D, N]
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
        rotate: oe()
      }],
      /**
       * Rotate X
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-x": [{
        "rotate-x": oe()
      }],
      /**
       * Rotate Y
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-y": [{
        "rotate-y": oe()
      }],
      /**
       * Rotate Z
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-z": [{
        "rotate-z": oe()
      }],
      /**
       * Scale
       * @see https://tailwindcss.com/docs/scale
       */
      scale: [{
        scale: ie()
      }],
      /**
       * Scale X
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-x": [{
        "scale-x": ie()
      }],
      /**
       * Scale Y
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-y": [{
        "scale-y": ie()
      }],
      /**
       * Scale Z
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-z": [{
        "scale-z": ie()
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
        skew: Ee()
      }],
      /**
       * Skew X
       * @see https://tailwindcss.com/docs/skew
       */
      "skew-x": [{
        "skew-x": Ee()
      }],
      /**
       * Skew Y
       * @see https://tailwindcss.com/docs/skew
       */
      "skew-y": [{
        "skew-y": Ee()
      }],
      /**
       * Transform
       * @see https://tailwindcss.com/docs/transform
       */
      transform: [{
        transform: [D, N, "", "none", "gpu", "cpu"]
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
        translate: ee()
      }],
      /**
       * Translate X
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-x": [{
        "translate-x": ee()
      }],
      /**
       * Translate Y
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-y": [{
        "translate-y": ee()
      }],
      /**
       * Translate Z
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-z": [{
        "translate-z": ee()
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
        zoom: [he, D, N]
      }],
      // ---------------------
      // --- Interactivity ---
      // ---------------------
      /**
       * Accent Color
       * @see https://tailwindcss.com/docs/accent-color
       */
      accent: [{
        accent: k()
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
        caret: k()
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
        cursor: ["auto", "default", "pointer", "wait", "text", "move", "help", "not-allowed", "none", "context-menu", "progress", "cell", "crosshair", "vertical-text", "alias", "copy", "no-drop", "grab", "grabbing", "all-scroll", "col-resize", "row-resize", "n-resize", "e-resize", "s-resize", "w-resize", "ne-resize", "nw-resize", "se-resize", "sw-resize", "ew-resize", "ns-resize", "nesw-resize", "nwse-resize", "zoom-in", "zoom-out", D, N]
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
        "scrollbar-thumb": k()
      }],
      /**
       * Scrollbar Track Color
       * @see https://tailwindcss.com/docs/scrollbar-color
       */
      "scrollbar-track-color": [{
        "scrollbar-track": k()
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
        "scroll-m": y()
      }],
      /**
       * Scroll Margin Inline
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mx": [{
        "scroll-mx": y()
      }],
      /**
       * Scroll Margin Block
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-my": [{
        "scroll-my": y()
      }],
      /**
       * Scroll Margin Inline Start
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-ms": [{
        "scroll-ms": y()
      }],
      /**
       * Scroll Margin Inline End
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-me": [{
        "scroll-me": y()
      }],
      /**
       * Scroll Margin Block Start
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mbs": [{
        "scroll-mbs": y()
      }],
      /**
       * Scroll Margin Block End
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mbe": [{
        "scroll-mbe": y()
      }],
      /**
       * Scroll Margin Top
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mt": [{
        "scroll-mt": y()
      }],
      /**
       * Scroll Margin Right
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mr": [{
        "scroll-mr": y()
      }],
      /**
       * Scroll Margin Bottom
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mb": [{
        "scroll-mb": y()
      }],
      /**
       * Scroll Margin Left
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-ml": [{
        "scroll-ml": y()
      }],
      /**
       * Scroll Padding
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-p": [{
        "scroll-p": y()
      }],
      /**
       * Scroll Padding Inline
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-px": [{
        "scroll-px": y()
      }],
      /**
       * Scroll Padding Block
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-py": [{
        "scroll-py": y()
      }],
      /**
       * Scroll Padding Inline Start
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-ps": [{
        "scroll-ps": y()
      }],
      /**
       * Scroll Padding Inline End
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pe": [{
        "scroll-pe": y()
      }],
      /**
       * Scroll Padding Block Start
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pbs": [{
        "scroll-pbs": y()
      }],
      /**
       * Scroll Padding Block End
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pbe": [{
        "scroll-pbe": y()
      }],
      /**
       * Scroll Padding Top
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pt": [{
        "scroll-pt": y()
      }],
      /**
       * Scroll Padding Right
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pr": [{
        "scroll-pr": y()
      }],
      /**
       * Scroll Padding Bottom
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pb": [{
        "scroll-pb": y()
      }],
      /**
       * Scroll Padding Left
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pl": [{
        "scroll-pl": y()
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
        "will-change": ["auto", "scroll", "contents", "transform", D, N]
      }],
      // -----------
      // --- SVG ---
      // -----------
      /**
       * Fill
       * @see https://tailwindcss.com/docs/fill
       */
      fill: [{
        fill: ["none", ...k()]
      }],
      /**
       * Stroke Width
       * @see https://tailwindcss.com/docs/stroke-width
       */
      "stroke-w": [{
        stroke: [F, ht, Ue, jr]
      }],
      /**
       * Stroke
       * @see https://tailwindcss.com/docs/stroke
       */
      stroke: [{
        stroke: ["none", ...k()]
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
}, ku = /* @__PURE__ */ Jc(xu);
function He(...e) {
  return ku(ui(e));
}
const Eu = Pc(
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
);
l.forwardRef(function({ className: e, variant: t, size: n, asChild: r = !1, ...o }, i) {
  return /* @__PURE__ */ C(r ? bc : "button", { ref: i, className: He(Eu({ variant: t, size: n, className: e })), ...o });
});
function q(e, t, { checkForDefaultPrevented: n = !0 } = {}) {
  return function(r) {
    if (e == null || e(r), n === !1 || !r.defaultPrevented)
      return t == null ? void 0 : t(r);
  };
}
function Zn(e, t = []) {
  let n = [];
  function r(i, a) {
    const s = l.createContext(a);
    s.displayName = i + "Context";
    const u = n.length;
    n = [...n, a];
    const c = (f) => {
      var m;
      const { scope: h, children: b, ...p } = f, g = ((m = h == null ? void 0 : h[e]) == null ? void 0 : m[u]) || s, w = l.useMemo(() => p, Object.values(p));
      return /* @__PURE__ */ C(g.Provider, { value: w, children: b });
    };
    c.displayName = i + "Provider";
    function d(f, m) {
      var h;
      const b = ((h = m == null ? void 0 : m[e]) == null ? void 0 : h[u]) || s, p = l.useContext(b);
      if (p) return p;
      if (a !== void 0) return a;
      throw new Error(`\`${f}\` must be used within \`${i}\``);
    }
    return [c, d];
  }
  const o = () => {
    const i = n.map((a) => l.createContext(a));
    return function(a) {
      const s = (a == null ? void 0 : a[e]) || i;
      return l.useMemo(
        () => ({ [`__scope${e}`]: { ...a, [e]: s } }),
        [a, s]
      );
    };
  };
  return o.scopeName = e, [r, Cu(o, ...t)];
}
function Cu(...e) {
  const t = e[0];
  if (e.length === 1) return t;
  const n = () => {
    const r = e.map((o) => ({
      useScope: o(),
      scopeName: o.scopeName
    }));
    return function(o) {
      const i = r.reduce((a, { useScope: s, scopeName: u }) => {
        const c = s(o)[`__scope${u}`];
        return { ...a, ...c };
      }, {});
      return l.useMemo(() => ({ [`__scope${t.scopeName}`]: i }), [i]);
    };
  };
  return n.scopeName = t.scopeName, n;
}
var _e = globalThis != null && globalThis.document ? l.useLayoutEffect : () => {
}, Gr = l[" useEffectEvent ".trim().toString()], Ur = l[" useInsertionEffect ".trim().toString()];
function Su(e) {
  if (typeof Gr == "function")
    return Gr(e);
  const t = l.useRef(() => {
    throw new Error("Cannot call an event handler while rendering.");
  });
  return typeof Ur == "function" ? Ur(() => {
    t.current = e;
  }) : _e(() => {
    t.current = e;
  }), l.useMemo(() => (...n) => {
    var r;
    return (r = t.current) == null ? void 0 : r.call(t, ...n);
  }, []);
}
var Ru = [
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
], J = Ru.reduce((e, t) => {
  const n = /* @__PURE__ */ Xn(`Primitive.${t}`), r = l.forwardRef((o, i) => {
    const { asChild: a, ...s } = o, u = a ? n : t;
    return typeof window < "u" && (window[Symbol.for("radix-ui")] = !0), /* @__PURE__ */ C(u, { ...s, ref: i });
  });
  return r.displayName = `Primitive.${t}`, { ...e, [t]: r };
}, {});
function Pu(e, t) {
  e && vt.flushSync(() => e.dispatchEvent(t));
}
function gt(e) {
  const t = l.useRef(e);
  return l.useEffect(() => {
    t.current = e;
  }), l.useMemo(() => (...n) => {
    var r;
    return (r = t.current) == null ? void 0 : r.call(t, ...n);
  }, []);
}
var Nu = "DismissableLayer", zn = "dismissableLayer.update", Ou = "dismissableLayer.pointerDownOutside", Au = "dismissableLayer.focusOutside", Vr, qn = l.createContext({
  layers: /* @__PURE__ */ new Set(),
  layersWithOutsidePointerEventsDisabled: /* @__PURE__ */ new Set(),
  branches: /* @__PURE__ */ new Set(),
  // Outside elements that belong to a layer's own dismiss affordance (eg, a
  // dialog overlay). Pressing them should dismiss the layer regardless of
  // whether or not they stop propagation.
  //
  // See https://github.com/radix-ui/primitives/issues/3346
  dismissableSurfaces: /* @__PURE__ */ new Set()
}), Qn = l.forwardRef(
  (e, t) => {
    const {
      disableOutsidePointerEvents: n = !1,
      deferPointerDownOutside: r = !1,
      onEscapeKeyDown: o,
      onPointerDownOutside: i,
      onFocusOutside: a,
      onInteractOutside: s,
      onDismiss: u,
      ...c
    } = e, d = l.useContext(qn), [f, m] = l.useState(null), h = (f == null ? void 0 : f.ownerDocument) ?? (globalThis == null ? void 0 : globalThis.document), [, b] = l.useState({}), p = ue(t, m), g = Array.from(d.layers), [w] = [...d.layersWithOutsidePointerEventsDisabled].slice(-1), v = g.indexOf(w), x = f ? g.indexOf(f) : -1, E = d.layersWithOutsidePointerEventsDisabled.size > 0, S = x >= v, R = l.useRef(!1), T = Lu(
      (O) => {
        const L = O.target;
        if (!(L instanceof Node))
          return;
        const V = [...d.branches].some(
          (j) => j.contains(L)
        );
        !S || V || (i == null || i(O), s == null || s(O), O.defaultPrevented || u == null || u());
      },
      {
        ownerDocument: h,
        deferPointerDownOutside: r,
        isDeferredPointerDownOutsideRef: R,
        dismissableSurfaces: d.dismissableSurfaces
      }
    ), y = Iu((O) => {
      if (r && R.current)
        return;
      const L = O.target;
      [...d.branches].some((V) => V.contains(L)) || (a == null || a(O), s == null || s(O), O.defaultPrevented || u == null || u());
    }, h), M = f ? x === g.length - 1 : !1, W = Su((O) => {
      O.key === "Escape" && (o == null || o(O), !O.defaultPrevented && u && (O.preventDefault(), u()));
    });
    return l.useEffect(() => {
      if (M)
        return h.addEventListener("keydown", W, { capture: !0 }), () => h.removeEventListener("keydown", W, { capture: !0 });
    }, [h, M]), l.useEffect(() => {
      if (f)
        return n && (d.layersWithOutsidePointerEventsDisabled.size === 0 && (Vr = h.body.style.pointerEvents, h.body.style.pointerEvents = "none"), d.layersWithOutsidePointerEventsDisabled.add(f)), d.layers.add(f), Hr(), () => {
          n && (d.layersWithOutsidePointerEventsDisabled.delete(f), d.layersWithOutsidePointerEventsDisabled.size === 0 && (h.body.style.pointerEvents = Vr));
        };
    }, [f, h, n, d]), l.useEffect(() => () => {
      f && (d.layers.delete(f), d.layersWithOutsidePointerEventsDisabled.delete(f), Hr());
    }, [f, d]), l.useEffect(() => {
      const O = () => b({});
      return document.addEventListener(zn, O), () => document.removeEventListener(zn, O);
    }, []), /* @__PURE__ */ C(
      J.div,
      {
        ...c,
        ref: p,
        style: {
          pointerEvents: E ? S ? "auto" : "none" : void 0,
          ...e.style
        },
        onFocusCapture: q(e.onFocusCapture, y.onFocusCapture),
        onBlurCapture: q(e.onBlurCapture, y.onBlurCapture),
        onPointerDownCapture: q(
          e.onPointerDownCapture,
          T.onPointerDownCapture
        )
      }
    );
  }
);
Qn.displayName = Nu;
var Du = "DismissableLayerBranch", Tu = l.forwardRef((e, t) => {
  const n = l.useContext(qn), r = l.useRef(null), o = ue(t, r);
  return l.useEffect(() => {
    const i = r.current;
    if (i)
      return n.branches.add(i), () => {
        n.branches.delete(i);
      };
  }, [n.branches]), /* @__PURE__ */ C(J.div, { ...e, ref: o });
});
Tu.displayName = Du;
function Mu() {
  const e = l.useContext(qn), [t, n] = l.useState(null);
  return l.useEffect(() => {
    if (t)
      return e.dismissableSurfaces.add(t), () => {
        e.dismissableSurfaces.delete(t);
      };
  }, [t, e.dismissableSurfaces]), n;
}
function Lu(e, t) {
  const {
    ownerDocument: n = globalThis == null ? void 0 : globalThis.document,
    deferPointerDownOutside: r = !1,
    isDeferredPointerDownOutsideRef: o,
    dismissableSurfaces: i
  } = t, a = gt(e), s = l.useRef(!1), u = l.useRef(!1), c = l.useRef(/* @__PURE__ */ new Map()), d = l.useRef(() => {
  });
  return l.useEffect(() => {
    function f() {
      u.current = !1, o.current = !1, c.current.clear();
    }
    function m() {
      return Array.from(c.current.values()).some(Boolean);
    }
    function h(v) {
      if (!u.current)
        return;
      const x = v.target;
      x instanceof Node && [...i].some((E) => E.contains(x)) || c.current.set(v.type, !0), v.type === "click" && window.setTimeout(() => {
        u.current && d.current();
      }, 0);
    }
    function b(v) {
      u.current && c.current.set(v.type, !1);
    }
    const p = (v) => {
      if (v.target && !s.current) {
        let x = function() {
          n.removeEventListener("click", d.current);
          const S = m();
          f(), S || Pi(
            Ou,
            a,
            E,
            { discrete: !0 }
          );
        };
        const E = { originalEvent: v };
        u.current = !0, o.current = r && v.button === 0, c.current.clear(), !r || v.button !== 0 ? x() : (n.removeEventListener("click", d.current), d.current = x, n.addEventListener("click", d.current, { once: !0 }));
      } else
        n.removeEventListener("click", d.current), f();
      s.current = !1;
    }, g = [
      "pointerup",
      "mousedown",
      "mouseup",
      "touchstart",
      "touchend",
      "click"
    ];
    for (const v of g)
      n.addEventListener(v, h, !0), n.addEventListener(v, b);
    const w = window.setTimeout(() => {
      n.addEventListener("pointerdown", p);
    }, 0);
    return () => {
      window.clearTimeout(w), n.removeEventListener("pointerdown", p), n.removeEventListener("click", d.current);
      for (const v of g)
        n.removeEventListener(v, h, !0), n.removeEventListener(v, b);
    };
  }, [
    n,
    a,
    r,
    o,
    i
  ]), {
    // ensures we check React component tree (not just DOM tree)
    onPointerDownCapture: () => s.current = !0
  };
}
function Iu(e, t = globalThis == null ? void 0 : globalThis.document) {
  const n = gt(e), r = l.useRef(!1);
  return l.useEffect(() => {
    const o = (i) => {
      i.target && !r.current && Pi(Au, n, { originalEvent: i }, {
        discrete: !1
      });
    };
    return t.addEventListener("focusin", o), () => t.removeEventListener("focusin", o);
  }, [t, n]), {
    onFocusCapture: () => r.current = !0,
    onBlurCapture: () => r.current = !1
  };
}
function Hr() {
  const e = new CustomEvent(zn);
  document.dispatchEvent(e);
}
function Pi(e, t, n, { discrete: r }) {
  const o = n.originalEvent.target, i = new CustomEvent(e, { bubbles: !1, cancelable: !0, detail: n });
  t && o.addEventListener(e, t, { once: !0 }), r ? Pu(o, i) : o.dispatchEvent(i);
}
var kn = "focusScope.autoFocusOnMount", En = "focusScope.autoFocusOnUnmount", Xr = { bubbles: !1, cancelable: !0 }, zu = "FocusScope", Ni = l.forwardRef((e, t) => {
  const {
    loop: n = !1,
    trapped: r = !1,
    onMountAutoFocus: o,
    onUnmountAutoFocus: i,
    ...a
  } = e, [s, u] = l.useState(null), c = gt(o), d = gt(i), f = l.useRef(null), m = ue(t, u), h = l.useRef({
    paused: !1,
    pause() {
      this.paused = !0;
    },
    resume() {
      this.paused = !1;
    }
  }).current;
  l.useEffect(() => {
    if (r) {
      let p = function(x) {
        if (h.paused || !s) return;
        const E = x.target;
        s.contains(E) ? f.current = E : Ie(f.current, { select: !0 });
      }, g = function(x) {
        if (h.paused || !s) return;
        const E = x.relatedTarget;
        E !== null && (s.contains(E) || Ie(f.current, { select: !0 }));
      }, w = function(x) {
        if (document.activeElement === document.body)
          for (const E of x)
            E.removedNodes.length > 0 && Ie(s);
      };
      document.addEventListener("focusin", p), document.addEventListener("focusout", g);
      const v = new MutationObserver(w);
      return s && v.observe(s, { childList: !0, subtree: !0 }), () => {
        document.removeEventListener("focusin", p), document.removeEventListener("focusout", g), v.disconnect();
      };
    }
  }, [r, s, h.paused]), l.useEffect(() => {
    if (s) {
      Kr.add(h);
      const p = document.activeElement;
      if (!s.contains(p)) {
        const g = new CustomEvent(kn, Xr);
        s.addEventListener(kn, c), s.dispatchEvent(g), g.defaultPrevented || (_u($u(Oi(s)), { select: !0 }), document.activeElement === p && Ie(s));
      }
      return () => {
        s.removeEventListener(kn, c), setTimeout(() => {
          const g = new CustomEvent(En, Xr);
          s.addEventListener(En, d), s.dispatchEvent(g), g.defaultPrevented || Ie(p ?? document.body, { select: !0 }), s.removeEventListener(En, d), Kr.remove(h);
        }, 0);
      };
    }
  }, [s, c, d, h]);
  const b = l.useCallback(
    (p) => {
      if (!n && !r || h.paused) return;
      const g = p.key === "Tab" && !p.altKey && !p.ctrlKey && !p.metaKey, w = document.activeElement;
      if (g && w) {
        const v = p.currentTarget, [x, E] = Fu(v);
        x && E ? !p.shiftKey && w === E ? (p.preventDefault(), n && Ie(x, { select: !0 })) : p.shiftKey && w === x && (p.preventDefault(), n && Ie(E, { select: !0 })) : w === v && p.preventDefault();
      }
    },
    [n, r, h.paused]
  );
  return /* @__PURE__ */ C(J.div, { tabIndex: -1, ...a, ref: m, onKeyDown: b });
});
Ni.displayName = zu;
function _u(e, { select: t = !1 } = {}) {
  const n = document.activeElement;
  for (const r of e)
    if (Ie(r, { select: t }), document.activeElement !== n) return;
}
function Fu(e) {
  const t = Oi(e), n = Yr(t, e), r = Yr(t.reverse(), e);
  return [n, r];
}
function Oi(e) {
  const t = [], n = document.createTreeWalker(e, NodeFilter.SHOW_ELEMENT, {
    acceptNode: (r) => {
      const o = r.tagName === "INPUT" && r.type === "hidden";
      return r.disabled || r.hidden || o ? NodeFilter.FILTER_SKIP : r.tabIndex >= 0 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;
    }
  });
  for (; n.nextNode(); ) t.push(n.currentNode);
  return t;
}
function Yr(e, t) {
  for (const n of e)
    if (!Wu(n, { upTo: t })) return n;
}
function Wu(e, { upTo: t }) {
  if (getComputedStyle(e).visibility === "hidden") return !0;
  for (; e; ) {
    if (t !== void 0 && e === t) return !1;
    if (getComputedStyle(e).display === "none") return !0;
    e = e.parentElement;
  }
  return !1;
}
function ju(e) {
  return e instanceof HTMLInputElement && "select" in e;
}
function Ie(e, { select: t = !1 } = {}) {
  if (e && e.focus) {
    const n = document.activeElement;
    e.focus({ preventScroll: !0 }), e !== n && ju(e) && t && e.select();
  }
}
var Kr = Bu();
function Bu() {
  let e = [];
  return {
    add(t) {
      const n = e[0];
      t !== n && (n == null || n.pause()), e = Zr(e, t), e.unshift(t);
    },
    remove(t) {
      var n;
      e = Zr(e, t), (n = e[0]) == null || n.resume();
    }
  };
}
function Zr(e, t) {
  const n = [...e], r = n.indexOf(t);
  return r !== -1 && n.splice(r, 1), n;
}
function $u(e) {
  return e.filter((t) => t.tagName !== "A");
}
var Gu = "Portal", Ai = l.forwardRef((e, t) => {
  var n;
  const { container: r, ...o } = e, [i, a] = l.useState(!1);
  _e(() => a(!0), []);
  const s = r || i && ((n = globalThis == null ? void 0 : globalThis.document) == null ? void 0 : n.body);
  return s ? vt.createPortal(/* @__PURE__ */ C(J.div, { ...o, ref: t }), s) : null;
});
Ai.displayName = Gu;
function Uu(e, t) {
  return l.useReducer((n, r) => t[n][r] ?? n, e);
}
var yt = (e) => {
  const { present: t, children: n } = e, r = Vu(t), o = typeof n == "function" ? n({ present: r.isPresent }) : l.Children.only(n), i = Hu(r.ref, Xu(o));
  return typeof n == "function" || r.isPresent ? l.cloneElement(o, { ref: i }) : null;
};
yt.displayName = "Presence";
function Vu(e) {
  const [t, n] = l.useState(), r = l.useRef(null), o = l.useRef(e), i = l.useRef("none"), a = e ? "mounted" : "unmounted", [s, u] = Uu(a, {
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
  return l.useEffect(() => {
    const c = Lt(r.current);
    i.current = s === "mounted" ? c : "none";
  }, [s]), _e(() => {
    const c = r.current, d = o.current;
    if (d !== e) {
      const f = i.current, m = Lt(c);
      e ? u("MOUNT") : m === "none" || (c == null ? void 0 : c.display) === "none" ? u("UNMOUNT") : u(d && f !== m ? "ANIMATION_OUT" : "UNMOUNT"), o.current = e;
    }
  }, [e, u]), _e(() => {
    if (t) {
      let c;
      const d = t.ownerDocument.defaultView ?? window, f = (h) => {
        const b = Lt(r.current).includes(CSS.escape(h.animationName));
        if (h.target === t && b && (u("ANIMATION_END"), !o.current)) {
          const p = t.style.animationFillMode;
          t.style.animationFillMode = "forwards", c = d.setTimeout(() => {
            t.style.animationFillMode === "forwards" && (t.style.animationFillMode = p);
          });
        }
      }, m = (h) => {
        h.target === t && (i.current = Lt(r.current));
      };
      return t.addEventListener("animationstart", m), t.addEventListener("animationcancel", f), t.addEventListener("animationend", f), () => {
        d.clearTimeout(c), t.removeEventListener("animationstart", m), t.removeEventListener("animationcancel", f), t.removeEventListener("animationend", f);
      };
    } else
      u("ANIMATION_END");
  }, [t, u]), {
    isPresent: ["mounted", "unmountSuspended"].includes(s),
    ref: l.useCallback((c) => {
      r.current = c ? getComputedStyle(c) : null, n(c);
    }, [])
  };
}
function qr(e, t) {
  if (typeof e == "function")
    return e(t);
  e != null && (e.current = t);
}
function Hu(...e) {
  const t = l.useRef(e);
  return t.current = e, l.useCallback((n) => {
    const r = t.current;
    let o = !1;
    const i = r.map((a) => {
      const s = qr(a, n);
      return !o && typeof s == "function" && (o = !0), s;
    });
    if (o)
      return () => {
        for (let a = 0; a < i.length; a++) {
          const s = i[a];
          typeof s == "function" ? s() : qr(r[a], null);
        }
      };
  }, []);
}
function Lt(e) {
  return (e == null ? void 0 : e.animationName) || "none";
}
function Xu(e) {
  var t, n;
  let r = (t = Object.getOwnPropertyDescriptor(e.props, "ref")) == null ? void 0 : t.get, o = r && "isReactWarning" in r && r.isReactWarning;
  return o ? e.ref : (r = (n = Object.getOwnPropertyDescriptor(e, "ref")) == null ? void 0 : n.get, o = r && "isReactWarning" in r && r.isReactWarning, o ? e.props.ref : e.props.ref || e.ref);
}
var It = 0, Me = null;
function Yu() {
  l.useEffect(() => {
    Me || (Me = { start: Qr(), end: Qr() });
    const { start: e, end: t } = Me;
    return document.body.firstElementChild !== e && document.body.insertAdjacentElement("afterbegin", e), document.body.lastElementChild !== t && document.body.insertAdjacentElement("beforeend", t), It++, () => {
      It === 1 && (Me == null || Me.start.remove(), Me == null || Me.end.remove(), Me = null), It = Math.max(0, It - 1);
    };
  }, []);
}
function Qr() {
  const e = document.createElement("span");
  return e.setAttribute("data-radix-focus-guard", ""), e.tabIndex = 0, e.style.outline = "none", e.style.opacity = "0", e.style.position = "fixed", e.style.pointerEvents = "none", e;
}
var be = function() {
  return be = Object.assign || function(e) {
    for (var t, n = 1, r = arguments.length; n < r; n++) {
      t = arguments[n];
      for (var o in t) Object.prototype.hasOwnProperty.call(t, o) && (e[o] = t[o]);
    }
    return e;
  }, be.apply(this, arguments);
};
function Di(e, t) {
  var n = {};
  for (var r in e) Object.prototype.hasOwnProperty.call(e, r) && t.indexOf(r) < 0 && (n[r] = e[r]);
  if (e != null && typeof Object.getOwnPropertySymbols == "function")
    for (var o = 0, r = Object.getOwnPropertySymbols(e); o < r.length; o++)
      t.indexOf(r[o]) < 0 && Object.prototype.propertyIsEnumerable.call(e, r[o]) && (n[r[o]] = e[r[o]]);
  return n;
}
function Ku(e, t, n) {
  for (var r = 0, o = t.length, i; r < o; r++)
    (i || !(r in t)) && (i || (i = Array.prototype.slice.call(t, 0, r)), i[r] = t[r]);
  return e.concat(i || Array.prototype.slice.call(t));
}
var Gt = "right-scroll-bar-position", Ut = "width-before-scroll-bar", Zu = "with-scroll-bars-hidden", qu = "--removed-body-scroll-bar-size";
function Cn(e, t) {
  return typeof e == "function" ? e(t) : e && (e.current = t), e;
}
function Qu(e, t) {
  var n = Ht(function() {
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
var Ju = typeof window < "u" ? l.useLayoutEffect : l.useEffect, Jr = /* @__PURE__ */ new WeakMap();
function ed(e, t) {
  var n = Qu(null, function(r) {
    return e.forEach(function(o) {
      return Cn(o, r);
    });
  });
  return Ju(function() {
    var r = Jr.get(n);
    if (r) {
      var o = new Set(r), i = new Set(e), a = n.current;
      o.forEach(function(s) {
        i.has(s) || Cn(s, null);
      }), i.forEach(function(s) {
        o.has(s) || Cn(s, a);
      });
    }
    Jr.set(n, e);
  }, [e]), n;
}
function td(e) {
  return e;
}
function nd(e, t) {
  t === void 0 && (t = td);
  var n = [], r = !1, o = {
    read: function() {
      if (r)
        throw new Error("Sidecar: could not `read` from an `assigned` medium. `read` could be used only with `useMedium`.");
      return n.length ? n[n.length - 1] : e;
    },
    useMedium: function(i) {
      var a = t(i, r);
      return n.push(a), function() {
        n = n.filter(function(s) {
          return s !== a;
        });
      };
    },
    assignSyncMedium: function(i) {
      for (r = !0; n.length; ) {
        var a = n;
        n = [], a.forEach(i);
      }
      n = {
        push: function(s) {
          return i(s);
        },
        filter: function() {
          return n;
        }
      };
    },
    assignMedium: function(i) {
      r = !0;
      var a = [];
      if (n.length) {
        var s = n;
        n = [], s.forEach(i), a = n;
      }
      var u = function() {
        var d = a;
        a = [], d.forEach(i);
      }, c = function() {
        return Promise.resolve().then(u);
      };
      c(), n = {
        push: function(d) {
          a.push(d), c();
        },
        filter: function(d) {
          return a = a.filter(d), n;
        }
      };
    }
  };
  return o;
}
function rd(e) {
  e === void 0 && (e = {});
  var t = nd(null);
  return t.options = be({ async: !0, ssr: !1 }, e), t;
}
var Ti = function(e) {
  var t = e.sideCar, n = Di(e, ["sideCar"]);
  if (!t)
    throw new Error("Sidecar: please provide `sideCar` property to import the right car");
  var r = t.read();
  if (!r)
    throw new Error("Sidecar medium not found");
  return l.createElement(r, be({}, n));
};
Ti.isSideCarExport = !0;
function od(e, t) {
  return e.useMedium(t), Ti;
}
var Mi = rd(), Sn = function() {
}, an = l.forwardRef(function(e, t) {
  var n = l.useRef(null), r = l.useState({
    onScrollCapture: Sn,
    onWheelCapture: Sn,
    onTouchMoveCapture: Sn
  }), o = r[0], i = r[1], a = e.forwardProps, s = e.children, u = e.className, c = e.removeScrollBar, d = e.enabled, f = e.shards, m = e.sideCar, h = e.noRelative, b = e.noIsolation, p = e.inert, g = e.allowPinchZoom, w = e.as, v = w === void 0 ? "div" : w, x = e.gapMode, E = Di(e, ["forwardProps", "children", "className", "removeScrollBar", "enabled", "shards", "sideCar", "noRelative", "noIsolation", "inert", "allowPinchZoom", "as", "gapMode"]), S = m, R = ed([n, t]), T = be(be({}, E), o);
  return l.createElement(
    l.Fragment,
    null,
    d && l.createElement(S, { sideCar: Mi, removeScrollBar: c, shards: f, noRelative: h, noIsolation: b, inert: p, setCallbacks: i, allowPinchZoom: !!g, lockRef: n, gapMode: x }),
    a ? l.cloneElement(l.Children.only(s), be(be({}, T), { ref: R })) : l.createElement(v, be({}, T, { className: u, ref: R }), s)
  );
});
an.defaultProps = {
  enabled: !0,
  removeScrollBar: !0,
  inert: !1
};
an.classNames = {
  fullWidth: Ut,
  zeroRight: Gt
};
var id = function() {
  if (typeof __webpack_nonce__ < "u")
    return __webpack_nonce__;
};
function ad() {
  if (!document)
    return null;
  var e = document.createElement("style");
  e.type = "text/css";
  var t = id();
  return t && e.setAttribute("nonce", t), e;
}
function sd(e, t) {
  e.styleSheet ? e.styleSheet.cssText = t : e.appendChild(document.createTextNode(t));
}
function ld(e) {
  var t = document.head || document.getElementsByTagName("head")[0];
  t.appendChild(e);
}
var cd = function() {
  var e = 0, t = null;
  return {
    add: function(n) {
      e == 0 && (t = ad()) && (sd(t, n), ld(t)), e++;
    },
    remove: function() {
      e--, !e && t && (t.parentNode && t.parentNode.removeChild(t), t = null);
    }
  };
}, ud = function() {
  var e = cd();
  return function(t, n) {
    l.useEffect(function() {
      return e.add(t), function() {
        e.remove();
      };
    }, [t && n]);
  };
}, Li = function() {
  var e = ud(), t = function(n) {
    var r = n.styles, o = n.dynamic;
    return e(r, o), null;
  };
  return t;
}, dd = {
  left: 0,
  top: 0,
  right: 0,
  gap: 0
}, Rn = function(e) {
  return parseInt(e || "", 10) || 0;
}, fd = function(e) {
  var t = window.getComputedStyle(document.body), n = t[e === "padding" ? "paddingLeft" : "marginLeft"], r = t[e === "padding" ? "paddingTop" : "marginTop"], o = t[e === "padding" ? "paddingRight" : "marginRight"];
  return [Rn(n), Rn(r), Rn(o)];
}, pd = function(e) {
  if (e === void 0 && (e = "margin"), typeof window > "u")
    return dd;
  var t = fd(e), n = document.documentElement.clientWidth, r = window.innerWidth;
  return {
    left: t[0],
    top: t[1],
    right: t[2],
    gap: Math.max(0, r - n + t[2] - t[0])
  };
}, md = Li(), ot = "data-scroll-locked", hd = function(e, t, n, r) {
  var o = e.left, i = e.top, a = e.right, s = e.gap;
  return n === void 0 && (n = "margin"), `
  .`.concat(Zu, ` {
   overflow: hidden `).concat(r, `;
   padding-right: `).concat(s, "px ").concat(r, `;
  }
  body[`).concat(ot, `] {
    overflow: hidden `).concat(r, `;
    overscroll-behavior: contain;
    `).concat([
    t && "position: relative ".concat(r, ";"),
    n === "margin" && `
    padding-left: `.concat(o, `px;
    padding-top: `).concat(i, `px;
    padding-right: `).concat(a, `px;
    margin-left:0;
    margin-top:0;
    margin-right: `).concat(s, "px ").concat(r, `;
    `),
    n === "padding" && "padding-right: ".concat(s, "px ").concat(r, ";")
  ].filter(Boolean).join(""), `
  }
  
  .`).concat(Gt, ` {
    right: `).concat(s, "px ").concat(r, `;
  }
  
  .`).concat(Ut, ` {
    margin-right: `).concat(s, "px ").concat(r, `;
  }
  
  .`).concat(Gt, " .").concat(Gt, ` {
    right: 0 `).concat(r, `;
  }
  
  .`).concat(Ut, " .").concat(Ut, ` {
    margin-right: 0 `).concat(r, `;
  }
  
  body[`).concat(ot, `] {
    `).concat(qu, ": ").concat(s, `px;
  }
`);
}, eo = function() {
  var e = parseInt(document.body.getAttribute(ot) || "0", 10);
  return isFinite(e) ? e : 0;
}, gd = function() {
  l.useEffect(function() {
    return document.body.setAttribute(ot, (eo() + 1).toString()), function() {
      var e = eo() - 1;
      e <= 0 ? document.body.removeAttribute(ot) : document.body.setAttribute(ot, e.toString());
    };
  }, []);
}, bd = function(e) {
  var t = e.noRelative, n = e.noImportant, r = e.gapMode, o = r === void 0 ? "margin" : r;
  gd();
  var i = l.useMemo(function() {
    return pd(o);
  }, [o]);
  return l.createElement(md, { styles: hd(i, !t, o, n ? "" : "!important") });
}, _n = !1;
if (typeof window < "u")
  try {
    var zt = Object.defineProperty({}, "passive", {
      get: function() {
        return _n = !0, !0;
      }
    });
    window.addEventListener("test", zt, zt), window.removeEventListener("test", zt, zt);
  } catch {
    _n = !1;
  }
var et = _n ? { passive: !1 } : !1, vd = function(e) {
  return e.tagName === "TEXTAREA";
}, Ii = function(e, t) {
  if (!(e instanceof Element))
    return !1;
  var n = window.getComputedStyle(e);
  return (
    // not-not-scrollable
    n[t] !== "hidden" && // contains scroll inside self
    !(n.overflowY === n.overflowX && !vd(e) && n[t] === "visible")
  );
}, yd = function(e) {
  return Ii(e, "overflowY");
}, wd = function(e) {
  return Ii(e, "overflowX");
}, to = function(e, t) {
  var n = t.ownerDocument, r = t;
  do {
    typeof ShadowRoot < "u" && r instanceof ShadowRoot && (r = r.host);
    var o = zi(e, r);
    if (o) {
      var i = _i(e, r), a = i[1], s = i[2];
      if (a > s)
        return !0;
    }
    r = r.parentNode;
  } while (r && r !== n.body);
  return !1;
}, xd = function(e) {
  var t = e.scrollTop, n = e.scrollHeight, r = e.clientHeight;
  return [
    t,
    n,
    r
  ];
}, kd = function(e) {
  var t = e.scrollLeft, n = e.scrollWidth, r = e.clientWidth;
  return [
    t,
    n,
    r
  ];
}, zi = function(e, t) {
  return e === "v" ? yd(t) : wd(t);
}, _i = function(e, t) {
  return e === "v" ? xd(t) : kd(t);
}, Ed = function(e, t) {
  return e === "h" && t === "rtl" ? -1 : 1;
}, Cd = function(e, t, n, r, o) {
  var i = Ed(e, window.getComputedStyle(t).direction), a = i * r, s = n.target, u = t.contains(s), c = !1, d = a > 0, f = 0, m = 0;
  do {
    if (!s)
      break;
    var h = _i(e, s), b = h[0], p = h[1], g = h[2], w = p - g - i * b;
    (b || w) && zi(e, s) && (f += w, m += b);
    var v = s.parentNode;
    s = v && v.nodeType === Node.DOCUMENT_FRAGMENT_NODE ? v.host : v;
  } while (
    // portaled content
    !u && s !== document.body || // self content
    u && (t.contains(s) || t === s)
  );
  return (d && Math.abs(f) < 1 || !d && Math.abs(m) < 1) && (c = !0), c;
}, _t = function(e) {
  return "changedTouches" in e ? [e.changedTouches[0].clientX, e.changedTouches[0].clientY] : [0, 0];
}, no = function(e) {
  return [e.deltaX, e.deltaY];
}, ro = function(e) {
  return e && "current" in e ? e.current : e;
}, Sd = function(e, t) {
  return e[0] === t[0] && e[1] === t[1];
}, Rd = function(e) {
  return `
  .block-interactivity-`.concat(e, ` {pointer-events: none;}
  .allow-interactivity-`).concat(e, ` {pointer-events: all;}
`);
}, Pd = 0, tt = [];
function Nd(e) {
  var t = l.useRef([]), n = l.useRef([0, 0]), r = l.useRef(), o = l.useState(Pd++)[0], i = l.useState(Li)[0], a = l.useRef(e);
  l.useEffect(function() {
    a.current = e;
  }, [e]), l.useEffect(function() {
    if (e.inert) {
      document.body.classList.add("block-interactivity-".concat(o));
      var p = Ku([e.lockRef.current], (e.shards || []).map(ro)).filter(Boolean);
      return p.forEach(function(g) {
        return g.classList.add("allow-interactivity-".concat(o));
      }), function() {
        document.body.classList.remove("block-interactivity-".concat(o)), p.forEach(function(g) {
          return g.classList.remove("allow-interactivity-".concat(o));
        });
      };
    }
  }, [e.inert, e.lockRef.current, e.shards]);
  var s = l.useCallback(function(p, g) {
    if ("touches" in p && p.touches.length === 2 || p.type === "wheel" && p.ctrlKey)
      return !a.current.allowPinchZoom;
    var w = _t(p), v = n.current, x = "deltaX" in p ? p.deltaX : v[0] - w[0], E = "deltaY" in p ? p.deltaY : v[1] - w[1], S, R = p.target, T = Math.abs(x) > Math.abs(E) ? "h" : "v";
    if ("touches" in p && T === "h" && R.type === "range")
      return !1;
    var y = window.getSelection(), M = y && y.anchorNode, W = M ? M === R || M.contains(R) : !1;
    if (W)
      return !1;
    var O = to(T, R);
    if (!O)
      return !0;
    if (O ? S = T : (S = T === "v" ? "h" : "v", O = to(T, R)), !O)
      return !1;
    if (!r.current && "changedTouches" in p && (x || E) && (r.current = S), !S)
      return !0;
    var L = r.current || S;
    return Cd(L, g, p, L === "h" ? x : E);
  }, []), u = l.useCallback(function(p) {
    var g = p;
    if (!(!tt.length || tt[tt.length - 1] !== i)) {
      var w = "deltaY" in g ? no(g) : _t(g), v = t.current.filter(function(S) {
        return S.name === g.type && (S.target === g.target || g.target === S.shadowParent) && Sd(S.delta, w);
      })[0];
      if (v && v.should) {
        g.cancelable && g.preventDefault();
        return;
      }
      if (!v) {
        var x = (a.current.shards || []).map(ro).filter(Boolean).filter(function(S) {
          return S.contains(g.target);
        }), E = x.length > 0 ? s(g, x[0]) : !a.current.noIsolation;
        E && g.cancelable && g.preventDefault();
      }
    }
  }, []), c = l.useCallback(function(p, g, w, v) {
    var x = { name: p, delta: g, target: w, should: v, shadowParent: Od(w) };
    t.current.push(x), setTimeout(function() {
      t.current = t.current.filter(function(E) {
        return E !== x;
      });
    }, 1);
  }, []), d = l.useCallback(function(p) {
    n.current = _t(p), r.current = void 0;
  }, []), f = l.useCallback(function(p) {
    c(p.type, no(p), p.target, s(p, e.lockRef.current));
  }, []), m = l.useCallback(function(p) {
    c(p.type, _t(p), p.target, s(p, e.lockRef.current));
  }, []);
  l.useEffect(function() {
    return tt.push(i), e.setCallbacks({
      onScrollCapture: f,
      onWheelCapture: f,
      onTouchMoveCapture: m
    }), document.addEventListener("wheel", u, et), document.addEventListener("touchmove", u, et), document.addEventListener("touchstart", d, et), function() {
      tt = tt.filter(function(p) {
        return p !== i;
      }), document.removeEventListener("wheel", u, et), document.removeEventListener("touchmove", u, et), document.removeEventListener("touchstart", d, et);
    };
  }, []);
  var h = e.removeScrollBar, b = e.inert;
  return l.createElement(
    l.Fragment,
    null,
    b ? l.createElement(i, { styles: Rd(o) }) : null,
    h ? l.createElement(bd, { noRelative: e.noRelative, gapMode: e.gapMode }) : null
  );
}
function Od(e) {
  for (var t = null; e !== null; )
    e instanceof ShadowRoot && (t = e.host, e = e.host), e = e.parentNode;
  return t;
}
const Ad = od(Mi, Nd);
var Fi = l.forwardRef(function(e, t) {
  return l.createElement(an, be({}, e, { ref: t, sideCar: Ad }));
});
Fi.classNames = an.classNames;
var Dd = function(e) {
  if (typeof document > "u")
    return null;
  var t = Array.isArray(e) ? e[0] : e;
  return t.ownerDocument.body;
}, nt = /* @__PURE__ */ new WeakMap(), Ft = /* @__PURE__ */ new WeakMap(), Wt = {}, Pn = 0, Wi = function(e) {
  return e && (e.host || Wi(e.parentNode));
}, Td = function(e, t) {
  return t.map(function(n) {
    if (e.contains(n))
      return n;
    var r = Wi(n);
    return r && e.contains(r) ? r : (console.error("aria-hidden", n, "in not contained inside", e, ". Doing nothing"), null);
  }).filter(function(n) {
    return !!n;
  });
}, Md = function(e, t, n, r) {
  var o = Td(t, Array.isArray(e) ? e : [e]);
  Wt[n] || (Wt[n] = /* @__PURE__ */ new WeakMap());
  var i = Wt[n], a = [], s = /* @__PURE__ */ new Set(), u = new Set(o), c = function(f) {
    !f || s.has(f) || (s.add(f), c(f.parentNode));
  };
  o.forEach(c);
  var d = function(f) {
    !f || u.has(f) || Array.prototype.forEach.call(f.children, function(m) {
      if (s.has(m))
        d(m);
      else
        try {
          var h = m.getAttribute(r), b = h !== null && h !== "false", p = (nt.get(m) || 0) + 1, g = (i.get(m) || 0) + 1;
          nt.set(m, p), i.set(m, g), a.push(m), p === 1 && b && Ft.set(m, !0), g === 1 && m.setAttribute(n, "true"), b || m.setAttribute(r, "true");
        } catch (w) {
          console.error("aria-hidden: cannot operate on ", m, w);
        }
    });
  };
  return d(t), s.clear(), Pn++, function() {
    a.forEach(function(f) {
      var m = nt.get(f) - 1, h = i.get(f) - 1;
      nt.set(f, m), i.set(f, h), m || (Ft.has(f) || f.removeAttribute(r), Ft.delete(f)), h || f.removeAttribute(n);
    }), Pn--, Pn || (nt = /* @__PURE__ */ new WeakMap(), nt = /* @__PURE__ */ new WeakMap(), Ft = /* @__PURE__ */ new WeakMap(), Wt = {});
  };
}, Ld = function(e, t, n) {
  n === void 0 && (n = "data-aria-hidden");
  var r = Array.from(Array.isArray(e) ? e : [e]), o = Dd(e);
  return o ? (r.push.apply(r, Array.from(o.querySelectorAll("[aria-live], script"))), Md(r, o, n, "aria-hidden")) : function() {
    return null;
  };
}, ji = "Dialog", [Bi] = Zn(ji), [Ap, de] = Bi(ji), $i = "DialogTrigger", Id = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = de($i, n), i = ue(t, o.triggerRef);
    return /* @__PURE__ */ C(
      J.button,
      {
        type: "button",
        "aria-haspopup": "dialog",
        "aria-expanded": o.open,
        "aria-controls": o.open ? o.contentId : void 0,
        "data-state": er(o.open),
        ...r,
        ref: i,
        onClick: q(e.onClick, o.onOpenToggle)
      }
    );
  }
);
Id.displayName = $i;
var Jn = "DialogPortal", [zd, Gi] = Bi(Jn, {
  forceMount: void 0
}), Ui = (e) => {
  const { __scopeDialog: t, forceMount: n, children: r, container: o } = e, i = de(Jn, t);
  return /* @__PURE__ */ C(zd, { scope: t, forceMount: n, children: l.Children.map(r, (a) => /* @__PURE__ */ C(yt, { present: n || i.open, children: /* @__PURE__ */ C(Ai, { asChild: !0, container: o, children: a }) })) });
};
Ui.displayName = Jn;
var qt = "DialogOverlay", Vi = l.forwardRef(
  (e, t) => {
    const n = Gi(qt, e.__scopeDialog), { forceMount: r = n.forceMount, ...o } = e, i = de(qt, e.__scopeDialog);
    return i.modal ? /* @__PURE__ */ C(yt, { present: r || i.open, children: /* @__PURE__ */ C(Fd, { ...o, ref: t }) }) : null;
  }
);
Vi.displayName = qt;
var _d = /* @__PURE__ */ Xn("DialogOverlay.RemoveScroll"), Fd = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = de(qt, n), i = Mu(), a = ue(t, i);
    return (
      // Make sure `Content` is scrollable even when it doesn't live inside `RemoveScroll`
      // ie. when `Overlay` and `Content` are siblings
      /* @__PURE__ */ C(Fi, { as: _d, allowPinchZoom: !0, shards: [o.contentRef], children: /* @__PURE__ */ C(
        J.div,
        {
          "data-state": er(o.open),
          ...r,
          ref: a,
          style: { pointerEvents: "auto", ...r.style }
        }
      ) })
    );
  }
), lt = "DialogContent", Hi = l.forwardRef(
  (e, t) => {
    const n = Gi(lt, e.__scopeDialog), { forceMount: r = n.forceMount, ...o } = e, i = de(lt, e.__scopeDialog);
    return /* @__PURE__ */ C(yt, { present: r || i.open, children: i.modal ? /* @__PURE__ */ C(Wd, { ...o, ref: t }) : /* @__PURE__ */ C(jd, { ...o, ref: t }) });
  }
);
Hi.displayName = lt;
var Wd = l.forwardRef(
  (e, t) => {
    const n = de(lt, e.__scopeDialog), r = l.useRef(null), o = ue(t, n.contentRef, r);
    return l.useEffect(() => {
      const i = r.current;
      if (i) return Ld(i);
    }, []), /* @__PURE__ */ C(
      Xi,
      {
        ...e,
        ref: o,
        trapFocus: n.open,
        disableOutsidePointerEvents: n.open,
        onCloseAutoFocus: q(e.onCloseAutoFocus, (i) => {
          var a;
          i.preventDefault(), (a = n.triggerRef.current) == null || a.focus();
        }),
        onPointerDownOutside: q(e.onPointerDownOutside, (i) => {
          const a = i.detail.originalEvent, s = a.button === 0 && a.ctrlKey === !0;
          (a.button === 2 || s) && i.preventDefault();
        }),
        onFocusOutside: q(
          e.onFocusOutside,
          (i) => i.preventDefault()
        )
      }
    );
  }
), jd = l.forwardRef(
  (e, t) => {
    const n = de(lt, e.__scopeDialog), r = l.useRef(!1), o = l.useRef(!1);
    return /* @__PURE__ */ C(
      Xi,
      {
        ...e,
        ref: t,
        trapFocus: !1,
        disableOutsidePointerEvents: !1,
        onCloseAutoFocus: (i) => {
          var a, s;
          (a = e.onCloseAutoFocus) == null || a.call(e, i), i.defaultPrevented || (r.current || (s = n.triggerRef.current) == null || s.focus(), i.preventDefault()), r.current = !1, o.current = !1;
        },
        onInteractOutside: (i) => {
          var a, s;
          (a = e.onInteractOutside) == null || a.call(e, i), i.defaultPrevented || (r.current = !0, i.detail.originalEvent.type === "pointerdown" && (o.current = !0));
          const u = i.target;
          (s = n.triggerRef.current) != null && s.contains(u) && i.preventDefault(), i.detail.originalEvent.type === "focusin" && o.current && i.preventDefault();
        }
      }
    );
  }
), Xi = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, trapFocus: r, onOpenAutoFocus: o, onCloseAutoFocus: i, ...a } = e, s = de(lt, n);
    return Yu(), /* @__PURE__ */ C(bo, { children: /* @__PURE__ */ C(
      Ni,
      {
        asChild: !0,
        loop: !0,
        trapped: r,
        onMountAutoFocus: o,
        onUnmountAutoFocus: i,
        children: /* @__PURE__ */ C(
          Qn,
          {
            role: "dialog",
            id: s.contentId,
            "aria-describedby": s.descriptionId,
            "aria-labelledby": s.titleId,
            "data-state": er(s.open),
            ...a,
            ref: t,
            deferPointerDownOutside: !0,
            onDismiss: () => s.onOpenChange(!1)
          }
        )
      }
    ) });
  }
), Yi = "DialogTitle", Ki = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = de(Yi, n);
    return /* @__PURE__ */ C(J.h2, { id: o.titleId, ...r, ref: t });
  }
);
Ki.displayName = Yi;
var Zi = "DialogDescription", qi = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = de(Zi, n);
    return /* @__PURE__ */ C(J.p, { id: o.descriptionId, ...r, ref: t });
  }
);
qi.displayName = Zi;
var Qi = "DialogClose", Ji = l.forwardRef(
  (e, t) => {
    const { __scopeDialog: n, ...r } = e, o = de(Qi, n);
    return /* @__PURE__ */ C(
      J.button,
      {
        type: "button",
        ...r,
        ref: t,
        onClick: q(e.onClick, () => o.onOpenChange(!1))
      }
    );
  }
);
Ji.displayName = Qi;
function er(e) {
  return e ? "open" : "closed";
}
function Bd({ ...e }) {
  return /* @__PURE__ */ C(Ui, { ...e });
}
const $d = l.forwardRef(function({ className: e, ...t }, n) {
  return /* @__PURE__ */ C(
    Vi,
    {
      ref: n,
      className: He("fixed inset-0 z-50 bg-black/50 animate-in fade-in-0", e),
      ...t
    }
  );
});
l.forwardRef(function({ className: e, children: t, side: n = "right", ...r }, o) {
  return /* @__PURE__ */ Q(Bd, { children: [
    /* @__PURE__ */ C($d, {}),
    /* @__PURE__ */ Q(
      Hi,
      {
        ref: o,
        className: He(
          "fixed z-50 flex flex-col gap-4 bg-nr-bg text-nr-fg shadow-lg transition ease-in-out animate-in",
          n === "right" && "inset-y-0 right-0 h-full w-3/4 border-l border-nr-border sm:max-w-sm",
          n === "left" && "inset-y-0 left-0 h-full w-3/4 border-r border-nr-border sm:max-w-sm",
          n === "top" && "inset-x-0 top-0 h-auto border-b border-nr-border",
          n === "bottom" && "inset-x-0 bottom-0 h-auto border-t border-nr-border",
          e
        ),
        ...r,
        children: [
          t,
          /* @__PURE__ */ Q(Ji, { className: "absolute right-4 top-4 rounded-sm opacity-70 transition-opacity hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-nr-accent/25", children: [
            /* @__PURE__ */ C(Dc, { className: "h-4 w-4" }),
            /* @__PURE__ */ C("span", { className: "sr-only", children: "Close" })
          ] })
        ]
      }
    )
  ] });
});
l.forwardRef(function({ className: e, ...t }, n) {
  return /* @__PURE__ */ C(Ki, { ref: n, className: He("font-semibold text-nr-fg", e), ...t });
});
l.forwardRef(function({ className: e, ...t }, n) {
  return /* @__PURE__ */ C(qi, { ref: n, className: He("text-sm text-nr-muted", e), ...t });
});
const Gd = ["top", "right", "bottom", "left"], Fe = Math.min, te = Math.max, Qt = Math.round, jt = Math.floor, ye = (e) => ({
  x: e,
  y: e
}), Ud = {
  left: "right",
  right: "left",
  bottom: "top",
  top: "bottom"
};
function Fn(e, t, n) {
  return te(e, Fe(t, n));
}
function Re(e, t) {
  return typeof e == "function" ? e(t) : e;
}
function Pe(e) {
  return e.split("-")[0];
}
function dt(e) {
  return e.split("-")[1];
}
function tr(e) {
  return e === "x" ? "y" : "x";
}
function nr(e) {
  return e === "y" ? "height" : "width";
}
function ve(e) {
  const t = e[0];
  return t === "t" || t === "b" ? "y" : "x";
}
function rr(e) {
  return tr(ve(e));
}
function Vd(e, t, n) {
  n === void 0 && (n = !1);
  const r = dt(e), o = rr(e), i = nr(o);
  let a = o === "x" ? r === (n ? "end" : "start") ? "right" : "left" : r === "start" ? "bottom" : "top";
  return t.reference[i] > t.floating[i] && (a = Jt(a)), [a, Jt(a)];
}
function Hd(e) {
  const t = Jt(e);
  return [Wn(e), t, Wn(t)];
}
function Wn(e) {
  return e.includes("start") ? e.replace("start", "end") : e.replace("end", "start");
}
const oo = ["left", "right"], io = ["right", "left"], Xd = ["top", "bottom"], Yd = ["bottom", "top"];
function Kd(e, t, n) {
  switch (e) {
    case "top":
    case "bottom":
      return n ? t ? io : oo : t ? oo : io;
    case "left":
    case "right":
      return t ? Xd : Yd;
    default:
      return [];
  }
}
function Zd(e, t, n, r) {
  const o = dt(e);
  let i = Kd(Pe(e), n === "start", r);
  return o && (i = i.map((a) => a + "-" + o), t && (i = i.concat(i.map(Wn)))), i;
}
function Jt(e) {
  const t = Pe(e);
  return Ud[t] + e.slice(t.length);
}
function qd(e) {
  return {
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    ...e
  };
}
function ea(e) {
  return typeof e != "number" ? qd(e) : {
    top: e,
    right: e,
    bottom: e,
    left: e
  };
}
function en(e) {
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
function ao(e, t, n) {
  let {
    reference: r,
    floating: o
  } = e;
  const i = ve(t), a = rr(t), s = nr(a), u = Pe(t), c = i === "y", d = r.x + r.width / 2 - o.width / 2, f = r.y + r.height / 2 - o.height / 2, m = r[s] / 2 - o[s] / 2;
  let h;
  switch (u) {
    case "top":
      h = {
        x: d,
        y: r.y - o.height
      };
      break;
    case "bottom":
      h = {
        x: d,
        y: r.y + r.height
      };
      break;
    case "right":
      h = {
        x: r.x + r.width,
        y: f
      };
      break;
    case "left":
      h = {
        x: r.x - o.width,
        y: f
      };
      break;
    default:
      h = {
        x: r.x,
        y: r.y
      };
  }
  switch (dt(t)) {
    case "start":
      h[a] -= m * (n && c ? -1 : 1);
      break;
    case "end":
      h[a] += m * (n && c ? -1 : 1);
      break;
  }
  return h;
}
async function Qd(e, t) {
  var n;
  t === void 0 && (t = {});
  const {
    x: r,
    y: o,
    platform: i,
    rects: a,
    elements: s,
    strategy: u
  } = e, {
    boundary: c = "clippingAncestors",
    rootBoundary: d = "viewport",
    elementContext: f = "floating",
    altBoundary: m = !1,
    padding: h = 0
  } = Re(t, e), b = ea(h), p = s[m ? f === "floating" ? "reference" : "floating" : f], g = en(await i.getClippingRect({
    element: (n = await (i.isElement == null ? void 0 : i.isElement(p))) == null || n ? p : p.contextElement || await (i.getDocumentElement == null ? void 0 : i.getDocumentElement(s.floating)),
    boundary: c,
    rootBoundary: d,
    strategy: u
  })), w = f === "floating" ? {
    x: r,
    y: o,
    width: a.floating.width,
    height: a.floating.height
  } : a.reference, v = await (i.getOffsetParent == null ? void 0 : i.getOffsetParent(s.floating)), x = await (i.isElement == null ? void 0 : i.isElement(v)) ? await (i.getScale == null ? void 0 : i.getScale(v)) || {
    x: 1,
    y: 1
  } : {
    x: 1,
    y: 1
  }, E = en(i.convertOffsetParentRelativeRectToViewportRelativeRect ? await i.convertOffsetParentRelativeRectToViewportRelativeRect({
    elements: s,
    rect: w,
    offsetParent: v,
    strategy: u
  }) : w);
  return {
    top: (g.top - E.top + b.top) / x.y,
    bottom: (E.bottom - g.bottom + b.bottom) / x.y,
    left: (g.left - E.left + b.left) / x.x,
    right: (E.right - g.right + b.right) / x.x
  };
}
const Jd = 50, ef = async (e, t, n) => {
  const {
    placement: r = "bottom",
    strategy: o = "absolute",
    middleware: i = [],
    platform: a
  } = n, s = a.detectOverflow ? a : {
    ...a,
    detectOverflow: Qd
  }, u = await (a.isRTL == null ? void 0 : a.isRTL(t));
  let c = await a.getElementRects({
    reference: e,
    floating: t,
    strategy: o
  }), {
    x: d,
    y: f
  } = ao(c, r, u), m = r, h = 0;
  const b = {};
  for (let p = 0; p < i.length; p++) {
    const g = i[p];
    if (!g)
      continue;
    const {
      name: w,
      fn: v
    } = g, {
      x,
      y: E,
      data: S,
      reset: R
    } = await v({
      x: d,
      y: f,
      initialPlacement: r,
      placement: m,
      strategy: o,
      middlewareData: b,
      rects: c,
      platform: s,
      elements: {
        reference: e,
        floating: t
      }
    });
    d = x ?? d, f = E ?? f, b[w] = {
      ...b[w],
      ...S
    }, R && h < Jd && (h++, typeof R == "object" && (R.placement && (m = R.placement), R.rects && (c = R.rects === !0 ? await a.getElementRects({
      reference: e,
      floating: t,
      strategy: o
    }) : R.rects), {
      x: d,
      y: f
    } = ao(c, m, u)), p = -1);
  }
  return {
    x: d,
    y: f,
    placement: m,
    strategy: o,
    middlewareData: b
  };
}, tf = (e) => ({
  name: "arrow",
  options: e,
  async fn(t) {
    const {
      x: n,
      y: r,
      placement: o,
      rects: i,
      platform: a,
      elements: s,
      middlewareData: u
    } = t, {
      element: c,
      padding: d = 0
    } = Re(e, t) || {};
    if (c == null)
      return {};
    const f = ea(d), m = {
      x: n,
      y: r
    }, h = rr(o), b = nr(h), p = await a.getDimensions(c), g = h === "y", w = g ? "top" : "left", v = g ? "bottom" : "right", x = g ? "clientHeight" : "clientWidth", E = i.reference[b] + i.reference[h] - m[h] - i.floating[b], S = m[h] - i.reference[h], R = await (a.getOffsetParent == null ? void 0 : a.getOffsetParent(c));
    let T = R ? R[x] : 0;
    (!T || !await (a.isElement == null ? void 0 : a.isElement(R))) && (T = s.floating[x] || i.floating[b]);
    const y = E / 2 - S / 2, M = T / 2 - p[b] / 2 - 1, W = Fe(f[w], M), O = Fe(f[v], M), L = W, V = T - p[b] - O, j = T / 2 - p[b] / 2 + y, X = Fn(L, j, V), I = !u.arrow && dt(o) != null && j !== X && i.reference[b] / 2 - (j < L ? W : O) - p[b] / 2 < 0, B = I ? j < L ? j - L : j - V : 0;
    return {
      [h]: m[h] + B,
      data: {
        [h]: X,
        centerOffset: j - X - B,
        ...I && {
          alignmentOffset: B
        }
      },
      reset: I
    };
  }
}), nf = function(e) {
  return e === void 0 && (e = {}), {
    name: "flip",
    options: e,
    async fn(t) {
      var n, r;
      const {
        placement: o,
        middlewareData: i,
        rects: a,
        initialPlacement: s,
        platform: u,
        elements: c
      } = t, {
        mainAxis: d = !0,
        crossAxis: f = !0,
        fallbackPlacements: m,
        fallbackStrategy: h = "bestFit",
        fallbackAxisSideDirection: b = "none",
        flipAlignment: p = !0,
        ...g
      } = Re(e, t);
      if ((n = i.arrow) != null && n.alignmentOffset)
        return {};
      const w = Pe(o), v = ve(s), x = Pe(s) === s, E = await (u.isRTL == null ? void 0 : u.isRTL(c.floating)), S = m || (x || !p ? [Jt(s)] : Hd(s)), R = b !== "none";
      !m && R && S.push(...Zd(s, p, b, E));
      const T = [s, ...S], y = await u.detectOverflow(t, g), M = [];
      let W = ((r = i.flip) == null ? void 0 : r.overflows) || [];
      if (d && M.push(y[w]), f) {
        const j = Vd(o, a, E);
        M.push(y[j[0]], y[j[1]]);
      }
      if (W = [...W, {
        placement: o,
        overflows: M
      }], !M.every((j) => j <= 0)) {
        var O, L;
        const j = (((O = i.flip) == null ? void 0 : O.index) || 0) + 1, X = T[j];
        if (X && (!(f === "alignment" && v !== ve(X)) || // We leave the current main axis only if every placement on that axis
        // overflows the main axis.
        W.every((B) => ve(B.placement) === v ? B.overflows[0] > 0 : !0)))
          return {
            data: {
              index: j,
              overflows: W
            },
            reset: {
              placement: X
            }
          };
        let I = (L = W.filter((B) => B.overflows[0] <= 0).sort((B, $) => B.overflows[1] - $.overflows[1])[0]) == null ? void 0 : L.placement;
        if (!I)
          switch (h) {
            case "bestFit": {
              var V;
              const B = (V = W.filter(($) => {
                if (R) {
                  const H = ve($.placement);
                  return H === v || // Create a bias to the `y` side axis due to horizontal
                  // reading directions favoring greater width.
                  H === "y";
                }
                return !0;
              }).map(($) => [$.placement, $.overflows.filter((H) => H > 0).reduce((H, k) => H + k, 0)]).sort(($, H) => $[1] - H[1])[0]) == null ? void 0 : V[0];
              B && (I = B);
              break;
            }
            case "initialPlacement":
              I = s;
              break;
          }
        if (o !== I)
          return {
            reset: {
              placement: I
            }
          };
      }
      return {};
    }
  };
};
function so(e, t) {
  return {
    top: e.top - t.height,
    right: e.right - t.width,
    bottom: e.bottom - t.height,
    left: e.left - t.width
  };
}
function lo(e) {
  return Gd.some((t) => e[t] >= 0);
}
const rf = function(e) {
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
      } = Re(e, t);
      switch (o) {
        case "referenceHidden": {
          const a = await r.detectOverflow(t, {
            ...i,
            elementContext: "reference"
          }), s = so(a, n.reference);
          return {
            data: {
              referenceHiddenOffsets: s,
              referenceHidden: lo(s)
            }
          };
        }
        case "escaped": {
          const a = await r.detectOverflow(t, {
            ...i,
            altBoundary: !0
          }), s = so(a, n.floating);
          return {
            data: {
              escapedOffsets: s,
              escaped: lo(s)
            }
          };
        }
        default:
          return {};
      }
    }
  };
}, ta = /* @__PURE__ */ new Set(["left", "top"]);
async function of(e, t) {
  const {
    placement: n,
    platform: r,
    elements: o
  } = e, i = await (r.isRTL == null ? void 0 : r.isRTL(o.floating)), a = Pe(n), s = dt(n), u = ve(n) === "y", c = ta.has(a) ? -1 : 1, d = i && u ? -1 : 1, f = Re(t, e);
  let {
    mainAxis: m,
    crossAxis: h,
    alignmentAxis: b
  } = typeof f == "number" ? {
    mainAxis: f,
    crossAxis: 0,
    alignmentAxis: null
  } : {
    mainAxis: f.mainAxis || 0,
    crossAxis: f.crossAxis || 0,
    alignmentAxis: f.alignmentAxis
  };
  return s && typeof b == "number" && (h = s === "end" ? b * -1 : b), u ? {
    x: h * d,
    y: m * c
  } : {
    x: m * c,
    y: h * d
  };
}
const af = function(e) {
  return e === void 0 && (e = 0), {
    name: "offset",
    options: e,
    async fn(t) {
      var n, r;
      const {
        x: o,
        y: i,
        placement: a,
        middlewareData: s
      } = t, u = await of(t, e);
      return a === ((n = s.offset) == null ? void 0 : n.placement) && (r = s.arrow) != null && r.alignmentOffset ? {} : {
        x: o + u.x,
        y: i + u.y,
        data: {
          ...u,
          placement: a
        }
      };
    }
  };
}, sf = function(e) {
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
        mainAxis: a = !0,
        crossAxis: s = !1,
        limiter: u = {
          fn: (w) => {
            let {
              x: v,
              y: x
            } = w;
            return {
              x: v,
              y: x
            };
          }
        },
        ...c
      } = Re(e, t), d = {
        x: n,
        y: r
      }, f = await i.detectOverflow(t, c), m = ve(Pe(o)), h = tr(m);
      let b = d[h], p = d[m];
      if (a) {
        const w = h === "y" ? "top" : "left", v = h === "y" ? "bottom" : "right", x = b + f[w], E = b - f[v];
        b = Fn(x, b, E);
      }
      if (s) {
        const w = m === "y" ? "top" : "left", v = m === "y" ? "bottom" : "right", x = p + f[w], E = p - f[v];
        p = Fn(x, p, E);
      }
      const g = u.fn({
        ...t,
        [h]: b,
        [m]: p
      });
      return {
        ...g,
        data: {
          x: g.x - n,
          y: g.y - r,
          enabled: {
            [h]: a,
            [m]: s
          }
        }
      };
    }
  };
}, lf = function(e) {
  return e === void 0 && (e = {}), {
    options: e,
    fn(t) {
      const {
        x: n,
        y: r,
        placement: o,
        rects: i,
        middlewareData: a
      } = t, {
        offset: s = 0,
        mainAxis: u = !0,
        crossAxis: c = !0
      } = Re(e, t), d = {
        x: n,
        y: r
      }, f = ve(o), m = tr(f);
      let h = d[m], b = d[f];
      const p = Re(s, t), g = typeof p == "number" ? {
        mainAxis: p,
        crossAxis: 0
      } : {
        mainAxis: 0,
        crossAxis: 0,
        ...p
      };
      if (u) {
        const x = m === "y" ? "height" : "width", E = i.reference[m] - i.floating[x] + g.mainAxis, S = i.reference[m] + i.reference[x] - g.mainAxis;
        h < E ? h = E : h > S && (h = S);
      }
      if (c) {
        var w, v;
        const x = m === "y" ? "width" : "height", E = ta.has(Pe(o)), S = i.reference[f] - i.floating[x] + (E && ((w = a.offset) == null ? void 0 : w[f]) || 0) + (E ? 0 : g.crossAxis), R = i.reference[f] + i.reference[x] + (E ? 0 : ((v = a.offset) == null ? void 0 : v[f]) || 0) - (E ? g.crossAxis : 0);
        b < S ? b = S : b > R && (b = R);
      }
      return {
        [m]: h,
        [f]: b
      };
    }
  };
}, cf = function(e) {
  return e === void 0 && (e = {}), {
    name: "size",
    options: e,
    async fn(t) {
      var n, r;
      const {
        placement: o,
        rects: i,
        platform: a,
        elements: s
      } = t, {
        apply: u = () => {
        },
        ...c
      } = Re(e, t), d = await a.detectOverflow(t, c), f = Pe(o), m = dt(o), h = ve(o) === "y", {
        width: b,
        height: p
      } = i.floating;
      let g, w;
      f === "top" || f === "bottom" ? (g = f, w = m === (await (a.isRTL == null ? void 0 : a.isRTL(s.floating)) ? "start" : "end") ? "left" : "right") : (w = f, g = m === "end" ? "top" : "bottom");
      const v = p - d.top - d.bottom, x = b - d.left - d.right, E = Fe(p - d[g], v), S = Fe(b - d[w], x), R = !t.middlewareData.shift;
      let T = E, y = S;
      if ((n = t.middlewareData.shift) != null && n.enabled.x && (y = x), (r = t.middlewareData.shift) != null && r.enabled.y && (T = v), R && !m) {
        const W = te(d.left, 0), O = te(d.right, 0), L = te(d.top, 0), V = te(d.bottom, 0);
        h ? y = b - 2 * (W !== 0 || O !== 0 ? W + O : te(d.left, d.right)) : T = p - 2 * (L !== 0 || V !== 0 ? L + V : te(d.top, d.bottom));
      }
      await u({
        ...t,
        availableWidth: y,
        availableHeight: T
      });
      const M = await a.getDimensions(s.floating);
      return b !== M.width || p !== M.height ? {
        reset: {
          rects: !0
        }
      } : {};
    }
  };
};
function sn() {
  return typeof window < "u";
}
function ft(e) {
  return na(e) ? (e.nodeName || "").toLowerCase() : "#document";
}
function ne(e) {
  var t;
  return (e == null || (t = e.ownerDocument) == null ? void 0 : t.defaultView) || window;
}
function we(e) {
  var t;
  return (t = (na(e) ? e.ownerDocument : e.document) || window.document) == null ? void 0 : t.documentElement;
}
function na(e) {
  return sn() ? e instanceof Node || e instanceof ne(e).Node : !1;
}
function se(e) {
  return sn() ? e instanceof Element || e instanceof ne(e).Element : !1;
}
function Oe(e) {
  return sn() ? e instanceof HTMLElement || e instanceof ne(e).HTMLElement : !1;
}
function co(e) {
  return !sn() || typeof ShadowRoot > "u" ? !1 : e instanceof ShadowRoot || e instanceof ne(e).ShadowRoot;
}
function wt(e) {
  const {
    overflow: t,
    overflowX: n,
    overflowY: r,
    display: o
  } = le(e);
  return /auto|scroll|overlay|hidden|clip/.test(t + r + n) && o !== "inline" && o !== "contents";
}
function uf(e) {
  return /^(table|td|th)$/.test(ft(e));
}
function ln(e) {
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
const df = /transform|translate|scale|rotate|perspective|filter/, ff = /paint|layout|strict|content/, Ve = (e) => !!e && e !== "none";
let Nn;
function or(e) {
  const t = se(e) ? le(e) : e;
  return Ve(t.transform) || Ve(t.translate) || Ve(t.scale) || Ve(t.rotate) || Ve(t.perspective) || !ir() && (Ve(t.backdropFilter) || Ve(t.filter)) || df.test(t.willChange || "") || ff.test(t.contain || "");
}
function pf(e) {
  let t = We(e);
  for (; Oe(t) && !ct(t); ) {
    if (or(t))
      return t;
    if (ln(t))
      return null;
    t = We(t);
  }
  return null;
}
function ir() {
  return Nn == null && (Nn = typeof CSS < "u" && CSS.supports && CSS.supports("-webkit-backdrop-filter", "none")), Nn;
}
function ct(e) {
  return /^(html|body|#document)$/.test(ft(e));
}
function le(e) {
  return ne(e).getComputedStyle(e);
}
function cn(e) {
  return se(e) ? {
    scrollLeft: e.scrollLeft,
    scrollTop: e.scrollTop
  } : {
    scrollLeft: e.scrollX,
    scrollTop: e.scrollY
  };
}
function We(e) {
  if (ft(e) === "html")
    return e;
  const t = (
    // Step into the shadow DOM of the parent of a slotted node.
    e.assignedSlot || // DOM Element detected.
    e.parentNode || // ShadowRoot detected.
    co(e) && e.host || // Fallback.
    we(e)
  );
  return co(t) ? t.host : t;
}
function ra(e) {
  const t = We(e);
  return ct(t) ? e.ownerDocument ? e.ownerDocument.body : e.body : Oe(t) && wt(t) ? t : ra(t);
}
function bt(e, t, n) {
  var r;
  t === void 0 && (t = []), n === void 0 && (n = !0);
  const o = ra(e), i = o === ((r = e.ownerDocument) == null ? void 0 : r.body), a = ne(o);
  if (i) {
    const s = jn(a);
    return t.concat(a, a.visualViewport || [], wt(o) ? o : [], s && n ? bt(s) : []);
  } else
    return t.concat(o, bt(o, [], n));
}
function jn(e) {
  return e.parent && Object.getPrototypeOf(e.parent) ? e.frameElement : null;
}
function oa(e) {
  const t = le(e);
  let n = parseFloat(t.width) || 0, r = parseFloat(t.height) || 0;
  const o = Oe(e), i = o ? e.offsetWidth : n, a = o ? e.offsetHeight : r, s = Qt(n) !== i || Qt(r) !== a;
  return s && (n = i, r = a), {
    width: n,
    height: r,
    $: s
  };
}
function ar(e) {
  return se(e) ? e : e.contextElement;
}
function it(e) {
  const t = ar(e);
  if (!Oe(t))
    return ye(1);
  const n = t.getBoundingClientRect(), {
    width: r,
    height: o,
    $: i
  } = oa(t);
  let a = (i ? Qt(n.width) : n.width) / r, s = (i ? Qt(n.height) : n.height) / o;
  return (!a || !Number.isFinite(a)) && (a = 1), (!s || !Number.isFinite(s)) && (s = 1), {
    x: a,
    y: s
  };
}
const mf = /* @__PURE__ */ ye(0);
function ia(e) {
  const t = ne(e);
  return !ir() || !t.visualViewport ? mf : {
    x: t.visualViewport.offsetLeft,
    y: t.visualViewport.offsetTop
  };
}
function hf(e, t, n) {
  return t === void 0 && (t = !1), !n || t && n !== ne(e) ? !1 : t;
}
function Xe(e, t, n, r) {
  t === void 0 && (t = !1), n === void 0 && (n = !1);
  const o = e.getBoundingClientRect(), i = ar(e);
  let a = ye(1);
  t && (r ? se(r) && (a = it(r)) : a = it(e));
  const s = hf(i, n, r) ? ia(i) : ye(0);
  let u = (o.left + s.x) / a.x, c = (o.top + s.y) / a.y, d = o.width / a.x, f = o.height / a.y;
  if (i) {
    const m = ne(i), h = r && se(r) ? ne(r) : r;
    let b = m, p = jn(b);
    for (; p && r && h !== b; ) {
      const g = it(p), w = p.getBoundingClientRect(), v = le(p), x = w.left + (p.clientLeft + parseFloat(v.paddingLeft)) * g.x, E = w.top + (p.clientTop + parseFloat(v.paddingTop)) * g.y;
      u *= g.x, c *= g.y, d *= g.x, f *= g.y, u += x, c += E, b = ne(p), p = jn(b);
    }
  }
  return en({
    width: d,
    height: f,
    x: u,
    y: c
  });
}
function un(e, t) {
  const n = cn(e).scrollLeft;
  return t ? t.left + n : Xe(we(e)).left + n;
}
function aa(e, t) {
  const n = e.getBoundingClientRect(), r = n.left + t.scrollLeft - un(e, n), o = n.top + t.scrollTop;
  return {
    x: r,
    y: o
  };
}
function gf(e) {
  let {
    elements: t,
    rect: n,
    offsetParent: r,
    strategy: o
  } = e;
  const i = o === "fixed", a = we(r), s = t ? ln(t.floating) : !1;
  if (r === a || s && i)
    return n;
  let u = {
    scrollLeft: 0,
    scrollTop: 0
  }, c = ye(1);
  const d = ye(0), f = Oe(r);
  if ((f || !f && !i) && ((ft(r) !== "body" || wt(a)) && (u = cn(r)), f)) {
    const h = Xe(r);
    c = it(r), d.x = h.x + r.clientLeft, d.y = h.y + r.clientTop;
  }
  const m = a && !f && !i ? aa(a, u) : ye(0);
  return {
    width: n.width * c.x,
    height: n.height * c.y,
    x: n.x * c.x - u.scrollLeft * c.x + d.x + m.x,
    y: n.y * c.y - u.scrollTop * c.y + d.y + m.y
  };
}
function bf(e) {
  return Array.from(e.getClientRects());
}
function vf(e) {
  const t = we(e), n = cn(e), r = e.ownerDocument.body, o = te(t.scrollWidth, t.clientWidth, r.scrollWidth, r.clientWidth), i = te(t.scrollHeight, t.clientHeight, r.scrollHeight, r.clientHeight);
  let a = -n.scrollLeft + un(e);
  const s = -n.scrollTop;
  return le(r).direction === "rtl" && (a += te(t.clientWidth, r.clientWidth) - o), {
    width: o,
    height: i,
    x: a,
    y: s
  };
}
const uo = 25;
function yf(e, t) {
  const n = ne(e), r = we(e), o = n.visualViewport;
  let i = r.clientWidth, a = r.clientHeight, s = 0, u = 0;
  if (o) {
    i = o.width, a = o.height;
    const d = ir();
    (!d || d && t === "fixed") && (s = o.offsetLeft, u = o.offsetTop);
  }
  const c = un(r);
  if (c <= 0) {
    const d = r.ownerDocument, f = d.body, m = getComputedStyle(f), h = d.compatMode === "CSS1Compat" && parseFloat(m.marginLeft) + parseFloat(m.marginRight) || 0, b = Math.abs(r.clientWidth - f.clientWidth - h);
    b <= uo && (i -= b);
  } else c <= uo && (i += c);
  return {
    width: i,
    height: a,
    x: s,
    y: u
  };
}
function wf(e, t) {
  const n = Xe(e, !0, t === "fixed"), r = n.top + e.clientTop, o = n.left + e.clientLeft, i = Oe(e) ? it(e) : ye(1), a = e.clientWidth * i.x, s = e.clientHeight * i.y, u = o * i.x, c = r * i.y;
  return {
    width: a,
    height: s,
    x: u,
    y: c
  };
}
function fo(e, t, n) {
  let r;
  if (t === "viewport")
    r = yf(e, n);
  else if (t === "document")
    r = vf(we(e));
  else if (se(t))
    r = wf(t, n);
  else {
    const o = ia(e);
    r = {
      x: t.x - o.x,
      y: t.y - o.y,
      width: t.width,
      height: t.height
    };
  }
  return en(r);
}
function sa(e, t) {
  const n = We(e);
  return n === t || !se(n) || ct(n) ? !1 : le(n).position === "fixed" || sa(n, t);
}
function xf(e, t) {
  const n = t.get(e);
  if (n)
    return n;
  let r = bt(e, [], !1).filter((s) => se(s) && ft(s) !== "body"), o = null;
  const i = le(e).position === "fixed";
  let a = i ? We(e) : e;
  for (; se(a) && !ct(a); ) {
    const s = le(a), u = or(a);
    !u && s.position === "fixed" && (o = null), (i ? !u && !o : !u && s.position === "static" && o && (o.position === "absolute" || o.position === "fixed") || wt(a) && !u && sa(e, a)) ? r = r.filter((c) => c !== a) : o = s, a = We(a);
  }
  return t.set(e, r), r;
}
function kf(e) {
  let {
    element: t,
    boundary: n,
    rootBoundary: r,
    strategy: o
  } = e;
  const i = [...n === "clippingAncestors" ? ln(t) ? [] : xf(t, this._c) : [].concat(n), r], a = fo(t, i[0], o);
  let s = a.top, u = a.right, c = a.bottom, d = a.left;
  for (let f = 1; f < i.length; f++) {
    const m = fo(t, i[f], o);
    s = te(m.top, s), u = Fe(m.right, u), c = Fe(m.bottom, c), d = te(m.left, d);
  }
  return {
    width: u - d,
    height: c - s,
    x: d,
    y: s
  };
}
function Ef(e) {
  const {
    width: t,
    height: n
  } = oa(e);
  return {
    width: t,
    height: n
  };
}
function Cf(e, t, n) {
  const r = Oe(t), o = we(t), i = n === "fixed", a = Xe(e, !0, i, t);
  let s = {
    scrollLeft: 0,
    scrollTop: 0
  };
  const u = ye(0);
  function c() {
    u.x = un(o);
  }
  if (r || !r && !i)
    if ((ft(t) !== "body" || wt(o)) && (s = cn(t)), r) {
      const h = Xe(t, !0, i, t);
      u.x = h.x + t.clientLeft, u.y = h.y + t.clientTop;
    } else o && c();
  i && !r && o && c();
  const d = o && !r && !i ? aa(o, s) : ye(0), f = a.left + s.scrollLeft - u.x - d.x, m = a.top + s.scrollTop - u.y - d.y;
  return {
    x: f,
    y: m,
    width: a.width,
    height: a.height
  };
}
function On(e) {
  return le(e).position === "static";
}
function po(e, t) {
  if (!Oe(e) || le(e).position === "fixed")
    return null;
  if (t)
    return t(e);
  let n = e.offsetParent;
  return we(e) === n && (n = n.ownerDocument.body), n;
}
function la(e, t) {
  const n = ne(e);
  if (ln(e))
    return n;
  if (!Oe(e)) {
    let o = We(e);
    for (; o && !ct(o); ) {
      if (se(o) && !On(o))
        return o;
      o = We(o);
    }
    return n;
  }
  let r = po(e, t);
  for (; r && uf(r) && On(r); )
    r = po(r, t);
  return r && ct(r) && On(r) && !or(r) ? n : r || pf(e) || n;
}
const Sf = async function(e) {
  const t = this.getOffsetParent || la, n = this.getDimensions, r = await n(e.floating);
  return {
    reference: Cf(e.reference, await t(e.floating), e.strategy),
    floating: {
      x: 0,
      y: 0,
      width: r.width,
      height: r.height
    }
  };
};
function Rf(e) {
  return le(e).direction === "rtl";
}
const Pf = {
  convertOffsetParentRelativeRectToViewportRelativeRect: gf,
  getDocumentElement: we,
  getClippingRect: kf,
  getOffsetParent: la,
  getElementRects: Sf,
  getClientRects: bf,
  getDimensions: Ef,
  getScale: it,
  isElement: se,
  isRTL: Rf
};
function ca(e, t) {
  return e.x === t.x && e.y === t.y && e.width === t.width && e.height === t.height;
}
function Nf(e, t) {
  let n = null, r;
  const o = we(e);
  function i() {
    var s;
    clearTimeout(r), (s = n) == null || s.disconnect(), n = null;
  }
  function a(s, u) {
    s === void 0 && (s = !1), u === void 0 && (u = 1), i();
    const c = e.getBoundingClientRect(), {
      left: d,
      top: f,
      width: m,
      height: h
    } = c;
    if (s || t(), !m || !h)
      return;
    const b = jt(f), p = jt(o.clientWidth - (d + m)), g = jt(o.clientHeight - (f + h)), w = jt(d), v = {
      rootMargin: -b + "px " + -p + "px " + -g + "px " + -w + "px",
      threshold: te(0, Fe(1, u)) || 1
    };
    let x = !0;
    function E(S) {
      const R = S[0].intersectionRatio;
      if (R !== u) {
        if (!x)
          return a();
        R ? a(!1, R) : r = setTimeout(() => {
          a(!1, 1e-7);
        }, 1e3);
      }
      R === 1 && !ca(c, e.getBoundingClientRect()) && a(), x = !1;
    }
    try {
      n = new IntersectionObserver(E, {
        ...v,
        // Handle <iframe>s
        root: o.ownerDocument
      });
    } catch {
      n = new IntersectionObserver(E, v);
    }
    n.observe(e);
  }
  return a(!0), i;
}
function Of(e, t, n, r) {
  r === void 0 && (r = {});
  const {
    ancestorScroll: o = !0,
    ancestorResize: i = !0,
    elementResize: a = typeof ResizeObserver == "function",
    layoutShift: s = typeof IntersectionObserver == "function",
    animationFrame: u = !1
  } = r, c = ar(e), d = o || i ? [...c ? bt(c) : [], ...t ? bt(t) : []] : [];
  d.forEach((w) => {
    o && w.addEventListener("scroll", n, {
      passive: !0
    }), i && w.addEventListener("resize", n);
  });
  const f = c && s ? Nf(c, n) : null;
  let m = -1, h = null;
  a && (h = new ResizeObserver((w) => {
    let [v] = w;
    v && v.target === c && h && t && (h.unobserve(t), cancelAnimationFrame(m), m = requestAnimationFrame(() => {
      var x;
      (x = h) == null || x.observe(t);
    })), n();
  }), c && !u && h.observe(c), t && h.observe(t));
  let b, p = u ? Xe(e) : null;
  u && g();
  function g() {
    const w = Xe(e);
    p && !ca(p, w) && n(), p = w, b = requestAnimationFrame(g);
  }
  return n(), () => {
    var w;
    d.forEach((v) => {
      o && v.removeEventListener("scroll", n), i && v.removeEventListener("resize", n);
    }), f == null || f(), (w = h) == null || w.disconnect(), h = null, u && cancelAnimationFrame(b);
  };
}
const Af = af, Df = sf, Tf = nf, Mf = cf, Lf = rf, mo = tf, If = lf, zf = (e, t, n) => {
  const r = /* @__PURE__ */ new Map(), o = {
    platform: Pf,
    ...n
  }, i = {
    ...o.platform,
    _c: r
  };
  return ef(e, t, {
    ...o,
    platform: i
  });
};
var _f = typeof document < "u", Ff = function() {
}, Vt = _f ? Ta : Ff;
function tn(e, t) {
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
        if (!tn(e[r], t[r]))
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
      if (!(i === "_owner" && e.$$typeof) && !tn(e[i], t[i]))
        return !1;
    }
    return !0;
  }
  return e !== e && t !== t;
}
function ua(e) {
  return typeof window > "u" ? 1 : (e.ownerDocument.defaultView || window).devicePixelRatio || 1;
}
function ho(e, t) {
  const n = ua(e);
  return Math.round(t * n) / n;
}
function An(e) {
  const t = l.useRef(e);
  return Vt(() => {
    t.current = e;
  }), t;
}
function Wf(e) {
  e === void 0 && (e = {});
  const {
    placement: t = "bottom",
    strategy: n = "absolute",
    middleware: r = [],
    platform: o,
    elements: {
      reference: i,
      floating: a
    } = {},
    transform: s = !0,
    whileElementsMounted: u,
    open: c
  } = e, [d, f] = l.useState({
    x: 0,
    y: 0,
    strategy: n,
    placement: t,
    middlewareData: {},
    isPositioned: !1
  }), [m, h] = l.useState(r);
  tn(m, r) || h(r);
  const [b, p] = l.useState(null), [g, w] = l.useState(null), v = l.useCallback(($) => {
    $ !== R.current && (R.current = $, p($));
  }, []), x = l.useCallback(($) => {
    $ !== T.current && (T.current = $, w($));
  }, []), E = i || b, S = a || g, R = l.useRef(null), T = l.useRef(null), y = l.useRef(d), M = u != null, W = An(u), O = An(o), L = An(c), V = l.useCallback(() => {
    if (!R.current || !T.current)
      return;
    const $ = {
      placement: t,
      strategy: n,
      middleware: m
    };
    O.current && ($.platform = O.current), zf(R.current, T.current, $).then((H) => {
      const k = {
        ...H,
        // The floating element's position may be recomputed while it's closed
        // but still mounted (such as when transitioning out). To ensure
        // `isPositioned` will be `false` initially on the next open, avoid
        // setting it to `true` when `open === false` (must be specified).
        isPositioned: L.current !== !1
      };
      j.current && !tn(y.current, k) && (y.current = k, vt.flushSync(() => {
        f(k);
      }));
    });
  }, [m, t, n, O, L]);
  Vt(() => {
    c === !1 && y.current.isPositioned && (y.current.isPositioned = !1, f(($) => ({
      ...$,
      isPositioned: !1
    })));
  }, [c]);
  const j = l.useRef(!1);
  Vt(() => (j.current = !0, () => {
    j.current = !1;
  }), []), Vt(() => {
    if (E && (R.current = E), S && (T.current = S), E && S) {
      if (W.current)
        return W.current(E, S, V);
      V();
    }
  }, [E, S, V, W, M]);
  const X = l.useMemo(() => ({
    reference: R,
    floating: T,
    setReference: v,
    setFloating: x
  }), [v, x]), I = l.useMemo(() => ({
    reference: E,
    floating: S
  }), [E, S]), B = l.useMemo(() => {
    const $ = {
      position: n,
      left: 0,
      top: 0
    };
    if (!I.floating)
      return $;
    const H = ho(I.floating, d.x), k = ho(I.floating, d.y);
    return s ? {
      ...$,
      transform: "translate(" + H + "px, " + k + "px)",
      ...ua(I.floating) >= 1.5 && {
        willChange: "transform"
      }
    } : {
      position: n,
      left: H,
      top: k
    };
  }, [n, s, I.floating, d.x, d.y]);
  return l.useMemo(() => ({
    ...d,
    update: V,
    refs: X,
    elements: I,
    floatingStyles: B
  }), [d, V, X, I, B]);
}
const jf = (e) => {
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
      return r && t(r) ? r.current != null ? mo({
        element: r.current,
        padding: o
      }).fn(n) : {} : r ? mo({
        element: r,
        padding: o
      }).fn(n) : {};
    }
  };
}, Bf = (e, t) => {
  const n = Af(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, $f = (e, t) => {
  const n = Df(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, Gf = (e, t) => ({
  fn: If(e).fn,
  options: [e, t]
}), Uf = (e, t) => {
  const n = Tf(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, Vf = (e, t) => {
  const n = Mf(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, Hf = (e, t) => {
  const n = Lf(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
}, Xf = (e, t) => {
  const n = jf(e);
  return {
    name: n.name,
    fn: n.fn,
    options: [e, t]
  };
};
var Yf = "Arrow", da = l.forwardRef((e, t) => {
  const { children: n, width: r = 10, height: o = 5, ...i } = e;
  return /* @__PURE__ */ C(
    J.svg,
    {
      ...i,
      ref: t,
      width: r,
      height: o,
      viewBox: "0 0 30 10",
      preserveAspectRatio: "none",
      children: e.asChild ? n : /* @__PURE__ */ C("polygon", { points: "0,0 30,0 15,10" })
    }
  );
});
da.displayName = Yf;
var Kf = da;
function Zf(e) {
  const [t, n] = l.useState(void 0);
  return _e(() => {
    if (e) {
      n({ width: e.offsetWidth, height: e.offsetHeight });
      const r = new ResizeObserver((o) => {
        if (!Array.isArray(o) || !o.length)
          return;
        const i = o[0];
        let a, s;
        if ("borderBoxSize" in i) {
          const u = i.borderBoxSize, c = Array.isArray(u) ? u[0] : u;
          a = c.inlineSize, s = c.blockSize;
        } else
          a = e.offsetWidth, s = e.offsetHeight;
        n({ width: a, height: s });
      });
      return r.observe(e, { box: "border-box" }), () => r.unobserve(e);
    } else
      n(void 0);
  }, [e]), t;
}
var fa = "Popper", [pa, ma] = Zn(fa), [Dp, ha] = pa(fa), ga = "PopperAnchor", ba = l.forwardRef(
  (e, t) => {
    const { __scopePopper: n, virtualRef: r, ...o } = e, i = ha(ga, n), a = l.useRef(null), s = i.onAnchorChange, u = l.useCallback(
      (b) => {
        a.current = b, b && s(b);
      },
      [s]
    ), c = ue(t, u), d = l.useRef(null);
    l.useEffect(() => {
      if (!r)
        return;
      const b = d.current;
      d.current = r.current, b !== d.current && s(d.current);
    });
    const f = i.placementState && lr(i.placementState), m = f == null ? void 0 : f[0], h = f == null ? void 0 : f[1];
    return r ? null : /* @__PURE__ */ C(
      J.div,
      {
        "data-radix-popper-side": m,
        "data-radix-popper-align": h,
        ...o,
        ref: c
      }
    );
  }
);
ba.displayName = ga;
var sr = "PopperContent", [qf, Qf] = pa(sr), va = l.forwardRef(
  (e, t) => {
    var n, r, o, i, a, s;
    const {
      __scopePopper: u,
      side: c = "bottom",
      sideOffset: d = 0,
      align: f = "center",
      alignOffset: m = 0,
      arrowPadding: h = 0,
      avoidCollisions: b = !0,
      collisionBoundary: p = [],
      collisionPadding: g = 0,
      sticky: w = "partial",
      hideWhenDetached: v = !1,
      updatePositionStrategy: x = "optimized",
      onPlaced: E,
      ...S
    } = e, R = ha(sr, u), [T, y] = l.useState(null), M = ue(t, y), [W, O] = l.useState(null), L = Zf(W), V = (L == null ? void 0 : L.width) ?? 0, j = (L == null ? void 0 : L.height) ?? 0, X = c + (f !== "center" ? "-" + f : ""), I = typeof g == "number" ? g : { top: 0, right: 0, bottom: 0, left: 0, ...g }, B = Array.isArray(p) ? p : [p], $ = B.length > 0, H = {
      padding: I,
      boundary: B.filter(ep),
      // with `strategy: 'fixed'`, this is the only way to get it to respect boundaries
      altBoundary: $
    }, { refs: k, floatingStyles: Ae, placement: xe, isPositioned: fe, middlewareData: Z } = Wf({
      // default to `fixed` strategy so users don't have to pick and we also avoid focus scroll issues
      strategy: "fixed",
      placement: X,
      whileElementsMounted: (...ee) => Of(...ee, {
        animationFrame: x === "always"
      }),
      elements: {
        reference: R.anchor
      },
      middleware: [
        Bf({ mainAxis: d + j, alignmentAxis: m }),
        b && $f({
          mainAxis: !0,
          crossAxis: !1,
          limiter: w === "partial" ? Gf() : void 0,
          ...H
        }),
        b && Uf({ ...H }),
        Vf({
          ...H,
          apply: ({ elements: ee, rects: Ra, availableWidth: Pa, availableHeight: Na }) => {
            const { width: Oa, height: Aa } = Ra.reference, xt = ee.floating.style;
            xt.setProperty("--radix-popper-available-width", `${Pa}px`), xt.setProperty("--radix-popper-available-height", `${Na}px`), xt.setProperty("--radix-popper-anchor-width", `${Oa}px`), xt.setProperty("--radix-popper-anchor-height", `${Aa}px`);
          }
        }),
        W && Xf({ element: W, padding: h }),
        tp({ arrowWidth: V, arrowHeight: j }),
        v && Hf({
          strategy: "referenceHidden",
          ...H,
          // `hide` detects whether the anchor (reference) is clipped, so when
          // no explicit `collisionBoundary` is set we fall back to Floating
          // UI's default clipping ancestors (e.g. a scrollable menu). This
          // lets an occluded submenu hide once its anchor scrolls out of view
          // (#3237). The collision/size middlewares deliberately keep the
          // viewport-based default to avoid clamping content rendered inside
          // transformed or overflow-clipping portal containers.
          boundary: $ ? H.boundary : void 0
        })
      ]
    }), G = R.setPlacementState;
    _e(() => (G(xe), () => {
      G(void 0);
    }), [xe, G]);
    const [U, re] = lr(xe), ke = gt(E);
    _e(() => {
      fe && (ke == null || ke());
    }, [fe, ke]);
    const z = (n = Z.arrow) == null ? void 0 : n.x, $e = (r = Z.arrow) == null ? void 0 : r.y, oe = ((o = Z.arrow) == null ? void 0 : o.centerOffset) !== 0, [ie, Ee] = l.useState();
    return _e(() => {
      T && Ee(window.getComputedStyle(T).zIndex);
    }, [T]), /* @__PURE__ */ C(
      "div",
      {
        ref: k.setFloating,
        "data-radix-popper-content-wrapper": "",
        style: {
          ...Ae,
          transform: fe ? Ae.transform : "translate(0, -200%)",
          // keep off the page when measuring
          minWidth: "max-content",
          zIndex: ie,
          "--radix-popper-transform-origin": [
            (i = Z.transformOrigin) == null ? void 0 : i.x,
            (a = Z.transformOrigin) == null ? void 0 : a.y
          ].join(" "),
          // hide the content if using the hide middleware and should be hidden
          // set visibility to hidden and disable pointer events so the UI behaves
          // as if the PopperContent isn't there at all
          ...((s = Z.hide) == null ? void 0 : s.referenceHidden) && {
            visibility: "hidden",
            pointerEvents: "none"
          }
        },
        dir: e.dir,
        children: /* @__PURE__ */ C(
          qf,
          {
            scope: u,
            placedSide: U,
            placedAlign: re,
            onArrowChange: O,
            arrowX: z,
            arrowY: $e,
            shouldHideArrow: oe,
            children: /* @__PURE__ */ C(
              J.div,
              {
                "data-side": U,
                "data-align": re,
                ...S,
                ref: M,
                style: {
                  ...S.style,
                  // if the PopperContent hasn't been placed yet (not all measurements done)
                  // we prevent animations so that users's animation don't kick in too early referring wrong sides
                  animation: fe ? void 0 : "none"
                }
              }
            )
          }
        )
      }
    );
  }
);
va.displayName = sr;
var ya = "PopperArrow", Jf = {
  top: "bottom",
  right: "left",
  bottom: "top",
  left: "right"
}, wa = l.forwardRef(function(e, t) {
  const { __scopePopper: n, ...r } = e, o = Qf(ya, n), i = Jf[o.placedSide];
  return (
    // we have to use an extra wrapper because `ResizeObserver` (used by `useSize`)
    // doesn't report size as we'd expect on SVG elements.
    // it reports their bounding box which is effectively the largest path inside the SVG.
    /* @__PURE__ */ C(
      "span",
      {
        ref: o.onArrowChange,
        style: {
          position: "absolute",
          left: o.arrowX,
          top: o.arrowY,
          [i]: 0,
          transformOrigin: {
            top: "",
            right: "0 0",
            bottom: "center 0",
            left: "100% 0"
          }[o.placedSide],
          transform: {
            top: "translateY(100%)",
            right: "translateY(50%) rotate(90deg) translateX(-50%)",
            bottom: "rotate(180deg)",
            left: "translateY(50%) rotate(-90deg) translateX(50%)"
          }[o.placedSide],
          visibility: o.shouldHideArrow ? "hidden" : void 0
        },
        children: /* @__PURE__ */ C(
          Kf,
          {
            ...r,
            ref: t,
            style: {
              ...r.style,
              // ensures the element can be measured correctly (mostly for if SVG)
              display: "block"
            }
          }
        )
      }
    )
  );
});
wa.displayName = ya;
function ep(e) {
  return e !== null;
}
var tp = (e) => ({
  name: "transformOrigin",
  options: e,
  fn(t) {
    var n, r, o;
    const { placement: i, rects: a, middlewareData: s } = t, u = ((n = s.arrow) == null ? void 0 : n.centerOffset) !== 0, c = u ? 0 : e.arrowWidth, d = u ? 0 : e.arrowHeight, [f, m] = lr(i), h = { start: "0%", center: "50%", end: "100%" }[m], b = (((r = s.arrow) == null ? void 0 : r.x) ?? 0) + c / 2, p = (((o = s.arrow) == null ? void 0 : o.y) ?? 0) + d / 2;
    let g = "", w = "";
    return f === "bottom" ? (g = u ? h : `${b}px`, w = `${-d}px`) : f === "top" ? (g = u ? h : `${b}px`, w = `${a.floating.height + d}px`) : f === "right" ? (g = `${-d}px`, w = u ? h : `${p}px`) : f === "left" && (g = `${a.floating.width + d}px`, w = u ? h : `${p}px`), { data: { x: g, y: w } };
  }
});
function lr(e) {
  const [t, n = "center"] = e.split("-");
  return [t, n];
}
var np = ba, rp = va, op = wa, ip = Object.freeze({
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
}), ap = "VisuallyHidden", xa = l.forwardRef(
  (e, t) => /* @__PURE__ */ C(
    J.span,
    {
      ...e,
      ref: t,
      style: { ...ip, ...e.style }
    }
  )
);
xa.displayName = ap;
var sp = xa, [dn] = Zn("Tooltip", [
  ma
]), cr = ma(), lp = "TooltipProvider", go = "tooltip.open", [Tp, ka] = dn(lp), Ea = "Tooltip", [Mp, fn] = dn(Ea), Bn = "TooltipTrigger", cp = l.forwardRef(
  (e, t) => {
    const { __scopeTooltip: n, ...r } = e, o = fn(Bn, n), i = ka(Bn, n), a = cr(n), s = l.useRef(null), u = ue(t, s, o.onTriggerChange), c = l.useRef(!1), d = l.useRef(!1), f = l.useCallback(() => c.current = !1, []);
    return l.useEffect(() => () => document.removeEventListener("pointerup", f), [f]), /* @__PURE__ */ C(np, { asChild: !0, ...a, children: /* @__PURE__ */ C(
      J.button,
      {
        "aria-describedby": o.open ? o.contentId : void 0,
        "data-state": o.stateAttribute,
        ...r,
        ref: u,
        onPointerMove: q(e.onPointerMove, (m) => {
          m.pointerType !== "touch" && !d.current && !i.isPointerInTransitRef.current && (o.onTriggerEnter(), d.current = !0);
        }),
        onPointerLeave: q(e.onPointerLeave, () => {
          o.onTriggerLeave(), d.current = !1;
        }),
        onPointerDown: q(e.onPointerDown, () => {
          o.open && o.onClose(), c.current = !0, document.addEventListener("pointerup", f, { once: !0 });
        }),
        onFocus: q(e.onFocus, () => {
          c.current || o.onOpen();
        }),
        onBlur: q(e.onBlur, o.onClose),
        onClick: q(e.onClick, o.onClose)
      }
    ) });
  }
);
cp.displayName = Bn;
var up = "TooltipPortal", [Lp, dp] = dn(up, {
  forceMount: void 0
}), ut = "TooltipContent", fp = l.forwardRef(
  (e, t) => {
    const n = dp(ut, e.__scopeTooltip), { forceMount: r = n.forceMount, side: o = "top", ...i } = e, a = fn(ut, e.__scopeTooltip);
    return /* @__PURE__ */ C(yt, { present: r || a.open, children: a.disableHoverableContent ? /* @__PURE__ */ C(Ca, { side: o, ...i, ref: t }) : /* @__PURE__ */ C(pp, { side: o, ...i, ref: t }) });
  }
), pp = l.forwardRef((e, t) => {
  const n = fn(ut, e.__scopeTooltip), r = ka(ut, e.__scopeTooltip), o = l.useRef(null), i = ue(t, o), [a, s] = l.useState(null), { trigger: u, onClose: c } = n, d = o.current, { onPointerInTransitChange: f } = r, m = l.useCallback(() => {
    s(null), f(!1);
  }, [f]), h = l.useCallback(
    (b, p) => {
      const g = b.currentTarget, w = { x: b.clientX, y: b.clientY }, v = vp(w, g.getBoundingClientRect()), x = yp(w, v), E = wp(p.getBoundingClientRect()), S = kp([...x, ...E]);
      s(S), f(!0);
    },
    [f]
  );
  return l.useEffect(() => () => m(), [m]), l.useEffect(() => {
    if (u && d) {
      const b = (g) => h(g, d), p = (g) => h(g, u);
      return u.addEventListener("pointerleave", b), d.addEventListener("pointerleave", p), () => {
        u.removeEventListener("pointerleave", b), d.removeEventListener("pointerleave", p);
      };
    }
  }, [u, d, h, m]), l.useEffect(() => {
    if (a) {
      const b = (p) => {
        const g = p.target, w = { x: p.clientX, y: p.clientY }, v = (u == null ? void 0 : u.contains(g)) || (d == null ? void 0 : d.contains(g)), x = !xp(w, a);
        v ? m() : x && (m(), c());
      };
      return document.addEventListener("pointermove", b), () => document.removeEventListener("pointermove", b);
    }
  }, [u, d, a, c, m]), /* @__PURE__ */ C(Ca, { ...e, ref: i });
}), [mp, hp] = dn(Ea, { isInside: !1 }), gp = /* @__PURE__ */ vc("TooltipContent"), Ca = l.forwardRef(
  (e, t) => {
    const {
      __scopeTooltip: n,
      children: r,
      "aria-label": o,
      onEscapeKeyDown: i,
      onPointerDownOutside: a,
      ...s
    } = e, u = fn(ut, n), c = cr(n), { onClose: d } = u;
    return l.useEffect(() => (document.addEventListener(go, d), () => document.removeEventListener(go, d)), [d]), l.useEffect(() => {
      if (u.trigger) {
        const f = (m) => {
          m.target instanceof Node && m.target.contains(u.trigger) && d();
        };
        return window.addEventListener("scroll", f, { capture: !0 }), () => window.removeEventListener("scroll", f, { capture: !0 });
      }
    }, [u.trigger, d]), /* @__PURE__ */ C(
      Qn,
      {
        asChild: !0,
        disableOutsidePointerEvents: !1,
        onEscapeKeyDown: i,
        onPointerDownOutside: a,
        onFocusOutside: (f) => f.preventDefault(),
        onDismiss: d,
        children: /* @__PURE__ */ Q(
          rp,
          {
            "data-state": u.stateAttribute,
            ...c,
            ...s,
            ref: t,
            style: {
              ...s.style,
              "--radix-tooltip-content-transform-origin": "var(--radix-popper-transform-origin)",
              "--radix-tooltip-content-available-width": "var(--radix-popper-available-width)",
              "--radix-tooltip-content-available-height": "var(--radix-popper-available-height)",
              "--radix-tooltip-trigger-width": "var(--radix-popper-anchor-width)",
              "--radix-tooltip-trigger-height": "var(--radix-popper-anchor-height)"
            },
            children: [
              /* @__PURE__ */ C(gp, { children: r }),
              /* @__PURE__ */ C(mp, { scope: n, isInside: !0, children: /* @__PURE__ */ C(sp, { id: u.contentId, role: "tooltip", children: o || r }) })
            ]
          }
        )
      }
    );
  }
);
fp.displayName = ut;
var Sa = "TooltipArrow", bp = l.forwardRef(
  (e, t) => {
    const { __scopeTooltip: n, ...r } = e, o = cr(n);
    return hp(
      Sa,
      n
    ).isInside ? null : /* @__PURE__ */ C(op, { ...o, ...r, ref: t });
  }
);
bp.displayName = Sa;
function vp(e, t) {
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
function yp(e, t, n = 5) {
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
function wp(e) {
  const { top: t, right: n, bottom: r, left: o } = e;
  return [
    { x: o, y: t },
    { x: n, y: t },
    { x: n, y: r },
    { x: o, y: r }
  ];
}
function xp(e, t) {
  const { x: n, y: r } = e;
  let o = !1;
  for (let i = 0, a = t.length - 1; i < t.length; a = i++) {
    const s = t[i], u = t[a], c = s.x, d = s.y, f = u.x, m = u.y;
    d > r != m > r && n < (f - c) * (r - d) / (m - d) + c && (o = !o);
  }
  return o;
}
function kp(e) {
  const t = e.slice();
  return t.sort((n, r) => n.x < r.x ? -1 : n.x > r.x ? 1 : n.y < r.y ? -1 : n.y > r.y ? 1 : 0), Ep(t);
}
function Ep(e) {
  if (e.length <= 1) return e.slice();
  const t = [];
  for (let r = 0; r < e.length; r++) {
    const o = e[r];
    for (; t.length >= 2; ) {
      const i = t[t.length - 1], a = t[t.length - 2];
      if ((i.x - a.x) * (o.y - a.y) >= (i.y - a.y) * (o.x - a.x)) t.pop();
      else break;
    }
    t.push(o);
  }
  t.pop();
  const n = [];
  for (let r = e.length - 1; r >= 0; r--) {
    const o = e[r];
    for (; n.length >= 2; ) {
      const i = n[n.length - 1], a = n[n.length - 2];
      if ((i.x - a.x) * (o.y - a.y) >= (i.y - a.y) * (o.x - a.x)) n.pop();
      else break;
    }
    n.push(o);
  }
  return n.pop(), t.length === 1 && n.length === 1 && t[0].x === n[0].x && t[0].y === n[0].y ? t : t.concat(n);
}
l.createContext(null);
function Ip({
  items: e,
  active: t,
  onSelect: n,
  badge: r,
  className: o,
  "aria-label": i = "section navigation"
}) {
  const a = hc(e);
  return /* @__PURE__ */ C(
    "nav",
    {
      "aria-label": i,
      className: He("nav-rail flex min-w-0 flex-col gap-2 text-nr-fg", o),
      children: a.map((s, u) => /* @__PURE__ */ Q("div", { className: "flex flex-col gap-1", children: [
        s.label && /* @__PURE__ */ C("div", { className: "px-2 text-xs font-medium text-nr-muted", children: s.label }),
        s.items.map((c) => {
          const d = t === c.id, f = c.icon, m = r == null ? void 0 : r(c.id);
          return /* @__PURE__ */ Q(
            "button",
            {
              type: "button",
              role: "tab",
              "aria-label": c.label,
              "aria-current": d ? "page" : void 0,
              "aria-selected": d,
              onClick: () => n(c.id),
              className: He(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none ring-nr-accent transition-colors focus-visible:ring-2",
                "[&>svg]:h-4 [&>svg]:w-4 [&>svg]:shrink-0",
                d ? "bg-nr-bg font-medium text-nr-fg" : "text-nr-muted hover:bg-nr-bg hover:text-nr-fg"
              ),
              children: [
                f && /* @__PURE__ */ C(f, {}),
                /* @__PURE__ */ C("span", { className: "min-w-0 flex-1 truncate", children: c.label }),
                m ? /* @__PURE__ */ C("span", { className: "rounded-full bg-nr-accent/15 px-1.5 text-[10px] text-nr-accent", children: m }) : null
              ]
            },
            c.id
          );
        })
      ] }, s.label ?? `__default-${u}`))
    }
  );
}
export {
  Op as KV,
  Ip as NavMenu,
  Rp as Panel,
  Np as PropTable,
  pc as ResizeHandle,
  Pp as Section,
  mc as useResizable
};
