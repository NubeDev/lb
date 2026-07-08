var jg = Object.defineProperty;
var $g = (e, t, r) => t in e ? jg(e, t, { enumerable: !0, configurable: !0, writable: !0, value: r }) : e[t] = r;
var mi = (e, t, r) => $g(e, typeof t != "symbol" ? t + "" : t, r);
import { jsxs as it, jsx as me, Fragment as Rg } from "react/jsx-runtime";
import * as A from "react";
import sn, { isValidElement as Pt, forwardRef as ze, createContext as Xe, useContext as ct, useMemo as sr, useState as _e, useRef as Q, useCallback as ie, useEffect as ve, useImperativeHandle as wv, useLayoutEffect as qe, cloneElement as Ca, createElement as Av, Children as Lg, memo as Bu, PureComponent as zg, StrictMode as Bg } from "react";
import Fg, { createPortal as Ov } from "react-dom";
function Wg(e) {
  return e && e.__esModule && Object.prototype.hasOwnProperty.call(e, "default") ? e.default : e;
}
var Yo, yi = Fg;
if (process.env.NODE_ENV === "production")
  Yo = yi.createRoot, yi.hydrateRoot;
else {
  var gc = yi.__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED;
  Yo = function(e, t) {
    gc.usingClientEntryPoint = !0;
    try {
      return yi.createRoot(e, t);
    } finally {
      gc.usingClientEntryPoint = !1;
    }
  };
}
function Ev(e) {
  var t, r, n = "";
  if (typeof e == "string" || typeof e == "number") n += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var i = e.length;
    for (t = 0; t < i; t++) e[t] && (r = Ev(e[t])) && (n && (n += " "), n += r);
  } else for (r in e) e[r] && (n && (n += " "), n += r);
  return n;
}
function ce() {
  for (var e, t, r = 0, n = "", i = arguments.length; r < i; r++) (e = arguments[r]) && (t = Ev(e)) && (n && (n += " "), n += t);
  return n;
}
var Ug = ["dangerouslySetInnerHTML", "onCopy", "onCopyCapture", "onCut", "onCutCapture", "onPaste", "onPasteCapture", "onCompositionEnd", "onCompositionEndCapture", "onCompositionStart", "onCompositionStartCapture", "onCompositionUpdate", "onCompositionUpdateCapture", "onFocus", "onFocusCapture", "onBlur", "onBlurCapture", "onChange", "onChangeCapture", "onBeforeInput", "onBeforeInputCapture", "onInput", "onInputCapture", "onReset", "onResetCapture", "onSubmit", "onSubmitCapture", "onInvalid", "onInvalidCapture", "onLoad", "onLoadCapture", "onError", "onErrorCapture", "onKeyDown", "onKeyDownCapture", "onKeyPress", "onKeyPressCapture", "onKeyUp", "onKeyUpCapture", "onAbort", "onAbortCapture", "onCanPlay", "onCanPlayCapture", "onCanPlayThrough", "onCanPlayThroughCapture", "onDurationChange", "onDurationChangeCapture", "onEmptied", "onEmptiedCapture", "onEncrypted", "onEncryptedCapture", "onEnded", "onEndedCapture", "onLoadedData", "onLoadedDataCapture", "onLoadedMetadata", "onLoadedMetadataCapture", "onLoadStart", "onLoadStartCapture", "onPause", "onPauseCapture", "onPlay", "onPlayCapture", "onPlaying", "onPlayingCapture", "onProgress", "onProgressCapture", "onRateChange", "onRateChangeCapture", "onSeeked", "onSeekedCapture", "onSeeking", "onSeekingCapture", "onStalled", "onStalledCapture", "onSuspend", "onSuspendCapture", "onTimeUpdate", "onTimeUpdateCapture", "onVolumeChange", "onVolumeChangeCapture", "onWaiting", "onWaitingCapture", "onAuxClick", "onAuxClickCapture", "onClick", "onClickCapture", "onContextMenu", "onContextMenuCapture", "onDoubleClick", "onDoubleClickCapture", "onDrag", "onDragCapture", "onDragEnd", "onDragEndCapture", "onDragEnter", "onDragEnterCapture", "onDragExit", "onDragExitCapture", "onDragLeave", "onDragLeaveCapture", "onDragOver", "onDragOverCapture", "onDragStart", "onDragStartCapture", "onDrop", "onDropCapture", "onMouseDown", "onMouseDownCapture", "onMouseEnter", "onMouseLeave", "onMouseMove", "onMouseMoveCapture", "onMouseOut", "onMouseOutCapture", "onMouseOver", "onMouseOverCapture", "onMouseUp", "onMouseUpCapture", "onSelect", "onSelectCapture", "onTouchCancel", "onTouchCancelCapture", "onTouchEnd", "onTouchEndCapture", "onTouchMove", "onTouchMoveCapture", "onTouchStart", "onTouchStartCapture", "onPointerDown", "onPointerDownCapture", "onPointerMove", "onPointerMoveCapture", "onPointerUp", "onPointerUpCapture", "onPointerCancel", "onPointerCancelCapture", "onPointerEnter", "onPointerEnterCapture", "onPointerLeave", "onPointerLeaveCapture", "onPointerOver", "onPointerOverCapture", "onPointerOut", "onPointerOutCapture", "onGotPointerCapture", "onGotPointerCaptureCapture", "onLostPointerCapture", "onLostPointerCaptureCapture", "onScroll", "onScrollCapture", "onWheel", "onWheelCapture", "onAnimationStart", "onAnimationStartCapture", "onAnimationEnd", "onAnimationEndCapture", "onAnimationIteration", "onAnimationIterationCapture", "onTransitionEnd", "onTransitionEndCapture"];
function Fu(e) {
  if (typeof e != "string")
    return !1;
  var t = Ug;
  return t.includes(e);
}
var Vg = [
  "aria-activedescendant",
  "aria-atomic",
  "aria-autocomplete",
  "aria-busy",
  "aria-checked",
  "aria-colcount",
  "aria-colindex",
  "aria-colspan",
  "aria-controls",
  "aria-current",
  "aria-describedby",
  "aria-details",
  "aria-disabled",
  "aria-errormessage",
  "aria-expanded",
  "aria-flowto",
  "aria-haspopup",
  "aria-hidden",
  "aria-invalid",
  "aria-keyshortcuts",
  "aria-label",
  "aria-labelledby",
  "aria-level",
  "aria-live",
  "aria-modal",
  "aria-multiline",
  "aria-multiselectable",
  "aria-orientation",
  "aria-owns",
  "aria-placeholder",
  "aria-posinset",
  "aria-pressed",
  "aria-readonly",
  "aria-relevant",
  "aria-required",
  "aria-roledescription",
  "aria-rowcount",
  "aria-rowindex",
  "aria-rowspan",
  "aria-selected",
  "aria-setsize",
  "aria-sort",
  "aria-valuemax",
  "aria-valuemin",
  "aria-valuenow",
  "aria-valuetext",
  "className",
  "color",
  "height",
  "id",
  "lang",
  "max",
  "media",
  "method",
  "min",
  "name",
  "style",
  /*
   * removed 'type' SVGElementPropKey because we do not currently use any SVG elements
   * that can use it, and it conflicts with the recharts prop 'type'
   * https://github.com/recharts/recharts/pull/3327
   * https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/type
   */
  // 'type',
  "target",
  "width",
  "role",
  "tabIndex",
  "accentHeight",
  "accumulate",
  "additive",
  "alignmentBaseline",
  "allowReorder",
  "alphabetic",
  "amplitude",
  "arabicForm",
  "ascent",
  "attributeName",
  "attributeType",
  "autoReverse",
  "azimuth",
  "baseFrequency",
  "baselineShift",
  "baseProfile",
  "bbox",
  "begin",
  "bias",
  "by",
  "calcMode",
  "capHeight",
  "clip",
  "clipPath",
  "clipPathUnits",
  "clipRule",
  "colorInterpolation",
  "colorInterpolationFilters",
  "colorProfile",
  "colorRendering",
  "contentScriptType",
  "contentStyleType",
  "cursor",
  "cx",
  "cy",
  "d",
  "decelerate",
  "descent",
  "diffuseConstant",
  "direction",
  "display",
  "divisor",
  "dominantBaseline",
  "dur",
  "dx",
  "dy",
  "edgeMode",
  "elevation",
  "enableBackground",
  "end",
  "exponent",
  "externalResourcesRequired",
  "fill",
  "fillOpacity",
  "fillRule",
  "filter",
  "filterRes",
  "filterUnits",
  "floodColor",
  "floodOpacity",
  "focusable",
  "fontFamily",
  "fontSize",
  "fontSizeAdjust",
  "fontStretch",
  "fontStyle",
  "fontVariant",
  "fontWeight",
  "format",
  "from",
  "fx",
  "fy",
  "g1",
  "g2",
  "glyphName",
  "glyphOrientationHorizontal",
  "glyphOrientationVertical",
  "glyphRef",
  "gradientTransform",
  "gradientUnits",
  "hanging",
  "horizAdvX",
  "horizOriginX",
  "href",
  "ideographic",
  "imageRendering",
  "in2",
  "in",
  "intercept",
  "k1",
  "k2",
  "k3",
  "k4",
  "k",
  "kernelMatrix",
  "kernelUnitLength",
  "kerning",
  "keyPoints",
  "keySplines",
  "keyTimes",
  "lengthAdjust",
  "letterSpacing",
  "lightingColor",
  "limitingConeAngle",
  "local",
  "markerEnd",
  "markerHeight",
  "markerMid",
  "markerStart",
  "markerUnits",
  "markerWidth",
  "mask",
  "maskContentUnits",
  "maskUnits",
  "mathematical",
  "mode",
  "numOctaves",
  "offset",
  "opacity",
  "operator",
  "order",
  "orient",
  "orientation",
  "origin",
  "overflow",
  "overlinePosition",
  "overlineThickness",
  "paintOrder",
  "panose1",
  "pathLength",
  "patternContentUnits",
  "patternTransform",
  "patternUnits",
  "pointerEvents",
  "pointsAtX",
  "pointsAtY",
  "pointsAtZ",
  "preserveAlpha",
  "preserveAspectRatio",
  "primitiveUnits",
  "r",
  "radius",
  "refX",
  "refY",
  "renderingIntent",
  "repeatCount",
  "repeatDur",
  "requiredExtensions",
  "requiredFeatures",
  "restart",
  "result",
  "rotate",
  "rx",
  "ry",
  "seed",
  "shapeRendering",
  "slope",
  "spacing",
  "specularConstant",
  "specularExponent",
  "speed",
  "spreadMethod",
  "startOffset",
  "stdDeviation",
  "stemh",
  "stemv",
  "stitchTiles",
  "stopColor",
  "stopOpacity",
  "strikethroughPosition",
  "strikethroughThickness",
  "string",
  "stroke",
  "strokeDasharray",
  "strokeDashoffset",
  "strokeLinecap",
  "strokeLinejoin",
  "strokeMiterlimit",
  "strokeOpacity",
  "strokeWidth",
  "surfaceScale",
  "systemLanguage",
  "tableValues",
  "targetX",
  "targetY",
  "textAnchor",
  "textDecoration",
  "textLength",
  "textRendering",
  "to",
  "transform",
  "u1",
  "u2",
  "underlinePosition",
  "underlineThickness",
  "unicode",
  "unicodeBidi",
  "unicodeRange",
  "unitsPerEm",
  "vAlphabetic",
  "values",
  "vectorEffect",
  "version",
  "vertAdvY",
  "vertOriginX",
  "vertOriginY",
  "vHanging",
  "vIdeographic",
  "viewTarget",
  "visibility",
  "vMathematical",
  "widths",
  "wordSpacing",
  "writingMode",
  "x1",
  "x2",
  "x",
  "xChannelSelector",
  "xHeight",
  "xlinkActuate",
  "xlinkArcrole",
  "xlinkHref",
  "xlinkRole",
  "xlinkShow",
  "xlinkTitle",
  "xlinkType",
  "xmlBase",
  "xmlLang",
  "xmlns",
  "xmlnsXlink",
  "xmlSpace",
  "y1",
  "y2",
  "y",
  "yChannelSelector",
  "z",
  "zoomAndPan",
  "ref",
  "key",
  "angle"
], Kg = new Set(Vg);
function Sv(e) {
  return typeof e != "string" ? !1 : Kg.has(e);
}
function _v(e) {
  return typeof e == "string" && e.startsWith("data-");
}
function Ft(e) {
  if (typeof e != "object" || e === null)
    return {};
  var t = {};
  for (var r in e)
    Object.prototype.hasOwnProperty.call(e, r) && (Sv(r) || _v(r)) && (t[r] = e[r]);
  return t;
}
function Wu(e) {
  if (e == null)
    return null;
  if (/* @__PURE__ */ Pt(e) && typeof e.props == "object" && e.props !== null) {
    var t = e.props;
    return Ft(t);
  }
  return typeof e == "object" && !Array.isArray(e) ? Ft(e) : null;
}
function Wt(e) {
  var t = {};
  for (var r in e)
    Object.prototype.hasOwnProperty.call(e, r) && (Sv(r) || _v(r) || Fu(r)) && (t[r] = e[r]);
  return t;
}
var Hg = ["children", "width", "height", "viewBox", "className", "style", "title", "desc"];
function Go() {
  return Go = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Go.apply(null, arguments);
}
function Yg(e, t) {
  if (e == null) return {};
  var r, n, i = Gg(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function Gg(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var Pv = /* @__PURE__ */ ze((e, t) => {
  var r = e.children, n = e.width, i = e.height, a = e.viewBox, o = e.className, u = e.style, l = e.title, c = e.desc, s = Yg(e, Hg), f = a || {
    width: n,
    height: i,
    x: 0,
    y: 0
  }, d = ce("recharts-surface", o);
  return /* @__PURE__ */ A.createElement("svg", Go({}, Wt(s), {
    className: d,
    width: n,
    height: i,
    style: u,
    viewBox: "".concat(f.x, " ").concat(f.y, " ").concat(f.width, " ").concat(f.height),
    ref: t
  }), /* @__PURE__ */ A.createElement("title", null, l), /* @__PURE__ */ A.createElement("desc", null, c), r);
}), qg = ["children", "className"];
function qo() {
  return qo = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, qo.apply(null, arguments);
}
function Xg(e, t) {
  if (e == null) return {};
  var r, n, i = Zg(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function Zg(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var Ct = /* @__PURE__ */ A.forwardRef((e, t) => {
  var r = e.children, n = e.className, i = Xg(e, qg), a = ce("recharts-layer", n);
  return /* @__PURE__ */ A.createElement("g", qo({
    className: a
  }, Wt(i), {
    ref: t
  }), r);
}), Qg = /* @__PURE__ */ Xe(null);
function fe(e) {
  return function() {
    return e;
  };
}
const Xo = Math.PI, Zo = 2 * Xo, br = 1e-6, Jg = Zo - br;
function Iv(e) {
  this._ += e[0];
  for (let t = 1, r = e.length; t < r; ++t)
    this._ += arguments[t] + e[t];
}
function e0(e) {
  let t = Math.floor(e);
  if (!(t >= 0)) throw new Error(`invalid digits: ${e}`);
  if (t > 15) return Iv;
  const r = 10 ** t;
  return function(n) {
    this._ += n[0];
    for (let i = 1, a = n.length; i < a; ++i)
      this._ += Math.round(arguments[i] * r) / r + n[i];
  };
}
class t0 {
  constructor(t) {
    this._x0 = this._y0 = // start of current subpath
    this._x1 = this._y1 = null, this._ = "", this._append = t == null ? Iv : e0(t);
  }
  moveTo(t, r) {
    this._append`M${this._x0 = this._x1 = +t},${this._y0 = this._y1 = +r}`;
  }
  closePath() {
    this._x1 !== null && (this._x1 = this._x0, this._y1 = this._y0, this._append`Z`);
  }
  lineTo(t, r) {
    this._append`L${this._x1 = +t},${this._y1 = +r}`;
  }
  quadraticCurveTo(t, r, n, i) {
    this._append`Q${+t},${+r},${this._x1 = +n},${this._y1 = +i}`;
  }
  bezierCurveTo(t, r, n, i, a, o) {
    this._append`C${+t},${+r},${+n},${+i},${this._x1 = +a},${this._y1 = +o}`;
  }
  arcTo(t, r, n, i, a) {
    if (t = +t, r = +r, n = +n, i = +i, a = +a, a < 0) throw new Error(`negative radius: ${a}`);
    let o = this._x1, u = this._y1, l = n - t, c = i - r, s = o - t, f = u - r, d = s * s + f * f;
    if (this._x1 === null)
      this._append`M${this._x1 = t},${this._y1 = r}`;
    else if (d > br) if (!(Math.abs(f * l - c * s) > br) || !a)
      this._append`L${this._x1 = t},${this._y1 = r}`;
    else {
      let h = n - o, p = i - u, v = l * l + c * c, m = h * h + p * p, y = Math.sqrt(v), b = Math.sqrt(d), x = a * Math.tan((Xo - Math.acos((v + d - m) / (2 * y * b))) / 2), w = x / b, O = x / y;
      Math.abs(w - 1) > br && this._append`L${t + w * s},${r + w * f}`, this._append`A${a},${a},0,0,${+(f * h > s * p)},${this._x1 = t + O * l},${this._y1 = r + O * c}`;
    }
  }
  arc(t, r, n, i, a, o) {
    if (t = +t, r = +r, n = +n, o = !!o, n < 0) throw new Error(`negative radius: ${n}`);
    let u = n * Math.cos(i), l = n * Math.sin(i), c = t + u, s = r + l, f = 1 ^ o, d = o ? i - a : a - i;
    this._x1 === null ? this._append`M${c},${s}` : (Math.abs(this._x1 - c) > br || Math.abs(this._y1 - s) > br) && this._append`L${c},${s}`, n && (d < 0 && (d = d % Zo + Zo), d > Jg ? this._append`A${n},${n},0,1,${f},${t - u},${r - l}A${n},${n},0,1,${f},${this._x1 = c},${this._y1 = s}` : d > br && this._append`A${n},${n},0,${+(d >= Xo)},${f},${this._x1 = t + n * Math.cos(a)},${this._y1 = r + n * Math.sin(a)}`);
  }
  rect(t, r, n, i) {
    this._append`M${this._x0 = this._x1 = +t},${this._y0 = this._y1 = +r}h${n = +n}v${+i}h${-n}Z`;
  }
  toString() {
    return this._;
  }
}
function kv(e) {
  let t = 3;
  return e.digits = function(r) {
    if (!arguments.length) return t;
    if (r == null)
      t = null;
    else {
      const n = Math.floor(r);
      if (!(n >= 0)) throw new RangeError(`invalid digits: ${r}`);
      t = n;
    }
    return e;
  }, () => new t0(t);
}
function Uu(e) {
  return typeof e == "object" && "length" in e ? e : Array.from(e);
}
function Cv(e) {
  this._context = e;
}
Cv.prototype = {
  areaStart: function() {
    this._line = 0;
  },
  areaEnd: function() {
    this._line = NaN;
  },
  lineStart: function() {
    this._point = 0;
  },
  lineEnd: function() {
    (this._line || this._line !== 0 && this._point === 1) && this._context.closePath(), this._line = 1 - this._line;
  },
  point: function(e, t) {
    switch (e = +e, t = +t, this._point) {
      case 0:
        this._point = 1, this._line ? this._context.lineTo(e, t) : this._context.moveTo(e, t);
        break;
      case 1:
        this._point = 2;
      default:
        this._context.lineTo(e, t);
        break;
    }
  }
};
function Ta(e) {
  return new Cv(e);
}
function Tv(e) {
  return e[0];
}
function Mv(e) {
  return e[1];
}
function Dv(e, t) {
  var r = fe(!0), n = null, i = Ta, a = null, o = kv(u);
  e = typeof e == "function" ? e : e === void 0 ? Tv : fe(e), t = typeof t == "function" ? t : t === void 0 ? Mv : fe(t);
  function u(l) {
    var c, s = (l = Uu(l)).length, f, d = !1, h;
    for (n == null && (a = i(h = o())), c = 0; c <= s; ++c)
      !(c < s && r(f = l[c], c, l)) === d && ((d = !d) ? a.lineStart() : a.lineEnd()), d && a.point(+e(f, c, l), +t(f, c, l));
    if (h) return a = null, h + "" || null;
  }
  return u.x = function(l) {
    return arguments.length ? (e = typeof l == "function" ? l : fe(+l), u) : e;
  }, u.y = function(l) {
    return arguments.length ? (t = typeof l == "function" ? l : fe(+l), u) : t;
  }, u.defined = function(l) {
    return arguments.length ? (r = typeof l == "function" ? l : fe(!!l), u) : r;
  }, u.curve = function(l) {
    return arguments.length ? (i = l, n != null && (a = i(n)), u) : i;
  }, u.context = function(l) {
    return arguments.length ? (l == null ? n = a = null : a = i(n = l), u) : n;
  }, u;
}
function gi(e, t, r) {
  var n = null, i = fe(!0), a = null, o = Ta, u = null, l = kv(c);
  e = typeof e == "function" ? e : e === void 0 ? Tv : fe(+e), t = typeof t == "function" ? t : fe(t === void 0 ? 0 : +t), r = typeof r == "function" ? r : r === void 0 ? Mv : fe(+r);
  function c(f) {
    var d, h, p, v = (f = Uu(f)).length, m, y = !1, b, x = new Array(v), w = new Array(v);
    for (a == null && (u = o(b = l())), d = 0; d <= v; ++d) {
      if (!(d < v && i(m = f[d], d, f)) === y)
        if (y = !y)
          h = d, u.areaStart(), u.lineStart();
        else {
          for (u.lineEnd(), u.lineStart(), p = d - 1; p >= h; --p)
            u.point(x[p], w[p]);
          u.lineEnd(), u.areaEnd();
        }
      y && (x[d] = +e(m, d, f), w[d] = +t(m, d, f), u.point(n ? +n(m, d, f) : x[d], r ? +r(m, d, f) : w[d]));
    }
    if (b) return u = null, b + "" || null;
  }
  function s() {
    return Dv().defined(i).curve(o).context(a);
  }
  return c.x = function(f) {
    return arguments.length ? (e = typeof f == "function" ? f : fe(+f), n = null, c) : e;
  }, c.x0 = function(f) {
    return arguments.length ? (e = typeof f == "function" ? f : fe(+f), c) : e;
  }, c.x1 = function(f) {
    return arguments.length ? (n = f == null ? null : typeof f == "function" ? f : fe(+f), c) : n;
  }, c.y = function(f) {
    return arguments.length ? (t = typeof f == "function" ? f : fe(+f), r = null, c) : t;
  }, c.y0 = function(f) {
    return arguments.length ? (t = typeof f == "function" ? f : fe(+f), c) : t;
  }, c.y1 = function(f) {
    return arguments.length ? (r = f == null ? null : typeof f == "function" ? f : fe(+f), c) : r;
  }, c.lineX0 = c.lineY0 = function() {
    return s().x(e).y(t);
  }, c.lineY1 = function() {
    return s().x(e).y(r);
  }, c.lineX1 = function() {
    return s().x(n).y(t);
  }, c.defined = function(f) {
    return arguments.length ? (i = typeof f == "function" ? f : fe(!!f), c) : i;
  }, c.curve = function(f) {
    return arguments.length ? (o = f, a != null && (u = o(a)), c) : o;
  }, c.context = function(f) {
    return arguments.length ? (f == null ? a = u = null : u = o(a = f), c) : a;
  }, c;
}
class Nv {
  constructor(t, r) {
    this._context = t, this._x = r;
  }
  areaStart() {
    this._line = 0;
  }
  areaEnd() {
    this._line = NaN;
  }
  lineStart() {
    this._point = 0;
  }
  lineEnd() {
    (this._line || this._line !== 0 && this._point === 1) && this._context.closePath(), this._line = 1 - this._line;
  }
  point(t, r) {
    switch (t = +t, r = +r, this._point) {
      case 0: {
        this._point = 1, this._line ? this._context.lineTo(t, r) : this._context.moveTo(t, r);
        break;
      }
      case 1:
        this._point = 2;
      default: {
        this._x ? this._context.bezierCurveTo(this._x0 = (this._x0 + t) / 2, this._y0, this._x0, r, t, r) : this._context.bezierCurveTo(this._x0, this._y0 = (this._y0 + r) / 2, t, this._y0, t, r);
        break;
      }
    }
    this._x0 = t, this._y0 = r;
  }
}
function r0(e) {
  return new Nv(e, !0);
}
function n0(e) {
  return new Nv(e, !1);
}
function Wi() {
}
function Ui(e, t, r) {
  e._context.bezierCurveTo(
    (2 * e._x0 + e._x1) / 3,
    (2 * e._y0 + e._y1) / 3,
    (e._x0 + 2 * e._x1) / 3,
    (e._y0 + 2 * e._y1) / 3,
    (e._x0 + 4 * e._x1 + t) / 6,
    (e._y0 + 4 * e._y1 + r) / 6
  );
}
function jv(e) {
  this._context = e;
}
jv.prototype = {
  areaStart: function() {
    this._line = 0;
  },
  areaEnd: function() {
    this._line = NaN;
  },
  lineStart: function() {
    this._x0 = this._x1 = this._y0 = this._y1 = NaN, this._point = 0;
  },
  lineEnd: function() {
    switch (this._point) {
      case 3:
        Ui(this, this._x1, this._y1);
      case 2:
        this._context.lineTo(this._x1, this._y1);
        break;
    }
    (this._line || this._line !== 0 && this._point === 1) && this._context.closePath(), this._line = 1 - this._line;
  },
  point: function(e, t) {
    switch (e = +e, t = +t, this._point) {
      case 0:
        this._point = 1, this._line ? this._context.lineTo(e, t) : this._context.moveTo(e, t);
        break;
      case 1:
        this._point = 2;
        break;
      case 2:
        this._point = 3, this._context.lineTo((5 * this._x0 + this._x1) / 6, (5 * this._y0 + this._y1) / 6);
      default:
        Ui(this, e, t);
        break;
    }
    this._x0 = this._x1, this._x1 = e, this._y0 = this._y1, this._y1 = t;
  }
};
function i0(e) {
  return new jv(e);
}
function $v(e) {
  this._context = e;
}
$v.prototype = {
  areaStart: Wi,
  areaEnd: Wi,
  lineStart: function() {
    this._x0 = this._x1 = this._x2 = this._x3 = this._x4 = this._y0 = this._y1 = this._y2 = this._y3 = this._y4 = NaN, this._point = 0;
  },
  lineEnd: function() {
    switch (this._point) {
      case 1: {
        this._context.moveTo(this._x2, this._y2), this._context.closePath();
        break;
      }
      case 2: {
        this._context.moveTo((this._x2 + 2 * this._x3) / 3, (this._y2 + 2 * this._y3) / 3), this._context.lineTo((this._x3 + 2 * this._x2) / 3, (this._y3 + 2 * this._y2) / 3), this._context.closePath();
        break;
      }
      case 3: {
        this.point(this._x2, this._y2), this.point(this._x3, this._y3), this.point(this._x4, this._y4);
        break;
      }
    }
  },
  point: function(e, t) {
    switch (e = +e, t = +t, this._point) {
      case 0:
        this._point = 1, this._x2 = e, this._y2 = t;
        break;
      case 1:
        this._point = 2, this._x3 = e, this._y3 = t;
        break;
      case 2:
        this._point = 3, this._x4 = e, this._y4 = t, this._context.moveTo((this._x0 + 4 * this._x1 + e) / 6, (this._y0 + 4 * this._y1 + t) / 6);
        break;
      default:
        Ui(this, e, t);
        break;
    }
    this._x0 = this._x1, this._x1 = e, this._y0 = this._y1, this._y1 = t;
  }
};
function a0(e) {
  return new $v(e);
}
function Rv(e) {
  this._context = e;
}
Rv.prototype = {
  areaStart: function() {
    this._line = 0;
  },
  areaEnd: function() {
    this._line = NaN;
  },
  lineStart: function() {
    this._x0 = this._x1 = this._y0 = this._y1 = NaN, this._point = 0;
  },
  lineEnd: function() {
    (this._line || this._line !== 0 && this._point === 3) && this._context.closePath(), this._line = 1 - this._line;
  },
  point: function(e, t) {
    switch (e = +e, t = +t, this._point) {
      case 0:
        this._point = 1;
        break;
      case 1:
        this._point = 2;
        break;
      case 2:
        this._point = 3;
        var r = (this._x0 + 4 * this._x1 + e) / 6, n = (this._y0 + 4 * this._y1 + t) / 6;
        this._line ? this._context.lineTo(r, n) : this._context.moveTo(r, n);
        break;
      case 3:
        this._point = 4;
      default:
        Ui(this, e, t);
        break;
    }
    this._x0 = this._x1, this._x1 = e, this._y0 = this._y1, this._y1 = t;
  }
};
function o0(e) {
  return new Rv(e);
}
function Lv(e) {
  this._context = e;
}
Lv.prototype = {
  areaStart: Wi,
  areaEnd: Wi,
  lineStart: function() {
    this._point = 0;
  },
  lineEnd: function() {
    this._point && this._context.closePath();
  },
  point: function(e, t) {
    e = +e, t = +t, this._point ? this._context.lineTo(e, t) : (this._point = 1, this._context.moveTo(e, t));
  }
};
function u0(e) {
  return new Lv(e);
}
function bc(e) {
  return e < 0 ? -1 : 1;
}
function xc(e, t, r) {
  var n = e._x1 - e._x0, i = t - e._x1, a = (e._y1 - e._y0) / (n || i < 0 && -0), o = (r - e._y1) / (i || n < 0 && -0), u = (a * i + o * n) / (n + i);
  return (bc(a) + bc(o)) * Math.min(Math.abs(a), Math.abs(o), 0.5 * Math.abs(u)) || 0;
}
function wc(e, t) {
  var r = e._x1 - e._x0;
  return r ? (3 * (e._y1 - e._y0) / r - t) / 2 : t;
}
function mo(e, t, r) {
  var n = e._x0, i = e._y0, a = e._x1, o = e._y1, u = (a - n) / 3;
  e._context.bezierCurveTo(n + u, i + u * t, a - u, o - u * r, a, o);
}
function Vi(e) {
  this._context = e;
}
Vi.prototype = {
  areaStart: function() {
    this._line = 0;
  },
  areaEnd: function() {
    this._line = NaN;
  },
  lineStart: function() {
    this._x0 = this._x1 = this._y0 = this._y1 = this._t0 = NaN, this._point = 0;
  },
  lineEnd: function() {
    switch (this._point) {
      case 2:
        this._context.lineTo(this._x1, this._y1);
        break;
      case 3:
        mo(this, this._t0, wc(this, this._t0));
        break;
    }
    (this._line || this._line !== 0 && this._point === 1) && this._context.closePath(), this._line = 1 - this._line;
  },
  point: function(e, t) {
    var r = NaN;
    if (e = +e, t = +t, !(e === this._x1 && t === this._y1)) {
      switch (this._point) {
        case 0:
          this._point = 1, this._line ? this._context.lineTo(e, t) : this._context.moveTo(e, t);
          break;
        case 1:
          this._point = 2;
          break;
        case 2:
          this._point = 3, mo(this, wc(this, r = xc(this, e, t)), r);
          break;
        default:
          mo(this, this._t0, r = xc(this, e, t));
          break;
      }
      this._x0 = this._x1, this._x1 = e, this._y0 = this._y1, this._y1 = t, this._t0 = r;
    }
  }
};
function zv(e) {
  this._context = new Bv(e);
}
(zv.prototype = Object.create(Vi.prototype)).point = function(e, t) {
  Vi.prototype.point.call(this, t, e);
};
function Bv(e) {
  this._context = e;
}
Bv.prototype = {
  moveTo: function(e, t) {
    this._context.moveTo(t, e);
  },
  closePath: function() {
    this._context.closePath();
  },
  lineTo: function(e, t) {
    this._context.lineTo(t, e);
  },
  bezierCurveTo: function(e, t, r, n, i, a) {
    this._context.bezierCurveTo(t, e, n, r, a, i);
  }
};
function l0(e) {
  return new Vi(e);
}
function c0(e) {
  return new zv(e);
}
function Fv(e) {
  this._context = e;
}
Fv.prototype = {
  areaStart: function() {
    this._line = 0;
  },
  areaEnd: function() {
    this._line = NaN;
  },
  lineStart: function() {
    this._x = [], this._y = [];
  },
  lineEnd: function() {
    var e = this._x, t = this._y, r = e.length;
    if (r)
      if (this._line ? this._context.lineTo(e[0], t[0]) : this._context.moveTo(e[0], t[0]), r === 2)
        this._context.lineTo(e[1], t[1]);
      else
        for (var n = Ac(e), i = Ac(t), a = 0, o = 1; o < r; ++a, ++o)
          this._context.bezierCurveTo(n[0][a], i[0][a], n[1][a], i[1][a], e[o], t[o]);
    (this._line || this._line !== 0 && r === 1) && this._context.closePath(), this._line = 1 - this._line, this._x = this._y = null;
  },
  point: function(e, t) {
    this._x.push(+e), this._y.push(+t);
  }
};
function Ac(e) {
  var t, r = e.length - 1, n, i = new Array(r), a = new Array(r), o = new Array(r);
  for (i[0] = 0, a[0] = 2, o[0] = e[0] + 2 * e[1], t = 1; t < r - 1; ++t) i[t] = 1, a[t] = 4, o[t] = 4 * e[t] + 2 * e[t + 1];
  for (i[r - 1] = 2, a[r - 1] = 7, o[r - 1] = 8 * e[r - 1] + e[r], t = 1; t < r; ++t) n = i[t] / a[t - 1], a[t] -= n, o[t] -= n * o[t - 1];
  for (i[r - 1] = o[r - 1] / a[r - 1], t = r - 2; t >= 0; --t) i[t] = (o[t] - i[t + 1]) / a[t];
  for (a[r - 1] = (e[r] + i[r - 1]) / 2, t = 0; t < r - 1; ++t) a[t] = 2 * e[t + 1] - i[t + 1];
  return [i, a];
}
function s0(e) {
  return new Fv(e);
}
function Ma(e, t) {
  this._context = e, this._t = t;
}
Ma.prototype = {
  areaStart: function() {
    this._line = 0;
  },
  areaEnd: function() {
    this._line = NaN;
  },
  lineStart: function() {
    this._x = this._y = NaN, this._point = 0;
  },
  lineEnd: function() {
    0 < this._t && this._t < 1 && this._point === 2 && this._context.lineTo(this._x, this._y), (this._line || this._line !== 0 && this._point === 1) && this._context.closePath(), this._line >= 0 && (this._t = 1 - this._t, this._line = 1 - this._line);
  },
  point: function(e, t) {
    switch (e = +e, t = +t, this._point) {
      case 0:
        this._point = 1, this._line ? this._context.lineTo(e, t) : this._context.moveTo(e, t);
        break;
      case 1:
        this._point = 2;
      default: {
        if (this._t <= 0)
          this._context.lineTo(this._x, t), this._context.lineTo(e, t);
        else {
          var r = this._x * (1 - this._t) + e * this._t;
          this._context.lineTo(r, this._y), this._context.lineTo(r, t);
        }
        break;
      }
    }
    this._x = e, this._y = t;
  }
};
function f0(e) {
  return new Ma(e, 0.5);
}
function d0(e) {
  return new Ma(e, 0);
}
function v0(e) {
  return new Ma(e, 1);
}
function Tr(e, t) {
  if ((o = e.length) > 1)
    for (var r = 1, n, i, a = e[t[0]], o, u = a.length; r < o; ++r)
      for (i = a, a = e[t[r]], n = 0; n < u; ++n)
        a[n][1] += a[n][0] = isNaN(i[n][1]) ? i[n][0] : i[n][1];
}
function Qo(e) {
  for (var t = e.length, r = new Array(t); --t >= 0; ) r[t] = t;
  return r;
}
function h0(e, t) {
  return e[t];
}
function p0(e) {
  const t = [];
  return t.key = e, t;
}
function m0() {
  var e = fe([]), t = Qo, r = Tr, n = h0;
  function i(a) {
    var o = Array.from(e.apply(this, arguments), p0), u, l = o.length, c = -1, s;
    for (const f of a)
      for (u = 0, ++c; u < l; ++u)
        (o[u][c] = [0, +n(f, o[u].key, c, a)]).data = f;
    for (u = 0, s = Uu(t(o)); u < l; ++u)
      o[s[u]].index = u;
    return r(o, s), o;
  }
  return i.keys = function(a) {
    return arguments.length ? (e = typeof a == "function" ? a : fe(Array.from(a)), i) : e;
  }, i.value = function(a) {
    return arguments.length ? (n = typeof a == "function" ? a : fe(+a), i) : n;
  }, i.order = function(a) {
    return arguments.length ? (t = a == null ? Qo : typeof a == "function" ? a : fe(Array.from(a)), i) : t;
  }, i.offset = function(a) {
    return arguments.length ? (r = a ?? Tr, i) : r;
  }, i;
}
function y0(e, t) {
  if ((n = e.length) > 0) {
    for (var r, n, i = 0, a = e[0].length, o; i < a; ++i) {
      for (o = r = 0; r < n; ++r) o += e[r][i][1] || 0;
      if (o) for (r = 0; r < n; ++r) e[r][i][1] /= o;
    }
    Tr(e, t);
  }
}
function g0(e, t) {
  if ((i = e.length) > 0) {
    for (var r = 0, n = e[t[0]], i, a = n.length; r < a; ++r) {
      for (var o = 0, u = 0; o < i; ++o) u += e[o][r][1] || 0;
      n[r][1] += n[r][0] = -u / 2;
    }
    Tr(e, t);
  }
}
function b0(e, t) {
  if (!(!((o = e.length) > 0) || !((a = (i = e[t[0]]).length) > 0))) {
    for (var r = 0, n = 1, i, a, o; n < a; ++n) {
      for (var u = 0, l = 0, c = 0; u < o; ++u) {
        for (var s = e[t[u]], f = s[n][1] || 0, d = s[n - 1][1] || 0, h = (f - d) / 2, p = 0; p < u; ++p) {
          var v = e[t[p]], m = v[n][1] || 0, y = v[n - 1][1] || 0;
          h += m - y;
        }
        l += f, c += h * f;
      }
      i[n - 1][1] += i[n - 1][0] = r, l && (r -= c / l);
    }
    i[n - 1][1] += i[n - 1][0] = r, Tr(e, t);
  }
}
function Jo(e) {
  return e === "__proto__";
}
function Wv(e) {
  switch (typeof e) {
    case "number":
    case "symbol":
      return !1;
    case "string":
      return e.includes(".") || e.includes("[") || e.includes("]");
  }
}
function Vu(e) {
  var t;
  return typeof e == "string" || typeof e == "symbol" ? e : Object.is((t = e == null ? void 0 : e.valueOf) == null ? void 0 : t.call(e), -0) ? "-0" : String(e);
}
function Uv(e) {
  if (e == null) return "";
  if (typeof e == "string") return e;
  if (Array.isArray(e)) return e.map(Uv).join(",");
  const t = String(e);
  return t === "0" && Object.is(Number(e), -0) ? "-0" : t;
}
function Ku(e) {
  if (Array.isArray(e)) return e.map(Vu);
  if (typeof e == "symbol") return [e];
  e = Uv(e);
  const t = [], r = e.length;
  if (r === 0) return t;
  let n = 0, i = "", a = "", o = !1;
  for (e.charCodeAt(0) === 46 && t.push(""); n < r; ) {
    const u = e[n];
    if (a) u === "\\" && n + 1 < r ? (n++, i += e[n]) : u === a ? a = "" : i += u;
    else if (o) u === '"' || u === "'" ? a = u : u === "]" ? (o = !1, t.push(i), i = "") : i += u;
    else if (u === "[")
      o = !0, i && (t.push(i), i = "");
    else if (u === ".") {
      i && (t.push(i), i = "");
      const l = e[n + 1];
      (l === void 0 || l === ".") && t.push("");
    } else i += u;
    n++;
  }
  return i && t.push(i), t;
}
function Ut(e, t, r) {
  if (e == null) return r;
  switch (typeof t) {
    case "string": {
      if (Jo(t)) return r;
      const n = e[t];
      return n === void 0 ? Wv(t) && !Object.hasOwn(e, t) ? Ut(e, Ku(t), r) : r : n;
    }
    case "number":
    case "symbol": {
      typeof t == "number" && (t = Vu(t));
      const n = e[t];
      return n === void 0 ? r : n;
    }
    default: {
      if (Array.isArray(t)) return x0(e, t, r);
      if (Object.is(t == null ? void 0 : t.valueOf(), -0) ? t = "-0" : t = String(t), Jo(t)) return r;
      const n = e[t];
      return n === void 0 ? r : n;
    }
  }
}
function x0(e, t, r) {
  if (t.length === 0) return r;
  let n = e;
  for (let i = 0; i < t.length; i++) {
    if (n == null || Jo(t[i])) return r;
    n = n[t[i]];
  }
  return n === void 0 ? r : n;
}
var w0 = 4;
function or(e) {
  var t = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : w0, r = 10 ** t, n = Math.round(e * r) / r;
  return Object.is(n, -0) ? 0 : n;
}
function Me(e) {
  for (var t = arguments.length, r = new Array(t > 1 ? t - 1 : 0), n = 1; n < t; n++)
    r[n - 1] = arguments[n];
  return e.reduce((i, a, o) => {
    var u = r[o - 1];
    return typeof u == "string" ? i + u + a : u !== void 0 ? i + or(u) + a : i + a;
  }, "");
}
var He = (e) => e === 0 ? 0 : e > 0 ? 1 : -1, Tt = (e) => typeof e == "number" && e != +e, Mr = (e) => typeof e == "string" && e.length > 1 && e.indexOf("%") === e.length - 1, N = (e) => (typeof e == "number" || e instanceof Number) && !Tt(e), Mt = (e) => N(e) || typeof e == "string", A0 = 0, zn = (e) => {
  var t = ++A0;
  return "".concat(e || "").concat(t);
}, yt = function(t, r) {
  var n = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : 0, i = arguments.length > 3 && arguments[3] !== void 0 ? arguments[3] : !1;
  if (!N(t) && typeof t != "string")
    return n;
  var a;
  if (Mr(t)) {
    if (r == null)
      return n;
    var o = t.indexOf("%");
    a = r * parseFloat(t.slice(0, o)) / 100;
  } else
    a = +t;
  return Tt(a) && (a = n), i && r != null && a > r && (a = r), a;
}, Vv = (e) => {
  if (!Array.isArray(e))
    return !1;
  for (var t = e.length, r = {}, n = 0; n < t; n++)
    if (!r[String(e[n])])
      r[String(e[n])] = !0;
    else
      return !0;
  return !1;
};
function $e(e, t, r) {
  return N(e) && N(t) ? or(e + r * (t - e)) : t;
}
function O0(e, t, r) {
  if (!(!e || !e.length))
    return e.find((n) => n && (typeof t == "function" ? t(n) : Ut(n, t)) === r);
}
var we = (e) => e === null || typeof e > "u", Hu = (e) => we(e) ? e : "".concat(e.charAt(0).toUpperCase()).concat(e.slice(1));
function Ye(e) {
  return e != null;
}
function fn() {
}
var Kv = (e) => "radius" in e && "startAngle" in e && "endAngle" in e, E0 = (e, t) => {
  if (!e || typeof e == "function" || typeof e == "boolean")
    return null;
  var r = e;
  if (/* @__PURE__ */ Pt(e) && (r = e.props), typeof r != "object" && typeof r != "function")
    return null;
  var n = {};
  return Object.keys(r).forEach((i) => {
    Fu(i) && typeof r[i] == "function" && (n[i] = (a) => r[i](r, a));
  }), n;
}, S0 = (e, t, r) => (n) => (e(t, r, n), null), Yu = (e, t, r) => {
  if (e === null || typeof e != "object" && typeof e != "function")
    return null;
  var n = null;
  return Object.keys(e).forEach((i) => {
    var a = e[i];
    Fu(i) && typeof a == "function" && (n || (n = {}), n[i] = S0(a, t, r));
  }), n;
};
function Oc(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function _0(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Oc(Object(r), !0).forEach(function(n) {
      P0(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Oc(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function P0(e, t, r) {
  return (t = I0(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function I0(e) {
  var t = k0(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function k0(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function st(e, t) {
  var r = _0({}, e), n = t, i = Object.keys(t), a = i.reduce((o, u) => (o[u] === void 0 && n[u] !== void 0 && (o[u] = n[u]), o), r);
  return a;
}
function C0(e, t) {
  const r = /* @__PURE__ */ new Map();
  for (let n = 0; n < e.length; n++) {
    const i = e[n], a = t(i, n, e);
    r.has(a) || r.set(a, i);
  }
  return Array.from(r.values());
}
function T0(e, t) {
  return function(...r) {
    return e.apply(this, r.slice(0, t));
  };
}
function Hv(e) {
  return e;
}
function M0(e) {
  return function(t) {
    return Ut(t, e);
  };
}
function eu(e) {
  return e == null || typeof e != "object" && typeof e != "function";
}
function D0(e) {
  return ArrayBuffer.isView(e) && !(e instanceof DataView);
}
function N0(e) {
  return Object.getOwnPropertySymbols(e).filter((t) => Object.prototype.propertyIsEnumerable.call(e, t));
}
function Gu(e) {
  return e == null ? e === void 0 ? "[object Undefined]" : "[object Null]" : Object.prototype.toString.call(e);
}
const j0 = "[object RegExp]", Yv = "[object String]", Gv = "[object Number]", qv = "[object Boolean]", Xv = "[object Arguments]", $0 = "[object Symbol]", R0 = "[object Date]", L0 = "[object Map]", z0 = "[object Set]", B0 = "[object Array]", F0 = "[object ArrayBuffer]", W0 = "[object Object]", U0 = "[object DataView]", V0 = "[object Uint8Array]", K0 = "[object Uint8ClampedArray]", H0 = "[object Uint16Array]", Y0 = "[object Uint32Array]", G0 = "[object Int8Array]", q0 = "[object Int16Array]", X0 = "[object Int32Array]", Z0 = "[object Float32Array]", Q0 = "[object Float64Array]", Ec = typeof globalThis == "object" && globalThis || typeof window == "object" && window || typeof self == "object" && self || typeof global == "object" && global || /* @__PURE__ */ function() {
  return this;
}();
function J0(e) {
  return typeof Ec.Buffer < "u" && Ec.Buffer.isBuffer(e);
}
function eb(e, t) {
  return Ar(e, void 0, e, /* @__PURE__ */ new Map(), t);
}
function Ar(e, t, r, n = /* @__PURE__ */ new Map(), i = void 0) {
  const a = i == null ? void 0 : i(e, t, r, n);
  if (a !== void 0) return a;
  if (eu(e)) return e;
  if (n.has(e)) return n.get(e);
  if (Array.isArray(e)) {
    const o = new Array(e.length);
    n.set(e, o);
    for (let u = 0; u < e.length; u++) o[u] = Ar(e[u], u, r, n, i);
    return Object.hasOwn(e, "index") && (o.index = e.index), Object.hasOwn(e, "input") && (o.input = e.input), o;
  }
  if (e instanceof Date) return new Date(e.getTime());
  if (e instanceof RegExp) {
    const o = new RegExp(e.source, e.flags);
    return o.lastIndex = e.lastIndex, o;
  }
  if (e instanceof Map) {
    const o = /* @__PURE__ */ new Map();
    n.set(e, o);
    for (const [u, l] of e) o.set(u, Ar(l, u, r, n, i));
    return o;
  }
  if (e instanceof Set) {
    const o = /* @__PURE__ */ new Set();
    n.set(e, o);
    for (const u of e) o.add(Ar(u, void 0, r, n, i));
    return o;
  }
  if (J0(e)) return e.subarray();
  if (D0(e)) {
    const o = new (Object.getPrototypeOf(e)).constructor(e.length);
    n.set(e, o);
    for (let u = 0; u < e.length; u++) o[u] = Ar(e[u], u, r, n, i);
    return o;
  }
  if (e instanceof ArrayBuffer || typeof SharedArrayBuffer < "u" && e instanceof SharedArrayBuffer) return e.slice(0);
  if (e instanceof DataView) {
    const o = new DataView(e.buffer.slice(0), e.byteOffset, e.byteLength);
    return n.set(e, o), pt(o, e, r, n, i), o;
  }
  if (typeof File < "u" && e instanceof File) {
    const o = new File([e], e.name, { type: e.type });
    return n.set(e, o), pt(o, e, r, n, i), o;
  }
  if (typeof Blob < "u" && e instanceof Blob) {
    const o = new Blob([e], { type: e.type });
    return n.set(e, o), pt(o, e, r, n, i), o;
  }
  if (e instanceof Error) {
    const o = structuredClone(e);
    return n.set(e, o), o.message = e.message, o.name = e.name, o.stack = e.stack, o.cause = e.cause, o.constructor = e.constructor, pt(o, e, r, n, i), o;
  }
  if (e instanceof Boolean) {
    const o = new Boolean(e.valueOf());
    return n.set(e, o), pt(o, e, r, n, i), o;
  }
  if (e instanceof Number) {
    const o = new Number(e.valueOf());
    return n.set(e, o), pt(o, e, r, n, i), o;
  }
  if (e instanceof String) {
    const o = new String(e.valueOf());
    return n.set(e, o), pt(o, e, r, n, i), o;
  }
  if (typeof e == "object" && tb(e)) {
    const o = Object.create(Object.getPrototypeOf(e));
    return n.set(e, o), pt(o, e, r, n, i), o;
  }
  return e;
}
function pt(e, t, r = e, n, i) {
  const a = [...Object.keys(t), ...N0(t)];
  for (let o = 0; o < a.length; o++) {
    const u = a[o], l = Object.getOwnPropertyDescriptor(e, u);
    (l == null || l.writable) && (e[u] = Ar(t[u], u, r, n, i));
  }
}
function tb(e) {
  switch (Gu(e)) {
    case Xv:
    case B0:
    case F0:
    case U0:
    case qv:
    case R0:
    case Z0:
    case Q0:
    case G0:
    case q0:
    case X0:
    case L0:
    case Gv:
    case W0:
    case j0:
    case z0:
    case Yv:
    case $0:
    case V0:
    case K0:
    case H0:
    case Y0:
      return !0;
    default:
      return !1;
  }
}
function rb(e) {
  return Ar(e, void 0, e, /* @__PURE__ */ new Map(), void 0);
}
function $i(e, t) {
  return e === t || Number.isNaN(e) && Number.isNaN(t);
}
function Zv(e) {
  return e !== null && (typeof e == "object" || typeof e == "function");
}
function Qv(e, t, r) {
  return typeof r != "function" ? Qv(e, t, () => {
  }) : tu(e, t, function n(i, a, o, u, l, c) {
    const s = r(i, a, o, u, l, c);
    return s !== void 0 ? !!s : tu(i, a, n, c, !1);
  }, /* @__PURE__ */ new Map(), !0);
}
function tu(e, t, r, n, i = !1) {
  if (t === e) return !0;
  switch (typeof t) {
    case "object":
      return nb(e, t, r, n);
    case "function":
      return Object.keys(t).length > 0 ? tu(e, { ...t }, r, n, i) : $i(e, t);
    default:
      return Zv(e) && i ? typeof t == "string" ? t === "" : !0 : $i(e, t);
  }
}
function nb(e, t, r, n) {
  if (t == null) return !0;
  if (Array.isArray(t)) return Jv(e, t, r, n);
  if (t instanceof Map) return ib(e, t, r, n);
  if (t instanceof Set) return ab(e, t, r, n);
  const i = Object.keys(t);
  if (e == null || eu(e)) return i.length === 0;
  if (i.length === 0) return !0;
  if (n != null && n.has(t)) return n.get(t) === e;
  n == null || n.set(t, e);
  try {
    for (let a = 0; a < i.length; a++) {
      const o = i[a];
      if (!eu(e) && !(o in e) || t[o] === void 0 && e[o] !== void 0 || t[o] === null && e[o] !== null || !r(e[o], t[o], o, e, t, n)) return !1;
    }
    return !0;
  } finally {
    n == null || n.delete(t);
  }
}
function ib(e, t, r, n) {
  if (t.size === 0) return !0;
  if (!(e instanceof Map)) return !1;
  for (const [i, a] of t.entries()) if (r(e.get(i), a, i, e, t, n) === !1) return !1;
  return !0;
}
function Jv(e, t, r, n) {
  if (t.length === 0) return !0;
  if (!Array.isArray(e)) return !1;
  const i = /* @__PURE__ */ new Set();
  for (let a = 0; a < t.length; a++) {
    const o = t[a];
    let u = !1;
    for (let l = 0; l < e.length; l++) {
      if (i.has(l)) continue;
      const c = e[l];
      let s = !1;
      if (r(c, o, a, e, t, n) && (s = !0), s) {
        i.add(l), u = !0;
        break;
      }
    }
    if (!u) return !1;
  }
  return !0;
}
function ab(e, t, r, n) {
  return t.size === 0 ? !0 : e instanceof Set ? Jv([...e], [...t], r, n) : !1;
}
function eh(e, t) {
  return Qv(e, t, () => {
  });
}
function ob(e) {
  return e = rb(e), (t) => eh(t, e);
}
function ub(e, t) {
  return eb(e, (r, n, i, a) => {
    if (typeof e == "object") {
      if (Gu(e) === "[object Object]" && typeof e.constructor != "function") {
        const o = {};
        return a.set(e, o), pt(o, e, i, a), o;
      }
      switch (Object.prototype.toString.call(e)) {
        case Gv:
        case Yv:
        case qv: {
          const o = new e.constructor(e == null ? void 0 : e.valueOf());
          return pt(o, e), o;
        }
        case Xv: {
          const o = {};
          return pt(o, e), o.length = e.length, o[Symbol.iterator] = e[Symbol.iterator], o;
        }
        default:
          return;
      }
    }
  });
}
function lb(e) {
  return ub(e);
}
const cb = /^(?:0|[1-9]\d*)$/;
function th(e, t = Number.MAX_SAFE_INTEGER) {
  switch (typeof e) {
    case "number":
      return Number.isInteger(e) && e >= 0 && e < t;
    case "symbol":
      return !1;
    case "string":
      return cb.test(e);
  }
}
function sb(e) {
  return e !== null && typeof e == "object" && Gu(e) === "[object Arguments]";
}
function fb(e, t) {
  let r;
  if (Array.isArray(t) ? r = t : typeof t == "string" && Wv(t) && (e == null ? void 0 : e[t]) == null ? r = Ku(t) : r = [t], r.length === 0) return !1;
  let n = e;
  for (let i = 0; i < r.length; i++) {
    const a = r[i];
    if ((n == null || !Object.hasOwn(n, a)) && !((Array.isArray(n) || sb(n)) && th(a) && a < n.length))
      return !1;
    n = n[a];
  }
  return !0;
}
function db(e, t) {
  switch (typeof e) {
    case "object":
      Object.is(e == null ? void 0 : e.valueOf(), -0) && (e = "-0");
      break;
    case "number":
      e = Vu(e);
      break;
  }
  return t = lb(t), function(r) {
    const n = Ut(r, e);
    return n === void 0 ? fb(r, e) : t === void 0 ? n === void 0 : eh(n, t);
  };
}
function vb(e) {
  if (e == null) return Hv;
  switch (typeof e) {
    case "function":
      return e;
    case "object":
      return Array.isArray(e) && e.length === 2 ? db(e[0], e[1]) : ob(e);
    case "string":
    case "symbol":
    case "number":
      return M0(e);
  }
}
function hb(e) {
  return Number.isSafeInteger(e) && e >= 0;
}
function rh(e) {
  return e != null && typeof e != "function" && hb(e.length);
}
function pb(e) {
  return typeof e == "object" && e !== null;
}
function mb(e) {
  return pb(e) && rh(e);
}
function Sc(e, t = Hv) {
  return mb(e) ? C0(Array.from(e), T0(vb(t), 1)) : [];
}
function yb(e, t, r) {
  return t === !0 ? Sc(e, r) : typeof t == "function" ? Sc(e, t) : e;
}
var ru = { exports: {} }, yo = {}, bi = { exports: {} }, go = {};
/**
 * @license React
 * use-sync-external-store-shim.production.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var _c;
function gb() {
  if (_c) return go;
  _c = 1;
  var e = sn;
  function t(f, d) {
    return f === d && (f !== 0 || 1 / f === 1 / d) || f !== f && d !== d;
  }
  var r = typeof Object.is == "function" ? Object.is : t, n = e.useState, i = e.useEffect, a = e.useLayoutEffect, o = e.useDebugValue;
  function u(f, d) {
    var h = d(), p = n({ inst: { value: h, getSnapshot: d } }), v = p[0].inst, m = p[1];
    return a(
      function() {
        v.value = h, v.getSnapshot = d, l(v) && m({ inst: v });
      },
      [f, h, d]
    ), i(
      function() {
        return l(v) && m({ inst: v }), f(function() {
          l(v) && m({ inst: v });
        });
      },
      [f]
    ), o(h), h;
  }
  function l(f) {
    var d = f.getSnapshot;
    f = f.value;
    try {
      var h = d();
      return !r(f, h);
    } catch {
      return !0;
    }
  }
  function c(f, d) {
    return d();
  }
  var s = typeof window > "u" || typeof window.document > "u" || typeof window.document.createElement > "u" ? c : u;
  return go.useSyncExternalStore = e.useSyncExternalStore !== void 0 ? e.useSyncExternalStore : s, go;
}
var bo = {};
/**
 * @license React
 * use-sync-external-store-shim.development.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var Pc;
function bb() {
  return Pc || (Pc = 1, process.env.NODE_ENV !== "production" && function() {
    function e(h, p) {
      return h === p && (h !== 0 || 1 / h === 1 / p) || h !== h && p !== p;
    }
    function t(h, p) {
      s || i.startTransition === void 0 || (s = !0, console.error(
        "You are using an outdated, pre-release alpha of React 18 that does not support useSyncExternalStore. The use-sync-external-store shim will not work correctly. Upgrade to a newer pre-release."
      ));
      var v = p();
      if (!f) {
        var m = p();
        a(v, m) || (console.error(
          "The result of getSnapshot should be cached to avoid an infinite loop"
        ), f = !0);
      }
      m = o({
        inst: { value: v, getSnapshot: p }
      });
      var y = m[0].inst, b = m[1];
      return l(
        function() {
          y.value = v, y.getSnapshot = p, r(y) && b({ inst: y });
        },
        [h, v, p]
      ), u(
        function() {
          return r(y) && b({ inst: y }), h(function() {
            r(y) && b({ inst: y });
          });
        },
        [h]
      ), c(v), v;
    }
    function r(h) {
      var p = h.getSnapshot;
      h = h.value;
      try {
        var v = p();
        return !a(h, v);
      } catch {
        return !0;
      }
    }
    function n(h, p) {
      return p();
    }
    typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart(Error());
    var i = sn, a = typeof Object.is == "function" ? Object.is : e, o = i.useState, u = i.useEffect, l = i.useLayoutEffect, c = i.useDebugValue, s = !1, f = !1, d = typeof window > "u" || typeof window.document > "u" || typeof window.document.createElement > "u" ? n : t;
    bo.useSyncExternalStore = i.useSyncExternalStore !== void 0 ? i.useSyncExternalStore : d, typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop(Error());
  }()), bo;
}
var Ic;
function nh() {
  return Ic || (Ic = 1, process.env.NODE_ENV === "production" ? bi.exports = gb() : bi.exports = bb()), bi.exports;
}
/**
 * @license React
 * use-sync-external-store-shim/with-selector.production.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var kc;
function xb() {
  if (kc) return yo;
  kc = 1;
  var e = sn, t = nh();
  function r(c, s) {
    return c === s && (c !== 0 || 1 / c === 1 / s) || c !== c && s !== s;
  }
  var n = typeof Object.is == "function" ? Object.is : r, i = t.useSyncExternalStore, a = e.useRef, o = e.useEffect, u = e.useMemo, l = e.useDebugValue;
  return yo.useSyncExternalStoreWithSelector = function(c, s, f, d, h) {
    var p = a(null);
    if (p.current === null) {
      var v = { hasValue: !1, value: null };
      p.current = v;
    } else v = p.current;
    p = u(
      function() {
        function y(g) {
          if (!b) {
            if (b = !0, x = g, g = d(g), h !== void 0 && v.hasValue) {
              var S = v.value;
              if (h(S, g))
                return w = S;
            }
            return w = g;
          }
          if (S = w, n(x, g)) return S;
          var P = d(g);
          return h !== void 0 && h(S, P) ? (x = g, S) : (x = g, w = P);
        }
        var b = !1, x, w, O = f === void 0 ? null : f;
        return [
          function() {
            return y(s());
          },
          O === null ? void 0 : function() {
            return y(O());
          }
        ];
      },
      [s, f, d, h]
    );
    var m = i(c, p[0], p[1]);
    return o(
      function() {
        v.hasValue = !0, v.value = m;
      },
      [m]
    ), l(m), m;
  }, yo;
}
var xo = {};
/**
 * @license React
 * use-sync-external-store-shim/with-selector.development.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var Cc;
function wb() {
  return Cc || (Cc = 1, process.env.NODE_ENV !== "production" && function() {
    function e(c, s) {
      return c === s && (c !== 0 || 1 / c === 1 / s) || c !== c && s !== s;
    }
    typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart(Error());
    var t = sn, r = nh(), n = typeof Object.is == "function" ? Object.is : e, i = r.useSyncExternalStore, a = t.useRef, o = t.useEffect, u = t.useMemo, l = t.useDebugValue;
    xo.useSyncExternalStoreWithSelector = function(c, s, f, d, h) {
      var p = a(null);
      if (p.current === null) {
        var v = { hasValue: !1, value: null };
        p.current = v;
      } else v = p.current;
      p = u(
        function() {
          function y(g) {
            if (!b) {
              if (b = !0, x = g, g = d(g), h !== void 0 && v.hasValue) {
                var S = v.value;
                if (h(S, g))
                  return w = S;
              }
              return w = g;
            }
            if (S = w, n(x, g))
              return S;
            var P = d(g);
            return h !== void 0 && h(S, P) ? (x = g, S) : (x = g, w = P);
          }
          var b = !1, x, w, O = f === void 0 ? null : f;
          return [
            function() {
              return y(s());
            },
            O === null ? void 0 : function() {
              return y(O());
            }
          ];
        },
        [s, f, d, h]
      );
      var m = i(c, p[0], p[1]);
      return o(
        function() {
          v.hasValue = !0, v.value = m;
        },
        [m]
      ), l(m), m;
    }, typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop(Error());
  }()), xo;
}
process.env.NODE_ENV === "production" ? ru.exports = xb() : ru.exports = wb();
var Ab = ru.exports, qu = /* @__PURE__ */ Xe(null), Ob = (e) => e, se = () => {
  var e = ct(qu);
  return e ? e.store.dispatch : Ob;
}, Ri = () => {
}, Eb = () => Ri, Sb = (e, t) => e === t;
function j(e) {
  var t = ct(qu), r = sr(() => t ? (n) => {
    if (n != null)
      return e(n);
  } : Ri, [t, e]);
  return Ab.useSyncExternalStoreWithSelector(t ? t.subscription.addNestedSub : Eb, t ? t.store.getState : Ri, t ? t.store.getState : Ri, r, Sb);
}
var _b = (e, t, r) => {
  if (t.length === 1 && t[0] === r) {
    let n = !1;
    try {
      const i = {};
      e(i) === i && (n = !0);
    } catch {
    }
    if (n) {
      let i;
      try {
        throw new Error();
      } catch (a) {
        ({ stack: i } = a);
      }
      console.warn(
        `The result function returned its own inputs without modification. e.g
\`createSelector([state => state.todos], todos => todos)\`
This could lead to inefficient memoization and unnecessary re-renders.
Ensure transformation logic is in the result function, and extraction logic is in the input selectors.`,
        { stack: i }
      );
    }
  }
}, Pb = (e, t, r) => {
  const { memoize: n, memoizeOptions: i } = t, { inputSelectorResults: a, inputSelectorResultsCopy: o } = e, u = n(() => ({}), ...i);
  if (!(u.apply(null, a) === u.apply(null, o))) {
    let c;
    try {
      throw new Error();
    } catch (s) {
      ({ stack: c } = s);
    }
    console.warn(
      `An input selector returned a different result when passed same arguments.
This means your output selector will likely run more frequently than intended.
Avoid returning a new reference inside your input selector, e.g.
\`createSelector([state => state.todos.map(todo => todo.id)], todoIds => todoIds.length)\``,
      {
        arguments: r,
        firstInputs: a,
        secondInputs: o,
        stack: c
      }
    );
  }
}, Ib = {
  inputStabilityCheck: "once",
  identityFunctionCheck: "once"
};
function kb(e, t = `expected a function, instead received ${typeof e}`) {
  if (typeof e != "function")
    throw new TypeError(t);
}
function Cb(e, t = "expected all items to be functions, instead received the following types: ") {
  if (!e.every((r) => typeof r == "function")) {
    const r = e.map(
      (n) => typeof n == "function" ? `function ${n.name || "unnamed"}()` : typeof n
    ).join(", ");
    throw new TypeError(`${t}[${r}]`);
  }
}
var Tc = (e) => Array.isArray(e) ? e : [e];
function Tb(e) {
  const t = Array.isArray(e[0]) ? e[0] : e;
  return Cb(
    t,
    "createSelector expects all input-selectors to be functions, but received the following types: "
  ), t;
}
function Mc(e, t) {
  const r = [], { length: n } = e;
  for (let i = 0; i < n; i++)
    r.push(e[i].apply(null, t));
  return r;
}
var Mb = (e, t) => {
  const { identityFunctionCheck: r, inputStabilityCheck: n } = {
    ...Ib,
    ...t
  };
  return {
    identityFunctionCheck: {
      shouldRun: r === "always" || r === "once" && e,
      run: _b
    },
    inputStabilityCheck: {
      shouldRun: n === "always" || n === "once" && e,
      run: Pb
    }
  };
}, Db = class {
  constructor(e) {
    this.value = e;
  }
  deref() {
    return this.value;
  }
}, Nb = () => typeof WeakRef > "u" ? Db : WeakRef, ih = /* @__PURE__ */ Nb(), jb = 0, Dc = 1;
function xi() {
  return {
    s: jb,
    v: void 0,
    o: null,
    p: null
  };
}
function $b(e) {
  return e instanceof ih ? e.deref() : e;
}
function ah(e, t = {}) {
  let r = xi();
  const { resultEqualityCheck: n } = t;
  let i, a = 0;
  function o() {
    let u = r;
    const { length: l } = arguments;
    for (let f = 0, d = l; f < d; f++) {
      const h = arguments[f];
      if (typeof h == "function" || typeof h == "object" && h !== null) {
        let p = u.o;
        p === null && (u.o = p = /* @__PURE__ */ new WeakMap());
        const v = p.get(h);
        v === void 0 ? (u = xi(), p.set(h, u)) : u = v;
      } else {
        let p = u.p;
        p === null && (u.p = p = /* @__PURE__ */ new Map());
        const v = p.get(h);
        v === void 0 ? (u = xi(), p.set(h, u)) : u = v;
      }
    }
    const c = u;
    let s;
    if (u.s === Dc)
      s = u.v;
    else if (s = e.apply(null, arguments), a++, n) {
      const f = $b(i);
      f != null && n(f, s) && (s = f, a !== 0 && a--), i = typeof s == "object" && s !== null || typeof s == "function" ? /* @__PURE__ */ new ih(s) : s;
    }
    return c.s = Dc, c.v = s, s;
  }
  return o.clearCache = () => {
    r = xi(), o.resetResultsCount();
  }, o.resultsCount = () => a, o.resetResultsCount = () => {
    a = 0;
  }, o;
}
function Rb(e, ...t) {
  const r = typeof e == "function" ? {
    memoize: e,
    memoizeOptions: t
  } : e, n = (...i) => {
    let a = 0, o = 0, u, l = {}, c = i.pop();
    typeof c == "object" && (l = c, c = i.pop()), kb(
      c,
      `createSelector expects an output function after the inputs, but received: [${typeof c}]`
    );
    const s = {
      ...r,
      ...l
    }, {
      memoize: f,
      memoizeOptions: d = [],
      argsMemoize: h = ah,
      argsMemoizeOptions: p = []
    } = s, v = Tc(d), m = Tc(p), y = Tb(i), b = f(function() {
      return a++, c.apply(
        null,
        arguments
      );
    }, ...v);
    let x = !0;
    const w = h(function() {
      o++;
      const g = Mc(
        y,
        arguments
      );
      if (u = b.apply(null, g), process.env.NODE_ENV !== "production") {
        const { devModeChecks: S = {} } = s, { identityFunctionCheck: P, inputStabilityCheck: I } = Mb(x, S);
        if (P.shouldRun && P.run(
          c,
          g,
          u
        ), I.shouldRun) {
          const C = Mc(
            y,
            arguments
          );
          I.run(
            { inputSelectorResults: g, inputSelectorResultsCopy: C },
            { memoize: f, memoizeOptions: v },
            arguments
          );
        }
        x && (x = !1);
      }
      return u;
    }, ...m);
    return Object.assign(w, {
      resultFunc: c,
      memoizedResultFunc: b,
      dependencies: y,
      dependencyRecomputations: () => o,
      resetDependencyRecomputations: () => {
        o = 0;
      },
      lastResult: () => u,
      recomputations: () => a,
      resetRecomputations: () => {
        a = 0;
      },
      memoize: f,
      argsMemoize: h
    });
  };
  return Object.assign(n, {
    withTypes: () => n
  }), n;
}
var E = /* @__PURE__ */ Rb(ah);
function Lb(e, t = 1) {
  const r = [], n = Math.floor(t), i = (a, o) => {
    for (let u = 0; u < a.length; u++) {
      const l = a[u];
      Array.isArray(l) && o < n ? i(l, o + 1) : r.push(l);
    }
  };
  return i(e, 0), r;
}
function nu(e, t, r) {
  return Zv(r) && (typeof t == "number" && rh(r) && th(t) && t < r.length || typeof t == "string" && t in r) ? $i(r[t], e) : !1;
}
function Nc(e) {
  return typeof e == "symbol" ? 1 : e === null ? 2 : e === void 0 ? 3 : e !== e ? 4 : 0;
}
const zb = (e, t, r) => {
  if (e !== t) {
    const n = Nc(e), i = Nc(t);
    if (n === i && n === 0) {
      if (e < t) return r === "desc" ? 1 : -1;
      if (e > t) return r === "desc" ? -1 : 1;
    }
    return r === "desc" ? i - n : n - i;
  }
  return 0;
};
function oh(e) {
  return typeof e == "symbol" || e instanceof Symbol;
}
const Bb = /\.|\[(?:[^[\]]*|(["'])(?:(?!\1)[^\\]|\\.)*?\1)\]/, Fb = /^\w*$/;
function Wb(e, t) {
  return Array.isArray(e) ? !1 : typeof e == "number" || typeof e == "boolean" || e == null || oh(e) ? !0 : typeof e == "string" && (Fb.test(e) || !Bb.test(e)) || t != null;
}
function Ub(e, t, r, n) {
  if (e == null) return [];
  r = r, Array.isArray(e) || (e = Object.values(e)), Array.isArray(t) || (t = t == null ? [null] : [t]), t.length === 0 && (t = [null]), Array.isArray(r) || (r = r == null ? [] : [r]), r = r.map((u) => String(u));
  const i = (u, l) => {
    let c = u;
    for (let s = 0; s < l.length && c != null; ++s) c = c[l[s]];
    return c;
  }, a = (u, l) => l == null || u == null ? l : typeof u == "object" && "key" in u ? Object.hasOwn(l, u.key) ? l[u.key] : i(l, u.path) : typeof u == "function" ? u(l) : Array.isArray(u) ? i(l, u) : typeof l == "object" ? l[u] : l, o = t.map((u) => (Array.isArray(u) && u.length === 1 && (u = u[0]), u == null || typeof u == "function" || Array.isArray(u) || Wb(u) ? u : {
    key: u,
    path: Ku(u)
  }));
  return e.map((u) => ({
    original: u,
    criteria: o.map((l) => a(l, u))
  })).slice().sort((u, l) => {
    for (let c = 0; c < o.length; c++) {
      const s = zb(u.criteria[c], l.criteria[c], r[c]);
      if (s !== 0) return s;
    }
    return 0;
  }).map((u) => u.original);
}
function Da(e, ...t) {
  const r = t.length;
  return r > 1 && nu(e, t[0], t[1]) ? t = [] : r > 2 && nu(t[0], t[1], t[2]) && (t = [t[0]]), Ub(e, Lb(t), ["asc"]);
}
var uh = (e) => e.legend.settings, Vb = (e) => e.legend.size, Kb = (e) => e.legend.payload;
E([Kb, uh], (e, t) => {
  var r = t.itemSorter, n = e.flat(1);
  return r ? Da(n, r) : n;
});
function Hb(e, t) {
  return Xb(e) || qb(e, t) || Gb(e, t) || Yb();
}
function Yb() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function Gb(e, t) {
  if (e) {
    if (typeof e == "string") return jc(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? jc(e, t) : void 0;
  }
}
function jc(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function qb(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function Xb(e) {
  if (Array.isArray(e)) return e;
}
var wi = 1;
function $c(e, t) {
  return Math.abs(e.height - t.height) > wi || Math.abs(e.left - t.left) > wi || Math.abs(e.top - t.top) > wi || Math.abs(e.width - t.width) > wi;
}
function Rc(e) {
  var t = e.getBoundingClientRect();
  return {
    height: t.height,
    left: t.left,
    top: t.top,
    width: t.width
  };
}
function Zb() {
  var e = arguments.length > 0 && arguments[0] !== void 0 ? arguments[0] : [], t = _e({
    height: 0,
    left: 0,
    top: 0,
    width: 0
  }), r = Hb(t, 2), n = r[0], i = r[1], a = Q(null), o = Q(n);
  o.current = n;
  var u = ie(
    (l) => {
      if (a.current != null && (a.current.disconnect(), a.current = null), l != null) {
        var c = Rc(l);
        if ($c(c, o.current) && i(c), typeof ResizeObserver < "u") {
          var s = new ResizeObserver(() => {
            var f = Rc(l);
            $c(f, o.current) && i(f);
          });
          s.observe(l), a.current = s;
        }
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [...e]
  );
  return ve(() => () => {
    var l;
    (l = a.current) === null || l === void 0 || l.disconnect();
  }, []), [n, u];
}
function Te(e) {
  return `Minified Redux error #${e}; visit https://redux.js.org/Errors?code=${e} for the full message or use the non-minified dev environment for full errors. `;
}
var Qb = typeof Symbol == "function" && Symbol.observable || "@@observable", Lc = Qb, wo = () => Math.random().toString(36).substring(7).split("").join("."), Jb = {
  INIT: `@@redux/INIT${/* @__PURE__ */ wo()}`,
  REPLACE: `@@redux/REPLACE${/* @__PURE__ */ wo()}`,
  PROBE_UNKNOWN_ACTION: () => `@@redux/PROBE_UNKNOWN_ACTION${wo()}`
}, Pr = Jb;
function Zn(e) {
  if (typeof e != "object" || e === null)
    return !1;
  let t = e;
  for (; Object.getPrototypeOf(t) !== null; )
    t = Object.getPrototypeOf(t);
  return Object.getPrototypeOf(e) === t || Object.getPrototypeOf(e) === null;
}
function ex(e) {
  if (e === void 0)
    return "undefined";
  if (e === null)
    return "null";
  const t = typeof e;
  switch (t) {
    case "boolean":
    case "string":
    case "number":
    case "symbol":
    case "function":
      return t;
  }
  if (Array.isArray(e))
    return "array";
  if (nx(e))
    return "date";
  if (rx(e))
    return "error";
  const r = tx(e);
  switch (r) {
    case "Symbol":
    case "Promise":
    case "WeakMap":
    case "WeakSet":
    case "Map":
    case "Set":
      return r;
  }
  return Object.prototype.toString.call(e).slice(8, -1).toLowerCase().replace(/\s/g, "");
}
function tx(e) {
  return typeof e.constructor == "function" ? e.constructor.name : null;
}
function rx(e) {
  return e instanceof Error || typeof e.message == "string" && e.constructor && typeof e.constructor.stackTraceLimit == "number";
}
function nx(e) {
  return e instanceof Date ? !0 : typeof e.toDateString == "function" && typeof e.getDate == "function" && typeof e.setDate == "function";
}
function rr(e) {
  let t = typeof e;
  return process.env.NODE_ENV !== "production" && (t = ex(e)), t;
}
function lh(e, t, r) {
  if (typeof e != "function")
    throw new Error(process.env.NODE_ENV === "production" ? Te(2) : `Expected the root reducer to be a function. Instead, received: '${rr(e)}'`);
  if (typeof t == "function" && typeof r == "function" || typeof r == "function" && typeof arguments[3] == "function")
    throw new Error(process.env.NODE_ENV === "production" ? Te(0) : "It looks like you are passing several store enhancers to createStore(). This is not supported. Instead, compose them together to a single function. See https://redux.js.org/tutorials/fundamentals/part-4-store#creating-a-store-with-enhancers for an example.");
  if (typeof t == "function" && typeof r > "u" && (r = t, t = void 0), typeof r < "u") {
    if (typeof r != "function")
      throw new Error(process.env.NODE_ENV === "production" ? Te(1) : `Expected the enhancer to be a function. Instead, received: '${rr(r)}'`);
    return r(lh)(e, t);
  }
  let n = e, i = t, a = /* @__PURE__ */ new Map(), o = a, u = 0, l = !1;
  function c() {
    o === a && (o = /* @__PURE__ */ new Map(), a.forEach((m, y) => {
      o.set(y, m);
    }));
  }
  function s() {
    if (l)
      throw new Error(process.env.NODE_ENV === "production" ? Te(3) : "You may not call store.getState() while the reducer is executing. The reducer has already received the state as an argument. Pass it down from the top reducer instead of reading it from the store.");
    return i;
  }
  function f(m) {
    if (typeof m != "function")
      throw new Error(process.env.NODE_ENV === "production" ? Te(4) : `Expected the listener to be a function. Instead, received: '${rr(m)}'`);
    if (l)
      throw new Error(process.env.NODE_ENV === "production" ? Te(5) : "You may not call store.subscribe() while the reducer is executing. If you would like to be notified after the store has been updated, subscribe from a component and invoke store.getState() in the callback to access the latest state. See https://redux.js.org/api/store#subscribelistener for more details.");
    let y = !0;
    c();
    const b = u++;
    return o.set(b, m), function() {
      if (y) {
        if (l)
          throw new Error(process.env.NODE_ENV === "production" ? Te(6) : "You may not unsubscribe from a store listener while the reducer is executing. See https://redux.js.org/api/store#subscribelistener for more details.");
        y = !1, c(), o.delete(b), a = null;
      }
    };
  }
  function d(m) {
    if (!Zn(m))
      throw new Error(process.env.NODE_ENV === "production" ? Te(7) : `Actions must be plain objects. Instead, the actual type was: '${rr(m)}'. You may need to add middleware to your store setup to handle dispatching other values, such as 'redux-thunk' to handle dispatching functions. See https://redux.js.org/tutorials/fundamentals/part-4-store#middleware and https://redux.js.org/tutorials/fundamentals/part-6-async-logic#using-the-redux-thunk-middleware for examples.`);
    if (typeof m.type > "u")
      throw new Error(process.env.NODE_ENV === "production" ? Te(8) : 'Actions may not have an undefined "type" property. You may have misspelled an action type string constant.');
    if (typeof m.type != "string")
      throw new Error(process.env.NODE_ENV === "production" ? Te(17) : `Action "type" property must be a string. Instead, the actual type was: '${rr(m.type)}'. Value was: '${m.type}' (stringified)`);
    if (l)
      throw new Error(process.env.NODE_ENV === "production" ? Te(9) : "Reducers may not dispatch actions.");
    try {
      l = !0, i = n(i, m);
    } finally {
      l = !1;
    }
    return (a = o).forEach((b) => {
      b();
    }), m;
  }
  function h(m) {
    if (typeof m != "function")
      throw new Error(process.env.NODE_ENV === "production" ? Te(10) : `Expected the nextReducer to be a function. Instead, received: '${rr(m)}`);
    n = m, d({
      type: Pr.REPLACE
    });
  }
  function p() {
    const m = f;
    return {
      /**
       * The minimal observable subscription method.
       * @param observer Any object that can be used as an observer.
       * The observer object should have a `next` method.
       * @returns An object with an `unsubscribe` method that can
       * be used to unsubscribe the observable from the store, and prevent further
       * emission of values from the observable.
       */
      subscribe(y) {
        if (typeof y != "object" || y === null)
          throw new Error(process.env.NODE_ENV === "production" ? Te(11) : `Expected the observer to be an object. Instead, received: '${rr(y)}'`);
        function b() {
          const w = y;
          w.next && w.next(s());
        }
        return b(), {
          unsubscribe: m(b)
        };
      },
      [Lc]() {
        return this;
      }
    };
  }
  return d({
    type: Pr.INIT
  }), {
    dispatch: d,
    subscribe: f,
    getState: s,
    replaceReducer: h,
    [Lc]: p
  };
}
function zc(e) {
  typeof console < "u" && typeof console.error == "function" && console.error(e);
  try {
    throw new Error(e);
  } catch {
  }
}
function ix(e, t, r, n) {
  const i = Object.keys(t), a = r && r.type === Pr.INIT ? "preloadedState argument passed to createStore" : "previous state received by the reducer";
  if (i.length === 0)
    return "Store does not have a valid reducer. Make sure the argument passed to combineReducers is an object whose values are reducers.";
  if (!Zn(e))
    return `The ${a} has unexpected type of "${rr(e)}". Expected argument to be an object with the following keys: "${i.join('", "')}"`;
  const o = Object.keys(e).filter((u) => !t.hasOwnProperty(u) && !n[u]);
  if (o.forEach((u) => {
    n[u] = !0;
  }), !(r && r.type === Pr.REPLACE) && o.length > 0)
    return `Unexpected ${o.length > 1 ? "keys" : "key"} "${o.join('", "')}" found in ${a}. Expected to find one of the known reducer keys instead: "${i.join('", "')}". Unexpected keys will be ignored.`;
}
function ax(e) {
  Object.keys(e).forEach((t) => {
    const r = e[t];
    if (typeof r(void 0, {
      type: Pr.INIT
    }) > "u")
      throw new Error(process.env.NODE_ENV === "production" ? Te(12) : `The slice reducer for key "${t}" returned undefined during initialization. If the state passed to the reducer is undefined, you must explicitly return the initial state. The initial state may not be undefined. If you don't want to set a value for this reducer, you can use null instead of undefined.`);
    if (typeof r(void 0, {
      type: Pr.PROBE_UNKNOWN_ACTION()
    }) > "u")
      throw new Error(process.env.NODE_ENV === "production" ? Te(13) : `The slice reducer for key "${t}" returned undefined when probed with a random type. Don't try to handle '${Pr.INIT}' or other actions in "redux/*" namespace. They are considered private. Instead, you must return the current state for any unknown actions, unless it is undefined, in which case you must return the initial state, regardless of the action type. The initial state may not be undefined, but can be null.`);
  });
}
function ch(e) {
  const t = Object.keys(e), r = {};
  for (let o = 0; o < t.length; o++) {
    const u = t[o];
    process.env.NODE_ENV !== "production" && typeof e[u] > "u" && zc(`No reducer provided for key "${u}"`), typeof e[u] == "function" && (r[u] = e[u]);
  }
  const n = Object.keys(r);
  let i;
  process.env.NODE_ENV !== "production" && (i = {});
  let a;
  try {
    ax(r);
  } catch (o) {
    a = o;
  }
  return function(u = {}, l) {
    if (a)
      throw a;
    if (process.env.NODE_ENV !== "production") {
      const f = ix(u, r, l, i);
      f && zc(f);
    }
    let c = !1;
    const s = {};
    for (let f = 0; f < n.length; f++) {
      const d = n[f], h = r[d], p = u[d], v = h(p, l);
      if (typeof v > "u") {
        const m = l && l.type;
        throw new Error(process.env.NODE_ENV === "production" ? Te(14) : `When called with an action of type ${m ? `"${String(m)}"` : "(unknown type)"}, the slice reducer for key "${d}" returned undefined. To ignore an action, you must explicitly return the previous state. If you want this reducer to hold no value, you can return null instead of undefined.`);
      }
      s[d] = v, c = c || v !== p;
    }
    return c = c || n.length !== Object.keys(u).length, c ? s : u;
  };
}
function Ki(...e) {
  return e.length === 0 ? (t) => t : e.length === 1 ? e[0] : e.reduce((t, r) => (...n) => t(r(...n)));
}
function ox(...e) {
  return (t) => (r, n) => {
    const i = t(r, n);
    let a = () => {
      throw new Error(process.env.NODE_ENV === "production" ? Te(15) : "Dispatching while constructing your middleware is not allowed. Other middleware would not be applied to this dispatch.");
    };
    const o = {
      getState: i.getState,
      dispatch: (l, ...c) => a(l, ...c)
    }, u = e.map((l) => l(o));
    return a = Ki(...u)(i.dispatch), {
      ...i,
      dispatch: a
    };
  };
}
function Xu(e) {
  return Zn(e) && "type" in e && typeof e.type == "string";
}
var sh = Symbol.for("immer-nothing"), Bc = Symbol.for("immer-draftable"), Ke = Symbol.for("immer-state"), ux = process.env.NODE_ENV !== "production" ? [
  // All error codes, starting by 0:
  function(e) {
    return `The plugin for '${e}' has not been loaded into Immer. To enable the plugin, import and call \`enable${e}()\` when initializing your application.`;
  },
  function(e) {
    return `produce can only be called on things that are draftable: plain objects, arrays, Map, Set or classes that are marked with '[immerable]: true'. Got '${e}'`;
  },
  "This object has been frozen and should not be mutated",
  function(e) {
    return "Cannot use a proxy that has been revoked. Did you pass an object from inside an immer function to an async process? " + e;
  },
  "An immer producer returned a new value *and* modified its draft. Either return a new value *or* modify the draft.",
  "Immer forbids circular references",
  "The first or second argument to `produce` must be a function",
  "The third argument to `produce` must be a function or undefined",
  "First argument to `createDraft` must be a plain object, an array, or an immerable object",
  "First argument to `finishDraft` must be a draft returned by `createDraft`",
  function(e) {
    return `'current' expects a draft, got: ${e}`;
  },
  "Object.defineProperty() cannot be used on an Immer draft",
  "Object.setPrototypeOf() cannot be used on an Immer draft",
  "Immer only supports deleting array indices",
  "Immer only supports setting array indices and the 'length' property",
  function(e) {
    return `'original' expects a draft, got: ${e}`;
  }
  // Note: if more errors are added, the errorOffset in Patches.ts should be increased
  // See Patches.ts for additional errors
] : [];
function et(e, ...t) {
  if (process.env.NODE_ENV !== "production") {
    const r = ux[e], n = wr(r) ? r.apply(null, t) : r;
    throw new Error(`[Immer] ${n}`);
  }
  throw new Error(
    `[Immer] minified error nr: ${e}. Full error at: https://bit.ly/3cXEKWf`
  );
}
var rt = Object, en = rt.getPrototypeOf, Hi = "constructor", Na = "prototype", iu = "configurable", Yi = "enumerable", Li = "writable", Bn = "value", Vt = (e) => !!e && !!e[Ke];
function gt(e) {
  var t;
  return e ? fh(e) || $a(e) || !!e[Bc] || !!((t = e[Hi]) != null && t[Bc]) || Ra(e) || La(e) : !1;
}
var lx = rt[Na][Hi].toString(), Fc = /* @__PURE__ */ new WeakMap();
function fh(e) {
  if (!e || !Zu(e))
    return !1;
  const t = en(e);
  if (t === null || t === rt[Na])
    return !0;
  const r = rt.hasOwnProperty.call(t, Hi) && t[Hi];
  if (r === Object)
    return !0;
  if (!wr(r))
    return !1;
  let n = Fc.get(r);
  return n === void 0 && (n = Function.toString.call(r), Fc.set(r, n)), n === lx;
}
function ja(e, t, r = !0) {
  Qn(e) === 0 ? (r ? Reflect.ownKeys(e) : rt.keys(e)).forEach((i) => {
    t(i, e[i], e);
  }) : e.forEach((n, i) => t(i, n, e));
}
function Qn(e) {
  const t = e[Ke];
  return t ? t.type_ : $a(e) ? 1 : Ra(e) ? 2 : La(e) ? 3 : 0;
}
var Ao = (e, t, r = Qn(e)) => r === 2 ? e.has(t) : rt[Na].hasOwnProperty.call(e, t), au = (e, t, r = Qn(e)) => (
  // @ts-ignore
  r === 2 ? e.get(t) : e[t]
), Gi = (e, t, r, n = Qn(e)) => {
  n === 2 ? e.set(t, r) : n === 3 ? e.add(r) : e[t] = r;
};
function cx(e, t) {
  return e === t ? e !== 0 || 1 / e === 1 / t : e !== e && t !== t;
}
var $a = Array.isArray, Ra = (e) => e instanceof Map, La = (e) => e instanceof Set, Zu = (e) => typeof e == "object", wr = (e) => typeof e == "function", Oo = (e) => typeof e == "boolean";
function sx(e) {
  const t = +e;
  return Number.isInteger(t) && String(t) === e;
}
var Et = (e) => e.copy_ || e.base_, Qu = (e) => e.modified_ ? e.copy_ : e.base_;
function ou(e, t) {
  if (Ra(e))
    return new Map(e);
  if (La(e))
    return new Set(e);
  if ($a(e))
    return Array[Na].slice.call(e);
  const r = fh(e);
  if (t === !0 || t === "class_only" && !r) {
    const n = rt.getOwnPropertyDescriptors(e);
    delete n[Ke];
    let i = Reflect.ownKeys(n);
    for (let a = 0; a < i.length; a++) {
      const o = i[a], u = n[o];
      u[Li] === !1 && (u[Li] = !0, u[iu] = !0), (u.get || u.set) && (n[o] = {
        [iu]: !0,
        [Li]: !0,
        // could live with !!desc.set as well here...
        [Yi]: u[Yi],
        [Bn]: e[o]
      });
    }
    return rt.create(en(e), n);
  } else {
    const n = en(e);
    if (n !== null && r)
      return { ...e };
    const i = rt.create(n);
    return rt.assign(i, e);
  }
}
function Ju(e, t = !1) {
  return za(e) || Vt(e) || !gt(e) || (Qn(e) > 1 && rt.defineProperties(e, {
    set: Ai,
    add: Ai,
    clear: Ai,
    delete: Ai
  }), rt.freeze(e), t && ja(
    e,
    (r, n) => {
      Ju(n, !0);
    },
    !1
  )), e;
}
function fx() {
  et(2);
}
var Ai = {
  [Bn]: fx
};
function za(e) {
  return e === null || !Zu(e) ? !0 : rt.isFrozen(e);
}
var qi = "MapSet", uu = "Patches", Wc = "ArrayMethods", dh = {};
function Dr(e) {
  const t = dh[e];
  return t || et(0, e), t;
}
var Uc = (e) => !!dh[e], Fn, vh = () => Fn, dx = (e, t) => ({
  drafts_: [],
  parent_: e,
  immer_: t,
  // Whenever the modified draft contains a draft from another scope, we
  // need to prevent auto-freezing so the unowned draft can be finalized.
  canAutoFreeze_: !0,
  unfinalizedDrafts_: 0,
  handledSet_: /* @__PURE__ */ new Set(),
  processedForPatches_: /* @__PURE__ */ new Set(),
  mapSetPlugin_: Uc(qi) ? Dr(qi) : void 0,
  arrayMethodsPlugin_: Uc(Wc) ? Dr(Wc) : void 0
});
function Vc(e, t) {
  t && (e.patchPlugin_ = Dr(uu), e.patches_ = [], e.inversePatches_ = [], e.patchListener_ = t);
}
function lu(e) {
  cu(e), e.drafts_.forEach(vx), e.drafts_ = null;
}
function cu(e) {
  e === Fn && (Fn = e.parent_);
}
var Kc = (e) => Fn = dx(Fn, e);
function vx(e) {
  const t = e[Ke];
  t.type_ === 0 || t.type_ === 1 ? t.revoke_() : t.revoked_ = !0;
}
function Hc(e, t) {
  t.unfinalizedDrafts_ = t.drafts_.length;
  const r = t.drafts_[0];
  if (e !== void 0 && e !== r) {
    r[Ke].modified_ && (lu(t), et(4)), gt(e) && (e = Yc(t, e));
    const { patchPlugin_: i } = t;
    i && i.generateReplacementPatches_(
      r[Ke].base_,
      e,
      t
    );
  } else
    e = Yc(t, r);
  return hx(t, e, !0), lu(t), t.patches_ && t.patchListener_(t.patches_, t.inversePatches_), e !== sh ? e : void 0;
}
function Yc(e, t) {
  if (za(t))
    return t;
  const r = t[Ke];
  if (!r)
    return Xi(t, e.handledSet_, e);
  if (!Ba(r, e))
    return t;
  if (!r.modified_)
    return r.base_;
  if (!r.finalized_) {
    const { callbacks_: n } = r;
    if (n)
      for (; n.length > 0; )
        n.pop()(e);
    mh(r, e);
  }
  return r.copy_;
}
function hx(e, t, r = !1) {
  !e.parent_ && e.immer_.autoFreeze_ && e.canAutoFreeze_ && Ju(t, r);
}
function hh(e) {
  e.finalized_ = !0, e.scope_.unfinalizedDrafts_--;
}
var Ba = (e, t) => e.scope_ === t, px = [];
function ph(e, t, r, n) {
  const i = Et(e), a = e.type_;
  if (n !== void 0 && au(i, n, a) === t) {
    Gi(i, n, r, a);
    return;
  }
  if (!e.draftLocations_) {
    const u = e.draftLocations_ = /* @__PURE__ */ new Map();
    ja(i, (l, c) => {
      if (Vt(c)) {
        const s = u.get(c) || [];
        s.push(l), u.set(c, s);
      }
    });
  }
  const o = e.draftLocations_.get(t) ?? px;
  for (const u of o)
    Gi(i, u, r, a);
}
function mx(e, t, r) {
  e.callbacks_.push(function(i) {
    var u;
    const a = t;
    if (!a || !Ba(a, i))
      return;
    (u = i.mapSetPlugin_) == null || u.fixSetContents(a);
    const o = Qu(a);
    ph(e, a.draft_ ?? a, o, r), mh(a, i);
  });
}
function mh(e, t) {
  var n;
  if (e.modified_ && !e.finalized_ && (e.type_ === 3 || e.type_ === 1 && e.allIndicesReassigned_ || (((n = e.assigned_) == null ? void 0 : n.size) ?? 0) > 0)) {
    const { patchPlugin_: i } = t;
    if (i) {
      const a = i.getPath(e);
      a && i.generatePatches_(e, a, t);
    }
    hh(e);
  }
}
function yx(e, t, r) {
  const { scope_: n } = e;
  if (Vt(r)) {
    const i = r[Ke];
    Ba(i, n) && i.callbacks_.push(function() {
      zi(e);
      const o = Qu(i);
      ph(e, r, o, t);
    });
  } else gt(r) && e.callbacks_.push(function() {
    const a = Et(e);
    e.type_ === 3 ? a.has(r) && Xi(r, n.handledSet_, n) : au(a, t, e.type_) === r && n.drafts_.length > 1 && (e.assigned_.get(t) ?? !1) === !0 && e.copy_ && Xi(
      au(e.copy_, t, e.type_),
      n.handledSet_,
      n
    );
  });
}
function Xi(e, t, r) {
  return !r.immer_.autoFreeze_ && r.unfinalizedDrafts_ < 1 || Vt(e) || t.has(e) || !gt(e) || za(e) || (t.add(e), ja(e, (n, i) => {
    if (Vt(i)) {
      const a = i[Ke];
      if (Ba(a, r)) {
        const o = Qu(a);
        Gi(e, n, o, e.type_), hh(a);
      }
    } else gt(i) && Xi(i, t, r);
  })), e;
}
function gx(e, t) {
  const r = $a(e), n = {
    type_: r ? 1 : 0,
    // Track which produce call this is associated with.
    scope_: t ? t.scope_ : vh(),
    // True for both shallow and deep changes.
    modified_: !1,
    // Used during finalization.
    finalized_: !1,
    // Track which properties have been assigned (true) or deleted (false).
    // actually instantiated in `prepareCopy()`
    assigned_: void 0,
    // The parent draft state.
    parent_: t,
    // The base state.
    base_: e,
    // The base proxy.
    draft_: null,
    // set below
    // The base copy with any updated values.
    copy_: null,
    // Called by the `produce` function.
    revoke_: null,
    isManual_: !1,
    // `callbacks` actually gets assigned in `createProxy`
    callbacks_: void 0
  };
  let i = n, a = Zi;
  r && (i = [n], a = Wn);
  const { revoke: o, proxy: u } = Proxy.revocable(i, a);
  return n.draft_ = u, n.revoke_ = o, [u, n];
}
var Zi = {
  get(e, t) {
    if (t === Ke)
      return e;
    if (t === "constructor" || t === "__proto__") {
      const u = Et(e)[t];
      return new Proxy(u || {}, {
        get: (l, c) => c === "__proto__" || c === "prototype" ? Object.freeze(/* @__PURE__ */ Object.create(null)) : Reflect.get(l, c),
        set: () => !0,
        apply: (l, c, s) => Reflect.apply(l, c, s)
      });
    }
    let r = e.scope_.arrayMethodsPlugin_;
    const n = e.type_ === 1 && typeof t == "string";
    if (n && r != null && r.isArrayOperationMethod(t))
      return r.createMethodInterceptor(e, t);
    const i = Et(e);
    if (!Ao(i, t, e.type_))
      return bx(e, i, t);
    const a = i[t];
    if (e.finalized_ || !gt(a) || n && e.operationMethod && (r != null && r.isMutatingArrayMethod(
      e.operationMethod
    )) && sx(t))
      return a;
    if (a === Eo(e.base_, t)) {
      zi(e);
      const o = e.type_ === 1 ? +t : t, u = fu(e.scope_, a, e, o);
      return e.copy_[o] = u;
    }
    return a;
  },
  has(e, t) {
    return t === "constructor" || t === "__proto__" || t === "prototype" ? !1 : t in Et(e);
  },
  ownKeys(e) {
    return Reflect.ownKeys(Et(e));
  },
  set(e, t, r) {
    if (t === "constructor" || t === "__proto__" || t === "prototype")
      return !0;
    const n = yh(Et(e), t);
    if (n != null && n.set)
      return n.set.call(e.draft_, r), !0;
    if (!e.modified_) {
      const i = Eo(Et(e), t), a = i == null ? void 0 : i[Ke];
      if (a && a.base_ === r)
        return e.copy_[t] = r, e.assigned_.set(t, !1), !0;
      if (cx(r, i) && (r !== void 0 || Ao(e.base_, t, e.type_)))
        return !0;
      zi(e), su(e);
    }
    return e.copy_[t] === r && // special case: handle new props with value 'undefined'
    (r !== void 0 || Ao(e.copy_, t, e.type_)) || // special case: NaN
    Number.isNaN(r) && Number.isNaN(e.copy_[t]) || (e.copy_[t] = r, e.assigned_.set(t, !0), yx(e, t, r)), !0;
  },
  deleteProperty(e, t) {
    return zi(e), Eo(e.base_, t) !== void 0 || t in e.base_ ? (e.assigned_.set(t, !1), su(e)) : e.assigned_.delete(t), e.copy_ && delete e.copy_[t], !0;
  },
  // Note: We never coerce `desc.value` into an Immer draft, because we can't make
  // the same guarantee in ES5 mode.
  getOwnPropertyDescriptor(e, t) {
    const r = Et(e), n = Reflect.getOwnPropertyDescriptor(r, t);
    return n && {
      [Li]: !0,
      [iu]: e.type_ !== 1 || t !== "length",
      [Yi]: n[Yi],
      [Bn]: r[t]
    };
  },
  defineProperty() {
    et(11);
  },
  getPrototypeOf(e) {
    return en(e.base_);
  },
  setPrototypeOf() {
    et(12);
  }
}, Wn = {};
for (let e in Zi) {
  let t = Zi[e];
  Wn[e] = function() {
    const r = arguments;
    return r[0] = r[0][0], t.apply(this, r);
  };
}
Wn.deleteProperty = function(e, t) {
  return process.env.NODE_ENV !== "production" && isNaN(parseInt(t)) && et(13), Wn.set.call(this, e, t, void 0);
};
Wn.set = function(e, t, r) {
  return process.env.NODE_ENV !== "production" && t !== "length" && isNaN(parseInt(t)) && et(14), Zi.set.call(this, e[0], t, r, e[0]);
};
function Eo(e, t) {
  const r = e[Ke];
  return (r ? Et(r) : e)[t];
}
function bx(e, t, r) {
  var i;
  const n = yh(t, r);
  return n ? Bn in n ? n[Bn] : (
    // This is a very special case, if the prop is a getter defined by the
    // prototype, we should invoke it with the draft as context!
    (i = n.get) == null ? void 0 : i.call(e.draft_)
  ) : void 0;
}
function yh(e, t) {
  if (!(t in e))
    return;
  let r = en(e);
  for (; r; ) {
    const n = Object.getOwnPropertyDescriptor(r, t);
    if (n)
      return n;
    r = en(r);
  }
}
function su(e) {
  e.modified_ || (e.modified_ = !0, e.parent_ && su(e.parent_));
}
function zi(e) {
  e.copy_ || (e.assigned_ = /* @__PURE__ */ new Map(), e.copy_ = ou(
    e.base_,
    e.scope_.immer_.useStrictShallowCopy_
  ));
}
var xx = class {
  constructor(e) {
    this.autoFreeze_ = !0, this.useStrictShallowCopy_ = !1, this.useStrictIteration_ = !1, this.produce = (t, r, n) => {
      if (wr(t) && !wr(r)) {
        const a = r;
        r = t;
        const o = this;
        return function(l = a, ...c) {
          return o.produce(l, (s) => r.call(this, s, ...c));
        };
      }
      wr(r) || et(6), n !== void 0 && !wr(n) && et(7);
      let i;
      if (gt(t)) {
        const a = Kc(this), o = fu(a, t, void 0);
        let u = !0;
        try {
          i = r(o), u = !1;
        } finally {
          u ? lu(a) : cu(a);
        }
        return Vc(a, n), Hc(i, a);
      } else if (!t || !Zu(t)) {
        if (i = r(t), i === void 0 && (i = t), i === sh && (i = void 0), this.autoFreeze_ && Ju(i, !0), n) {
          const a = [], o = [];
          Dr(uu).generateReplacementPatches_(t, i, {
            patches_: a,
            inversePatches_: o
          }), n(a, o);
        }
        return i;
      } else
        et(1, t);
    }, this.produceWithPatches = (t, r) => {
      if (wr(t))
        return (o, ...u) => this.produceWithPatches(o, (l) => t(l, ...u));
      let n, i;
      return [this.produce(t, r, (o, u) => {
        n = o, i = u;
      }), n, i];
    }, Oo(e == null ? void 0 : e.autoFreeze) && this.setAutoFreeze(e.autoFreeze), Oo(e == null ? void 0 : e.useStrictShallowCopy) && this.setUseStrictShallowCopy(e.useStrictShallowCopy), Oo(e == null ? void 0 : e.useStrictIteration) && this.setUseStrictIteration(e.useStrictIteration);
  }
  createDraft(e) {
    gt(e) || et(8), Vt(e) && (e = ot(e));
    const t = Kc(this), r = fu(t, e, void 0);
    return r[Ke].isManual_ = !0, cu(t), r;
  }
  finishDraft(e, t) {
    const r = e && e[Ke];
    (!r || !r.isManual_) && et(9);
    const { scope_: n } = r;
    return Vc(n, t), Hc(void 0, n);
  }
  /**
   * Pass true to automatically freeze all copies created by Immer.
   *
   * By default, auto-freezing is enabled.
   */
  setAutoFreeze(e) {
    this.autoFreeze_ = e;
  }
  /**
   * Pass true to enable strict shallow copy.
   *
   * By default, immer does not copy the object descriptors such as getter, setter and non-enumrable properties.
   */
  setUseStrictShallowCopy(e) {
    this.useStrictShallowCopy_ = e;
  }
  /**
   * Pass false to use faster iteration that skips non-enumerable properties
   * but still handles symbols for compatibility.
   *
   * By default, strict iteration is enabled (includes all own properties).
   */
  setUseStrictIteration(e) {
    this.useStrictIteration_ = e;
  }
  shouldUseStrictIteration() {
    return this.useStrictIteration_;
  }
  applyPatches(e, t) {
    let r;
    for (r = t.length - 1; r >= 0; r--) {
      const i = t[r];
      if (i.path.length === 0 && i.op === "replace") {
        e = i.value;
        break;
      }
    }
    r > -1 && (t = t.slice(r + 1));
    const n = Dr(uu).applyPatches_;
    return Vt(e) ? n(e, t) : this.produce(
      e,
      (i) => n(i, t)
    );
  }
};
function fu(e, t, r, n) {
  const [i, a] = Ra(t) ? Dr(qi).proxyMap_(t, r) : La(t) ? Dr(qi).proxySet_(t, r) : gx(t, r);
  return ((r == null ? void 0 : r.scope_) ?? vh()).drafts_.push(i), a.callbacks_ = (r == null ? void 0 : r.callbacks_) ?? [], a.key_ = n, r && n !== void 0 ? mx(r, a, n) : a.callbacks_.push(function(l) {
    var s;
    (s = l.mapSetPlugin_) == null || s.fixSetContents(a);
    const { patchPlugin_: c } = l;
    a.modified_ && c && c.generatePatches_(a, [], l);
  }), i;
}
function ot(e) {
  return Vt(e) || et(10, e), gh(e);
}
function gh(e) {
  if (!gt(e) || za(e))
    return e;
  const t = e[Ke];
  let r, n = !0;
  if (t) {
    if (!t.modified_)
      return t.base_;
    t.finalized_ = !0, r = ou(e, t.scope_.immer_.useStrictShallowCopy_), n = t.scope_.immer_.shouldUseStrictIteration();
  } else
    r = ou(e, !0);
  return ja(
    r,
    (i, a) => {
      Gi(r, i, gh(a));
    },
    n
  ), t && (t.finalized_ = !1), r;
}
var wx = new xx(), bh = wx.produce, q = (e) => e;
function xh(e) {
  return ({ dispatch: r, getState: n }) => (i) => (a) => typeof a == "function" ? a(r, n, e) : i(a);
}
var Ax = xh(), Ox = xh, Ex = typeof window < "u" && window.__REDUX_DEVTOOLS_EXTENSION_COMPOSE__ ? window.__REDUX_DEVTOOLS_EXTENSION_COMPOSE__ : function() {
  if (arguments.length !== 0)
    return typeof arguments[0] == "object" ? Ki : Ki.apply(null, arguments);
}, Sx = (e) => e && typeof e.match == "function";
function nt(e, t) {
  function r(...n) {
    if (t) {
      let i = t(...n);
      if (!i)
        throw new Error(process.env.NODE_ENV === "production" ? Z(0) : "prepareAction did not return an object");
      return {
        type: e,
        payload: i.payload,
        ..."meta" in i && {
          meta: i.meta
        },
        ..."error" in i && {
          error: i.error
        }
      };
    }
    return {
      type: e,
      payload: n[0]
    };
  }
  return r.toString = () => `${e}`, r.type = e, r.match = (n) => Xu(n) && n.type === e, r;
}
function _x(e) {
  return typeof e == "function" && "type" in e && // hasMatchFunction only wants Matchers but I don't see the point in rewriting it
  Sx(e);
}
function Px(e) {
  const t = e ? `${e}`.split("/") : [], r = t[t.length - 1] || "actionCreator";
  return `Detected an action creator with type "${e || "unknown"}" being dispatched.
Make sure you're calling the action creator before dispatching, i.e. \`dispatch(${r}())\` instead of \`dispatch(${r})\`. This is necessary even if the action has no payload.`;
}
function Ix(e = {}) {
  if (process.env.NODE_ENV === "production")
    return () => (r) => (n) => r(n);
  const {
    isActionCreator: t = _x
  } = e;
  return () => (r) => (n) => (t(n) && console.warn(Px(n.type)), r(n));
}
function wh(e, t) {
  let r = 0;
  return {
    measureTime(n) {
      const i = Date.now();
      try {
        return n();
      } finally {
        const a = Date.now();
        r += a - i;
      }
    },
    warnIfExceeded() {
      r > e && console.warn(`${t} took ${r}ms, which is more than the warning threshold of ${e}ms. 
If your state or actions are very large, you may want to disable the middleware as it might cause too much of a slowdown in development mode. See https://redux-toolkit.js.org/api/getDefaultMiddleware for instructions.
It is disabled in production builds, so you don't need to worry about that.`);
    }
  };
}
var Ah = class Nn extends Array {
  constructor(...t) {
    super(...t), Object.setPrototypeOf(this, Nn.prototype);
  }
  static get [Symbol.species]() {
    return Nn;
  }
  concat(...t) {
    return super.concat.apply(this, t);
  }
  prepend(...t) {
    return t.length === 1 && Array.isArray(t[0]) ? new Nn(...t[0].concat(this)) : new Nn(...t.concat(this));
  }
};
function Gc(e) {
  return gt(e) ? bh(e, () => {
  }) : e;
}
function Oi(e, t, r) {
  return e.has(t) ? e.get(t) : e.set(t, r(t)).get(t);
}
function kx(e) {
  return typeof e != "object" || e == null || Object.isFrozen(e);
}
function Cx(e, t, r) {
  const n = Oh(e, t, r);
  return {
    detectMutations() {
      return Eh(e, t, n, r);
    }
  };
}
function Oh(e, t = [], r, n = "", i = /* @__PURE__ */ new Set()) {
  const a = {
    value: r
  };
  if (!e(r) && !i.has(r)) {
    i.add(r), a.children = {};
    const o = t.length > 0;
    for (const u in r) {
      const l = n ? n + "." + u : u;
      o && t.some((s) => s instanceof RegExp ? s.test(l) : l === s) || (a.children[u] = Oh(e, t, r[u], l));
    }
  }
  return a;
}
function Eh(e, t = [], r, n, i = !1, a = "") {
  const o = r ? r.value : void 0, u = o === n;
  if (i && !u && !Number.isNaN(n))
    return {
      wasMutated: !0,
      path: a
    };
  if (e(o) || e(n))
    return {
      wasMutated: !1
    };
  const l = {};
  for (let s in r.children)
    l[s] = !0;
  for (let s in n)
    l[s] = !0;
  const c = t.length > 0;
  for (let s in l) {
    const f = a ? a + "." + s : s;
    if (c && t.some((p) => p instanceof RegExp ? p.test(f) : f === p))
      continue;
    const d = Eh(e, t, r.children[s], n[s], u, f);
    if (d.wasMutated)
      return d;
  }
  return {
    wasMutated: !1
  };
}
function Tx(e = {}) {
  if (process.env.NODE_ENV === "production")
    return () => (t) => (r) => t(r);
  {
    let t = function(u, l, c, s) {
      return JSON.stringify(u, r(l, s), c);
    }, r = function(u, l) {
      let c = [], s = [];
      return l || (l = function(f, d) {
        return c[0] === d ? "[Circular ~]" : "[Circular ~." + s.slice(0, c.indexOf(d)).join(".") + "]";
      }), function(f, d) {
        if (c.length > 0) {
          var h = c.indexOf(this);
          ~h ? c.splice(h + 1) : c.push(this), ~h ? s.splice(h, 1 / 0, f) : s.push(f), ~c.indexOf(d) && (d = l.call(this, f, d));
        } else c.push(d);
        return u == null ? d : u.call(this, f, d);
      };
    }, {
      isImmutable: n = kx,
      ignoredPaths: i,
      warnAfter: a = 32
    } = e;
    const o = Cx.bind(null, n, i);
    return ({
      getState: u
    }) => {
      let l = u(), c = o(l), s;
      return (f) => (d) => {
        const h = wh(a, "ImmutableStateInvariantMiddleware");
        h.measureTime(() => {
          if (l = u(), s = c.detectMutations(), c = o(l), s.wasMutated)
            throw new Error(process.env.NODE_ENV === "production" ? Z(19) : `A state mutation was detected between dispatches, in the path '${s.path || ""}'.  This may cause incorrect behavior. (https://redux.js.org/style-guide/style-guide#do-not-mutate-state)`);
        });
        const p = f(d);
        return h.measureTime(() => {
          if (l = u(), s = c.detectMutations(), c = o(l), s.wasMutated)
            throw new Error(process.env.NODE_ENV === "production" ? Z(20) : `A state mutation was detected inside a dispatch, in the path: ${s.path || ""}. Take a look at the reducer(s) handling the action ${t(d)}. (https://redux.js.org/style-guide/style-guide#do-not-mutate-state)`);
        }), h.warnIfExceeded(), p;
      };
    };
  }
}
function Sh(e) {
  const t = typeof e;
  return e == null || t === "string" || t === "boolean" || t === "number" || Array.isArray(e) || Zn(e);
}
function du(e, t = "", r = Sh, n, i = [], a) {
  let o;
  if (!r(e))
    return {
      keyPath: t || "<root>",
      value: e
    };
  if (typeof e != "object" || e === null || a != null && a.has(e)) return !1;
  const u = n != null ? n(e) : Object.entries(e), l = i.length > 0;
  for (const [c, s] of u) {
    const f = t ? t + "." + c : c;
    if (!(l && i.some((h) => h instanceof RegExp ? h.test(f) : f === h))) {
      if (!r(s))
        return {
          keyPath: f,
          value: s
        };
      if (typeof s == "object" && (o = du(s, f, r, n, i, a), o))
        return o;
    }
  }
  return a && _h(e) && a.add(e), !1;
}
function _h(e) {
  if (!Object.isFrozen(e)) return !1;
  for (const t of Object.values(e))
    if (!(typeof t != "object" || t === null) && !_h(t))
      return !1;
  return !0;
}
function Mx(e = {}) {
  if (process.env.NODE_ENV === "production")
    return () => (t) => (r) => t(r);
  {
    const {
      isSerializable: t = Sh,
      getEntries: r,
      ignoredActions: n = [],
      ignoredActionPaths: i = ["meta.arg", "meta.baseQueryMeta"],
      ignoredPaths: a = [],
      warnAfter: o = 32,
      ignoreState: u = !1,
      ignoreActions: l = !1,
      disableCache: c = !1
    } = e, s = !c && WeakSet ? /* @__PURE__ */ new WeakSet() : void 0;
    return (f) => (d) => (h) => {
      if (!Xu(h))
        return d(h);
      const p = d(h), v = wh(o, "SerializableStateInvariantMiddleware");
      return !l && !(n.length && n.indexOf(h.type) !== -1) && v.measureTime(() => {
        const m = du(h, "", t, r, i, s);
        if (m) {
          const {
            keyPath: y,
            value: b
          } = m;
          console.error(`A non-serializable value was detected in an action, in the path: \`${y}\`. Value:`, b, `
Take a look at the logic that dispatched this action: `, h, `
(See https://redux.js.org/faq/actions#why-should-type-be-a-string-or-at-least-serializable-why-should-my-action-types-be-constants)`, `
(To allow non-serializable values see: https://redux-toolkit.js.org/usage/usage-guide#working-with-non-serializable-data)`);
        }
      }), u || (v.measureTime(() => {
        const m = f.getState(), y = du(m, "", t, r, a, s);
        if (y) {
          const {
            keyPath: b,
            value: x
          } = y;
          console.error(`A non-serializable value was detected in the state, in the path: \`${b}\`. Value:`, x, `
Take a look at the reducer(s) handling this action type: ${h.type}.
(See https://redux.js.org/faq/organizing-state#can-i-put-functions-promises-or-other-non-serializable-items-in-my-store-state)`);
        }
      }), v.warnIfExceeded()), p;
    };
  }
}
function Ei(e) {
  return typeof e == "boolean";
}
var Dx = () => function(t) {
  const {
    thunk: r = !0,
    immutableCheck: n = !0,
    serializableCheck: i = !0,
    actionCreatorCheck: a = !0
  } = t ?? {};
  let o = new Ah();
  if (r && (Ei(r) ? o.push(Ax) : o.push(Ox(r.extraArgument))), process.env.NODE_ENV !== "production") {
    if (n) {
      let u = {};
      Ei(n) || (u = n), o.unshift(Tx(u));
    }
    if (i) {
      let u = {};
      Ei(i) || (u = i), o.push(Mx(u));
    }
    if (a) {
      let u = {};
      Ei(a) || (u = a), o.unshift(Ix(u));
    }
  }
  return o;
}, Ph = "RTK_autoBatch", ae = () => (e) => ({
  payload: e,
  meta: {
    [Ph]: !0
  }
}), qc = (e) => (t) => {
  setTimeout(t, e);
}, Nx = (e, t) => (r) => {
  let n = !1;
  const i = () => {
    n || (n = !0, cancelAnimationFrame(a), clearTimeout(o), r());
  }, a = e(i), o = setTimeout(i, t);
}, Ih = (e = {
  type: "raf"
}) => (t) => (...r) => {
  const n = t(...r);
  let i = !0, a = !1, o = !1;
  const u = /* @__PURE__ */ new Set(), l = e.type === "tick" ? queueMicrotask : e.type === "raf" ? (
    // requestAnimationFrame won't exist in SSR environments. Fall back to a vague approximation just to keep from erroring.
    typeof window < "u" && window.requestAnimationFrame ? Nx(window.requestAnimationFrame, 100) : qc(10)
  ) : e.type === "callback" ? e.queueNotification : qc(e.timeout), c = () => {
    o = !1, a && (a = !1, u.forEach((s) => s()));
  };
  return Object.assign({}, n, {
    // Override the base `store.subscribe` method to keep original listeners
    // from running if we're delaying notifications
    subscribe(s) {
      const f = () => i && s(), d = n.subscribe(f);
      return u.add(s), () => {
        d(), u.delete(s);
      };
    },
    // Override the base `store.dispatch` method so that we can check actions
    // for the `shouldAutoBatch` flag and determine if batching is active
    dispatch(s) {
      var f;
      try {
        return i = !((f = s == null ? void 0 : s.meta) != null && f[Ph]), a = !i, a && (o || (o = !0, l(c))), n.dispatch(s);
      } finally {
        i = !0;
      }
    }
  });
}, jx = (e) => function(r) {
  const {
    autoBatch: n = !0
  } = r ?? {};
  let i = new Ah(e);
  return n && i.push(Ih(typeof n == "object" ? n : void 0)), i;
};
function $x(e) {
  const t = Dx(), {
    reducer: r = void 0,
    middleware: n,
    devTools: i = !0,
    duplicateMiddlewareCheck: a = !0,
    preloadedState: o = void 0,
    enhancers: u = void 0
  } = e || {};
  let l;
  if (typeof r == "function")
    l = r;
  else if (Zn(r))
    l = ch(r);
  else
    throw new Error(process.env.NODE_ENV === "production" ? Z(1) : "`reducer` is a required argument, and must be a function or an object of functions that can be passed to combineReducers");
  if (process.env.NODE_ENV !== "production" && n && typeof n != "function")
    throw new Error(process.env.NODE_ENV === "production" ? Z(2) : "`middleware` field must be a callback");
  let c;
  if (typeof n == "function") {
    if (c = n(t), process.env.NODE_ENV !== "production" && !Array.isArray(c))
      throw new Error(process.env.NODE_ENV === "production" ? Z(3) : "when using a middleware builder function, an array of middleware must be returned");
  } else
    c = t();
  if (process.env.NODE_ENV !== "production" && c.some((v) => typeof v != "function"))
    throw new Error(process.env.NODE_ENV === "production" ? Z(4) : "each middleware provided to configureStore must be a function");
  if (process.env.NODE_ENV !== "production" && a) {
    let v = /* @__PURE__ */ new Set();
    c.forEach((m) => {
      if (v.has(m))
        throw new Error(process.env.NODE_ENV === "production" ? Z(42) : "Duplicate middleware references found when creating the store. Ensure that each middleware is only included once.");
      v.add(m);
    });
  }
  let s = Ki;
  i && (s = Ex({
    // Enable capture of stack traces for dispatched Redux actions
    trace: process.env.NODE_ENV !== "production",
    ...typeof i == "object" && i
  }));
  const f = ox(...c), d = jx(f);
  if (process.env.NODE_ENV !== "production" && u && typeof u != "function")
    throw new Error(process.env.NODE_ENV === "production" ? Z(5) : "`enhancers` field must be a callback");
  let h = typeof u == "function" ? u(d) : d();
  if (process.env.NODE_ENV !== "production" && !Array.isArray(h))
    throw new Error(process.env.NODE_ENV === "production" ? Z(6) : "`enhancers` callback must return an array");
  if (process.env.NODE_ENV !== "production" && h.some((v) => typeof v != "function"))
    throw new Error(process.env.NODE_ENV === "production" ? Z(7) : "each enhancer provided to configureStore must be a function");
  process.env.NODE_ENV !== "production" && c.length && !h.includes(f) && console.error("middlewares were provided, but middleware enhancer was not included in final enhancers - make sure to call `getDefaultEnhancers`");
  const p = s(...h);
  return lh(l, o, p);
}
function kh(e) {
  const t = {}, r = [];
  let n;
  const i = {
    addCase(a, o) {
      if (process.env.NODE_ENV !== "production") {
        if (r.length > 0)
          throw new Error(process.env.NODE_ENV === "production" ? Z(26) : "`builder.addCase` should only be called before calling `builder.addMatcher`");
        if (n)
          throw new Error(process.env.NODE_ENV === "production" ? Z(27) : "`builder.addCase` should only be called before calling `builder.addDefaultCase`");
      }
      const u = typeof a == "string" ? a : a.type;
      if (!u)
        throw new Error(process.env.NODE_ENV === "production" ? Z(28) : "`builder.addCase` cannot be called with an empty action type");
      if (u in t)
        throw new Error(process.env.NODE_ENV === "production" ? Z(29) : `\`builder.addCase\` cannot be called with two reducers for the same action type '${u}'`);
      return t[u] = o, i;
    },
    addAsyncThunk(a, o) {
      if (process.env.NODE_ENV !== "production" && n)
        throw new Error(process.env.NODE_ENV === "production" ? Z(43) : "`builder.addAsyncThunk` should only be called before calling `builder.addDefaultCase`");
      return o.pending && (t[a.pending.type] = o.pending), o.rejected && (t[a.rejected.type] = o.rejected), o.fulfilled && (t[a.fulfilled.type] = o.fulfilled), o.settled && r.push({
        matcher: a.settled,
        reducer: o.settled
      }), i;
    },
    addMatcher(a, o) {
      if (process.env.NODE_ENV !== "production" && n)
        throw new Error(process.env.NODE_ENV === "production" ? Z(30) : "`builder.addMatcher` should only be called before calling `builder.addDefaultCase`");
      return r.push({
        matcher: a,
        reducer: o
      }), i;
    },
    addDefaultCase(a) {
      if (process.env.NODE_ENV !== "production" && n)
        throw new Error(process.env.NODE_ENV === "production" ? Z(31) : "`builder.addDefaultCase` can only be called once");
      return n = a, i;
    }
  };
  return e(i), [t, r, n];
}
function Rx(e) {
  return typeof e == "function";
}
function Lx(e, t) {
  if (process.env.NODE_ENV !== "production" && typeof t == "object")
    throw new Error(process.env.NODE_ENV === "production" ? Z(8) : "The object notation for `createReducer` has been removed. Please use the 'builder callback' notation instead: https://redux-toolkit.js.org/api/createReducer");
  let [r, n, i] = kh(t), a;
  if (Rx(e))
    a = () => Gc(e());
  else {
    const u = Gc(e);
    a = () => u;
  }
  function o(u = a(), l) {
    let c = [r[l.type], ...n.filter(({
      matcher: s
    }) => s(l)).map(({
      reducer: s
    }) => s)];
    return c.filter((s) => !!s).length === 0 && (c = [i]), c.reduce((s, f) => {
      if (f)
        if (Vt(s)) {
          const h = f(s, l);
          return h === void 0 ? s : h;
        } else {
          if (gt(s))
            return bh(s, (d) => f(d, l));
          {
            const d = f(s, l);
            if (d === void 0) {
              if (s === null)
                return s;
              throw Error("A case reducer on a non-draftable value must not return undefined");
            }
            return d;
          }
        }
      return s;
    }, u);
  }
  return o.getInitialState = a, o;
}
var zx = "ModuleSymbhasOwnPr-0123456789ABCDEFGHNRVfgctiUvz_KqYTJkLxpZXIjQW", Bx = (e = 21) => {
  let t = "", r = e;
  for (; r--; )
    t += zx[Math.random() * 64 | 0];
  return t;
}, Fx = /* @__PURE__ */ Symbol.for("rtk-slice-createasyncthunk");
function Wx(e, t) {
  return `${e}/${t}`;
}
function Ux({
  creators: e
} = {}) {
  var r;
  const t = (r = e == null ? void 0 : e.asyncThunk) == null ? void 0 : r[Fx];
  return function(i) {
    const {
      name: a,
      reducerPath: o = a
    } = i;
    if (!a)
      throw new Error(process.env.NODE_ENV === "production" ? Z(11) : "`name` is a required option for createSlice");
    typeof process < "u" && process.env.NODE_ENV === "development" && i.initialState === void 0 && console.error("You must provide an `initialState` value that is not `undefined`. You may have misspelled `initialState`");
    const u = (typeof i.reducers == "function" ? i.reducers(Kx()) : i.reducers) || {}, l = Object.keys(u), c = {
      sliceCaseReducersByName: {},
      sliceCaseReducersByType: {},
      actionCreators: {},
      sliceMatchers: []
    }, s = {
      addCase(w, O) {
        const g = typeof w == "string" ? w : w.type;
        if (!g)
          throw new Error(process.env.NODE_ENV === "production" ? Z(12) : "`context.addCase` cannot be called with an empty action type");
        if (g in c.sliceCaseReducersByType)
          throw new Error(process.env.NODE_ENV === "production" ? Z(13) : "`context.addCase` cannot be called with two reducers for the same action type: " + g);
        return c.sliceCaseReducersByType[g] = O, s;
      },
      addMatcher(w, O) {
        return c.sliceMatchers.push({
          matcher: w,
          reducer: O
        }), s;
      },
      exposeAction(w, O) {
        return c.actionCreators[w] = O, s;
      },
      exposeCaseReducer(w, O) {
        return c.sliceCaseReducersByName[w] = O, s;
      }
    };
    l.forEach((w) => {
      const O = u[w], g = {
        reducerName: w,
        type: Wx(a, w),
        createNotation: typeof i.reducers == "function"
      };
      Yx(O) ? qx(g, O, s, t) : Hx(g, O, s);
    });
    function f() {
      if (process.env.NODE_ENV !== "production" && typeof i.extraReducers == "object")
        throw new Error(process.env.NODE_ENV === "production" ? Z(14) : "The object notation for `createSlice.extraReducers` has been removed. Please use the 'builder callback' notation instead: https://redux-toolkit.js.org/api/createSlice");
      const [w = {}, O = [], g = void 0] = typeof i.extraReducers == "function" ? kh(i.extraReducers) : [i.extraReducers], S = {
        ...w,
        ...c.sliceCaseReducersByType
      };
      return Lx(i.initialState, (P) => {
        for (let I in S)
          P.addCase(I, S[I]);
        for (let I of c.sliceMatchers)
          P.addMatcher(I.matcher, I.reducer);
        for (let I of O)
          P.addMatcher(I.matcher, I.reducer);
        g && P.addDefaultCase(g);
      });
    }
    const d = (w) => w, h = /* @__PURE__ */ new Map(), p = /* @__PURE__ */ new WeakMap();
    let v;
    function m(w, O) {
      return v || (v = f()), v(w, O);
    }
    function y() {
      return v || (v = f()), v.getInitialState();
    }
    function b(w, O = !1) {
      function g(P) {
        let I = P[w];
        if (typeof I > "u") {
          if (O)
            I = Oi(p, g, y);
          else if (process.env.NODE_ENV !== "production")
            throw new Error(process.env.NODE_ENV === "production" ? Z(15) : "selectSlice returned undefined for an uninjected slice reducer");
        }
        return I;
      }
      function S(P = d) {
        const I = Oi(h, O, () => /* @__PURE__ */ new WeakMap());
        return Oi(I, P, () => {
          const C = {};
          for (const [T, _] of Object.entries(i.selectors ?? {}))
            C[T] = Vx(_, P, () => Oi(p, P, y), O);
          return C;
        });
      }
      return {
        reducerPath: w,
        getSelectors: S,
        get selectors() {
          return S(g);
        },
        selectSlice: g
      };
    }
    const x = {
      name: a,
      reducer: m,
      actions: c.actionCreators,
      caseReducers: c.sliceCaseReducersByName,
      getInitialState: y,
      ...b(o),
      injectInto(w, {
        reducerPath: O,
        ...g
      } = {}) {
        const S = O ?? o;
        return w.inject({
          reducerPath: S,
          reducer: m
        }, g), {
          ...x,
          ...b(S, !0)
        };
      }
    };
    return x;
  };
}
function Vx(e, t, r, n) {
  function i(a, ...o) {
    let u = t(a);
    if (typeof u > "u") {
      if (n)
        u = r();
      else if (process.env.NODE_ENV !== "production")
        throw new Error(process.env.NODE_ENV === "production" ? Z(16) : "selectState returned undefined for an uninjected slice reducer");
    }
    return e(u, ...o);
  }
  return i.unwrapped = e, i;
}
var Be = /* @__PURE__ */ Ux();
function Kx() {
  function e(t, r) {
    return {
      _reducerDefinitionType: "asyncThunk",
      payloadCreator: t,
      ...r
    };
  }
  return e.withTypes = () => e, {
    reducer(t) {
      return Object.assign({
        // hack so the wrapping function has the same name as the original
        // we need to create a wrapper so the `reducerDefinitionType` is not assigned to the original
        [t.name](...r) {
          return t(...r);
        }
      }[t.name], {
        _reducerDefinitionType: "reducer"
        /* reducer */
      });
    },
    preparedReducer(t, r) {
      return {
        _reducerDefinitionType: "reducerWithPrepare",
        prepare: t,
        reducer: r
      };
    },
    asyncThunk: e
  };
}
function Hx({
  type: e,
  reducerName: t,
  createNotation: r
}, n, i) {
  let a, o;
  if ("reducer" in n) {
    if (r && !Gx(n))
      throw new Error(process.env.NODE_ENV === "production" ? Z(17) : "Please use the `create.preparedReducer` notation for prepared action creators with the `create` notation.");
    a = n.reducer, o = n.prepare;
  } else
    a = n;
  i.addCase(e, a).exposeCaseReducer(t, a).exposeAction(t, o ? nt(e, o) : nt(e));
}
function Yx(e) {
  return e._reducerDefinitionType === "asyncThunk";
}
function Gx(e) {
  return e._reducerDefinitionType === "reducerWithPrepare";
}
function qx({
  type: e,
  reducerName: t
}, r, n, i) {
  if (!i)
    throw new Error(process.env.NODE_ENV === "production" ? Z(18) : "Cannot use `create.asyncThunk` in the built-in `createSlice`. Use `buildCreateSlice({ creators: { asyncThunk: asyncThunkCreator } })` to create a customised version of `createSlice`.");
  const {
    payloadCreator: a,
    fulfilled: o,
    pending: u,
    rejected: l,
    settled: c,
    options: s
  } = r, f = i(e, a, s);
  n.exposeAction(t, f), o && n.addCase(f.fulfilled, o), u && n.addCase(f.pending, u), l && n.addCase(f.rejected, l), c && n.addMatcher(f.settled, c), n.exposeCaseReducer(t, {
    fulfilled: o || Si,
    pending: u || Si,
    rejected: l || Si,
    settled: c || Si
  });
}
function Si() {
}
var Xx = "task", Ch = "listener", Th = "completed", el = "cancelled", Zx = `task-${el}`, Qx = `task-${Th}`, vu = `${Ch}-${el}`, Jx = `${Ch}-${Th}`, Fa = class {
  constructor(e) {
    mi(this, "code");
    mi(this, "name", "TaskAbortError");
    mi(this, "message");
    this.code = e, this.message = `${Xx} ${el} (reason: ${e})`;
  }
}, tl = (e, t) => {
  if (typeof e != "function")
    throw new TypeError(process.env.NODE_ENV === "production" ? Z(32) : `${t} is not a function`);
}, Qi = () => {
}, Mh = (e, t = Qi) => (e.catch(t), e), Dh = (e, t) => (e.addEventListener("abort", t, {
  once: !0
}), () => e.removeEventListener("abort", t)), Ir = (e) => {
  if (e.aborted)
    throw new Fa(e.reason);
};
function Nh(e, t) {
  let r = Qi;
  return new Promise((n, i) => {
    const a = () => i(new Fa(e.reason));
    if (e.aborted) {
      a();
      return;
    }
    r = Dh(e, a), t.finally(() => r()).then(n, i);
  }).finally(() => {
    r = Qi;
  });
}
var ew = async (e, t) => {
  try {
    return await Promise.resolve(), {
      status: "ok",
      value: await e()
    };
  } catch (r) {
    return {
      status: r instanceof Fa ? "cancelled" : "rejected",
      error: r
    };
  } finally {
    t == null || t();
  }
}, Ji = (e) => (t) => Mh(Nh(e, t).then((r) => (Ir(e), r))), jh = (e) => {
  const t = Ji(e);
  return (r) => t(new Promise((n) => setTimeout(n, r)));
}, {
  assign: Zr
} = Object, Xc = {}, Jn = "listenerMiddleware", tw = (e, t) => {
  const r = (n) => Dh(e, () => n.abort(e.reason));
  return (n, i) => {
    tl(n, "taskExecutor");
    const a = new AbortController();
    r(a);
    const o = ew(async () => {
      Ir(e), Ir(a.signal);
      const u = await n({
        pause: Ji(a.signal),
        delay: jh(a.signal),
        signal: a.signal
      });
      return Ir(a.signal), u;
    }, () => a.abort(Qx));
    return i != null && i.autoJoin && t.push(o.catch(Qi)), {
      result: Ji(e)(o),
      cancel() {
        a.abort(Zx);
      }
    };
  };
}, rw = (e, t) => {
  const r = async (n, i) => {
    Ir(t);
    let a = () => {
    };
    const u = [new Promise((l, c) => {
      let s = e({
        predicate: n,
        effect: (f, d) => {
          d.unsubscribe(), l([f, d.getState(), d.getOriginalState()]);
        }
      });
      a = () => {
        s(), c();
      };
    })];
    i != null && u.push(new Promise((l) => setTimeout(l, i, null)));
    try {
      const l = await Nh(t, Promise.race(u));
      return Ir(t), l;
    } finally {
      a();
    }
  };
  return (n, i) => Mh(r(n, i));
}, $h = (e) => {
  let {
    type: t,
    actionCreator: r,
    matcher: n,
    predicate: i,
    effect: a
  } = e;
  if (t)
    i = nt(t).match;
  else if (r)
    t = r.type, i = r.match;
  else if (n)
    i = n;
  else if (!i) throw new Error(process.env.NODE_ENV === "production" ? Z(21) : "Creating or removing a listener requires one of the known fields for matching an action");
  return tl(a, "options.listener"), {
    predicate: i,
    type: t,
    effect: a
  };
}, Rh = /* @__PURE__ */ Zr((e) => {
  const {
    type: t,
    predicate: r,
    effect: n
  } = $h(e);
  return {
    id: Bx(),
    effect: n,
    type: t,
    predicate: r,
    pending: /* @__PURE__ */ new Set(),
    unsubscribe: () => {
      throw new Error(process.env.NODE_ENV === "production" ? Z(22) : "Unsubscribe not initialized");
    }
  };
}, {
  withTypes: () => Rh
}), Zc = (e, t) => {
  const {
    type: r,
    effect: n,
    predicate: i
  } = $h(t);
  return Array.from(e.values()).find((a) => (typeof r == "string" ? a.type === r : a.predicate === i) && a.effect === n);
}, hu = (e) => {
  e.pending.forEach((t) => {
    t.abort(vu);
  });
}, nw = (e, t) => () => {
  for (const r of t.keys())
    hu(r);
  e.clear();
}, Qc = (e, t, r) => {
  try {
    e(t, r);
  } catch (n) {
    setTimeout(() => {
      throw n;
    }, 0);
  }
}, Lh = /* @__PURE__ */ Zr(/* @__PURE__ */ nt(`${Jn}/add`), {
  withTypes: () => Lh
}), iw = /* @__PURE__ */ nt(`${Jn}/removeAll`), zh = /* @__PURE__ */ Zr(/* @__PURE__ */ nt(`${Jn}/remove`), {
  withTypes: () => zh
}), aw = (...e) => {
  console.error(`${Jn}/error`, ...e);
}, ei = (e = {}) => {
  const t = /* @__PURE__ */ new Map(), r = /* @__PURE__ */ new Map(), n = (h) => {
    const p = r.get(h) ?? 0;
    r.set(h, p + 1);
  }, i = (h) => {
    const p = r.get(h) ?? 1;
    p === 1 ? r.delete(h) : r.set(h, p - 1);
  }, {
    extra: a,
    onError: o = aw
  } = e;
  tl(o, "onError");
  const u = (h) => (h.unsubscribe = () => t.delete(h.id), t.set(h.id, h), (p) => {
    h.unsubscribe(), p != null && p.cancelActive && hu(h);
  }), l = (h) => {
    const p = Zc(t, h) ?? Rh(h);
    return u(p);
  };
  Zr(l, {
    withTypes: () => l
  });
  const c = (h) => {
    const p = Zc(t, h);
    return p && (p.unsubscribe(), h.cancelActive && hu(p)), !!p;
  };
  Zr(c, {
    withTypes: () => c
  });
  const s = async (h, p, v, m) => {
    const y = new AbortController(), b = rw(l, y.signal), x = [];
    try {
      h.pending.add(y), n(h), await Promise.resolve(h.effect(
        p,
        // Use assign() rather than ... to avoid extra helper functions added to bundle
        Zr({}, v, {
          getOriginalState: m,
          condition: (w, O) => b(w, O).then(Boolean),
          take: b,
          delay: jh(y.signal),
          pause: Ji(y.signal),
          extra: a,
          signal: y.signal,
          fork: tw(y.signal, x),
          unsubscribe: h.unsubscribe,
          subscribe: () => {
            t.set(h.id, h);
          },
          cancelActiveListeners: () => {
            h.pending.forEach((w, O, g) => {
              w !== y && (w.abort(vu), g.delete(w));
            });
          },
          cancel: () => {
            y.abort(vu), h.pending.delete(y);
          },
          throwIfCancelled: () => {
            Ir(y.signal);
          }
        })
      ));
    } catch (w) {
      w instanceof Fa || Qc(o, w, {
        raisedBy: "effect"
      });
    } finally {
      await Promise.all(x), y.abort(Jx), i(h), h.pending.delete(y);
    }
  }, f = nw(t, r);
  return {
    middleware: (h) => (p) => (v) => {
      if (!Xu(v))
        return p(v);
      if (Lh.match(v))
        return l(v.payload);
      if (iw.match(v)) {
        f();
        return;
      }
      if (zh.match(v))
        return c(v.payload);
      let m = h.getState();
      const y = () => {
        if (m === Xc)
          throw new Error(process.env.NODE_ENV === "production" ? Z(23) : `${Jn}: getOriginalState can only be called synchronously`);
        return m;
      };
      let b;
      try {
        if (b = p(v), t.size > 0) {
          const x = h.getState(), w = Array.from(t.values());
          for (const O of w) {
            let g = !1;
            try {
              g = O.predicate(v, x, m);
            } catch (S) {
              g = !1, Qc(o, S, {
                raisedBy: "predicate"
              });
            }
            g && s(O, v, h, y);
          }
        }
      } finally {
        m = Xc;
      }
      return b;
    },
    startListening: l,
    stopListening: c,
    clearListeners: f
  };
};
function Z(e) {
  return `Minified Redux Toolkit error #${e}; visit https://redux-toolkit.js.org/Errors?code=${e} for the full message or use the non-minified dev environment for full errors. `;
}
var ow = {
  layoutType: "horizontal",
  width: 0,
  height: 0,
  margin: {
    top: 5,
    right: 5,
    bottom: 5,
    left: 5
  },
  scale: 1
}, Bh = Be({
  name: "chartLayout",
  initialState: ow,
  reducers: {
    setLayout(e, t) {
      e.layoutType = t.payload;
    },
    setChartSize(e, t) {
      e.width = t.payload.width, e.height = t.payload.height;
    },
    setMargin(e, t) {
      var r, n, i, a;
      e.margin.top = (r = t.payload.top) !== null && r !== void 0 ? r : 0, e.margin.right = (n = t.payload.right) !== null && n !== void 0 ? n : 0, e.margin.bottom = (i = t.payload.bottom) !== null && i !== void 0 ? i : 0, e.margin.left = (a = t.payload.left) !== null && a !== void 0 ? a : 0;
    },
    setScale(e, t) {
      e.scale = t.payload;
    }
  }
}), Wa = Bh.actions, uw = Wa.setMargin, lw = Wa.setLayout, cw = Wa.setChartSize, sw = Wa.setScale, fw = Bh.reducer;
function Fh(e, t, r) {
  return Array.isArray(e) && e && t + r !== 0 ? e.slice(t, r + 1) : e;
}
function Y(e) {
  return Number.isFinite(e);
}
function Dt(e) {
  return typeof e == "number" && e > 0 && Number.isFinite(e);
}
function Jc(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function qr(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Jc(Object(r), !0).forEach(function(n) {
      dw(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Jc(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function dw(e, t, r) {
  return (t = vw(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function vw(e) {
  var t = hw(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function hw(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function Se(e, t, r) {
  return we(e) || we(t) ? r : Mt(t) ? Ut(e, t, r) : typeof t == "function" ? t(e) : r;
}
var pw = (e, t, r) => {
  if (t && r) {
    var n = r.width, i = r.height, a = t.align, o = t.verticalAlign, u = t.layout;
    if ((u === "vertical" || u === "horizontal" && o === "middle") && a !== "center" && N(e[a]))
      return qr(qr({}, e), {}, {
        [a]: e[a] + (n || 0)
      });
    if ((u === "horizontal" || u === "vertical" && a === "center") && o !== "middle" && N(e[o]))
      return qr(qr({}, e), {}, {
        [o]: e[o] + (i || 0)
      });
  }
  return e;
}, fr = (e, t) => e === "horizontal" && t === "xAxis" || e === "vertical" && t === "yAxis" || e === "centric" && t === "angleAxis" || e === "radial" && t === "radiusAxis", Wh = (e, t, r, n) => {
  if (n)
    return e.map((u) => u.coordinate);
  var i, a, o = e.map((u) => (u.coordinate === t && (i = !0), u.coordinate === r && (a = !0), u.coordinate));
  return i || o.push(t), a || o.push(r), o;
}, Uh = (e, t, r) => {
  if (!e)
    return null;
  var n = e.duplicateDomain, i = e.type, a = e.range, o = e.scale, u = e.realScaleType, l = e.isCategorical, c = e.categoricalDomain, s = e.tickCount, f = e.ticks, d = e.niceTicks, h = e.axisType;
  if (!o)
    return null;
  var p = u === "scaleBand" && o.bandwidth ? o.bandwidth() / 2 : 2, v = i === "category" && o.bandwidth ? o.bandwidth() / p : 0;
  if (v = h === "angleAxis" && a && a.length >= 2 ? He(a[0] - a[1]) * 2 * v : v, f || d) {
    var m = (f || d || []).map((y, b) => {
      var x = n ? n.indexOf(y) : y, w = o.map(x);
      return Y(w) ? {
        // If the scaleContent is not a number, the coordinate will be NaN.
        // That could be the case for example with a PointScale and a string as domain.
        coordinate: w + v,
        value: y,
        offset: v,
        index: b
      } : null;
    }).filter(Ye);
    return m;
  }
  return l && c ? c.map((y, b) => {
    var x = o.map(y);
    return Y(x) ? {
      coordinate: x + v,
      value: y,
      index: b,
      offset: v
    } : null;
  }).filter(Ye) : o.ticks && s != null ? o.ticks(s).map((y, b) => {
    var x = o.map(y);
    return Y(x) ? {
      coordinate: x + v,
      value: y,
      index: b,
      offset: v
    } : null;
  }).filter(Ye) : o.domain().map((y, b) => {
    var x = o.map(y);
    return Y(x) ? {
      coordinate: x + v,
      // @ts-expect-error can't use Date as an index
      value: n ? n[y] : y,
      index: b,
      offset: v
    } : null;
  }).filter(Ye);
}, mw = (e, t) => {
  if (!t || t.length !== 2 || !N(t[0]) || !N(t[1]))
    return e;
  var r = Math.min(t[0], t[1]), n = Math.max(t[0], t[1]), i = [e[0], e[1]];
  return (!N(e[0]) || e[0] < r) && (i[0] = r), (!N(e[1]) || e[1] > n) && (i[1] = n), i[0] > n && (i[0] = n), i[1] < r && (i[1] = r), i;
}, yw = (e) => {
  var t, r = e.length;
  if (!(r <= 0)) {
    var n = (t = e[0]) === null || t === void 0 ? void 0 : t.length;
    if (!(n == null || n <= 0))
      for (var i = 0; i < n; ++i)
        for (var a = 0, o = 0, u = 0; u < r; ++u) {
          var l = e[u], c = l == null ? void 0 : l[i];
          if (c != null) {
            var s = c[1], f = c[0], d = Tt(s) ? f : s;
            d >= 0 ? (c[0] = a, a += d, c[1] = a) : (c[0] = o, o += d, c[1] = o);
          }
        }
  }
}, gw = (e) => {
  var t, r = e.length;
  if (!(r <= 0)) {
    var n = (t = e[0]) === null || t === void 0 ? void 0 : t.length;
    if (!(n == null || n <= 0))
      for (var i = 0; i < n; ++i)
        for (var a = 0, o = 0; o < r; ++o) {
          var u = e[o], l = u == null ? void 0 : u[i];
          if (l != null) {
            var c = Tt(l[1]) ? l[0] : l[1];
            c >= 0 ? (l[0] = a, a += c, l[1] = a) : (l[0] = 0, l[1] = 0);
          }
        }
  }
}, bw = {
  sign: yw,
  // @ts-expect-error definitelytyped types are incorrect
  expand: y0,
  // @ts-expect-error definitelytyped types are incorrect
  none: Tr,
  // @ts-expect-error definitelytyped types are incorrect
  silhouette: g0,
  // @ts-expect-error definitelytyped types are incorrect
  wiggle: b0,
  positive: gw
}, xw = (e, t, r) => {
  var n, i = (n = bw[r]) !== null && n !== void 0 ? n : Tr, a = m0().keys(t).value((u, l) => Number(Se(u, l, 0))).order(Qo).offset(i), o = a(e);
  return o.forEach((u, l) => {
    u.forEach((c, s) => {
      var f = Se(e[s], t[l], 0);
      Array.isArray(f) && f.length === 2 && N(f[0]) && N(f[1]) && (c[0] = f[0], c[1] = f[1]);
    });
  }), o;
};
function ww(e) {
  return e == null ? void 0 : String(e);
}
var es = (e) => {
  var t = e.axis, r = e.ticks, n = e.offset, i = e.bandSize, a = e.entry, o = e.index;
  if (t.type === "category")
    return r[o] ? r[o].coordinate + n : null;
  var u = Se(a, t.dataKey, t.scale.domain()[o]);
  if (we(u))
    return null;
  var l = t.scale.map(u);
  return N(l) ? l - i / 2 + n : null;
}, Aw = (e) => {
  var t = e.numericAxis, r = t.scale.domain();
  if (t.type === "number") {
    var n = Math.min(r[0], r[1]), i = Math.max(r[0], r[1]);
    return n <= 0 && i >= 0 ? 0 : i < 0 ? i : n;
  }
  return r[0];
}, Ow = (e) => {
  var t = e.flat(2).filter(N);
  return [Math.min(...t), Math.max(...t)];
}, Ew = (e) => [e[0] === 1 / 0 ? 0 : e[0], e[1] === -1 / 0 ? 0 : e[1]], Sw = (e, t, r) => {
  if (!(e == null || Object.keys(e).length === 0))
    return Ew(Object.keys(e).reduce((n, i) => {
      var a = e[i];
      if (!a)
        return n;
      var o = a.stackedData, u = o.reduce((l, c) => {
        var s = Fh(c, t, r), f = Ow(s);
        return !Y(f[0]) || !Y(f[1]) ? l : [Math.min(l[0], f[0]), Math.max(l[1], f[1])];
      }, [1 / 0, -1 / 0]);
      return [Math.min(u[0], n[0]), Math.max(u[1], n[1])];
    }, [1 / 0, -1 / 0]));
}, ts = /^dataMin[\s]*-[\s]*([0-9]+([.]{1}[0-9]+){0,1})$/, rs = /^dataMax[\s]*\+[\s]*([0-9]+([.]{1}[0-9]+){0,1})$/, ea = (e, t, r) => {
  if (e && e.scale && e.scale.bandwidth) {
    var n = e.scale.bandwidth();
    if (!r || n > 0)
      return n;
  }
  if (e && t && t.length >= 2) {
    for (var i = Da(t, (s) => s.coordinate), a = 1 / 0, o = 1, u = i.length; o < u; o++) {
      var l = i[o], c = i[o - 1];
      a = Math.min(((l == null ? void 0 : l.coordinate) || 0) - ((c == null ? void 0 : c.coordinate) || 0), a);
    }
    return a === 1 / 0 ? 0 : a;
  }
  return r ? void 0 : 0;
};
function ns(e) {
  var t = e.tooltipEntrySettings, r = e.dataKey, n = e.payload, i = e.value, a = e.name;
  return qr(qr({}, t), {}, {
    dataKey: r,
    payload: n,
    value: i,
    name: a
  });
}
function Vh(e, t) {
  if (e != null)
    return String(e);
  if (typeof t == "string")
    return t;
}
var _w = (e, t) => {
  if (t === "horizontal")
    return e.relativeX;
  if (t === "vertical")
    return e.relativeY;
}, Pw = (e, t) => t === "centric" ? e.angle : e.radius, Gt = (e) => e.layout.width, qt = (e) => e.layout.height, Iw = (e) => e.layout.scale, Kh = (e) => e.layout.margin, Ua = E((e) => e.cartesianAxis.xAxis, (e) => Object.values(e)), Va = E((e) => e.cartesianAxis.yAxis, (e) => Object.values(e)), kw = "data-recharts-item-index", Cw = "data-recharts-item-id", ti = 60;
function is(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function _i(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? is(Object(r), !0).forEach(function(n) {
      Tw(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : is(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function Tw(e, t, r) {
  return (t = Mw(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function Mw(e) {
  var t = Dw(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Dw(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var Nw = (e) => e.brush.height;
function jw(e) {
  var t = Va(e);
  return t.reduce((r, n) => {
    if (n.orientation === "left" && !n.mirror && !n.hide) {
      var i = typeof n.width == "number" ? n.width : ti;
      return r + i;
    }
    return r;
  }, 0);
}
function $w(e) {
  var t = Va(e);
  return t.reduce((r, n) => {
    if (n.orientation === "right" && !n.mirror && !n.hide) {
      var i = typeof n.width == "number" ? n.width : ti;
      return r + i;
    }
    return r;
  }, 0);
}
function Rw(e) {
  var t = Ua(e);
  return t.reduce((r, n) => n.orientation === "top" && !n.mirror && !n.hide ? r + n.height : r, 0);
}
function Lw(e) {
  var t = Ua(e);
  return t.reduce((r, n) => n.orientation === "bottom" && !n.mirror && !n.hide ? r + n.height : r, 0);
}
var Pe = E([Gt, qt, Kh, Nw, jw, $w, Rw, Lw, uh, Vb], (e, t, r, n, i, a, o, u, l, c) => {
  var s = {
    left: (r.left || 0) + i,
    right: (r.right || 0) + a
  }, f = {
    top: (r.top || 0) + o,
    bottom: (r.bottom || 0) + u
  }, d = _i(_i({}, f), s), h = d.bottom;
  d.bottom += n, d = pw(d, l, c);
  var p = e - d.left - d.right, v = t - d.top - d.bottom;
  return _i(_i({
    brushBottom: h
  }, d), {}, {
    // never return negative values for height and width
    width: Math.max(p, 0),
    height: Math.max(v, 0)
  });
}), zw = E(Pe, (e) => ({
  x: e.left,
  y: e.top,
  width: e.width,
  height: e.height
})), rl = E(Gt, qt, (e, t) => ({
  x: 0,
  y: 0,
  width: e,
  height: t
})), Bw = /* @__PURE__ */ Xe(null), Ze = () => ct(Bw) != null, Ka = (e) => e.brush, Ha = E([Ka, Pe, Kh], (e, t, r) => ({
  height: e.height,
  x: N(e.x) ? e.x : t.left,
  y: N(e.y) ? e.y : t.top + t.height + t.brushBottom - ((r == null ? void 0 : r.bottom) || 0),
  width: N(e.width) ? e.width : t.width
}));
function Fw(e, t, { signal: r, edges: n } = {}) {
  let i, a = null;
  const o = n != null && n.includes("leading"), u = n == null || n.includes("trailing"), l = () => {
    a !== null && (e.apply(i, a), i = void 0, a = null);
  }, c = () => {
    u && l(), h();
  };
  let s = null;
  const f = () => {
    s != null && clearTimeout(s), s = setTimeout(() => {
      s = null, c();
    }, t);
  }, d = () => {
    s !== null && (clearTimeout(s), s = null);
  }, h = () => {
    d(), i = void 0, a = null;
  }, p = () => {
    l();
  }, v = function(...m) {
    if (r != null && r.aborted) return;
    i = this, a = m;
    const y = s == null;
    f(), o && y && l();
  };
  return v.schedule = f, v.cancel = h, v.flush = p, r == null || r.addEventListener("abort", h, { once: !0 }), v;
}
function Ww(e, t = 0, r = {}) {
  typeof r != "object" && (r = {});
  const { leading: n = !1, trailing: i = !0, maxWait: a } = r, o = Array(2);
  n && (o[0] = "leading"), i && (o[1] = "trailing");
  let u, l = null;
  const c = Fw(function(...d) {
    u = e.apply(this, d), l = null;
  }, t, { edges: o }), s = function(...d) {
    return a != null && (l === null && (l = Date.now()), Date.now() - l >= a) ? (u = e.apply(this, d), l = Date.now(), c.cancel(), c.schedule(), u) : (c.apply(this, d), u);
  }, f = () => (c.flush(), u);
  return s.cancel = c.cancel, s.flush = f, s;
}
function Uw(e, t = 0, r = {}) {
  const { leading: n = !0, trailing: i = !0 } = r;
  return Ww(e, t, {
    leading: n,
    maxWait: t,
    trailing: i
  });
}
var ta = function(t, r) {
  for (var n = arguments.length, i = new Array(n > 2 ? n - 2 : 0), a = 2; a < n; a++)
    i[a - 2] = arguments[a];
  if (typeof console < "u" && console.warn && (r === void 0 && console.warn("LogUtils requires an error message argument"), !t))
    if (r === void 0)
      console.warn("Minified exception occurred; use the non-minified dev environment for the full error message and additional helpful warnings.");
    else {
      var o = 0;
      console.warn(r.replace(/%s/g, () => i[o++]));
    }
}, St = {
  width: "100%",
  height: "100%",
  debounce: 0,
  minWidth: 0,
  initialDimension: {
    width: -1,
    height: -1
  }
}, Hh = (e, t, r) => {
  var n = r.width, i = n === void 0 ? St.width : n, a = r.height, o = a === void 0 ? St.height : a, u = r.aspect, l = r.maxHeight, c = Mr(i) ? e : Number(i), s = Mr(o) ? t : Number(o);
  return u && u > 0 && (c ? s = c / u : s && (c = s * u), l && s != null && s > l && (s = l)), {
    calculatedWidth: c,
    calculatedHeight: s
  };
}, Vw = {
  width: 0,
  height: 0,
  overflow: "visible"
}, Kw = {
  width: 0,
  overflowX: "visible"
}, Hw = {
  height: 0,
  overflowY: "visible"
}, Yw = {}, Gw = (e) => {
  var t = e.width, r = e.height, n = Mr(t), i = Mr(r);
  return n && i ? Vw : n ? Kw : i ? Hw : Yw;
};
function qw(e) {
  var t = e.width, r = e.height, n = e.aspect, i = t, a = r;
  return i === void 0 && a === void 0 ? (i = St.width, a = St.height) : i === void 0 ? i = n && n > 0 ? void 0 : St.width : a === void 0 && (a = n && n > 0 ? void 0 : St.height), {
    width: i,
    height: a
  };
}
var Xw = ["aspect", "initialDimension", "width", "height", "minWidth", "minHeight", "maxHeight", "children", "debounce", "id", "className", "onResize", "style"];
function ra() {
  return ra = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, ra.apply(null, arguments);
}
function as(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function os(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? as(Object(r), !0).forEach(function(n) {
      Zw(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : as(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function Zw(e, t, r) {
  return (t = Qw(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function Qw(e) {
  var t = Jw(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Jw(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function eA(e, t) {
  return iA(e) || nA(e, t) || rA(e, t) || tA();
}
function tA() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function rA(e, t) {
  if (e) {
    if (typeof e == "string") return us(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? us(e, t) : void 0;
  }
}
function us(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function nA(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function iA(e) {
  if (Array.isArray(e)) return e;
}
function aA(e, t) {
  if (e == null) return {};
  var r, n, i = oA(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function oA(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var Yh = /* @__PURE__ */ Xe(St.initialDimension);
function uA(e) {
  return Dt(e.width) && Dt(e.height);
}
function Gh(e) {
  var t = e.children, r = e.width, n = e.height, i = sr(() => ({
    width: r,
    height: n
  }), [r, n]);
  return uA(i) ? /* @__PURE__ */ A.createElement(Yh.Provider, {
    value: i
  }, t) : null;
}
var nl = () => ct(Yh), lA = /* @__PURE__ */ ze((e, t) => {
  var r = e.aspect, n = e.initialDimension, i = n === void 0 ? St.initialDimension : n, a = e.width, o = e.height, u = e.minWidth, l = u === void 0 ? St.minWidth : u, c = e.minHeight, s = e.maxHeight, f = e.children, d = e.debounce, h = d === void 0 ? St.debounce : d, p = e.id, v = e.className, m = e.onResize, y = e.style, b = y === void 0 ? {} : y, x = aA(e, Xw), w = Q(null), O = Q();
  O.current = m, wv(t, () => w.current);
  var g = _e({
    containerWidth: i.width,
    containerHeight: i.height
  }), S = eA(g, 2), P = S[0], I = S[1], C = ie((V, K) => {
    I((L) => {
      var G = Math.round(V), F = Math.round(K);
      return L.containerWidth === G && L.containerHeight === F ? L : {
        containerWidth: G,
        containerHeight: F
      };
    });
  }, []);
  ve(() => {
    if (w.current == null || typeof ResizeObserver > "u")
      return fn;
    var V = (he) => {
      var pe, le = he[0];
      if (le != null) {
        var We = le.contentRect, Qe = We.width, vt = We.height;
        C(Qe, vt), (pe = O.current) === null || pe === void 0 || pe.call(O, Qe, vt);
      }
    };
    h > 0 && (V = Uw(V, h, {
      trailing: !0,
      leading: !1
    }));
    var K = new ResizeObserver(V), L = w.current.getBoundingClientRect(), G = L.width, F = L.height;
    return C(G, F), K.observe(w.current), () => {
      K.disconnect();
    };
  }, [C, h]);
  var T = P.containerWidth, _ = P.containerHeight;
  ta(!r || r > 0, "The aspect(%s) must be greater than zero.", r);
  var z = Hh(T, _, {
    width: a,
    height: o,
    aspect: r,
    maxHeight: s
  }), $ = z.calculatedWidth, U = z.calculatedHeight;
  return ta(T < 0 || _ < 0 || $ != null && $ > 0 || U != null && U > 0, `The width(%s) and height(%s) of chart should be greater than 0,
       please check the style of container, or the props width(%s) and height(%s),
       or add a minWidth(%s) or minHeight(%s) or use aspect(%s) to control the
       height and width.`, $, U, a, o, l, c, r), /* @__PURE__ */ A.createElement("div", ra({
    id: p ? "".concat(p) : void 0,
    className: ce("recharts-responsive-container", v),
    style: os(os({}, b), {}, {
      width: a,
      height: o,
      minWidth: l,
      minHeight: c,
      maxHeight: s
    }),
    ref: w
  }, x), /* @__PURE__ */ A.createElement("div", {
    style: Gw({
      width: a,
      height: o
    })
  }, /* @__PURE__ */ A.createElement(Gh, {
    width: $,
    height: U
  }, f)));
}), cA = /* @__PURE__ */ ze((e, t) => {
  var r = nl();
  if (Dt(r.width) && Dt(r.height))
    return e.children;
  var n = qw({
    width: e.width,
    height: e.height,
    aspect: e.aspect
  }), i = n.width, a = n.height, o = Hh(void 0, void 0, {
    width: i,
    height: a,
    aspect: e.aspect,
    maxHeight: e.maxHeight
  }), u = o.calculatedWidth, l = o.calculatedHeight;
  return N(u) && N(l) ? /* @__PURE__ */ A.createElement(Gh, {
    width: u,
    height: l
  }, e.children) : /* @__PURE__ */ A.createElement(lA, ra({}, e, {
    width: i,
    height: a,
    ref: t
  }));
});
function il(e) {
  if (e)
    return {
      x: e.x,
      y: e.y,
      upperWidth: "upperWidth" in e ? e.upperWidth : e.width,
      lowerWidth: "lowerWidth" in e ? e.lowerWidth : e.width,
      width: e.width,
      height: e.height
    };
}
var Ya = () => {
  var e, t = Ze(), r = j(zw), n = j(Ha), i = (e = j(Ka)) === null || e === void 0 ? void 0 : e.padding;
  return !t || !n || !i ? r : {
    width: n.width - i.left - i.right,
    height: n.height - i.top - i.bottom,
    x: i.left,
    y: i.top
  };
}, sA = {
  top: 0,
  bottom: 0,
  left: 0,
  right: 0,
  width: 0,
  height: 0,
  brushBottom: 0
}, qh = () => {
  var e;
  return (e = j(Pe)) !== null && e !== void 0 ? e : sA;
}, Xh = () => j(Gt), Zh = () => j(qt), ne = (e) => e.layout.layoutType, dn = () => j(ne), Qh = () => {
  var e = dn();
  if (e === "horizontal" || e === "vertical")
    return e;
}, Jh = (e) => {
  var t = e.layout.layoutType;
  if (t === "centric" || t === "radial")
    return t;
}, fA = () => {
  var e = dn();
  return e !== void 0;
}, ri = (e) => {
  var t = se(), r = Ze(), n = e.width, i = e.height, a = nl(), o = n, u = i;
  return a && (o = a.width > 0 ? a.width : n, u = a.height > 0 ? a.height : i), ve(() => {
    !r && Dt(o) && Dt(u) && t(cw({
      width: o,
      height: u
    }));
  }, [t, r, o, u]), null;
}, dA = {
  settings: {
    layout: "horizontal",
    align: "center",
    verticalAlign: "bottom",
    itemSorter: "value"
  },
  size: {
    width: 0,
    height: 0
  },
  payload: []
}, ep = Be({
  name: "legend",
  initialState: dA,
  reducers: {
    setLegendSize(e, t) {
      e.size.width = t.payload.width, e.size.height = t.payload.height;
    },
    setLegendSettings(e, t) {
      e.settings.align = t.payload.align, e.settings.layout = t.payload.layout, e.settings.verticalAlign = t.payload.verticalAlign, e.settings.itemSorter = t.payload.itemSorter;
    },
    addLegendPayload: {
      reducer(e, t) {
        e.payload.push(q(t.payload));
      },
      prepare: ae()
    },
    replaceLegendPayload: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next, a = ot(e).payload.indexOf(q(n));
        a > -1 && (e.payload[a] = q(i));
      },
      prepare: ae()
    },
    removeLegendPayload: {
      reducer(e, t) {
        var r = ot(e).payload.indexOf(q(t.payload));
        r > -1 && e.payload.splice(r, 1);
      },
      prepare: ae()
    }
  }
}), ni = ep.actions;
ni.setLegendSize;
ni.setLegendSettings;
var vA = ni.addLegendPayload, hA = ni.replaceLegendPayload, pA = ni.removeLegendPayload, mA = ep.reducer, So = {};
/**
 * @license React
 * use-sync-external-store-with-selector.production.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var ls;
function yA() {
  if (ls) return So;
  ls = 1;
  var e = sn;
  function t(l, c) {
    return l === c && (l !== 0 || 1 / l === 1 / c) || l !== l && c !== c;
  }
  var r = typeof Object.is == "function" ? Object.is : t, n = e.useSyncExternalStore, i = e.useRef, a = e.useEffect, o = e.useMemo, u = e.useDebugValue;
  return So.useSyncExternalStoreWithSelector = function(l, c, s, f, d) {
    var h = i(null);
    if (h.current === null) {
      var p = { hasValue: !1, value: null };
      h.current = p;
    } else p = h.current;
    h = o(
      function() {
        function m(O) {
          if (!y) {
            if (y = !0, b = O, O = f(O), d !== void 0 && p.hasValue) {
              var g = p.value;
              if (d(g, O))
                return x = g;
            }
            return x = O;
          }
          if (g = x, r(b, O)) return g;
          var S = f(O);
          return d !== void 0 && d(g, S) ? (b = O, g) : (b = O, x = S);
        }
        var y = !1, b, x, w = s === void 0 ? null : s;
        return [
          function() {
            return m(c());
          },
          w === null ? void 0 : function() {
            return m(w());
          }
        ];
      },
      [c, s, f, d]
    );
    var v = n(l, h[0], h[1]);
    return a(
      function() {
        p.hasValue = !0, p.value = v;
      },
      [v]
    ), u(v), v;
  }, So;
}
var _o = {};
/**
 * @license React
 * use-sync-external-store-with-selector.development.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var cs;
function gA() {
  return cs || (cs = 1, process.env.NODE_ENV !== "production" && function() {
    function e(l, c) {
      return l === c && (l !== 0 || 1 / l === 1 / c) || l !== l && c !== c;
    }
    typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart(Error());
    var t = sn, r = typeof Object.is == "function" ? Object.is : e, n = t.useSyncExternalStore, i = t.useRef, a = t.useEffect, o = t.useMemo, u = t.useDebugValue;
    _o.useSyncExternalStoreWithSelector = function(l, c, s, f, d) {
      var h = i(null);
      if (h.current === null) {
        var p = { hasValue: !1, value: null };
        h.current = p;
      } else p = h.current;
      h = o(
        function() {
          function m(O) {
            if (!y) {
              if (y = !0, b = O, O = f(O), d !== void 0 && p.hasValue) {
                var g = p.value;
                if (d(g, O))
                  return x = g;
              }
              return x = O;
            }
            if (g = x, r(b, O))
              return g;
            var S = f(O);
            return d !== void 0 && d(g, S) ? (b = O, g) : (b = O, x = S);
          }
          var y = !1, b, x, w = s === void 0 ? null : s;
          return [
            function() {
              return m(c());
            },
            w === null ? void 0 : function() {
              return m(w());
            }
          ];
        },
        [c, s, f, d]
      );
      var v = n(l, h[0], h[1]);
      return a(
        function() {
          p.hasValue = !0, p.value = v;
        },
        [v]
      ), u(v), v;
    }, typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop(Error());
  }()), _o;
}
process.env.NODE_ENV === "production" ? yA() : gA();
function bA(e) {
  e();
}
function xA() {
  let e = null, t = null;
  return {
    clear() {
      e = null, t = null;
    },
    notify() {
      bA(() => {
        let r = e;
        for (; r; )
          r.callback(), r = r.next;
      });
    },
    get() {
      const r = [];
      let n = e;
      for (; n; )
        r.push(n), n = n.next;
      return r;
    },
    subscribe(r) {
      let n = !0;
      const i = t = {
        callback: r,
        next: null,
        prev: t
      };
      return i.prev ? i.prev.next = i : e = i, function() {
        !n || e === null || (n = !1, i.next ? i.next.prev = i.prev : t = i.prev, i.prev ? i.prev.next = i.next : e = i.next);
      };
    }
  };
}
var ss = {
  notify() {
  },
  get: () => []
};
function wA(e, t) {
  let r, n = ss, i = 0, a = !1;
  function o(v) {
    s();
    const m = n.subscribe(v);
    let y = !1;
    return () => {
      y || (y = !0, m(), f());
    };
  }
  function u() {
    n.notify();
  }
  function l() {
    p.onStateChange && p.onStateChange();
  }
  function c() {
    return a;
  }
  function s() {
    i++, r || (r = e.subscribe(l), n = xA());
  }
  function f() {
    i--, r && i === 0 && (r(), r = void 0, n.clear(), n = ss);
  }
  function d() {
    a || (a = !0, s());
  }
  function h() {
    a && (a = !1, f());
  }
  const p = {
    addNestedSub: o,
    notifyNestedSubs: u,
    handleChangeWrapper: l,
    isSubscribed: c,
    trySubscribe: d,
    tryUnsubscribe: h,
    getListeners: () => n
  };
  return p;
}
var AA = () => typeof window < "u" && typeof window.document < "u" && typeof window.document.createElement < "u", OA = /* @__PURE__ */ AA(), EA = () => typeof navigator < "u" && navigator.product === "ReactNative", SA = /* @__PURE__ */ EA(), _A = () => OA || SA ? A.useLayoutEffect : A.useEffect, PA = /* @__PURE__ */ _A();
function fs(e, t) {
  return e === t ? e !== 0 || t !== 0 || 1 / e === 1 / t : e !== e && t !== t;
}
function IA(e, t) {
  if (fs(e, t)) return !0;
  if (typeof e != "object" || e === null || typeof t != "object" || t === null)
    return !1;
  const r = Object.keys(e), n = Object.keys(t);
  if (r.length !== n.length) return !1;
  for (let i = 0; i < r.length; i++)
    if (!Object.prototype.hasOwnProperty.call(t, r[i]) || !fs(e[r[i]], t[r[i]]))
      return !1;
  return !0;
}
var Po = /* @__PURE__ */ Symbol.for("react-redux-context"), Io = typeof globalThis < "u" ? globalThis : (
  /* fall back to a per-module scope (pre-8.1 behaviour) if `globalThis` is not available */
  {}
);
function kA() {
  if (!A.createContext) return {};
  const e = Io[Po] ?? (Io[Po] = /* @__PURE__ */ new Map());
  let t = e.get(A.createContext);
  return t || (t = A.createContext(
    null
  ), process.env.NODE_ENV !== "production" && (t.displayName = "ReactRedux"), e.set(A.createContext, t)), t;
}
var CA = /* @__PURE__ */ kA();
function TA(e) {
  const { children: t, context: r, serverState: n, store: i } = e, a = A.useMemo(() => {
    const l = wA(i), c = {
      store: i,
      subscription: l,
      getServerState: n ? () => n : void 0
    };
    if (process.env.NODE_ENV === "production")
      return c;
    {
      const { identityFunctionCheck: s = "once", stabilityCheck: f = "once" } = e;
      return /* @__PURE__ */ Object.assign(c, {
        stabilityCheck: f,
        identityFunctionCheck: s
      });
    }
  }, [i, n]), o = A.useMemo(() => i.getState(), [i]);
  PA(() => {
    const { subscription: l } = a;
    return l.onStateChange = l.notifyNestedSubs, l.trySubscribe(), o !== i.getState() && l.notifyNestedSubs(), () => {
      l.tryUnsubscribe(), l.onStateChange = void 0;
    };
  }, [a, o]);
  const u = r || CA;
  return /* @__PURE__ */ A.createElement(u.Provider, { value: a }, t);
}
var MA = TA, DA = /* @__PURE__ */ new Set([
  "axisLine",
  "tickLine",
  "activeBar",
  "activeDot",
  "activeLabel",
  "activeShape",
  "allowEscapeViewBox",
  "background",
  "cursor",
  "dot",
  "label",
  "line",
  "margin",
  "padding",
  "position",
  "shape",
  "style",
  "tick",
  "wrapperStyle",
  // radius can be an array of 4 numbers, easy to compare shallowly
  "radius",
  "throttledEvents"
]);
function NA(e, t) {
  return e == null && t == null ? !0 : typeof e == "number" && typeof t == "number" ? e === t || e !== e && t !== t : e === t;
}
function Ga(e, t) {
  var r = /* @__PURE__ */ new Set([...Object.keys(e), ...Object.keys(t)]);
  for (var n of r)
    if (DA.has(n)) {
      if (e[n] == null && t[n] == null)
        continue;
      if (!IA(e[n], t[n]))
        return !1;
    } else if (!NA(e[n], t[n]))
      return !1;
  return !0;
}
function pu() {
  return pu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, pu.apply(null, arguments);
}
function ds(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function En(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? ds(Object(r), !0).forEach(function(n) {
      jA(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : ds(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function jA(e, t, r) {
  return (t = $A(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function $A(e) {
  var t = RA(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function RA(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function LA(e, t) {
  return WA(e) || FA(e, t) || BA(e, t) || zA();
}
function zA() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function BA(e, t) {
  if (e) {
    if (typeof e == "string") return vs(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? vs(e, t) : void 0;
  }
}
function vs(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function FA(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function WA(e) {
  if (Array.isArray(e)) return e;
}
function UA(e) {
  return Array.isArray(e) && Mt(e[0]) && Mt(e[1]) ? e.join(" ~ ") : e;
}
var Wr = {
  separator: " : ",
  contentStyle: {
    margin: 0,
    padding: 10,
    backgroundColor: "#fff",
    border: "1px solid #ccc",
    whiteSpace: "nowrap"
  },
  itemStyle: {
    display: "block",
    paddingTop: 4,
    paddingBottom: 4,
    color: "#000"
  },
  labelStyle: {},
  accessibilityLayer: !1
};
function VA(e, t) {
  return t == null ? e : Da(e, t);
}
var KA = (e) => {
  var t = e.separator, r = t === void 0 ? Wr.separator : t, n = e.contentStyle, i = e.itemStyle, a = e.labelStyle, o = a === void 0 ? Wr.labelStyle : a, u = e.payload, l = e.formatter, c = e.itemSorter, s = e.wrapperClassName, f = e.labelClassName, d = e.label, h = e.labelFormatter, p = e.accessibilityLayer, v = p === void 0 ? Wr.accessibilityLayer : p, m = () => {
    if (u && u.length) {
      var P = {
        padding: 0,
        margin: 0
      }, I = VA(u, c), C = I.map((T, _) => {
        if (!T || T.type === "none")
          return null;
        var z = T.formatter || l || UA, $ = T.value, U = T.name, V = $, K = U;
        if (z) {
          var L = z($, U, T, _, u);
          if (Array.isArray(L)) {
            var G = LA(L, 2);
            V = G[0], K = G[1];
          } else if (L != null)
            V = L;
          else
            return null;
        }
        var F = En(En({}, Wr.itemStyle), {}, {
          color: T.color || Wr.itemStyle.color
        }, i);
        return /* @__PURE__ */ A.createElement("li", {
          className: "recharts-tooltip-item",
          key: "tooltip-item-".concat(_),
          style: F
        }, Mt(K) ? /* @__PURE__ */ A.createElement("span", {
          className: "recharts-tooltip-item-name"
        }, K) : null, Mt(K) ? /* @__PURE__ */ A.createElement("span", {
          className: "recharts-tooltip-item-separator"
        }, r) : null, /* @__PURE__ */ A.createElement("span", {
          className: "recharts-tooltip-item-value"
        }, V), /* @__PURE__ */ A.createElement("span", {
          className: "recharts-tooltip-item-unit"
        }, T.unit || ""));
      });
      return /* @__PURE__ */ A.createElement("ul", {
        className: "recharts-tooltip-item-list",
        style: P
      }, C);
    }
    return null;
  }, y = En(En({}, Wr.contentStyle), n), b = En({
    margin: 0
  }, o), x = !we(d), w = x ? d : "", O = ce("recharts-default-tooltip", s), g = ce("recharts-tooltip-label", f);
  x && h && u !== void 0 && u !== null && (w = h(d, u));
  var S = v ? {
    role: "status",
    "aria-live": "assertive"
  } : {};
  return /* @__PURE__ */ A.createElement("div", pu({
    className: O,
    style: y
  }, S), /* @__PURE__ */ A.createElement("p", {
    className: g,
    style: b
  }, /* @__PURE__ */ A.isValidElement(w) ? w : "".concat(w)), m());
}, Sn = "recharts-tooltip-wrapper", HA = {
  visibility: "hidden"
};
function YA(e) {
  var t = e.coordinate, r = e.translateX, n = e.translateY;
  return ce(Sn, {
    ["".concat(Sn, "-right")]: N(r) && t && N(t.x) && r >= t.x,
    ["".concat(Sn, "-left")]: N(r) && t && N(t.x) && r < t.x,
    ["".concat(Sn, "-bottom")]: N(n) && t && N(t.y) && n >= t.y,
    ["".concat(Sn, "-top")]: N(n) && t && N(t.y) && n < t.y
  });
}
function hs(e) {
  var t = e.allowEscapeViewBox, r = e.coordinate, n = e.key, i = e.offset, a = e.position, o = e.reverseDirection, u = e.tooltipDimension, l = e.viewBox, c = e.viewBoxDimension;
  if (a && N(a[n]))
    return a[n];
  var s = r[n] - u - (i > 0 ? i : 0), f = r[n] + i;
  if (t[n])
    return o[n] ? s : f;
  var d = l[n];
  if (d == null)
    return 0;
  if (o[n]) {
    var h = s, p = d;
    return h < p ? Math.max(f, d) : Math.max(s, d);
  }
  if (c == null)
    return 0;
  var v = f + u, m = d + c;
  return v > m ? Math.max(s, d) : Math.max(f, d);
}
function GA(e) {
  var t = e.translateX, r = e.translateY, n = e.useTranslate3d;
  return {
    transform: n ? "translate3d(".concat(t, "px, ").concat(r, "px, 0)") : "translate(".concat(t, "px, ").concat(r, "px)")
  };
}
function qA(e) {
  var t = e.allowEscapeViewBox, r = e.coordinate, n = e.offsetTop, i = e.offsetLeft, a = e.position, o = e.reverseDirection, u = e.tooltipBox, l = e.useTranslate3d, c = e.viewBox, s, f, d;
  return u.height > 0 && u.width > 0 && r ? (f = hs({
    allowEscapeViewBox: t,
    coordinate: r,
    key: "x",
    offset: i,
    position: a,
    reverseDirection: o,
    tooltipDimension: u.width,
    viewBox: c,
    viewBoxDimension: c.width
  }), d = hs({
    allowEscapeViewBox: t,
    coordinate: r,
    key: "y",
    offset: n,
    position: a,
    reverseDirection: o,
    tooltipDimension: u.height,
    viewBox: c,
    viewBoxDimension: c.height
  }), s = GA({
    translateX: f,
    translateY: d,
    useTranslate3d: l
  })) : s = HA, {
    cssProperties: s,
    cssClasses: YA({
      translateX: f,
      translateY: d,
      coordinate: r
    })
  };
}
var XA = () => !(typeof window < "u" && window.document && window.document.createElement && window.setTimeout), ii = {
  isSsr: XA()
};
function ZA(e, t) {
  return tO(e) || eO(e, t) || JA(e, t) || QA();
}
function QA() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function JA(e, t) {
  if (e) {
    if (typeof e == "string") return ps(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? ps(e, t) : void 0;
  }
}
function ps(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function eO(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function tO(e) {
  if (Array.isArray(e)) return e;
}
function tp() {
  var e = _e(() => ii.isSsr || !window.matchMedia ? !1 : window.matchMedia("(prefers-reduced-motion: reduce)").matches), t = ZA(e, 2), r = t[0], n = t[1];
  return ve(() => {
    if (window.matchMedia) {
      var i = window.matchMedia("(prefers-reduced-motion: reduce)"), a = () => {
        n(i.matches);
      };
      return i.addEventListener("change", a), () => {
        i.removeEventListener("change", a);
      };
    }
  }, []), r;
}
function ms(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Ur(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? ms(Object(r), !0).forEach(function(n) {
      rO(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : ms(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function rO(e, t, r) {
  return (t = nO(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function nO(e) {
  var t = iO(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function iO(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function aO(e, t) {
  return cO(e) || lO(e, t) || uO(e, t) || oO();
}
function oO() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function uO(e, t) {
  if (e) {
    if (typeof e == "string") return ys(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? ys(e, t) : void 0;
  }
}
function ys(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function lO(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function cO(e) {
  if (Array.isArray(e)) return e;
}
function sO(e) {
  if (!(e.prefersReducedMotion && e.isAnimationActive === "auto") && e.isAnimationActive && e.active) {
    var t = typeof e.animationEasing == "string" ? e.animationEasing : "ease";
    return "transform ".concat(e.animationDuration, "ms ").concat(t);
  }
}
function fO(e) {
  var t, r, n, i, a, o, u = tp(), l = A.useState(() => ({
    dismissed: !1,
    dismissedAtCoordinate: {
      x: 0,
      y: 0
    }
  })), c = aO(l, 2), s = c[0], f = c[1];
  A.useEffect(() => {
    var y = (b) => {
      if (b.key === "Escape") {
        var x, w, O, g;
        f({
          dismissed: !0,
          dismissedAtCoordinate: {
            x: (x = (w = e.coordinate) === null || w === void 0 ? void 0 : w.x) !== null && x !== void 0 ? x : 0,
            y: (O = (g = e.coordinate) === null || g === void 0 ? void 0 : g.y) !== null && O !== void 0 ? O : 0
          }
        });
      }
    };
    return document.addEventListener("keydown", y), () => {
      document.removeEventListener("keydown", y);
    };
  }, [(t = e.coordinate) === null || t === void 0 ? void 0 : t.x, (r = e.coordinate) === null || r === void 0 ? void 0 : r.y]), s.dismissed && (((n = (i = e.coordinate) === null || i === void 0 ? void 0 : i.x) !== null && n !== void 0 ? n : 0) !== s.dismissedAtCoordinate.x || ((a = (o = e.coordinate) === null || o === void 0 ? void 0 : o.y) !== null && a !== void 0 ? a : 0) !== s.dismissedAtCoordinate.y) && f(Ur(Ur({}, s), {}, {
    dismissed: !1
  }));
  var d = qA({
    allowEscapeViewBox: e.allowEscapeViewBox,
    coordinate: e.coordinate,
    offsetLeft: typeof e.offset == "number" ? e.offset : e.offset.x,
    offsetTop: typeof e.offset == "number" ? e.offset : e.offset.y,
    position: e.position,
    reverseDirection: e.reverseDirection,
    tooltipBox: {
      height: e.lastBoundingBox.height,
      width: e.lastBoundingBox.width
    },
    useTranslate3d: e.useTranslate3d,
    viewBox: e.viewBox
  }), h = d.cssClasses, p = d.cssProperties, v = e.hasPortalFromProps ? {} : Ur(Ur({
    transition: sO({
      prefersReducedMotion: u,
      isAnimationActive: e.isAnimationActive,
      active: e.active,
      animationDuration: e.animationDuration,
      animationEasing: e.animationEasing
    })
  }, p), {}, {
    pointerEvents: "none",
    position: "absolute",
    top: 0,
    left: 0
  }), m = Ur(Ur({}, v), {}, {
    visibility: !s.dismissed && e.active && e.hasPayload ? "visible" : "hidden"
  }, e.wrapperStyle);
  return /* @__PURE__ */ A.createElement("div", {
    // @ts-expect-error typescript library does not recognize xmlns attribute, but it's required for an HTML chunk inside SVG.
    xmlns: "http://www.w3.org/1999/xhtml",
    tabIndex: -1,
    className: h,
    style: m,
    ref: e.innerRef
  }, e.children);
}
var dO = /* @__PURE__ */ A.memo(fO), rp = () => {
  var e;
  return (e = j((t) => t.rootProps.accessibilityLayer)) !== null && e !== void 0 ? e : !0;
};
function mu() {
  return mu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, mu.apply(null, arguments);
}
function gs(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function bs(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? gs(Object(r), !0).forEach(function(n) {
      vO(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : gs(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function vO(e, t, r) {
  return (t = hO(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function hO(e) {
  var t = pO(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function pO(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var xs = {
  curveBasisClosed: a0,
  curveBasisOpen: o0,
  curveBasis: i0,
  curveBumpX: r0,
  curveBumpY: n0,
  curveLinearClosed: u0,
  curveLinear: Ta,
  curveMonotoneX: l0,
  curveMonotoneY: c0,
  curveNatural: s0,
  curveStep: f0,
  curveStepAfter: v0,
  curveStepBefore: d0
}, na = (e) => Y(e.x) && Y(e.y), ws = (e) => e.base != null && na(e.base) && na(e), _n = (e) => e.x, Pn = (e) => e.y, mO = (e, t) => {
  if (typeof e == "function")
    return e;
  var r = "curve".concat(Hu(e));
  if ((r === "curveMonotone" || r === "curveBump") && t) {
    var n = xs["".concat(r).concat(t === "vertical" ? "Y" : "X")];
    if (n)
      return n;
  }
  return xs[r] || Ta;
}, As = {
  connectNulls: !1,
  type: "linear"
}, yO = (e) => {
  var t = e.type, r = t === void 0 ? As.type : t, n = e.points, i = n === void 0 ? [] : n, a = e.baseLine, o = e.layout, u = e.connectNulls, l = u === void 0 ? As.connectNulls : u, c = mO(r, o), s = l ? i.filter(na) : i;
  if (Array.isArray(a)) {
    var f, d = i.map((y, b) => bs(bs({}, y), {}, {
      base: a[b]
    }));
    o === "vertical" ? f = gi().y(Pn).x1(_n).x0((y) => y.base.x) : f = gi().x(_n).y1(Pn).y0((y) => y.base.y);
    var h = f.defined(ws).curve(c), p = l ? d.filter(ws) : d;
    return h(p);
  }
  var v;
  o === "vertical" && N(a) ? v = gi().y(Pn).x1(_n).x0(a) : N(a) ? v = gi().x(_n).y1(Pn).y0(a) : v = Dv().x(_n).y(Pn);
  var m = v.defined(na).curve(c);
  return m(s);
}, gO = (e) => {
  var t = e.className, r = e.points, n = e.path, i = e.pathRef, a = dn();
  if ((!r || !r.length) && !n)
    return null;
  var o = {
    type: e.type,
    points: e.points,
    baseLine: e.baseLine,
    layout: e.layout || a,
    connectNulls: e.connectNulls
  }, u = r && r.length ? yO(o) : n;
  return /* @__PURE__ */ A.createElement("path", mu({}, Ft(e), E0(e), {
    className: ce("recharts-curve", t),
    d: u === null ? void 0 : u,
    ref: i
  }));
}, bO = ["x", "y", "top", "left", "width", "height", "className"];
function yu() {
  return yu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, yu.apply(null, arguments);
}
function Os(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function xO(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Os(Object(r), !0).forEach(function(n) {
      wO(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Os(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function wO(e, t, r) {
  return (t = AO(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function AO(e) {
  var t = OO(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function OO(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function EO(e, t) {
  if (e == null) return {};
  var r, n, i = SO(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function SO(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var _O = (e, t, r, n, i, a) => "M".concat(e, ",").concat(i, "v").concat(n, "M").concat(a, ",").concat(t, "h").concat(r), PO = (e) => {
  var t = e.x, r = t === void 0 ? 0 : t, n = e.y, i = n === void 0 ? 0 : n, a = e.top, o = a === void 0 ? 0 : a, u = e.left, l = u === void 0 ? 0 : u, c = e.width, s = c === void 0 ? 0 : c, f = e.height, d = f === void 0 ? 0 : f, h = e.className, p = EO(e, bO), v = xO({
    x: r,
    y: i,
    top: o,
    left: l,
    width: s,
    height: d
  }, p);
  return !N(r) || !N(i) || !N(s) || !N(d) || !N(o) || !N(l) ? null : /* @__PURE__ */ A.createElement("path", yu({}, Wt(v), {
    className: ce("recharts-cross", h),
    d: _O(r, i, s, d, o, l)
  }));
};
function IO(e, t, r, n) {
  var i = n / 2;
  return {
    stroke: "none",
    fill: "#ccc",
    x: e === "horizontal" ? t.x - i : r.left + 0.5,
    y: e === "horizontal" ? r.top + 0.5 : t.y - i,
    width: e === "horizontal" ? n : r.width - 1,
    height: e === "horizontal" ? r.height - 1 : n
  };
}
var ia = 1e-4, np = (e, t) => [0, 3 * e, 3 * t - 6 * e, 3 * e - 3 * t + 1], ip = (e, t) => e.map((r, n) => r * t ** n).reduce((r, n) => r + n), Es = (e, t) => (r) => {
  var n = np(e, t);
  return ip(n, r);
}, kO = (e, t) => (r) => {
  var n = np(e, t), i = [...n.map((a, o) => a * o).slice(1), 0];
  return ip(i, r);
}, CO = (e) => {
  var t, r = e.split("(");
  if (r.length !== 2 || r[0] !== "cubic-bezier")
    return null;
  var n = (t = r[1]) === null || t === void 0 || (t = t.split(")")[0]) === null || t === void 0 ? void 0 : t.split(",");
  if (n == null || n.length !== 4)
    return null;
  var i = n.map((a) => parseFloat(a));
  return [i[0], i[1], i[2], i[3]];
}, TO = function() {
  for (var t = arguments.length, r = new Array(t), n = 0; n < t; n++)
    r[n] = arguments[n];
  if (r.length === 1)
    switch (r[0]) {
      case "linear":
        return [0, 0, 1, 1];
      case "ease":
        return [0.25, 0.1, 0.25, 1];
      case "ease-in":
        return [0.42, 0, 1, 1];
      case "ease-out":
        return [0.42, 0, 0.58, 1];
      case "ease-in-out":
        return [0, 0, 0.58, 1];
      default: {
        var i = CO(r[0]);
        if (i)
          return i;
      }
    }
  return r.length === 4 ? r : [0, 0, 1, 1];
}, MO = (e, t, r, n) => {
  var i = Es(e, r), a = Es(t, n), o = kO(e, r), u = (c) => c > 1 ? 1 : c < 0 ? 0 : c, l = (c) => {
    for (var s = c > 1 ? 1 : c, f = s, d = 0; d < 8; ++d) {
      var h = i(f) - s, p = o(f);
      if (Math.abs(h - s) < ia || p < ia)
        return a(f);
      f = u(f - h / p);
    }
    return a(f);
  };
  return l.isStepper = !1, l;
}, Ss = function() {
  return MO(...TO(...arguments));
}, DO = function() {
  for (var t = arguments.length > 0 && arguments[0] !== void 0 ? arguments[0] : {}, r = t.stiff, n = r === void 0 ? 100 : r, i = t.damping, a = i === void 0 ? 8 : i, o = t.dt, u = o === void 0 ? 16.67 : o, l = 1, c = [0], s = 0, f = 0, d = 1e4, h = 0; h < d; ) {
    var p = -(s - l) * n, v = f * a;
    if (f += (p - v) * u / 1e3, s += f * u / 1e3, c.push(s), Math.abs(s - l) < ia && Math.abs(f) < ia)
      break;
    h++;
  }
  c[c.length - 1] = l;
  var m = c.length - 1;
  return (y) => {
    var b, x, w;
    if (y <= 0) return 0;
    if (y >= 1) return l;
    var O = y * m, g = Math.floor(O), S = O - g;
    return ((b = c[g]) !== null && b !== void 0 ? b : 0) + (((x = c[g + 1]) !== null && x !== void 0 ? x : 0) - ((w = c[g]) !== null && w !== void 0 ? w : 0)) * S;
  };
}, NO = (e) => {
  if (typeof e == "string")
    switch (e) {
      case "ease":
      case "ease-in-out":
      case "ease-out":
      case "ease-in":
      case "linear":
        return Ss(e);
      case "spring":
        return DO();
      default:
        if (e.split("(")[0] === "cubic-bezier")
          return Ss(e);
    }
  return typeof e == "function" ? e : null;
}, jO = (e, t, r) => {
  var n, i = (a) => {
    var o = t.tick(a);
    if (t.getState() === "active") {
      if (r(t.getInterpolated()), t.getProgress() === 1) {
        t.complete(), n = void 0;
        return;
      }
      n = e.setTimeout(i, o);
      return;
    }
    n = e.setTimeout(i, o);
  };
  return n = e.setTimeout(i, 0), () => {
    var a;
    return (a = n) === null || a === void 0 ? void 0 : a();
  };
}, ap = /* @__PURE__ */ Xe(jO);
ap.Provider;
function $O(e) {
  var t = ct(ap);
  return sr(() => e ?? t, [e, t]);
}
function RO(e, t, r) {
  return (t = LO(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function LO(e) {
  var t = zO(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function zO(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var _s = "init", Ps = "pending", Is = "active", BO = "completed";
function ko(e) {
  return Math.max(0, e);
}
class FO {
  /**
   * Returns the absolute time after the animationBegin delay has been completed,
   * and when the animationDuration started ticking.
   */
  getAnimationStartedTime() {
    return this.animationStartedTime;
  }
  /**
   * Returns the absolute time of when the animation began - now it will wait for {animationBegin} ms before the transition starts
   */
  getBeginStartedTime() {
    return this.beginStartedTime;
  }
  constructor(t) {
    var r;
    RO(this, "state", _s), this.animationId = t.animationId, this.onAnimationEnd = t.onAnimationEnd, this.animationDuration = ko(t.animationDuration), this.animationBegin = ko(t.animationBegin), this.progress = 0, this.from = t.from, this.to = t.to, this.easing = t.easing, (r = t.onAnimationStart) === null || r === void 0 || r.call(t);
  }
  /**
   * Returns the state machine current state
   * - `init`:       animation had just been created. It immediately calls `onAnimationStart`
   * - `pending`:    animation is now paused for `animationBegin` milliseconds until the transition begins
   * - `active`:     animation is transitioning items on screen
   * - `completed`:  animation has completed its transition and executed `onAnimationEnd`.
   *                 This state is final and the animation is no longer allowed to transition to other states.
   */
  getState() {
    return this.state;
  }
  /**
   * Returns the easing input or function
   */
  getEasing() {
    return this.easing;
  }
  /**
   * Returns the configuration - the duration of the transition.
   * Does not change in time, does not change when state changes, this is a static value.
   */
  getAnimationDuration() {
    return this.animationDuration;
  }
  /**
   * Sets the current time of the animation. The animation sets its internal state and progress accordingly.
   * This is current, absolute time; not additive!
   * This allows you to essentially "travel back in time" based on the value you pass in here.
   *
   * Returns the (relative) time remaining until the current activity is over.
   * Meaning: if the state is in a middle of a delay, returns the time left until the delay is finished.
   * If the state is in the middle of a transition, returns time left until that transition is complete.
   * This is useful because it's the same number you can take and put into setTimeout(fn, X)
   * as that's how much time we need to wait until the next state transition happens.
   */
  tick(t) {
    if (this.getState() === _s)
      return this.state = Ps, this.beginStartedTime = t, this.animationBegin;
    if (this.getState() === Ps) {
      if (this.beginStartedTime == null)
        throw new Error();
      var r = t - this.beginStartedTime;
      return r >= this.animationBegin ? (this.state = Is, this.animationStartedTime = t, this.nextAnimationUpdate(0)) : ko(this.animationBegin - r);
    }
    if (this.getState() === Is) {
      if (this.animationStartedTime == null)
        throw new Error();
      var n = t - this.animationStartedTime;
      return this.setProgress(n / this.animationDuration), this.nextAnimationUpdate(n);
    }
    return 0;
  }
  setProgress(t) {
    this.progress = Math.min(1, Math.max(0, t));
  }
  /**
   * Returns an abstract "progress" which is number between 0 and 1 which shows the distance of transition.
   * This progress depends on the animation state:
   * - `init`: 0
   * - `pending`: 0
   * - `active`: transitioning between [0, 1] based on the time elapsed
   * - `completed`: 1
   *
   * The progress is hard-capped to be between 0 and 1 (inclusive) to avoid overshooting caused by coarse timers.
   * For this reason, the easing function must be applied _after_ this animation state,
   * so that one has a chance to construct dynamic "overshoot" animations.
   *
   * The progress is linear with time.
   * If you wish for easing, use `getInterpolated()` instead.
   */
  getProgress() {
    return this.progress;
  }
  /**
   * Completes the animation. Completed animation:
   * - cannot be manipulated anymore
   * - its progress is set to 1
   * - tick function doesn't do anything
   * - getState() always returns 'completed'
   */
  complete() {
    if (this.progress = 1, this.state === "active") {
      var t;
      (t = this.onAnimationEnd) === null || t === void 0 || t.call(this);
    }
    this.state = BO;
  }
  /**
   * Returns the starting value of the animation.
   * Does not include progress, easing, interpolation, none of that - just the static starting value
   */
  getFrom() {
    return this.from;
  }
  /**
   * Returns the end value of the animation.
   * Does not include progress, easing, interpolation, none of that - just the static end value
   */
  getTo() {
    return this.to;
  }
  /**
   * Unique identifier of an animation
   */
  getAnimationId() {
    return this.animationId;
  }
  /**
   * Returns the configuration - the duration of delay in between animation initialization, and transition.
   * Does not change in time, does not change when state changes, this is a static value.
   */
  getAnimationBegin() {
    return this.animationBegin;
  }
  /**
   * Returns value of the transition at the current time.
   * The exact details differ based on the animation type
   */
  /**
   * Returns the duration of time of when the controller should ask for the next update
   */
}
class WO extends FO {
  // eslint-disable-next-line class-methods-use-this
  nextAnimationUpdate() {
    return 0;
  }
  /**
   * Returns value of the animation after its easing function had been applied.
   * This value, unlike getProgress(), can escape the [0..1] range
   * because this is entirely within the easing function control. Spring typically does this.
   */
  getInterpolated() {
    return this.easing($e(this.getFrom(), this.getTo(), this.getProgress()));
  }
}
class UO {
  setTimeout(t) {
    var r = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : 0, n = performance.now(), i = null, a = (o) => {
      o - n >= r ? t(o) : i = requestAnimationFrame(a);
    };
    return i = requestAnimationFrame(a), () => {
      i != null && cancelAnimationFrame(i);
    };
  }
}
function VO(e, t) {
  return GO(e) || YO(e, t) || HO(e, t) || KO();
}
function KO() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function HO(e, t) {
  if (e) {
    if (typeof e == "string") return ks(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? ks(e, t) : void 0;
  }
}
function ks(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function YO(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function GO(e) {
  if (Array.isArray(e)) return e;
}
var qO = {
  begin: 0,
  duration: 1e3,
  easing: "ease",
  isActive: !0,
  canBegin: !0,
  onAnimationEnd: () => {
  },
  onAnimationStart: () => {
  }
}, Cs = 0, Co = 1;
function op(e) {
  var t = st(e, qO), r = t.animationId, n = t.isActive, i = t.canBegin, a = t.duration, o = t.easing, u = t.begin, l = t.onAnimationEnd, c = t.onAnimationStart, s = t.children, f = tp(), d = n === "auto" ? !ii.isSsr && !f : n, h = $O(t.animationController), p = _e(d ? Cs : Co), v = VO(p, 2), m = v[0], y = v[1];
  return ve(() => {
    d || y(Co);
  }, [d]), ve(() => {
    var b = NO(o);
    if (!d || !i || b == null)
      return fn;
    var x = new UO(), w = new WO({
      animationId: r,
      easing: b,
      animationDuration: a,
      animationBegin: u,
      onAnimationStart: c,
      onAnimationEnd: l,
      from: Cs,
      to: Co
    });
    return h(x, w, y);
  }, [h, r, d, i, a, o, u, c, l]), s(Number(m));
}
function up(e) {
  var t = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : "animation-", r = Q(zn(t)), n = Q(e);
  return n.current !== e && (r.current = zn(t), n.current = e), r.current;
}
var XO = (e) => e.replace(/([A-Z])/g, (t) => "-".concat(t.toLowerCase())), ZO = (e, t, r) => e.map((n) => "".concat(XO(n), " ").concat(t, "ms ").concat(r)).join(","), QO = ["radius"], JO = ["radius"], Ts, Ms, Ds, Ns, js, $s, Rs, Ls, zs, Bs;
function Fs(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Ws(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Fs(Object(r), !0).forEach(function(n) {
      e1(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Fs(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function e1(e, t, r) {
  return (t = t1(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function t1(e) {
  var t = r1(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function r1(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function aa() {
  return aa = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, aa.apply(null, arguments);
}
function Us(e, t) {
  if (e == null) return {};
  var r, n, i = n1(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function n1(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function i1(e, t) {
  return l1(e) || u1(e, t) || o1(e, t) || a1();
}
function a1() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function o1(e, t) {
  if (e) {
    if (typeof e == "string") return Vs(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Vs(e, t) : void 0;
  }
}
function Vs(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function u1(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function l1(e) {
  if (Array.isArray(e)) return e;
}
function wt(e, t) {
  return t || (t = e.slice(0)), Object.freeze(Object.defineProperties(e, { raw: { value: Object.freeze(t) } }));
}
var Ks = (e, t, r, n, i) => {
  var a = or(r), o = or(n), u = Math.min(Math.abs(a) / 2, Math.abs(o) / 2), l = o >= 0 ? 1 : -1, c = a >= 0 ? 1 : -1, s = o >= 0 && a >= 0 || o < 0 && a < 0 ? 1 : 0, f;
  if (u > 0 && Array.isArray(i)) {
    for (var d = [0, 0, 0, 0], h = 0, p = 4; h < p; h++) {
      var v, m = (v = i[h]) !== null && v !== void 0 ? v : 0;
      d[h] = m > u ? u : m;
    }
    f = Me(Ts || (Ts = wt(["M", ",", ""])), e, t + l * d[0]), d[0] > 0 && (f += Me(Ms || (Ms = wt(["A ", ",", ",0,0,", ",", ",", ""])), d[0], d[0], s, e + c * d[0], t)), f += Me(Ds || (Ds = wt(["L ", ",", ""])), e + r - c * d[1], t), d[1] > 0 && (f += Me(Ns || (Ns = wt(["A ", ",", ",0,0,", `,
        `, ",", ""])), d[1], d[1], s, e + r, t + l * d[1])), f += Me(js || (js = wt(["L ", ",", ""])), e + r, t + n - l * d[2]), d[2] > 0 && (f += Me($s || ($s = wt(["A ", ",", ",0,0,", `,
        `, ",", ""])), d[2], d[2], s, e + r - c * d[2], t + n)), f += Me(Rs || (Rs = wt(["L ", ",", ""])), e + c * d[3], t + n), d[3] > 0 && (f += Me(Ls || (Ls = wt(["A ", ",", ",0,0,", `,
        `, ",", ""])), d[3], d[3], s, e, t + n - l * d[3])), f += "Z";
  } else if (u > 0 && i === +i && i > 0) {
    var y = Math.min(u, i);
    f = Me(zs || (zs = wt(["M ", ",", `
            A `, ",", ",0,0,", ",", ",", `
            L `, ",", `
            A `, ",", ",0,0,", ",", ",", `
            L `, ",", `
            A `, ",", ",0,0,", ",", ",", `
            L `, ",", `
            A `, ",", ",0,0,", ",", ",", " Z"])), e, t + l * y, y, y, s, e + c * y, t, e + r - c * y, t, y, y, s, e + r, t + l * y, e + r, t + n - l * y, y, y, s, e + r - c * y, t + n, e + c * y, t + n, y, y, s, e, t + n - l * y);
  } else
    f = Me(Bs || (Bs = wt(["M ", ",", " h ", " v ", " h ", " Z"])), e, t, r, n, -r);
  return f;
}, Hs = {
  x: 0,
  y: 0,
  width: 0,
  height: 0,
  radius: 0,
  isAnimationActive: !1,
  isUpdateAnimationActive: !1,
  animationBegin: 0,
  animationDuration: 1500,
  animationEasing: "ease"
}, lp = (e) => {
  var t = st(e, Hs), r = Q(null), n = _e(-1), i = i1(n, 2), a = i[0], o = i[1];
  ve(() => {
    if (r.current && r.current.getTotalLength)
      try {
        var L = r.current.getTotalLength();
        L && o(L);
      } catch {
      }
  }, []);
  var u = t.x, l = t.y, c = t.width, s = t.height, f = t.radius, d = t.className, h = t.animationEasing, p = t.animationDuration, v = t.animationBegin, m = t.isAnimationActive, y = t.isUpdateAnimationActive, b = Q(c), x = Q(s), w = Q(u), O = Q(l), g = sr(() => ({
    x: u,
    y: l,
    width: c,
    height: s,
    radius: f
  }), [u, l, c, s, f]), S = up(g, "rectangle-");
  if (u !== +u || l !== +l || c !== +c || s !== +s || c === 0 || s === 0)
    return null;
  var P = ce("recharts-rectangle", d);
  if (!y) {
    var I = Wt(t);
    I.radius;
    var C = Us(I, QO);
    return /* @__PURE__ */ A.createElement("path", aa({}, C, {
      x: or(u),
      y: or(l),
      width: or(c),
      height: or(s),
      radius: typeof f == "number" ? f : void 0,
      className: P,
      d: Ks(u, l, c, s, f)
    }));
  }
  var T = b.current, _ = x.current, z = w.current, $ = O.current, U = "0px ".concat(a === -1 ? 1 : a, "px"), V = "".concat(a, "px ").concat(a, "px"), K = ZO(["strokeDasharray"], p, typeof h == "string" ? h : Hs.animationEasing);
  return /* @__PURE__ */ A.createElement(op, {
    animationId: S,
    key: S,
    canBegin: a > 0,
    duration: p,
    easing: h,
    isActive: y,
    begin: v
  }, (L) => {
    var G = $e(T, c, L), F = $e(_, s, L), he = $e(z, u, L), pe = $e($, l, L);
    r.current && (b.current = G, x.current = F, w.current = he, O.current = pe);
    var le;
    m ? L > 0 ? le = {
      transition: K,
      strokeDasharray: V
    } : le = {
      strokeDasharray: U
    } : le = {
      strokeDasharray: V
    };
    var We = Wt(t);
    We.radius;
    var Qe = Us(We, JO);
    return /* @__PURE__ */ A.createElement("path", aa({}, Qe, {
      radius: typeof f == "number" ? f : void 0,
      className: P,
      d: Ks(he, pe, G, F, f),
      ref: r,
      style: Ws(Ws({}, le), t.style)
    }));
  });
};
function Ys(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Gs(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Ys(Object(r), !0).forEach(function(n) {
      c1(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Ys(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function c1(e, t, r) {
  return (t = s1(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function s1(e) {
  var t = f1(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function f1(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var oa = Math.PI / 180, d1 = (e) => e * 180 / Math.PI, Ne = (e, t, r, n) => ({
  x: e + Math.cos(-oa * n) * r,
  y: t + Math.sin(-oa * n) * r
}), v1 = function(t, r) {
  var n = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : {
    top: 0,
    right: 0,
    bottom: 0,
    left: 0
  };
  return Math.min(Math.abs(t - (n.left || 0) - (n.right || 0)), Math.abs(r - (n.top || 0) - (n.bottom || 0))) / 2;
}, h1 = (e, t) => {
  var r = e.x, n = e.y, i = t.x, a = t.y;
  return Math.sqrt((r - i) ** 2 + (n - a) ** 2);
}, p1 = (e, t) => {
  var r = e.x, n = e.y, i = t.cx, a = t.cy, o = h1({
    x: r,
    y: n
  }, {
    x: i,
    y: a
  });
  if (o <= 0)
    return {
      radius: o,
      angle: 0
    };
  var u = (r - i) / o, l = Math.acos(u);
  return n > a && (l = 2 * Math.PI - l), {
    radius: o,
    angle: d1(l),
    angleInRadian: l
  };
}, m1 = (e) => {
  var t = e.startAngle, r = e.endAngle, n = Math.floor(t / 360), i = Math.floor(r / 360), a = Math.min(n, i);
  return {
    startAngle: t - a * 360,
    endAngle: r - a * 360
  };
}, y1 = (e, t) => {
  var r = t.startAngle, n = t.endAngle, i = Math.floor(r / 360), a = Math.floor(n / 360), o = Math.min(i, a);
  return e + o * 360;
}, g1 = (e, t) => {
  var r = e.relativeX, n = e.relativeY, i = p1({
    x: r,
    y: n
  }, t), a = i.radius, o = i.angle, u = t.innerRadius, l = t.outerRadius;
  if (a < u || a > l || a === 0)
    return null;
  var c = m1(t), s = c.startAngle, f = c.endAngle, d = o, h;
  if (s <= f) {
    for (; d > f; )
      d -= 360;
    for (; d < s; )
      d += 360;
    h = d >= s && d <= f;
  } else {
    for (; d > s; )
      d -= 360;
    for (; d < f; )
      d += 360;
    h = d >= f && d <= s;
  }
  return h ? Gs(Gs({}, t), {}, {
    radius: a,
    angle: y1(d, t)
  }) : null;
};
function cp(e) {
  var t = e.cx, r = e.cy, n = e.radius, i = e.startAngle, a = e.endAngle, o = Ne(t, r, n, i), u = Ne(t, r, n, a);
  return {
    points: [o, u],
    cx: t,
    cy: r,
    radius: n,
    startAngle: i,
    endAngle: a
  };
}
var qs, Xs, Zs, Qs, Js, ef, tf;
function gu() {
  return gu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, gu.apply(null, arguments);
}
function Or(e, t) {
  return t || (t = e.slice(0)), Object.freeze(Object.defineProperties(e, { raw: { value: Object.freeze(t) } }));
}
var b1 = (e, t) => {
  var r = He(t - e), n = Math.min(Math.abs(t - e), 359.999);
  return r * n;
}, Pi = (e) => {
  var t = e.cx, r = e.cy, n = e.radius, i = e.angle, a = e.sign, o = e.isExternal, u = e.cornerRadius, l = e.cornerIsExternal, c = u * (o ? 1 : -1) + n, s = Math.asin(u / c) / oa, f = l ? i : i + a * s, d = Ne(t, r, c, f), h = Ne(t, r, n, f), p = l ? i - a * s : i, v = Ne(t, r, c * Math.cos(s * oa), p);
  return {
    center: d,
    circleTangency: h,
    lineTangency: v,
    theta: s
  };
}, sp = (e) => {
  var t = e.cx, r = e.cy, n = e.innerRadius, i = e.outerRadius, a = e.startAngle, o = e.endAngle, u = b1(a, o), l = a + u, c = Ne(t, r, i, a), s = Ne(t, r, i, l), f = Me(qs || (qs = Or(["M ", ",", `
    A `, ",", `,0,
    `, ",", `,
    `, ",", `
  `])), c.x, c.y, i, i, +(Math.abs(u) > 180), +(a > l), s.x, s.y);
  if (n > 0) {
    var d = Ne(t, r, n, a), h = Ne(t, r, n, l);
    f += Me(Xs || (Xs = Or(["L ", ",", `
            A `, ",", `,0,
            `, ",", `,
            `, ",", " Z"])), h.x, h.y, n, n, +(Math.abs(u) > 180), +(a <= l), d.x, d.y);
  } else
    f += Me(Zs || (Zs = Or(["L ", ",", " Z"])), t, r);
  return f;
}, x1 = (e) => {
  var t = e.cx, r = e.cy, n = e.innerRadius, i = e.outerRadius, a = e.cornerRadius, o = e.forceCornerRadius, u = e.cornerIsExternal, l = e.startAngle, c = e.endAngle, s = He(c - l), f = Pi({
    cx: t,
    cy: r,
    radius: i,
    angle: l,
    sign: s,
    cornerRadius: a,
    cornerIsExternal: u
  }), d = f.circleTangency, h = f.lineTangency, p = f.theta, v = Pi({
    cx: t,
    cy: r,
    radius: i,
    angle: c,
    sign: -s,
    cornerRadius: a,
    cornerIsExternal: u
  }), m = v.circleTangency, y = v.lineTangency, b = v.theta, x = u ? Math.abs(l - c) : Math.abs(l - c) - p - b;
  if (x < 0)
    return o ? Me(Qs || (Qs = Or(["M ", ",", `
        a`, ",", ",0,0,1,", `,0
        a`, ",", ",0,0,1,", `,0
      `])), h.x, h.y, a, a, a * 2, a, a, -a * 2) : sp({
      cx: t,
      cy: r,
      innerRadius: n,
      outerRadius: i,
      startAngle: l,
      endAngle: c
    });
  var w = Me(Js || (Js = Or(["M ", ",", `
    A`, ",", ",0,0,", ",", ",", `
    A`, ",", ",0,", ",", ",", ",", `
    A`, ",", ",0,0,", ",", ",", `
  `])), h.x, h.y, a, a, +(s < 0), d.x, d.y, i, i, +(x > 180), +(s < 0), m.x, m.y, a, a, +(s < 0), y.x, y.y);
  if (n > 0) {
    var O = Pi({
      cx: t,
      cy: r,
      radius: n,
      angle: l,
      sign: s,
      isExternal: !0,
      cornerRadius: a,
      cornerIsExternal: u
    }), g = O.circleTangency, S = O.lineTangency, P = O.theta, I = Pi({
      cx: t,
      cy: r,
      radius: n,
      angle: c,
      sign: -s,
      isExternal: !0,
      cornerRadius: a,
      cornerIsExternal: u
    }), C = I.circleTangency, T = I.lineTangency, _ = I.theta, z = u ? Math.abs(l - c) : Math.abs(l - c) - P - _;
    if (z < 0 && a === 0)
      return "".concat(w, "L").concat(t, ",").concat(r, "Z");
    w += Me(ef || (ef = Or(["L", ",", `
      A`, ",", ",0,0,", ",", ",", `
      A`, ",", ",0,", ",", ",", ",", `
      A`, ",", ",0,0,", ",", ",", "Z"])), T.x, T.y, a, a, +(s < 0), C.x, C.y, n, n, +(z > 180), +(s > 0), g.x, g.y, a, a, +(s < 0), S.x, S.y);
  } else
    w += Me(tf || (tf = Or(["L", ",", "Z"])), t, r);
  return w;
}, w1 = {
  cx: 0,
  cy: 0,
  innerRadius: 0,
  outerRadius: 0,
  startAngle: 0,
  endAngle: 0,
  cornerRadius: 0,
  forceCornerRadius: !1,
  cornerIsExternal: !1
}, A1 = (e) => {
  var t = st(e, w1), r = t.cx, n = t.cy, i = t.innerRadius, a = t.outerRadius, o = t.cornerRadius, u = t.forceCornerRadius, l = t.cornerIsExternal, c = t.startAngle, s = t.endAngle, f = t.className;
  if (a < i || c === s)
    return null;
  var d = ce("recharts-sector", f), h = a - i, p = yt(o, h, 0, !0), v;
  return p > 0 && Math.abs(c - s) < 360 ? v = x1({
    cx: r,
    cy: n,
    innerRadius: i,
    outerRadius: a,
    cornerRadius: Math.min(p, h / 2),
    forceCornerRadius: u,
    cornerIsExternal: l,
    startAngle: c,
    endAngle: s
  }) : v = sp({
    cx: r,
    cy: n,
    innerRadius: i,
    outerRadius: a,
    startAngle: c,
    endAngle: s
  }), /* @__PURE__ */ A.createElement("path", gu({}, Wt(t), {
    className: d,
    d: v
  }));
};
function O1(e, t, r) {
  if (e === "horizontal")
    return [{
      x: t.x,
      y: r.top
    }, {
      x: t.x,
      y: r.top + r.height
    }];
  if (e === "vertical")
    return [{
      x: r.left,
      y: t.y
    }, {
      x: r.left + r.width,
      y: t.y
    }];
  if (Kv(t)) {
    if (e === "centric") {
      var n = t.cx, i = t.cy, a = t.innerRadius, o = t.outerRadius, u = t.angle, l = Ne(n, i, a, u), c = Ne(n, i, o, u);
      return [{
        x: l.x,
        y: l.y
      }, {
        x: c.x,
        y: c.y
      }];
    }
    return cp(t);
  }
}
function E1(e) {
  return oh(e) ? NaN : Number(e);
}
function To(e) {
  return e ? (e = E1(e), e === 1 / 0 || e === -1 / 0 ? (e < 0 ? -1 : 1) * Number.MAX_VALUE : e === e ? e : 0) : e === 0 ? e : 0;
}
function fp(e, t, r) {
  r && typeof r != "number" && nu(e, t, r) && (t = r = void 0), e = To(e), t === void 0 ? (t = e, e = 0) : t = To(t), r = r === void 0 ? e < t ? 1 : -1 : To(r);
  const n = Math.max(Math.ceil((t - e) / (r || 1)), 0), i = new Array(n);
  for (let a = 0; a < n; a++)
    i[a] = e, e += r;
  return i;
}
var xt = (e) => e.chartData, al = E([xt], (e) => {
  var t = e.chartData != null ? e.chartData.length - 1 : 0;
  return {
    chartData: e.chartData,
    computedData: e.computedData,
    dataEndIndex: t,
    dataStartIndex: 0
  };
}), qa = (e, t, r, n) => n ? al(e) : xt(e), S1 = (e, t, r) => r ? al(e) : xt(e), _1 = E([qa], (e) => {
  var t = e.chartData, r = e.dataStartIndex, n = e.dataEndIndex;
  return t != null ? t.slice(r, n + 1) : [];
});
E([al], (e) => {
  var t = e.chartData, r = e.dataStartIndex, n = e.dataEndIndex;
  return t != null ? t.slice(r, n + 1) : [];
});
var P1 = E([xt], (e) => {
  var t = e.chartData, r = e.dataStartIndex, n = e.dataEndIndex;
  return t != null ? t.slice(r, n + 1) : [];
});
function ol(e, t) {
  return T1(e) || C1(e, t) || k1(e, t) || I1();
}
function I1() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function k1(e, t) {
  if (e) {
    if (typeof e == "string") return rf(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? rf(e, t) : void 0;
  }
}
function rf(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function C1(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function T1(e) {
  if (Array.isArray(e)) return e;
}
function It(e) {
  if (Array.isArray(e) && e.length === 2) {
    var t = ol(e, 2), r = t[0], n = t[1];
    if (Y(r) && Y(n))
      return !0;
  }
  return !1;
}
function nf(e, t, r) {
  return r ? e : [Math.min(e[0], t[0]), Math.max(e[1], t[1])];
}
function dp(e, t) {
  if (t && typeof e != "function" && Array.isArray(e) && e.length === 2) {
    var r = ol(e, 2), n = r[0], i = r[1], a, o;
    if (Y(n))
      a = n;
    else if (typeof n == "function")
      return;
    if (Y(i))
      o = i;
    else if (typeof i == "function")
      return;
    var u = [a, o];
    if (It(u))
      return u;
  }
}
function M1(e, t, r) {
  if (!(!r && t == null)) {
    if (typeof e == "function" && t != null)
      try {
        var n = e(t, r);
        if (It(n))
          return nf(n, t, r);
      } catch {
      }
    if (Array.isArray(e) && e.length === 2) {
      var i = ol(e, 2), a = i[0], o = i[1], u, l;
      if (a === "auto")
        t != null && (u = Math.min(...t));
      else if (N(a))
        u = a;
      else if (typeof a == "function")
        try {
          t != null && (u = a(t == null ? void 0 : t[0]));
        } catch {
        }
      else if (typeof a == "string" && ts.test(a)) {
        var c = ts.exec(a);
        if (c == null || c[1] == null || t == null)
          u = void 0;
        else {
          var s = +c[1];
          u = t[0] - s;
        }
      } else
        u = t == null ? void 0 : t[0];
      if (o === "auto")
        t != null && (l = Math.max(...t));
      else if (N(o))
        l = o;
      else if (typeof o == "function")
        try {
          t != null && (l = o(t == null ? void 0 : t[1]));
        } catch {
        }
      else if (typeof o == "string" && rs.test(o)) {
        var f = rs.exec(o);
        if (f == null || f[1] == null || t == null)
          l = void 0;
        else {
          var d = +f[1];
          l = t[1] + d;
        }
      } else
        l = t == null ? void 0 : t[1];
      var h = [u, l];
      if (It(h))
        return t == null ? h : nf(h, t, r);
    }
  }
}
var vn = 1e9, D1 = {
  // These values must be integers within the stated ranges (inclusive).
  // Most of these values can be changed during run-time using `Decimal.config`.
  // The maximum number of significant digits of the result of a calculation or base conversion.
  // E.g. `Decimal.config({ precision: 20 });`
  precision: 20,
  // 1 to MAX_DIGITS
  // The rounding mode used by default by `toInteger`, `toDecimalPlaces`, `toExponential`,
  // `toFixed`, `toPrecision` and `toSignificantDigits`.
  //
  // ROUND_UP         0 Away from zero.
  // ROUND_DOWN       1 Towards zero.
  // ROUND_CEIL       2 Towards +Infinity.
  // ROUND_FLOOR      3 Towards -Infinity.
  // ROUND_HALF_UP    4 Towards nearest neighbour. If equidistant, up.
  // ROUND_HALF_DOWN  5 Towards nearest neighbour. If equidistant, down.
  // ROUND_HALF_EVEN  6 Towards nearest neighbour. If equidistant, towards even neighbour.
  // ROUND_HALF_CEIL  7 Towards nearest neighbour. If equidistant, towards +Infinity.
  // ROUND_HALF_FLOOR 8 Towards nearest neighbour. If equidistant, towards -Infinity.
  //
  // E.g.
  // `Decimal.rounding = 4;`
  // `Decimal.rounding = Decimal.ROUND_HALF_UP;`
  rounding: 4,
  // 0 to 8
  // The exponent value at and beneath which `toString` returns exponential notation.
  // JavaScript numbers: -7
  toExpNeg: -7,
  // 0 to -MAX_E
  // The exponent value at and above which `toString` returns exponential notation.
  // JavaScript numbers: 21
  toExpPos: 21,
  // 0 to MAX_E
  // The natural logarithm of 10.
  // 115 digits
  LN10: "2.302585092994045684017991454684364207601101488628772976033327900967572609677352480235997205089598298341967784042286"
}, ll, ue = !0, lt = "[DecimalError] ", kr = lt + "Invalid argument: ", ul = lt + "Exponent out of range: ", hn = Math.floor, xr = Math.pow, N1 = /^(\d+(\.\d*)?|\.\d+)(e[+-]?\d+)?$/i, tt, Ee = 1e7, oe = 7, vp = 9007199254740991, ua = hn(vp / oe), D = {};
D.absoluteValue = D.abs = function() {
  var e = new this.constructor(this);
  return e.s && (e.s = 1), e;
};
D.comparedTo = D.cmp = function(e) {
  var t, r, n, i, a = this;
  if (e = new a.constructor(e), a.s !== e.s) return a.s || -e.s;
  if (a.e !== e.e) return a.e > e.e ^ a.s < 0 ? 1 : -1;
  for (n = a.d.length, i = e.d.length, t = 0, r = n < i ? n : i; t < r; ++t)
    if (a.d[t] !== e.d[t]) return a.d[t] > e.d[t] ^ a.s < 0 ? 1 : -1;
  return n === i ? 0 : n > i ^ a.s < 0 ? 1 : -1;
};
D.decimalPlaces = D.dp = function() {
  var e = this, t = e.d.length - 1, r = (t - e.e) * oe;
  if (t = e.d[t], t) for (; t % 10 == 0; t /= 10) r--;
  return r < 0 ? 0 : r;
};
D.dividedBy = D.div = function(e) {
  return zt(this, new this.constructor(e));
};
D.dividedToIntegerBy = D.idiv = function(e) {
  var t = this, r = t.constructor;
  return re(zt(t, new r(e), 0, 1), r.precision);
};
D.equals = D.eq = function(e) {
  return !this.cmp(e);
};
D.exponent = function() {
  return ye(this);
};
D.greaterThan = D.gt = function(e) {
  return this.cmp(e) > 0;
};
D.greaterThanOrEqualTo = D.gte = function(e) {
  return this.cmp(e) >= 0;
};
D.isInteger = D.isint = function() {
  return this.e > this.d.length - 2;
};
D.isNegative = D.isneg = function() {
  return this.s < 0;
};
D.isPositive = D.ispos = function() {
  return this.s > 0;
};
D.isZero = function() {
  return this.s === 0;
};
D.lessThan = D.lt = function(e) {
  return this.cmp(e) < 0;
};
D.lessThanOrEqualTo = D.lte = function(e) {
  return this.cmp(e) < 1;
};
D.logarithm = D.log = function(e) {
  var t, r = this, n = r.constructor, i = n.precision, a = i + 5;
  if (e === void 0)
    e = new n(10);
  else if (e = new n(e), e.s < 1 || e.eq(tt)) throw Error(lt + "NaN");
  if (r.s < 1) throw Error(lt + (r.s ? "NaN" : "-Infinity"));
  return r.eq(tt) ? new n(0) : (ue = !1, t = zt(Un(r, a), Un(e, a), a), ue = !0, re(t, i));
};
D.minus = D.sub = function(e) {
  var t = this;
  return e = new t.constructor(e), t.s == e.s ? mp(t, e) : hp(t, (e.s = -e.s, e));
};
D.modulo = D.mod = function(e) {
  var t, r = this, n = r.constructor, i = n.precision;
  if (e = new n(e), !e.s) throw Error(lt + "NaN");
  return r.s ? (ue = !1, t = zt(r, e, 0, 1).times(e), ue = !0, r.minus(t)) : re(new n(r), i);
};
D.naturalExponential = D.exp = function() {
  return pp(this);
};
D.naturalLogarithm = D.ln = function() {
  return Un(this);
};
D.negated = D.neg = function() {
  var e = new this.constructor(this);
  return e.s = -e.s || 0, e;
};
D.plus = D.add = function(e) {
  var t = this;
  return e = new t.constructor(e), t.s == e.s ? hp(t, e) : mp(t, (e.s = -e.s, e));
};
D.precision = D.sd = function(e) {
  var t, r, n, i = this;
  if (e !== void 0 && e !== !!e && e !== 1 && e !== 0) throw Error(kr + e);
  if (t = ye(i) + 1, n = i.d.length - 1, r = n * oe + 1, n = i.d[n], n) {
    for (; n % 10 == 0; n /= 10) r--;
    for (n = i.d[0]; n >= 10; n /= 10) r++;
  }
  return e && t > r ? t : r;
};
D.squareRoot = D.sqrt = function() {
  var e, t, r, n, i, a, o, u = this, l = u.constructor;
  if (u.s < 1) {
    if (!u.s) return new l(0);
    throw Error(lt + "NaN");
  }
  for (e = ye(u), ue = !1, i = Math.sqrt(+u), i == 0 || i == 1 / 0 ? (t = _t(u.d), (t.length + e) % 2 == 0 && (t += "0"), i = Math.sqrt(t), e = hn((e + 1) / 2) - (e < 0 || e % 2), i == 1 / 0 ? t = "5e" + e : (t = i.toExponential(), t = t.slice(0, t.indexOf("e") + 1) + e), n = new l(t)) : n = new l(i.toString()), r = l.precision, i = o = r + 3; ; )
    if (a = n, n = a.plus(zt(u, a, o + 2)).times(0.5), _t(a.d).slice(0, o) === (t = _t(n.d)).slice(0, o)) {
      if (t = t.slice(o - 3, o + 1), i == o && t == "4999") {
        if (re(a, r + 1, 0), a.times(a).eq(u)) {
          n = a;
          break;
        }
      } else if (t != "9999")
        break;
      o += 4;
    }
  return ue = !0, re(n, r);
};
D.times = D.mul = function(e) {
  var t, r, n, i, a, o, u, l, c, s = this, f = s.constructor, d = s.d, h = (e = new f(e)).d;
  if (!s.s || !e.s) return new f(0);
  for (e.s *= s.s, r = s.e + e.e, l = d.length, c = h.length, l < c && (a = d, d = h, h = a, o = l, l = c, c = o), a = [], o = l + c, n = o; n--; ) a.push(0);
  for (n = c; --n >= 0; ) {
    for (t = 0, i = l + n; i > n; )
      u = a[i] + h[n] * d[i - n - 1] + t, a[i--] = u % Ee | 0, t = u / Ee | 0;
    a[i] = (a[i] + t) % Ee | 0;
  }
  for (; !a[--o]; ) a.pop();
  return t ? ++r : a.shift(), e.d = a, e.e = r, ue ? re(e, f.precision) : e;
};
D.toDecimalPlaces = D.todp = function(e, t) {
  var r = this, n = r.constructor;
  return r = new n(r), e === void 0 ? r : (Nt(e, 0, vn), t === void 0 ? t = n.rounding : Nt(t, 0, 8), re(r, e + ye(r) + 1, t));
};
D.toExponential = function(e, t) {
  var r, n = this, i = n.constructor;
  return e === void 0 ? r = Nr(n, !0) : (Nt(e, 0, vn), t === void 0 ? t = i.rounding : Nt(t, 0, 8), n = re(new i(n), e + 1, t), r = Nr(n, !0, e + 1)), r;
};
D.toFixed = function(e, t) {
  var r, n, i = this, a = i.constructor;
  return e === void 0 ? Nr(i) : (Nt(e, 0, vn), t === void 0 ? t = a.rounding : Nt(t, 0, 8), n = re(new a(i), e + ye(i) + 1, t), r = Nr(n.abs(), !1, e + ye(n) + 1), i.isneg() && !i.isZero() ? "-" + r : r);
};
D.toInteger = D.toint = function() {
  var e = this, t = e.constructor;
  return re(new t(e), ye(e) + 1, t.rounding);
};
D.toNumber = function() {
  return +this;
};
D.toPower = D.pow = function(e) {
  var t, r, n, i, a, o, u = this, l = u.constructor, c = 12, s = +(e = new l(e));
  if (!e.s) return new l(tt);
  if (u = new l(u), !u.s) {
    if (e.s < 1) throw Error(lt + "Infinity");
    return u;
  }
  if (u.eq(tt)) return u;
  if (n = l.precision, e.eq(tt)) return re(u, n);
  if (t = e.e, r = e.d.length - 1, o = t >= r, a = u.s, o) {
    if ((r = s < 0 ? -s : s) <= vp) {
      for (i = new l(tt), t = Math.ceil(n / oe + 4), ue = !1; r % 2 && (i = i.times(u), of(i.d, t)), r = hn(r / 2), r !== 0; )
        u = u.times(u), of(u.d, t);
      return ue = !0, e.s < 0 ? new l(tt).div(i) : re(i, n);
    }
  } else if (a < 0) throw Error(lt + "NaN");
  return a = a < 0 && e.d[Math.max(t, r)] & 1 ? -1 : 1, u.s = 1, ue = !1, i = e.times(Un(u, n + c)), ue = !0, i = pp(i), i.s = a, i;
};
D.toPrecision = function(e, t) {
  var r, n, i = this, a = i.constructor;
  return e === void 0 ? (r = ye(i), n = Nr(i, r <= a.toExpNeg || r >= a.toExpPos)) : (Nt(e, 1, vn), t === void 0 ? t = a.rounding : Nt(t, 0, 8), i = re(new a(i), e, t), r = ye(i), n = Nr(i, e <= r || r <= a.toExpNeg, e)), n;
};
D.toSignificantDigits = D.tosd = function(e, t) {
  var r = this, n = r.constructor;
  return e === void 0 ? (e = n.precision, t = n.rounding) : (Nt(e, 1, vn), t === void 0 ? t = n.rounding : Nt(t, 0, 8)), re(new n(r), e, t);
};
D.toString = D.valueOf = D.val = D.toJSON = D[Symbol.for("nodejs.util.inspect.custom")] = function() {
  var e = this, t = ye(e), r = e.constructor;
  return Nr(e, t <= r.toExpNeg || t >= r.toExpPos);
};
function hp(e, t) {
  var r, n, i, a, o, u, l, c, s = e.constructor, f = s.precision;
  if (!e.s || !t.s)
    return t.s || (t = new s(e)), ue ? re(t, f) : t;
  if (l = e.d, c = t.d, o = e.e, i = t.e, l = l.slice(), a = o - i, a) {
    for (a < 0 ? (n = l, a = -a, u = c.length) : (n = c, i = o, u = l.length), o = Math.ceil(f / oe), u = o > u ? o + 1 : u + 1, a > u && (a = u, n.length = 1), n.reverse(); a--; ) n.push(0);
    n.reverse();
  }
  for (u = l.length, a = c.length, u - a < 0 && (a = u, n = c, c = l, l = n), r = 0; a; )
    r = (l[--a] = l[a] + c[a] + r) / Ee | 0, l[a] %= Ee;
  for (r && (l.unshift(r), ++i), u = l.length; l[--u] == 0; ) l.pop();
  return t.d = l, t.e = i, ue ? re(t, f) : t;
}
function Nt(e, t, r) {
  if (e !== ~~e || e < t || e > r)
    throw Error(kr + e);
}
function _t(e) {
  var t, r, n, i = e.length - 1, a = "", o = e[0];
  if (i > 0) {
    for (a += o, t = 1; t < i; t++)
      n = e[t] + "", r = oe - n.length, r && (a += nr(r)), a += n;
    o = e[t], n = o + "", r = oe - n.length, r && (a += nr(r));
  } else if (o === 0)
    return "0";
  for (; o % 10 === 0; ) o /= 10;
  return a + o;
}
var zt = /* @__PURE__ */ function() {
  function e(n, i) {
    var a, o = 0, u = n.length;
    for (n = n.slice(); u--; )
      a = n[u] * i + o, n[u] = a % Ee | 0, o = a / Ee | 0;
    return o && n.unshift(o), n;
  }
  function t(n, i, a, o) {
    var u, l;
    if (a != o)
      l = a > o ? 1 : -1;
    else
      for (u = l = 0; u < a; u++)
        if (n[u] != i[u]) {
          l = n[u] > i[u] ? 1 : -1;
          break;
        }
    return l;
  }
  function r(n, i, a) {
    for (var o = 0; a--; )
      n[a] -= o, o = n[a] < i[a] ? 1 : 0, n[a] = o * Ee + n[a] - i[a];
    for (; !n[0] && n.length > 1; ) n.shift();
  }
  return function(n, i, a, o) {
    var u, l, c, s, f, d, h, p, v, m, y, b, x, w, O, g, S, P, I = n.constructor, C = n.s == i.s ? 1 : -1, T = n.d, _ = i.d;
    if (!n.s) return new I(n);
    if (!i.s) throw Error(lt + "Division by zero");
    for (l = n.e - i.e, S = _.length, O = T.length, h = new I(C), p = h.d = [], c = 0; _[c] == (T[c] || 0); ) ++c;
    if (_[c] > (T[c] || 0) && --l, a == null ? b = a = I.precision : o ? b = a + (ye(n) - ye(i)) + 1 : b = a, b < 0) return new I(0);
    if (b = b / oe + 2 | 0, c = 0, S == 1)
      for (s = 0, _ = _[0], b++; (c < O || s) && b--; c++)
        x = s * Ee + (T[c] || 0), p[c] = x / _ | 0, s = x % _ | 0;
    else {
      for (s = Ee / (_[0] + 1) | 0, s > 1 && (_ = e(_, s), T = e(T, s), S = _.length, O = T.length), w = S, v = T.slice(0, S), m = v.length; m < S; ) v[m++] = 0;
      P = _.slice(), P.unshift(0), g = _[0], _[1] >= Ee / 2 && ++g;
      do
        s = 0, u = t(_, v, S, m), u < 0 ? (y = v[0], S != m && (y = y * Ee + (v[1] || 0)), s = y / g | 0, s > 1 ? (s >= Ee && (s = Ee - 1), f = e(_, s), d = f.length, m = v.length, u = t(f, v, d, m), u == 1 && (s--, r(f, S < d ? P : _, d))) : (s == 0 && (u = s = 1), f = _.slice()), d = f.length, d < m && f.unshift(0), r(v, f, m), u == -1 && (m = v.length, u = t(_, v, S, m), u < 1 && (s++, r(v, S < m ? P : _, m))), m = v.length) : u === 0 && (s++, v = [0]), p[c++] = s, u && v[0] ? v[m++] = T[w] || 0 : (v = [T[w]], m = 1);
      while ((w++ < O || v[0] !== void 0) && b--);
    }
    return p[0] || p.shift(), h.e = l, re(h, o ? a + ye(h) + 1 : a);
  };
}();
function pp(e, t) {
  var r, n, i, a, o, u, l = 0, c = 0, s = e.constructor, f = s.precision;
  if (ye(e) > 16) throw Error(ul + ye(e));
  if (!e.s) return new s(tt);
  for (ue = !1, u = f, o = new s(0.03125); e.abs().gte(0.1); )
    e = e.times(o), c += 5;
  for (n = Math.log(xr(2, c)) / Math.LN10 * 2 + 5 | 0, u += n, r = i = a = new s(tt), s.precision = u; ; ) {
    if (i = re(i.times(e), u), r = r.times(++l), o = a.plus(zt(i, r, u)), _t(o.d).slice(0, u) === _t(a.d).slice(0, u)) {
      for (; c--; ) a = re(a.times(a), u);
      return s.precision = f, t == null ? (ue = !0, re(a, f)) : a;
    }
    a = o;
  }
}
function ye(e) {
  for (var t = e.e * oe, r = e.d[0]; r >= 10; r /= 10) t++;
  return t;
}
function Mo(e, t, r) {
  if (t > e.LN10.sd())
    throw ue = !0, r && (e.precision = r), Error(lt + "LN10 precision limit exceeded");
  return re(new e(e.LN10), t);
}
function nr(e) {
  for (var t = ""; e--; ) t += "0";
  return t;
}
function Un(e, t) {
  var r, n, i, a, o, u, l, c, s, f = 1, d = 10, h = e, p = h.d, v = h.constructor, m = v.precision;
  if (h.s < 1) throw Error(lt + (h.s ? "NaN" : "-Infinity"));
  if (h.eq(tt)) return new v(0);
  if (t == null ? (ue = !1, c = m) : c = t, h.eq(10))
    return t == null && (ue = !0), Mo(v, c);
  if (c += d, v.precision = c, r = _t(p), n = r.charAt(0), a = ye(h), Math.abs(a) < 15e14) {
    for (; n < 7 && n != 1 || n == 1 && r.charAt(1) > 3; )
      h = h.times(e), r = _t(h.d), n = r.charAt(0), f++;
    a = ye(h), n > 1 ? (h = new v("0." + r), a++) : h = new v(n + "." + r.slice(1));
  } else
    return l = Mo(v, c + 2, m).times(a + ""), h = Un(new v(n + "." + r.slice(1)), c - d).plus(l), v.precision = m, t == null ? (ue = !0, re(h, m)) : h;
  for (u = o = h = zt(h.minus(tt), h.plus(tt), c), s = re(h.times(h), c), i = 3; ; ) {
    if (o = re(o.times(s), c), l = u.plus(zt(o, new v(i), c)), _t(l.d).slice(0, c) === _t(u.d).slice(0, c))
      return u = u.times(2), a !== 0 && (u = u.plus(Mo(v, c + 2, m).times(a + ""))), u = zt(u, new v(f), c), v.precision = m, t == null ? (ue = !0, re(u, m)) : u;
    u = l, i += 2;
  }
}
function af(e, t) {
  var r, n, i;
  for ((r = t.indexOf(".")) > -1 && (t = t.replace(".", "")), (n = t.search(/e/i)) > 0 ? (r < 0 && (r = n), r += +t.slice(n + 1), t = t.substring(0, n)) : r < 0 && (r = t.length), n = 0; t.charCodeAt(n) === 48; ) ++n;
  for (i = t.length; t.charCodeAt(i - 1) === 48; ) --i;
  if (t = t.slice(n, i), t) {
    if (i -= n, r = r - n - 1, e.e = hn(r / oe), e.d = [], n = (r + 1) % oe, r < 0 && (n += oe), n < i) {
      for (n && e.d.push(+t.slice(0, n)), i -= oe; n < i; ) e.d.push(+t.slice(n, n += oe));
      t = t.slice(n), n = oe - t.length;
    } else
      n -= i;
    for (; n--; ) t += "0";
    if (e.d.push(+t), ue && (e.e > ua || e.e < -ua)) throw Error(ul + r);
  } else
    e.s = 0, e.e = 0, e.d = [0];
  return e;
}
function re(e, t, r) {
  var n, i, a, o, u, l, c, s, f = e.d;
  for (o = 1, a = f[0]; a >= 10; a /= 10) o++;
  if (n = t - o, n < 0)
    n += oe, i = t, c = f[s = 0];
  else {
    if (s = Math.ceil((n + 1) / oe), a = f.length, s >= a) return e;
    for (c = a = f[s], o = 1; a >= 10; a /= 10) o++;
    n %= oe, i = n - oe + o;
  }
  if (r !== void 0 && (a = xr(10, o - i - 1), u = c / a % 10 | 0, l = t < 0 || f[s + 1] !== void 0 || c % a, l = r < 4 ? (u || l) && (r == 0 || r == (e.s < 0 ? 3 : 2)) : u > 5 || u == 5 && (r == 4 || l || r == 6 && // Check whether the digit to the left of the rounding digit is odd.
  (n > 0 ? i > 0 ? c / xr(10, o - i) : 0 : f[s - 1]) % 10 & 1 || r == (e.s < 0 ? 8 : 7))), t < 1 || !f[0])
    return l ? (a = ye(e), f.length = 1, t = t - a - 1, f[0] = xr(10, (oe - t % oe) % oe), e.e = hn(-t / oe) || 0) : (f.length = 1, f[0] = e.e = e.s = 0), e;
  if (n == 0 ? (f.length = s, a = 1, s--) : (f.length = s + 1, a = xr(10, oe - n), f[s] = i > 0 ? (c / xr(10, o - i) % xr(10, i) | 0) * a : 0), l)
    for (; ; )
      if (s == 0) {
        (f[0] += a) == Ee && (f[0] = 1, ++e.e);
        break;
      } else {
        if (f[s] += a, f[s] != Ee) break;
        f[s--] = 0, a = 1;
      }
  for (n = f.length; f[--n] === 0; ) f.pop();
  if (ue && (e.e > ua || e.e < -ua))
    throw Error(ul + ye(e));
  return e;
}
function mp(e, t) {
  var r, n, i, a, o, u, l, c, s, f, d = e.constructor, h = d.precision;
  if (!e.s || !t.s)
    return t.s ? t.s = -t.s : t = new d(e), ue ? re(t, h) : t;
  if (l = e.d, f = t.d, n = t.e, c = e.e, l = l.slice(), o = c - n, o) {
    for (s = o < 0, s ? (r = l, o = -o, u = f.length) : (r = f, n = c, u = l.length), i = Math.max(Math.ceil(h / oe), u) + 2, o > i && (o = i, r.length = 1), r.reverse(), i = o; i--; ) r.push(0);
    r.reverse();
  } else {
    for (i = l.length, u = f.length, s = i < u, s && (u = i), i = 0; i < u; i++)
      if (l[i] != f[i]) {
        s = l[i] < f[i];
        break;
      }
    o = 0;
  }
  for (s && (r = l, l = f, f = r, t.s = -t.s), u = l.length, i = f.length - u; i > 0; --i) l[u++] = 0;
  for (i = f.length; i > o; ) {
    if (l[--i] < f[i]) {
      for (a = i; a && l[--a] === 0; ) l[a] = Ee - 1;
      --l[a], l[i] += Ee;
    }
    l[i] -= f[i];
  }
  for (; l[--u] === 0; ) l.pop();
  for (; l[0] === 0; l.shift()) --n;
  return l[0] ? (t.d = l, t.e = n, ue ? re(t, h) : t) : new d(0);
}
function Nr(e, t, r) {
  var n, i = ye(e), a = _t(e.d), o = a.length;
  return t ? (r && (n = r - o) > 0 ? a = a.charAt(0) + "." + a.slice(1) + nr(n) : o > 1 && (a = a.charAt(0) + "." + a.slice(1)), a = a + (i < 0 ? "e" : "e+") + i) : i < 0 ? (a = "0." + nr(-i - 1) + a, r && (n = r - o) > 0 && (a += nr(n))) : i >= o ? (a += nr(i + 1 - o), r && (n = r - i - 1) > 0 && (a = a + "." + nr(n))) : ((n = i + 1) < o && (a = a.slice(0, n) + "." + a.slice(n)), r && (n = r - o) > 0 && (i + 1 === o && (a += "."), a += nr(n))), e.s < 0 ? "-" + a : a;
}
function of(e, t) {
  if (e.length > t)
    return e.length = t, !0;
}
function yp(e) {
  var t, r, n;
  function i(a) {
    var o = this;
    if (!(o instanceof i)) return new i(a);
    if (o.constructor = i, a instanceof i) {
      o.s = a.s, o.e = a.e, o.d = (a = a.d) ? a.slice() : a;
      return;
    }
    if (typeof a == "number") {
      if (a * 0 !== 0)
        throw Error(kr + a);
      if (a > 0)
        o.s = 1;
      else if (a < 0)
        a = -a, o.s = -1;
      else {
        o.s = 0, o.e = 0, o.d = [0];
        return;
      }
      if (a === ~~a && a < 1e7) {
        o.e = 0, o.d = [a];
        return;
      }
      return af(o, a.toString());
    } else if (typeof a != "string")
      throw Error(kr + a);
    if (a.charCodeAt(0) === 45 ? (a = a.slice(1), o.s = -1) : o.s = 1, N1.test(a)) af(o, a);
    else throw Error(kr + a);
  }
  if (i.prototype = D, i.ROUND_UP = 0, i.ROUND_DOWN = 1, i.ROUND_CEIL = 2, i.ROUND_FLOOR = 3, i.ROUND_HALF_UP = 4, i.ROUND_HALF_DOWN = 5, i.ROUND_HALF_EVEN = 6, i.ROUND_HALF_CEIL = 7, i.ROUND_HALF_FLOOR = 8, i.clone = yp, i.config = i.set = j1, e === void 0 && (e = {}), e)
    for (n = ["precision", "rounding", "toExpNeg", "toExpPos", "LN10"], t = 0; t < n.length; ) e.hasOwnProperty(r = n[t++]) || (e[r] = this[r]);
  return i.config(e), i;
}
function j1(e) {
  if (!e || typeof e != "object")
    throw Error(lt + "Object expected");
  var t, r, n, i = [
    "precision",
    1,
    vn,
    "rounding",
    0,
    8,
    "toExpNeg",
    -1 / 0,
    0,
    "toExpPos",
    0,
    1 / 0
  ];
  for (t = 0; t < i.length; t += 3)
    if ((n = e[r = i[t]]) !== void 0)
      if (hn(n) === n && n >= i[t + 1] && n <= i[t + 2]) this[r] = n;
      else throw Error(kr + r + ": " + n);
  if ((n = e[r = "LN10"]) !== void 0)
    if (n == Math.LN10) this[r] = new this(n);
    else throw Error(kr + r + ": " + n);
  return this;
}
var ll = yp(D1);
tt = new ll(1);
const H = ll;
function gp(e) {
  var t;
  return e === 0 ? t = 1 : t = Math.floor(new H(e).abs().log(10).toNumber()) + 1, t;
}
function bp(e, t, r) {
  for (var n = new H(e), i = 0, a = []; n.lt(t) && i < 1e5; )
    a.push(n.toNumber()), n = n.add(r), i++;
  return a;
}
function Vn(e, t) {
  return z1(e) || L1(e, t) || R1(e, t) || $1();
}
function $1() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function R1(e, t) {
  if (e) {
    if (typeof e == "string") return uf(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? uf(e, t) : void 0;
  }
}
function uf(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function L1(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function z1(e) {
  if (Array.isArray(e)) return e;
}
var xp = (e) => {
  var t = Vn(e, 2), r = t[0], n = t[1], i = r, a = n;
  return r > n && (i = n, a = r), [i, a];
}, cl = (e, t, r) => {
  if (e.lte(0))
    return new H(0);
  var n = gp(e.toNumber()), i = new H(10).pow(n), a = e.div(i), o = n !== 1 ? 0.05 : 0.1, u = new H(Math.ceil(a.div(o).toNumber())).add(r).mul(o), l = u.mul(i);
  return t ? new H(l.toNumber()) : new H(Math.ceil(l.toNumber()));
}, wp = (e, t, r) => {
  var n;
  if (e.lte(0))
    return new H(0);
  var i = [1, 2, 2.5, 5], a = e.toNumber(), o = Math.floor(new H(a).abs().log(10).toNumber()), u = new H(10).pow(o), l = e.div(u).toNumber(), c = i.findIndex((h) => h >= l - 1e-10);
  if (c === -1 && (u = u.mul(10), c = 0), c += r, c >= i.length) {
    var s = Math.floor(c / i.length);
    c %= i.length, u = u.mul(new H(10).pow(s));
  }
  var f = (n = i[c]) !== null && n !== void 0 ? n : 1, d = new H(f).mul(u);
  return t ? d : new H(Math.ceil(d.toNumber()));
}, B1 = (e, t, r) => {
  var n = new H(1), i = new H(e);
  if (!i.isint() && r) {
    var a = Math.abs(e);
    a < 1 ? (n = new H(10).pow(gp(e) - 1), i = new H(Math.floor(i.div(n).toNumber())).mul(n)) : a > 1 && (i = new H(Math.floor(e)));
  } else e === 0 ? i = new H(Math.floor((t - 1) / 2)) : r || (i = new H(Math.floor(e)));
  for (var o = Math.floor((t - 1) / 2), u = [], l = 0; l < t; l++)
    u.push(i.add(new H(l - o).mul(n)).toNumber());
  return u;
}, Ap = function(t, r, n, i) {
  var a = arguments.length > 4 && arguments[4] !== void 0 ? arguments[4] : 0, o = arguments.length > 5 && arguments[5] !== void 0 ? arguments[5] : cl;
  if (!Number.isFinite((r - t) / (n - 1)))
    return {
      step: new H(0),
      tickMin: new H(0),
      tickMax: new H(0)
    };
  var u = o(new H(r).sub(t).div(n - 1), i, a), l;
  t <= 0 && r >= 0 ? l = new H(0) : (l = new H(t).add(r).div(2), l = l.sub(new H(l).mod(u)));
  var c = Math.ceil(l.sub(t).div(u).toNumber()), s = Math.ceil(new H(r).sub(l).div(u).toNumber()), f = c + s + 1;
  return f > n ? Ap(t, r, n, i, a + 1, o) : (f < n && (s = r > 0 ? s + (n - f) : s, c = r > 0 ? c : c + (n - f)), {
    step: u,
    tickMin: l.sub(new H(c).mul(u)),
    tickMax: l.add(new H(s).mul(u))
  });
}, lf = function(t) {
  var r = Vn(t, 2), n = r[0], i = r[1], a = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : 6, o = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : !0, u = arguments.length > 3 && arguments[3] !== void 0 ? arguments[3] : "auto", l = Math.max(a, 2), c = xp([n, i]), s = Vn(c, 2), f = s[0], d = s[1];
  if (f === -1 / 0 || d === 1 / 0) {
    var h = d === 1 / 0 ? [f, ...Array(a - 1).fill(1 / 0)] : [...Array(a - 1).fill(-1 / 0), d];
    return n > i ? h.reverse() : h;
  }
  if (f === d)
    return B1(f, a, o);
  var p = u === "snap125" ? wp : cl, v = Ap(f, d, l, o, 0, p), m = v.step, y = v.tickMin, b = v.tickMax, x = bp(y, b.add(new H(0.1).mul(m)), m);
  return n > i ? x.reverse() : x;
}, cf = function(t, r) {
  var n = Vn(t, 2), i = n[0], a = n[1], o = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : !0, u = arguments.length > 3 && arguments[3] !== void 0 ? arguments[3] : "auto", l = xp([i, a]), c = Vn(l, 2), s = c[0], f = c[1];
  if (s === -1 / 0 || f === 1 / 0)
    return [i, a];
  if (s === f)
    return [s];
  var d = u === "snap125" ? wp : cl, h = Math.max(r, 2), p = d(new H(f).sub(s).div(h - 1), o, 0), v = [...bp(new H(s), new H(f), p), f];
  if (o === !1) {
    v = v.map((y) => Math.round(y));
    var m = v.length - 1;
    m > 0 && v[m] === v[m - 1] && (v = v.slice(0, m));
  }
  return i > a ? v.reverse() : v;
}, Op = (e) => e.rootProps.maxBarSize, F1 = (e) => e.rootProps.barGap, Ep = (e) => e.rootProps.barCategoryGap, W1 = (e) => e.rootProps.barSize, Xa = (e) => e.rootProps.stackOffset, Sp = (e) => e.rootProps.reverseStackOrder, sl = (e) => e.options.chartName, fl = (e) => e.rootProps.syncId, _p = (e) => e.rootProps.syncMethod, dl = (e) => e.options.eventEmitter, Ue = {
  /**
   * CartesianGrid and PolarGrid
   */
  grid: -100,
  /**
   * Background of Bar and RadialBar.
   * This is not visible by default but can be enabled by setting background={true} on Bar or RadialBar.
   */
  barBackground: -50,
  /*
   * other chart elements or custom elements without specific zIndex
   * render in here, at zIndex 0
   */
  /**
   * Area, Pie, Radar, and ReferenceArea
   */
  area: 100,
  /**
   * Cursor is embedded inside Tooltip and controlled by it.
   * The Tooltip itself has a separate portal and is not included in the zIndex system;
   * Cursor is the decoration inside the chart area. CursorRectangle is a rectangle box.
   * It renders below bar so that in a stacked bar chart the cursor rectangle does not hide the other bars.
   */
  cursorRectangle: 200,
  /**
   * Bar and RadialBar
   */
  bar: 300,
  /**
   * Line and ReferenceLine, and ErrorBor
   */
  line: 400,
  /**
   * XAxis and YAxis and PolarAngleAxis and PolarRadiusAxis ticks and lines and children
   */
  axis: 500,
  /**
   * Scatter and ReferenceDot,
   * and Dots of Line and Area and Radar if they have dot=true
   */
  scatter: 600,
  /**
   * Hovering over a Bar or RadialBar renders a highlight rectangle
   */
  activeBar: 1e3,
  /**
   * Cursor is embedded inside Tooltip and controlled by it.
   * The Tooltip itself has a separate portal and is not included in the zIndex system;
   * Cursor is the decoration inside the chart area, usually a cross or a box.
   * CursorLine is a line cursor rendered in Line, Area, Scatter, Radar charts.
   * It renders above the Line and Scatter so that it is always visible.
   * It renders below active dot so that the dot is always visible and shows the current point.
   * We're also assuming that the active dot is small enough that it does not fully cover the cursor line.
   *
   * This also applies to the radial cursor in RadialBarChart.
   */
  cursorLine: 1100,
  /**
   * Hovering over a Point in Line, Area, Scatter, Radar renders a highlight dot
   */
  activeDot: 1200,
  /**
   * LabelList and Label, including Axis labels
   */
  label: 2e3
}, pr = {
  allowDecimals: !1,
  // if I set this to false then Tooltip synchronisation stops working in Radar, wtf
  allowDataOverflow: !1,
  angleAxisId: 0,
  reversed: !1,
  scale: "auto",
  tick: !0,
  type: "auto"
}, At = {
  allowDataOverflow: !1,
  allowDecimals: !1,
  allowDuplicatedCategory: !0,
  includeHidden: !1,
  radiusAxisId: 0,
  reversed: !1,
  scale: "auto",
  tick: !0,
  tickCount: 5,
  type: "auto"
}, Za = (e, t) => {
  if (!(!e || !t))
    return e != null && e.reversed ? [t[1], t[0]] : t;
};
function Qa(e, t, r) {
  if (r !== "auto")
    return r;
  if (e != null)
    return fr(e, t) ? "category" : "number";
}
function sf(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function la(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? sf(Object(r), !0).forEach(function(n) {
      U1(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : sf(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function U1(e, t, r) {
  return (t = V1(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function V1(e) {
  var t = K1(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function K1(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var ff = {
  allowDataOverflow: pr.allowDataOverflow,
  allowDecimals: pr.allowDecimals,
  allowDuplicatedCategory: !1,
  // defaultPolarAngleAxisProps.allowDuplicatedCategory has it set to true but the actual axis rendering ignores the prop because reasons,
  dataKey: void 0,
  domain: void 0,
  id: pr.angleAxisId,
  includeHidden: !1,
  name: void 0,
  reversed: pr.reversed,
  scale: pr.scale,
  tick: pr.tick,
  tickCount: void 0,
  ticks: void 0,
  type: pr.type,
  unit: void 0,
  niceTicks: "auto"
}, df = {
  allowDataOverflow: At.allowDataOverflow,
  allowDecimals: At.allowDecimals,
  allowDuplicatedCategory: At.allowDuplicatedCategory,
  dataKey: void 0,
  domain: void 0,
  id: At.radiusAxisId,
  includeHidden: At.includeHidden,
  name: void 0,
  reversed: At.reversed,
  scale: At.scale,
  tick: At.tick,
  tickCount: At.tickCount,
  ticks: void 0,
  type: At.type,
  unit: void 0,
  niceTicks: "auto"
}, H1 = (e, t) => {
  if (t != null)
    return e.polarAxis.angleAxis[t];
}, vl = E([H1, Jh], (e, t) => {
  var r;
  if (e != null)
    return e;
  var n = (r = Qa(t, "angleAxis", ff.type)) !== null && r !== void 0 ? r : "category";
  return la(la({}, ff), {}, {
    type: n
  });
}), Y1 = (e, t) => e.polarAxis.radiusAxis[t], hl = E([Y1, Jh], (e, t) => {
  var r;
  if (e != null)
    return e;
  var n = (r = Qa(t, "radiusAxis", df.type)) !== null && r !== void 0 ? r : "category";
  return la(la({}, df), {}, {
    type: n
  });
}), Ja = (e) => e.polarOptions, pl = E([Gt, qt, Pe], v1), Pp = E([Ja, pl], (e, t) => {
  if (e != null)
    return yt(e.innerRadius, t, 0);
}), Ip = E([Ja, pl], (e, t) => {
  if (e != null)
    return yt(e.outerRadius, t, t * 0.8);
}), G1 = (e) => {
  if (e == null)
    return [0, 0];
  var t = e.startAngle, r = e.endAngle;
  return [t, r];
}, kp = E([Ja], G1);
E([vl, kp], Za);
var Cp = E([pl, Pp, Ip], (e, t, r) => {
  if (!(e == null || t == null || r == null))
    return [t, r];
});
E([hl, Cp], Za);
var Tp = E([ne, Ja, Pp, Ip, Gt, qt], (e, t, r, n, i, a) => {
  if (!(e !== "centric" && e !== "radial" || t == null || r == null || n == null)) {
    var o = t.cx, u = t.cy, l = t.startAngle, c = t.endAngle;
    return {
      cx: yt(o, i, i / 2),
      cy: yt(u, a, a / 2),
      innerRadius: r,
      outerRadius: n,
      startAngle: l,
      endAngle: c,
      clockWise: !1
      // this property look useful, why not use it?
    };
  }
}), Ie = (e, t) => t, eo = (e, t, r) => r;
function ml(e) {
  return e == null ? void 0 : e.id;
}
function Mp(e, t, r) {
  var n = t.chartData, i = n === void 0 ? [] : n, a = r.allowDuplicatedCategory, o = r.dataKey, u = /* @__PURE__ */ new Map();
  return e.forEach((l) => {
    var c, s = (c = l.data) !== null && c !== void 0 ? c : i;
    if (!(s == null || s.length === 0)) {
      var f = ml(l);
      s.forEach((d, h) => {
        var p = o == null || a ? h : String(Se(d, o, null)), v = Se(d, l.dataKey, 0), m;
        u.has(p) ? m = u.get(p) : m = {}, Object.assign(m, {
          [f]: v
        }), u.set(p, m);
      });
    }
  }), Array.from(u.values());
}
function to(e) {
  return "stackId" in e && e.stackId != null && e.dataKey != null;
}
var ai = (e, t) => e === t ? !0 : e == null || t == null ? !1 : e[0] === t[0] && e[1] === t[1];
function ro(e, t) {
  return Array.isArray(e) && Array.isArray(t) && e.length === 0 && t.length === 0 ? !0 : e === t;
}
function q1(e, t) {
  if (e.length === t.length) {
    for (var r = 0; r < e.length; r++)
      if (e[r] !== t[r])
        return !1;
    return !0;
  }
  return !1;
}
var ke = (e) => {
  var t = ne(e);
  return t === "horizontal" ? "xAxis" : t === "vertical" ? "yAxis" : t === "centric" ? "angleAxis" : "radiusAxis";
}, pn = (e) => e.tooltip.settings.axisId;
function yl(e) {
  if (e != null) {
    var t = e.ticks, r = e.bandwidth, n = e.range(), i = [Math.min(...n), Math.max(...n)];
    return {
      domain: () => e.domain(),
      range: function(a) {
        function o() {
          return a.apply(this, arguments);
        }
        return o.toString = function() {
          return a.toString();
        }, o;
      }(() => i),
      rangeMin: () => i[0],
      rangeMax: () => i[1],
      isInRange(a) {
        var o = i[0], u = i[1];
        return o <= u ? a >= o && a <= u : a >= u && a <= o;
      },
      bandwidth: r ? () => r.call(e) : void 0,
      ticks: t ? (a) => t.call(e, a) : void 0,
      map: (a, o) => {
        var u = e(a);
        if (u != null) {
          if (e.bandwidth && o !== null && o !== void 0 && o.position) {
            var l = e.bandwidth();
            switch (o.position) {
              case "middle":
                u += l / 2;
                break;
              case "end":
                u += l;
                break;
            }
          }
          return u;
        }
      }
    };
  }
}
var X1 = (e, t) => {
  if (t != null)
    switch (e) {
      case "linear": {
        if (!It(t)) {
          for (var r, n, i = 0; i < t.length; i++) {
            var a = t[i];
            Y(a) && ((r === void 0 || a < r) && (r = a), (n === void 0 || a > n) && (n = a));
          }
          return r !== void 0 && n !== void 0 ? [r, n] : void 0;
        }
        return t;
      }
      default:
        return t;
    }
};
function ur(e, t) {
  return e == null || t == null ? NaN : e < t ? -1 : e > t ? 1 : e >= t ? 0 : NaN;
}
function Z1(e, t) {
  return e == null || t == null ? NaN : t < e ? -1 : t > e ? 1 : t >= e ? 0 : NaN;
}
function gl(e) {
  let t, r, n;
  e.length !== 2 ? (t = ur, r = (u, l) => ur(e(u), l), n = (u, l) => e(u) - l) : (t = e === ur || e === Z1 ? e : Q1, r = e, n = e);
  function i(u, l, c = 0, s = u.length) {
    if (c < s) {
      if (t(l, l) !== 0) return s;
      do {
        const f = c + s >>> 1;
        r(u[f], l) < 0 ? c = f + 1 : s = f;
      } while (c < s);
    }
    return c;
  }
  function a(u, l, c = 0, s = u.length) {
    if (c < s) {
      if (t(l, l) !== 0) return s;
      do {
        const f = c + s >>> 1;
        r(u[f], l) <= 0 ? c = f + 1 : s = f;
      } while (c < s);
    }
    return c;
  }
  function o(u, l, c = 0, s = u.length) {
    const f = i(u, l, c, s - 1);
    return f > c && n(u[f - 1], l) > -n(u[f], l) ? f - 1 : f;
  }
  return { left: i, center: o, right: a };
}
function Q1() {
  return 0;
}
function Dp(e) {
  return e === null ? NaN : +e;
}
function* J1(e, t) {
  for (let r of e)
    r != null && (r = +r) >= r && (yield r);
}
const eE = gl(ur), oi = eE.right;
gl(Dp).center;
class vf extends Map {
  constructor(t, r = nE) {
    if (super(), Object.defineProperties(this, { _intern: { value: /* @__PURE__ */ new Map() }, _key: { value: r } }), t != null) for (const [n, i] of t) this.set(n, i);
  }
  get(t) {
    return super.get(hf(this, t));
  }
  has(t) {
    return super.has(hf(this, t));
  }
  set(t, r) {
    return super.set(tE(this, t), r);
  }
  delete(t) {
    return super.delete(rE(this, t));
  }
}
function hf({ _intern: e, _key: t }, r) {
  const n = t(r);
  return e.has(n) ? e.get(n) : r;
}
function tE({ _intern: e, _key: t }, r) {
  const n = t(r);
  return e.has(n) ? e.get(n) : (e.set(n, r), r);
}
function rE({ _intern: e, _key: t }, r) {
  const n = t(r);
  return e.has(n) && (r = e.get(n), e.delete(n)), r;
}
function nE(e) {
  return e !== null && typeof e == "object" ? e.valueOf() : e;
}
function iE(e = ur) {
  if (e === ur) return Np;
  if (typeof e != "function") throw new TypeError("compare is not a function");
  return (t, r) => {
    const n = e(t, r);
    return n || n === 0 ? n : (e(r, r) === 0) - (e(t, t) === 0);
  };
}
function Np(e, t) {
  return (e == null || !(e >= e)) - (t == null || !(t >= t)) || (e < t ? -1 : e > t ? 1 : 0);
}
const aE = Math.sqrt(50), oE = Math.sqrt(10), uE = Math.sqrt(2);
function ca(e, t, r) {
  const n = (t - e) / Math.max(0, r), i = Math.floor(Math.log10(n)), a = n / Math.pow(10, i), o = a >= aE ? 10 : a >= oE ? 5 : a >= uE ? 2 : 1;
  let u, l, c;
  return i < 0 ? (c = Math.pow(10, -i) / o, u = Math.round(e * c), l = Math.round(t * c), u / c < e && ++u, l / c > t && --l, c = -c) : (c = Math.pow(10, i) * o, u = Math.round(e / c), l = Math.round(t / c), u * c < e && ++u, l * c > t && --l), l < u && 0.5 <= r && r < 2 ? ca(e, t, r * 2) : [u, l, c];
}
function bu(e, t, r) {
  if (t = +t, e = +e, r = +r, !(r > 0)) return [];
  if (e === t) return [e];
  const n = t < e, [i, a, o] = n ? ca(t, e, r) : ca(e, t, r);
  if (!(a >= i)) return [];
  const u = a - i + 1, l = new Array(u);
  if (n)
    if (o < 0) for (let c = 0; c < u; ++c) l[c] = (a - c) / -o;
    else for (let c = 0; c < u; ++c) l[c] = (a - c) * o;
  else if (o < 0) for (let c = 0; c < u; ++c) l[c] = (i + c) / -o;
  else for (let c = 0; c < u; ++c) l[c] = (i + c) * o;
  return l;
}
function xu(e, t, r) {
  return t = +t, e = +e, r = +r, ca(e, t, r)[2];
}
function wu(e, t, r) {
  t = +t, e = +e, r = +r;
  const n = t < e, i = n ? xu(t, e, r) : xu(e, t, r);
  return (n ? -1 : 1) * (i < 0 ? 1 / -i : i);
}
function pf(e, t) {
  let r;
  for (const n of e)
    n != null && (r < n || r === void 0 && n >= n) && (r = n);
  return r;
}
function mf(e, t) {
  let r;
  for (const n of e)
    n != null && (r > n || r === void 0 && n >= n) && (r = n);
  return r;
}
function jp(e, t, r = 0, n = 1 / 0, i) {
  if (t = Math.floor(t), r = Math.floor(Math.max(0, r)), n = Math.floor(Math.min(e.length - 1, n)), !(r <= t && t <= n)) return e;
  for (i = i === void 0 ? Np : iE(i); n > r; ) {
    if (n - r > 600) {
      const l = n - r + 1, c = t - r + 1, s = Math.log(l), f = 0.5 * Math.exp(2 * s / 3), d = 0.5 * Math.sqrt(s * f * (l - f) / l) * (c - l / 2 < 0 ? -1 : 1), h = Math.max(r, Math.floor(t - c * f / l + d)), p = Math.min(n, Math.floor(t + (l - c) * f / l + d));
      jp(e, t, h, p, i);
    }
    const a = e[t];
    let o = r, u = n;
    for (In(e, r, t), i(e[n], a) > 0 && In(e, r, n); o < u; ) {
      for (In(e, o, u), ++o, --u; i(e[o], a) < 0; ) ++o;
      for (; i(e[u], a) > 0; ) --u;
    }
    i(e[r], a) === 0 ? In(e, r, u) : (++u, In(e, u, n)), u <= t && (r = u + 1), t <= u && (n = u - 1);
  }
  return e;
}
function In(e, t, r) {
  const n = e[t];
  e[t] = e[r], e[r] = n;
}
function lE(e, t, r) {
  if (e = Float64Array.from(J1(e)), !(!(n = e.length) || isNaN(t = +t))) {
    if (t <= 0 || n < 2) return mf(e);
    if (t >= 1) return pf(e);
    var n, i = (n - 1) * t, a = Math.floor(i), o = pf(jp(e, a).subarray(0, a + 1)), u = mf(e.subarray(a + 1));
    return o + (u - o) * (i - a);
  }
}
function cE(e, t, r = Dp) {
  if (!(!(n = e.length) || isNaN(t = +t))) {
    if (t <= 0 || n < 2) return +r(e[0], 0, e);
    if (t >= 1) return +r(e[n - 1], n - 1, e);
    var n, i = (n - 1) * t, a = Math.floor(i), o = +r(e[a], a, e), u = +r(e[a + 1], a + 1, e);
    return o + (u - o) * (i - a);
  }
}
function sE(e, t, r) {
  e = +e, t = +t, r = (i = arguments.length) < 2 ? (t = e, e = 0, 1) : i < 3 ? 1 : +r;
  for (var n = -1, i = Math.max(0, Math.ceil((t - e) / r)) | 0, a = new Array(i); ++n < i; )
    a[n] = e + n * r;
  return a;
}
function ft(e, t) {
  switch (arguments.length) {
    case 0:
      break;
    case 1:
      this.range(e);
      break;
    default:
      this.range(t).domain(e);
      break;
  }
  return this;
}
function Xt(e, t) {
  switch (arguments.length) {
    case 0:
      break;
    case 1: {
      typeof e == "function" ? this.interpolator(e) : this.range(e);
      break;
    }
    default: {
      this.domain(e), typeof t == "function" ? this.interpolator(t) : this.range(t);
      break;
    }
  }
  return this;
}
const Au = Symbol("implicit");
function bl() {
  var e = new vf(), t = [], r = [], n = Au;
  function i(a) {
    let o = e.get(a);
    if (o === void 0) {
      if (n !== Au) return n;
      e.set(a, o = t.push(a) - 1);
    }
    return r[o % r.length];
  }
  return i.domain = function(a) {
    if (!arguments.length) return t.slice();
    t = [], e = new vf();
    for (const o of a)
      e.has(o) || e.set(o, t.push(o) - 1);
    return i;
  }, i.range = function(a) {
    return arguments.length ? (r = Array.from(a), i) : r.slice();
  }, i.unknown = function(a) {
    return arguments.length ? (n = a, i) : n;
  }, i.copy = function() {
    return bl(t, r).unknown(n);
  }, ft.apply(i, arguments), i;
}
function xl() {
  var e = bl().unknown(void 0), t = e.domain, r = e.range, n = 0, i = 1, a, o, u = !1, l = 0, c = 0, s = 0.5;
  delete e.unknown;
  function f() {
    var d = t().length, h = i < n, p = h ? i : n, v = h ? n : i;
    a = (v - p) / Math.max(1, d - l + c * 2), u && (a = Math.floor(a)), p += (v - p - a * (d - l)) * s, o = a * (1 - l), u && (p = Math.round(p), o = Math.round(o));
    var m = sE(d).map(function(y) {
      return p + a * y;
    });
    return r(h ? m.reverse() : m);
  }
  return e.domain = function(d) {
    return arguments.length ? (t(d), f()) : t();
  }, e.range = function(d) {
    return arguments.length ? ([n, i] = d, n = +n, i = +i, f()) : [n, i];
  }, e.rangeRound = function(d) {
    return [n, i] = d, n = +n, i = +i, u = !0, f();
  }, e.bandwidth = function() {
    return o;
  }, e.step = function() {
    return a;
  }, e.round = function(d) {
    return arguments.length ? (u = !!d, f()) : u;
  }, e.padding = function(d) {
    return arguments.length ? (l = Math.min(1, c = +d), f()) : l;
  }, e.paddingInner = function(d) {
    return arguments.length ? (l = Math.min(1, d), f()) : l;
  }, e.paddingOuter = function(d) {
    return arguments.length ? (c = +d, f()) : c;
  }, e.align = function(d) {
    return arguments.length ? (s = Math.max(0, Math.min(1, d)), f()) : s;
  }, e.copy = function() {
    return xl(t(), [n, i]).round(u).paddingInner(l).paddingOuter(c).align(s);
  }, ft.apply(f(), arguments);
}
function $p(e) {
  var t = e.copy;
  return e.padding = e.paddingOuter, delete e.paddingInner, delete e.paddingOuter, e.copy = function() {
    return $p(t());
  }, e;
}
function fE() {
  return $p(xl.apply(null, arguments).paddingInner(1));
}
function wl(e, t, r) {
  e.prototype = t.prototype = r, r.constructor = e;
}
function Rp(e, t) {
  var r = Object.create(e.prototype);
  for (var n in t) r[n] = t[n];
  return r;
}
function ui() {
}
var Kn = 0.7, sa = 1 / Kn, Qr = "\\s*([+-]?\\d+)\\s*", Hn = "\\s*([+-]?(?:\\d*\\.)?\\d+(?:[eE][+-]?\\d+)?)\\s*", kt = "\\s*([+-]?(?:\\d*\\.)?\\d+(?:[eE][+-]?\\d+)?)%\\s*", dE = /^#([0-9a-f]{3,8})$/, vE = new RegExp(`^rgb\\(${Qr},${Qr},${Qr}\\)$`), hE = new RegExp(`^rgb\\(${kt},${kt},${kt}\\)$`), pE = new RegExp(`^rgba\\(${Qr},${Qr},${Qr},${Hn}\\)$`), mE = new RegExp(`^rgba\\(${kt},${kt},${kt},${Hn}\\)$`), yE = new RegExp(`^hsl\\(${Hn},${kt},${kt}\\)$`), gE = new RegExp(`^hsla\\(${Hn},${kt},${kt},${Hn}\\)$`), yf = {
  aliceblue: 15792383,
  antiquewhite: 16444375,
  aqua: 65535,
  aquamarine: 8388564,
  azure: 15794175,
  beige: 16119260,
  bisque: 16770244,
  black: 0,
  blanchedalmond: 16772045,
  blue: 255,
  blueviolet: 9055202,
  brown: 10824234,
  burlywood: 14596231,
  cadetblue: 6266528,
  chartreuse: 8388352,
  chocolate: 13789470,
  coral: 16744272,
  cornflowerblue: 6591981,
  cornsilk: 16775388,
  crimson: 14423100,
  cyan: 65535,
  darkblue: 139,
  darkcyan: 35723,
  darkgoldenrod: 12092939,
  darkgray: 11119017,
  darkgreen: 25600,
  darkgrey: 11119017,
  darkkhaki: 12433259,
  darkmagenta: 9109643,
  darkolivegreen: 5597999,
  darkorange: 16747520,
  darkorchid: 10040012,
  darkred: 9109504,
  darksalmon: 15308410,
  darkseagreen: 9419919,
  darkslateblue: 4734347,
  darkslategray: 3100495,
  darkslategrey: 3100495,
  darkturquoise: 52945,
  darkviolet: 9699539,
  deeppink: 16716947,
  deepskyblue: 49151,
  dimgray: 6908265,
  dimgrey: 6908265,
  dodgerblue: 2003199,
  firebrick: 11674146,
  floralwhite: 16775920,
  forestgreen: 2263842,
  fuchsia: 16711935,
  gainsboro: 14474460,
  ghostwhite: 16316671,
  gold: 16766720,
  goldenrod: 14329120,
  gray: 8421504,
  green: 32768,
  greenyellow: 11403055,
  grey: 8421504,
  honeydew: 15794160,
  hotpink: 16738740,
  indianred: 13458524,
  indigo: 4915330,
  ivory: 16777200,
  khaki: 15787660,
  lavender: 15132410,
  lavenderblush: 16773365,
  lawngreen: 8190976,
  lemonchiffon: 16775885,
  lightblue: 11393254,
  lightcoral: 15761536,
  lightcyan: 14745599,
  lightgoldenrodyellow: 16448210,
  lightgray: 13882323,
  lightgreen: 9498256,
  lightgrey: 13882323,
  lightpink: 16758465,
  lightsalmon: 16752762,
  lightseagreen: 2142890,
  lightskyblue: 8900346,
  lightslategray: 7833753,
  lightslategrey: 7833753,
  lightsteelblue: 11584734,
  lightyellow: 16777184,
  lime: 65280,
  limegreen: 3329330,
  linen: 16445670,
  magenta: 16711935,
  maroon: 8388608,
  mediumaquamarine: 6737322,
  mediumblue: 205,
  mediumorchid: 12211667,
  mediumpurple: 9662683,
  mediumseagreen: 3978097,
  mediumslateblue: 8087790,
  mediumspringgreen: 64154,
  mediumturquoise: 4772300,
  mediumvioletred: 13047173,
  midnightblue: 1644912,
  mintcream: 16121850,
  mistyrose: 16770273,
  moccasin: 16770229,
  navajowhite: 16768685,
  navy: 128,
  oldlace: 16643558,
  olive: 8421376,
  olivedrab: 7048739,
  orange: 16753920,
  orangered: 16729344,
  orchid: 14315734,
  palegoldenrod: 15657130,
  palegreen: 10025880,
  paleturquoise: 11529966,
  palevioletred: 14381203,
  papayawhip: 16773077,
  peachpuff: 16767673,
  peru: 13468991,
  pink: 16761035,
  plum: 14524637,
  powderblue: 11591910,
  purple: 8388736,
  rebeccapurple: 6697881,
  red: 16711680,
  rosybrown: 12357519,
  royalblue: 4286945,
  saddlebrown: 9127187,
  salmon: 16416882,
  sandybrown: 16032864,
  seagreen: 3050327,
  seashell: 16774638,
  sienna: 10506797,
  silver: 12632256,
  skyblue: 8900331,
  slateblue: 6970061,
  slategray: 7372944,
  slategrey: 7372944,
  snow: 16775930,
  springgreen: 65407,
  steelblue: 4620980,
  tan: 13808780,
  teal: 32896,
  thistle: 14204888,
  tomato: 16737095,
  turquoise: 4251856,
  violet: 15631086,
  wheat: 16113331,
  white: 16777215,
  whitesmoke: 16119285,
  yellow: 16776960,
  yellowgreen: 10145074
};
wl(ui, Yn, {
  copy(e) {
    return Object.assign(new this.constructor(), this, e);
  },
  displayable() {
    return this.rgb().displayable();
  },
  hex: gf,
  // Deprecated! Use color.formatHex.
  formatHex: gf,
  formatHex8: bE,
  formatHsl: xE,
  formatRgb: bf,
  toString: bf
});
function gf() {
  return this.rgb().formatHex();
}
function bE() {
  return this.rgb().formatHex8();
}
function xE() {
  return Lp(this).formatHsl();
}
function bf() {
  return this.rgb().formatRgb();
}
function Yn(e) {
  var t, r;
  return e = (e + "").trim().toLowerCase(), (t = dE.exec(e)) ? (r = t[1].length, t = parseInt(t[1], 16), r === 6 ? xf(t) : r === 3 ? new Ge(t >> 8 & 15 | t >> 4 & 240, t >> 4 & 15 | t & 240, (t & 15) << 4 | t & 15, 1) : r === 8 ? Ii(t >> 24 & 255, t >> 16 & 255, t >> 8 & 255, (t & 255) / 255) : r === 4 ? Ii(t >> 12 & 15 | t >> 8 & 240, t >> 8 & 15 | t >> 4 & 240, t >> 4 & 15 | t & 240, ((t & 15) << 4 | t & 15) / 255) : null) : (t = vE.exec(e)) ? new Ge(t[1], t[2], t[3], 1) : (t = hE.exec(e)) ? new Ge(t[1] * 255 / 100, t[2] * 255 / 100, t[3] * 255 / 100, 1) : (t = pE.exec(e)) ? Ii(t[1], t[2], t[3], t[4]) : (t = mE.exec(e)) ? Ii(t[1] * 255 / 100, t[2] * 255 / 100, t[3] * 255 / 100, t[4]) : (t = yE.exec(e)) ? Of(t[1], t[2] / 100, t[3] / 100, 1) : (t = gE.exec(e)) ? Of(t[1], t[2] / 100, t[3] / 100, t[4]) : yf.hasOwnProperty(e) ? xf(yf[e]) : e === "transparent" ? new Ge(NaN, NaN, NaN, 0) : null;
}
function xf(e) {
  return new Ge(e >> 16 & 255, e >> 8 & 255, e & 255, 1);
}
function Ii(e, t, r, n) {
  return n <= 0 && (e = t = r = NaN), new Ge(e, t, r, n);
}
function wE(e) {
  return e instanceof ui || (e = Yn(e)), e ? (e = e.rgb(), new Ge(e.r, e.g, e.b, e.opacity)) : new Ge();
}
function Ou(e, t, r, n) {
  return arguments.length === 1 ? wE(e) : new Ge(e, t, r, n ?? 1);
}
function Ge(e, t, r, n) {
  this.r = +e, this.g = +t, this.b = +r, this.opacity = +n;
}
wl(Ge, Ou, Rp(ui, {
  brighter(e) {
    return e = e == null ? sa : Math.pow(sa, e), new Ge(this.r * e, this.g * e, this.b * e, this.opacity);
  },
  darker(e) {
    return e = e == null ? Kn : Math.pow(Kn, e), new Ge(this.r * e, this.g * e, this.b * e, this.opacity);
  },
  rgb() {
    return this;
  },
  clamp() {
    return new Ge(Cr(this.r), Cr(this.g), Cr(this.b), fa(this.opacity));
  },
  displayable() {
    return -0.5 <= this.r && this.r < 255.5 && -0.5 <= this.g && this.g < 255.5 && -0.5 <= this.b && this.b < 255.5 && 0 <= this.opacity && this.opacity <= 1;
  },
  hex: wf,
  // Deprecated! Use color.formatHex.
  formatHex: wf,
  formatHex8: AE,
  formatRgb: Af,
  toString: Af
}));
function wf() {
  return `#${Er(this.r)}${Er(this.g)}${Er(this.b)}`;
}
function AE() {
  return `#${Er(this.r)}${Er(this.g)}${Er(this.b)}${Er((isNaN(this.opacity) ? 1 : this.opacity) * 255)}`;
}
function Af() {
  const e = fa(this.opacity);
  return `${e === 1 ? "rgb(" : "rgba("}${Cr(this.r)}, ${Cr(this.g)}, ${Cr(this.b)}${e === 1 ? ")" : `, ${e})`}`;
}
function fa(e) {
  return isNaN(e) ? 1 : Math.max(0, Math.min(1, e));
}
function Cr(e) {
  return Math.max(0, Math.min(255, Math.round(e) || 0));
}
function Er(e) {
  return e = Cr(e), (e < 16 ? "0" : "") + e.toString(16);
}
function Of(e, t, r, n) {
  return n <= 0 ? e = t = r = NaN : r <= 0 || r >= 1 ? e = t = NaN : t <= 0 && (e = NaN), new mt(e, t, r, n);
}
function Lp(e) {
  if (e instanceof mt) return new mt(e.h, e.s, e.l, e.opacity);
  if (e instanceof ui || (e = Yn(e)), !e) return new mt();
  if (e instanceof mt) return e;
  e = e.rgb();
  var t = e.r / 255, r = e.g / 255, n = e.b / 255, i = Math.min(t, r, n), a = Math.max(t, r, n), o = NaN, u = a - i, l = (a + i) / 2;
  return u ? (t === a ? o = (r - n) / u + (r < n) * 6 : r === a ? o = (n - t) / u + 2 : o = (t - r) / u + 4, u /= l < 0.5 ? a + i : 2 - a - i, o *= 60) : u = l > 0 && l < 1 ? 0 : o, new mt(o, u, l, e.opacity);
}
function OE(e, t, r, n) {
  return arguments.length === 1 ? Lp(e) : new mt(e, t, r, n ?? 1);
}
function mt(e, t, r, n) {
  this.h = +e, this.s = +t, this.l = +r, this.opacity = +n;
}
wl(mt, OE, Rp(ui, {
  brighter(e) {
    return e = e == null ? sa : Math.pow(sa, e), new mt(this.h, this.s, this.l * e, this.opacity);
  },
  darker(e) {
    return e = e == null ? Kn : Math.pow(Kn, e), new mt(this.h, this.s, this.l * e, this.opacity);
  },
  rgb() {
    var e = this.h % 360 + (this.h < 0) * 360, t = isNaN(e) || isNaN(this.s) ? 0 : this.s, r = this.l, n = r + (r < 0.5 ? r : 1 - r) * t, i = 2 * r - n;
    return new Ge(
      Do(e >= 240 ? e - 240 : e + 120, i, n),
      Do(e, i, n),
      Do(e < 120 ? e + 240 : e - 120, i, n),
      this.opacity
    );
  },
  clamp() {
    return new mt(Ef(this.h), ki(this.s), ki(this.l), fa(this.opacity));
  },
  displayable() {
    return (0 <= this.s && this.s <= 1 || isNaN(this.s)) && 0 <= this.l && this.l <= 1 && 0 <= this.opacity && this.opacity <= 1;
  },
  formatHsl() {
    const e = fa(this.opacity);
    return `${e === 1 ? "hsl(" : "hsla("}${Ef(this.h)}, ${ki(this.s) * 100}%, ${ki(this.l) * 100}%${e === 1 ? ")" : `, ${e})`}`;
  }
}));
function Ef(e) {
  return e = (e || 0) % 360, e < 0 ? e + 360 : e;
}
function ki(e) {
  return Math.max(0, Math.min(1, e || 0));
}
function Do(e, t, r) {
  return (e < 60 ? t + (r - t) * e / 60 : e < 180 ? r : e < 240 ? t + (r - t) * (240 - e) / 60 : t) * 255;
}
const Al = (e) => () => e;
function EE(e, t) {
  return function(r) {
    return e + r * t;
  };
}
function SE(e, t, r) {
  return e = Math.pow(e, r), t = Math.pow(t, r) - e, r = 1 / r, function(n) {
    return Math.pow(e + n * t, r);
  };
}
function _E(e) {
  return (e = +e) == 1 ? zp : function(t, r) {
    return r - t ? SE(t, r, e) : Al(isNaN(t) ? r : t);
  };
}
function zp(e, t) {
  var r = t - e;
  return r ? EE(e, r) : Al(isNaN(e) ? t : e);
}
const Sf = function e(t) {
  var r = _E(t);
  function n(i, a) {
    var o = r((i = Ou(i)).r, (a = Ou(a)).r), u = r(i.g, a.g), l = r(i.b, a.b), c = zp(i.opacity, a.opacity);
    return function(s) {
      return i.r = o(s), i.g = u(s), i.b = l(s), i.opacity = c(s), i + "";
    };
  }
  return n.gamma = e, n;
}(1);
function PE(e, t) {
  t || (t = []);
  var r = e ? Math.min(t.length, e.length) : 0, n = t.slice(), i;
  return function(a) {
    for (i = 0; i < r; ++i) n[i] = e[i] * (1 - a) + t[i] * a;
    return n;
  };
}
function IE(e) {
  return ArrayBuffer.isView(e) && !(e instanceof DataView);
}
function kE(e, t) {
  var r = t ? t.length : 0, n = e ? Math.min(r, e.length) : 0, i = new Array(n), a = new Array(r), o;
  for (o = 0; o < n; ++o) i[o] = mn(e[o], t[o]);
  for (; o < r; ++o) a[o] = t[o];
  return function(u) {
    for (o = 0; o < n; ++o) a[o] = i[o](u);
    return a;
  };
}
function CE(e, t) {
  var r = /* @__PURE__ */ new Date();
  return e = +e, t = +t, function(n) {
    return r.setTime(e * (1 - n) + t * n), r;
  };
}
function da(e, t) {
  return e = +e, t = +t, function(r) {
    return e * (1 - r) + t * r;
  };
}
function TE(e, t) {
  var r = {}, n = {}, i;
  (e === null || typeof e != "object") && (e = {}), (t === null || typeof t != "object") && (t = {});
  for (i in t)
    i in e ? r[i] = mn(e[i], t[i]) : n[i] = t[i];
  return function(a) {
    for (i in r) n[i] = r[i](a);
    return n;
  };
}
var Eu = /[-+]?(?:\d+\.?\d*|\.?\d+)(?:[eE][-+]?\d+)?/g, No = new RegExp(Eu.source, "g");
function ME(e) {
  return function() {
    return e;
  };
}
function DE(e) {
  return function(t) {
    return e(t) + "";
  };
}
function NE(e, t) {
  var r = Eu.lastIndex = No.lastIndex = 0, n, i, a, o = -1, u = [], l = [];
  for (e = e + "", t = t + ""; (n = Eu.exec(e)) && (i = No.exec(t)); )
    (a = i.index) > r && (a = t.slice(r, a), u[o] ? u[o] += a : u[++o] = a), (n = n[0]) === (i = i[0]) ? u[o] ? u[o] += i : u[++o] = i : (u[++o] = null, l.push({ i: o, x: da(n, i) })), r = No.lastIndex;
  return r < t.length && (a = t.slice(r), u[o] ? u[o] += a : u[++o] = a), u.length < 2 ? l[0] ? DE(l[0].x) : ME(t) : (t = l.length, function(c) {
    for (var s = 0, f; s < t; ++s) u[(f = l[s]).i] = f.x(c);
    return u.join("");
  });
}
function mn(e, t) {
  var r = typeof t, n;
  return t == null || r === "boolean" ? Al(t) : (r === "number" ? da : r === "string" ? (n = Yn(t)) ? (t = n, Sf) : NE : t instanceof Yn ? Sf : t instanceof Date ? CE : IE(t) ? PE : Array.isArray(t) ? kE : typeof t.valueOf != "function" && typeof t.toString != "function" || isNaN(t) ? TE : da)(e, t);
}
function Ol(e, t) {
  return e = +e, t = +t, function(r) {
    return Math.round(e * (1 - r) + t * r);
  };
}
function jE(e, t) {
  t === void 0 && (t = e, e = mn);
  for (var r = 0, n = t.length - 1, i = t[0], a = new Array(n < 0 ? 0 : n); r < n; ) a[r] = e(i, i = t[++r]);
  return function(o) {
    var u = Math.max(0, Math.min(n - 1, Math.floor(o *= n)));
    return a[u](o - u);
  };
}
function $E(e) {
  return function() {
    return e;
  };
}
function va(e) {
  return +e;
}
var _f = [0, 1];
function Ve(e) {
  return e;
}
function Su(e, t) {
  return (t -= e = +e) ? function(r) {
    return (r - e) / t;
  } : $E(isNaN(t) ? NaN : 0.5);
}
function RE(e, t) {
  var r;
  return e > t && (r = e, e = t, t = r), function(n) {
    return Math.max(e, Math.min(t, n));
  };
}
function LE(e, t, r) {
  var n = e[0], i = e[1], a = t[0], o = t[1];
  return i < n ? (n = Su(i, n), a = r(o, a)) : (n = Su(n, i), a = r(a, o)), function(u) {
    return a(n(u));
  };
}
function zE(e, t, r) {
  var n = Math.min(e.length, t.length) - 1, i = new Array(n), a = new Array(n), o = -1;
  for (e[n] < e[0] && (e = e.slice().reverse(), t = t.slice().reverse()); ++o < n; )
    i[o] = Su(e[o], e[o + 1]), a[o] = r(t[o], t[o + 1]);
  return function(u) {
    var l = oi(e, u, 1, n) - 1;
    return a[l](i[l](u));
  };
}
function li(e, t) {
  return t.domain(e.domain()).range(e.range()).interpolate(e.interpolate()).clamp(e.clamp()).unknown(e.unknown());
}
function no() {
  var e = _f, t = _f, r = mn, n, i, a, o = Ve, u, l, c;
  function s() {
    var d = Math.min(e.length, t.length);
    return o !== Ve && (o = RE(e[0], e[d - 1])), u = d > 2 ? zE : LE, l = c = null, f;
  }
  function f(d) {
    return d == null || isNaN(d = +d) ? a : (l || (l = u(e.map(n), t, r)))(n(o(d)));
  }
  return f.invert = function(d) {
    return o(i((c || (c = u(t, e.map(n), da)))(d)));
  }, f.domain = function(d) {
    return arguments.length ? (e = Array.from(d, va), s()) : e.slice();
  }, f.range = function(d) {
    return arguments.length ? (t = Array.from(d), s()) : t.slice();
  }, f.rangeRound = function(d) {
    return t = Array.from(d), r = Ol, s();
  }, f.clamp = function(d) {
    return arguments.length ? (o = d ? !0 : Ve, s()) : o !== Ve;
  }, f.interpolate = function(d) {
    return arguments.length ? (r = d, s()) : r;
  }, f.unknown = function(d) {
    return arguments.length ? (a = d, f) : a;
  }, function(d, h) {
    return n = d, i = h, s();
  };
}
function El() {
  return no()(Ve, Ve);
}
function BE(e) {
  return Math.abs(e = Math.round(e)) >= 1e21 ? e.toLocaleString("en").replace(/,/g, "") : e.toString(10);
}
function ha(e, t) {
  if (!isFinite(e) || e === 0) return null;
  var r = (e = t ? e.toExponential(t - 1) : e.toExponential()).indexOf("e"), n = e.slice(0, r);
  return [
    n.length > 1 ? n[0] + n.slice(2) : n,
    +e.slice(r + 1)
  ];
}
function tn(e) {
  return e = ha(Math.abs(e)), e ? e[1] : NaN;
}
function FE(e, t) {
  return function(r, n) {
    for (var i = r.length, a = [], o = 0, u = e[0], l = 0; i > 0 && u > 0 && (l + u + 1 > n && (u = Math.max(1, n - l)), a.push(r.substring(i -= u, i + u)), !((l += u + 1) > n)); )
      u = e[o = (o + 1) % e.length];
    return a.reverse().join(t);
  };
}
function WE(e) {
  return function(t) {
    return t.replace(/[0-9]/g, function(r) {
      return e[+r];
    });
  };
}
var UE = /^(?:(.)?([<>=^]))?([+\-( ])?([$#])?(0)?(\d+)?(,)?(\.\d+)?(~)?([a-z%])?$/i;
function Gn(e) {
  if (!(t = UE.exec(e))) throw new Error("invalid format: " + e);
  var t;
  return new Sl({
    fill: t[1],
    align: t[2],
    sign: t[3],
    symbol: t[4],
    zero: t[5],
    width: t[6],
    comma: t[7],
    precision: t[8] && t[8].slice(1),
    trim: t[9],
    type: t[10]
  });
}
Gn.prototype = Sl.prototype;
function Sl(e) {
  this.fill = e.fill === void 0 ? " " : e.fill + "", this.align = e.align === void 0 ? ">" : e.align + "", this.sign = e.sign === void 0 ? "-" : e.sign + "", this.symbol = e.symbol === void 0 ? "" : e.symbol + "", this.zero = !!e.zero, this.width = e.width === void 0 ? void 0 : +e.width, this.comma = !!e.comma, this.precision = e.precision === void 0 ? void 0 : +e.precision, this.trim = !!e.trim, this.type = e.type === void 0 ? "" : e.type + "";
}
Sl.prototype.toString = function() {
  return this.fill + this.align + this.sign + this.symbol + (this.zero ? "0" : "") + (this.width === void 0 ? "" : Math.max(1, this.width | 0)) + (this.comma ? "," : "") + (this.precision === void 0 ? "" : "." + Math.max(0, this.precision | 0)) + (this.trim ? "~" : "") + this.type;
};
function VE(e) {
  e: for (var t = e.length, r = 1, n = -1, i; r < t; ++r)
    switch (e[r]) {
      case ".":
        n = i = r;
        break;
      case "0":
        n === 0 && (n = r), i = r;
        break;
      default:
        if (!+e[r]) break e;
        n > 0 && (n = 0);
        break;
    }
  return n > 0 ? e.slice(0, n) + e.slice(i + 1) : e;
}
var pa;
function KE(e, t) {
  var r = ha(e, t);
  if (!r) return pa = void 0, e.toPrecision(t);
  var n = r[0], i = r[1], a = i - (pa = Math.max(-8, Math.min(8, Math.floor(i / 3))) * 3) + 1, o = n.length;
  return a === o ? n : a > o ? n + new Array(a - o + 1).join("0") : a > 0 ? n.slice(0, a) + "." + n.slice(a) : "0." + new Array(1 - a).join("0") + ha(e, Math.max(0, t + a - 1))[0];
}
function Pf(e, t) {
  var r = ha(e, t);
  if (!r) return e + "";
  var n = r[0], i = r[1];
  return i < 0 ? "0." + new Array(-i).join("0") + n : n.length > i + 1 ? n.slice(0, i + 1) + "." + n.slice(i + 1) : n + new Array(i - n.length + 2).join("0");
}
const If = {
  "%": (e, t) => (e * 100).toFixed(t),
  b: (e) => Math.round(e).toString(2),
  c: (e) => e + "",
  d: BE,
  e: (e, t) => e.toExponential(t),
  f: (e, t) => e.toFixed(t),
  g: (e, t) => e.toPrecision(t),
  o: (e) => Math.round(e).toString(8),
  p: (e, t) => Pf(e * 100, t),
  r: Pf,
  s: KE,
  X: (e) => Math.round(e).toString(16).toUpperCase(),
  x: (e) => Math.round(e).toString(16)
};
function kf(e) {
  return e;
}
var Cf = Array.prototype.map, Tf = ["y", "z", "a", "f", "p", "n", "µ", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y"];
function HE(e) {
  var t = e.grouping === void 0 || e.thousands === void 0 ? kf : FE(Cf.call(e.grouping, Number), e.thousands + ""), r = e.currency === void 0 ? "" : e.currency[0] + "", n = e.currency === void 0 ? "" : e.currency[1] + "", i = e.decimal === void 0 ? "." : e.decimal + "", a = e.numerals === void 0 ? kf : WE(Cf.call(e.numerals, String)), o = e.percent === void 0 ? "%" : e.percent + "", u = e.minus === void 0 ? "−" : e.minus + "", l = e.nan === void 0 ? "NaN" : e.nan + "";
  function c(f, d) {
    f = Gn(f);
    var h = f.fill, p = f.align, v = f.sign, m = f.symbol, y = f.zero, b = f.width, x = f.comma, w = f.precision, O = f.trim, g = f.type;
    g === "n" ? (x = !0, g = "g") : If[g] || (w === void 0 && (w = 12), O = !0, g = "g"), (y || h === "0" && p === "=") && (y = !0, h = "0", p = "=");
    var S = (d && d.prefix !== void 0 ? d.prefix : "") + (m === "$" ? r : m === "#" && /[boxX]/.test(g) ? "0" + g.toLowerCase() : ""), P = (m === "$" ? n : /[%p]/.test(g) ? o : "") + (d && d.suffix !== void 0 ? d.suffix : ""), I = If[g], C = /[defgprs%]/.test(g);
    w = w === void 0 ? 6 : /[gprs]/.test(g) ? Math.max(1, Math.min(21, w)) : Math.max(0, Math.min(20, w));
    function T(_) {
      var z = S, $ = P, U, V, K;
      if (g === "c")
        $ = I(_) + $, _ = "";
      else {
        _ = +_;
        var L = _ < 0 || 1 / _ < 0;
        if (_ = isNaN(_) ? l : I(Math.abs(_), w), O && (_ = VE(_)), L && +_ == 0 && v !== "+" && (L = !1), z = (L ? v === "(" ? v : u : v === "-" || v === "(" ? "" : v) + z, $ = (g === "s" && !isNaN(_) && pa !== void 0 ? Tf[8 + pa / 3] : "") + $ + (L && v === "(" ? ")" : ""), C) {
          for (U = -1, V = _.length; ++U < V; )
            if (K = _.charCodeAt(U), 48 > K || K > 57) {
              $ = (K === 46 ? i + _.slice(U + 1) : _.slice(U)) + $, _ = _.slice(0, U);
              break;
            }
        }
      }
      x && !y && (_ = t(_, 1 / 0));
      var G = z.length + _.length + $.length, F = G < b ? new Array(b - G + 1).join(h) : "";
      switch (x && y && (_ = t(F + _, F.length ? b - $.length : 1 / 0), F = ""), p) {
        case "<":
          _ = z + _ + $ + F;
          break;
        case "=":
          _ = z + F + _ + $;
          break;
        case "^":
          _ = F.slice(0, G = F.length >> 1) + z + _ + $ + F.slice(G);
          break;
        default:
          _ = F + z + _ + $;
          break;
      }
      return a(_);
    }
    return T.toString = function() {
      return f + "";
    }, T;
  }
  function s(f, d) {
    var h = Math.max(-8, Math.min(8, Math.floor(tn(d) / 3))) * 3, p = Math.pow(10, -h), v = c((f = Gn(f), f.type = "f", f), { suffix: Tf[8 + h / 3] });
    return function(m) {
      return v(p * m);
    };
  }
  return {
    format: c,
    formatPrefix: s
  };
}
var Ci, _l, Bp;
YE({
  thousands: ",",
  grouping: [3],
  currency: ["$", ""]
});
function YE(e) {
  return Ci = HE(e), _l = Ci.format, Bp = Ci.formatPrefix, Ci;
}
function GE(e) {
  return Math.max(0, -tn(Math.abs(e)));
}
function qE(e, t) {
  return Math.max(0, Math.max(-8, Math.min(8, Math.floor(tn(t) / 3))) * 3 - tn(Math.abs(e)));
}
function XE(e, t) {
  return e = Math.abs(e), t = Math.abs(t) - e, Math.max(0, tn(t) - tn(e)) + 1;
}
function Fp(e, t, r, n) {
  var i = wu(e, t, r), a;
  switch (n = Gn(n ?? ",f"), n.type) {
    case "s": {
      var o = Math.max(Math.abs(e), Math.abs(t));
      return n.precision == null && !isNaN(a = qE(i, o)) && (n.precision = a), Bp(n, o);
    }
    case "":
    case "e":
    case "g":
    case "p":
    case "r": {
      n.precision == null && !isNaN(a = XE(i, Math.max(Math.abs(e), Math.abs(t)))) && (n.precision = a - (n.type === "e"));
      break;
    }
    case "f":
    case "%": {
      n.precision == null && !isNaN(a = GE(i)) && (n.precision = a - (n.type === "%") * 2);
      break;
    }
  }
  return _l(n);
}
function dr(e) {
  var t = e.domain;
  return e.ticks = function(r) {
    var n = t();
    return bu(n[0], n[n.length - 1], r ?? 10);
  }, e.tickFormat = function(r, n) {
    var i = t();
    return Fp(i[0], i[i.length - 1], r ?? 10, n);
  }, e.nice = function(r) {
    r == null && (r = 10);
    var n = t(), i = 0, a = n.length - 1, o = n[i], u = n[a], l, c, s = 10;
    for (u < o && (c = o, o = u, u = c, c = i, i = a, a = c); s-- > 0; ) {
      if (c = xu(o, u, r), c === l)
        return n[i] = o, n[a] = u, t(n);
      if (c > 0)
        o = Math.floor(o / c) * c, u = Math.ceil(u / c) * c;
      else if (c < 0)
        o = Math.ceil(o * c) / c, u = Math.floor(u * c) / c;
      else
        break;
      l = c;
    }
    return e;
  }, e;
}
function Wp() {
  var e = El();
  return e.copy = function() {
    return li(e, Wp());
  }, ft.apply(e, arguments), dr(e);
}
function Up(e) {
  var t;
  function r(n) {
    return n == null || isNaN(n = +n) ? t : n;
  }
  return r.invert = r, r.domain = r.range = function(n) {
    return arguments.length ? (e = Array.from(n, va), r) : e.slice();
  }, r.unknown = function(n) {
    return arguments.length ? (t = n, r) : t;
  }, r.copy = function() {
    return Up(e).unknown(t);
  }, e = arguments.length ? Array.from(e, va) : [0, 1], dr(r);
}
function Vp(e, t) {
  e = e.slice();
  var r = 0, n = e.length - 1, i = e[r], a = e[n], o;
  return a < i && (o = r, r = n, n = o, o = i, i = a, a = o), e[r] = t.floor(i), e[n] = t.ceil(a), e;
}
function Mf(e) {
  return Math.log(e);
}
function Df(e) {
  return Math.exp(e);
}
function ZE(e) {
  return -Math.log(-e);
}
function QE(e) {
  return -Math.exp(-e);
}
function JE(e) {
  return isFinite(e) ? +("1e" + e) : e < 0 ? 0 : e;
}
function eS(e) {
  return e === 10 ? JE : e === Math.E ? Math.exp : (t) => Math.pow(e, t);
}
function tS(e) {
  return e === Math.E ? Math.log : e === 10 && Math.log10 || e === 2 && Math.log2 || (e = Math.log(e), (t) => Math.log(t) / e);
}
function Nf(e) {
  return (t, r) => -e(-t, r);
}
function Pl(e) {
  const t = e(Mf, Df), r = t.domain;
  let n = 10, i, a;
  function o() {
    return i = tS(n), a = eS(n), r()[0] < 0 ? (i = Nf(i), a = Nf(a), e(ZE, QE)) : e(Mf, Df), t;
  }
  return t.base = function(u) {
    return arguments.length ? (n = +u, o()) : n;
  }, t.domain = function(u) {
    return arguments.length ? (r(u), o()) : r();
  }, t.ticks = (u) => {
    const l = r();
    let c = l[0], s = l[l.length - 1];
    const f = s < c;
    f && ([c, s] = [s, c]);
    let d = i(c), h = i(s), p, v;
    const m = u == null ? 10 : +u;
    let y = [];
    if (!(n % 1) && h - d < m) {
      if (d = Math.floor(d), h = Math.ceil(h), c > 0) {
        for (; d <= h; ++d)
          for (p = 1; p < n; ++p)
            if (v = d < 0 ? p / a(-d) : p * a(d), !(v < c)) {
              if (v > s) break;
              y.push(v);
            }
      } else for (; d <= h; ++d)
        for (p = n - 1; p >= 1; --p)
          if (v = d > 0 ? p / a(-d) : p * a(d), !(v < c)) {
            if (v > s) break;
            y.push(v);
          }
      y.length * 2 < m && (y = bu(c, s, m));
    } else
      y = bu(d, h, Math.min(h - d, m)).map(a);
    return f ? y.reverse() : y;
  }, t.tickFormat = (u, l) => {
    if (u == null && (u = 10), l == null && (l = n === 10 ? "s" : ","), typeof l != "function" && (!(n % 1) && (l = Gn(l)).precision == null && (l.trim = !0), l = _l(l)), u === 1 / 0) return l;
    const c = Math.max(1, n * u / t.ticks().length);
    return (s) => {
      let f = s / a(Math.round(i(s)));
      return f * n < n - 0.5 && (f *= n), f <= c ? l(s) : "";
    };
  }, t.nice = () => r(Vp(r(), {
    floor: (u) => a(Math.floor(i(u))),
    ceil: (u) => a(Math.ceil(i(u)))
  })), t;
}
function Kp() {
  const e = Pl(no()).domain([1, 10]);
  return e.copy = () => li(e, Kp()).base(e.base()), ft.apply(e, arguments), e;
}
function jf(e) {
  return function(t) {
    return Math.sign(t) * Math.log1p(Math.abs(t / e));
  };
}
function $f(e) {
  return function(t) {
    return Math.sign(t) * Math.expm1(Math.abs(t)) * e;
  };
}
function Il(e) {
  var t = 1, r = e(jf(t), $f(t));
  return r.constant = function(n) {
    return arguments.length ? e(jf(t = +n), $f(t)) : t;
  }, dr(r);
}
function Hp() {
  var e = Il(no());
  return e.copy = function() {
    return li(e, Hp()).constant(e.constant());
  }, ft.apply(e, arguments);
}
function Rf(e) {
  return function(t) {
    return t < 0 ? -Math.pow(-t, e) : Math.pow(t, e);
  };
}
function rS(e) {
  return e < 0 ? -Math.sqrt(-e) : Math.sqrt(e);
}
function nS(e) {
  return e < 0 ? -e * e : e * e;
}
function kl(e) {
  var t = e(Ve, Ve), r = 1;
  function n() {
    return r === 1 ? e(Ve, Ve) : r === 0.5 ? e(rS, nS) : e(Rf(r), Rf(1 / r));
  }
  return t.exponent = function(i) {
    return arguments.length ? (r = +i, n()) : r;
  }, dr(t);
}
function Cl() {
  var e = kl(no());
  return e.copy = function() {
    return li(e, Cl()).exponent(e.exponent());
  }, ft.apply(e, arguments), e;
}
function iS() {
  return Cl.apply(null, arguments).exponent(0.5);
}
function Lf(e) {
  return Math.sign(e) * e * e;
}
function aS(e) {
  return Math.sign(e) * Math.sqrt(Math.abs(e));
}
function Yp() {
  var e = El(), t = [0, 1], r = !1, n;
  function i(a) {
    var o = aS(e(a));
    return isNaN(o) ? n : r ? Math.round(o) : o;
  }
  return i.invert = function(a) {
    return e.invert(Lf(a));
  }, i.domain = function(a) {
    return arguments.length ? (e.domain(a), i) : e.domain();
  }, i.range = function(a) {
    return arguments.length ? (e.range((t = Array.from(a, va)).map(Lf)), i) : t.slice();
  }, i.rangeRound = function(a) {
    return i.range(a).round(!0);
  }, i.round = function(a) {
    return arguments.length ? (r = !!a, i) : r;
  }, i.clamp = function(a) {
    return arguments.length ? (e.clamp(a), i) : e.clamp();
  }, i.unknown = function(a) {
    return arguments.length ? (n = a, i) : n;
  }, i.copy = function() {
    return Yp(e.domain(), t).round(r).clamp(e.clamp()).unknown(n);
  }, ft.apply(i, arguments), dr(i);
}
function Gp() {
  var e = [], t = [], r = [], n;
  function i() {
    var o = 0, u = Math.max(1, t.length);
    for (r = new Array(u - 1); ++o < u; ) r[o - 1] = cE(e, o / u);
    return a;
  }
  function a(o) {
    return o == null || isNaN(o = +o) ? n : t[oi(r, o)];
  }
  return a.invertExtent = function(o) {
    var u = t.indexOf(o);
    return u < 0 ? [NaN, NaN] : [
      u > 0 ? r[u - 1] : e[0],
      u < r.length ? r[u] : e[e.length - 1]
    ];
  }, a.domain = function(o) {
    if (!arguments.length) return e.slice();
    e = [];
    for (let u of o) u != null && !isNaN(u = +u) && e.push(u);
    return e.sort(ur), i();
  }, a.range = function(o) {
    return arguments.length ? (t = Array.from(o), i()) : t.slice();
  }, a.unknown = function(o) {
    return arguments.length ? (n = o, a) : n;
  }, a.quantiles = function() {
    return r.slice();
  }, a.copy = function() {
    return Gp().domain(e).range(t).unknown(n);
  }, ft.apply(a, arguments);
}
function qp() {
  var e = 0, t = 1, r = 1, n = [0.5], i = [0, 1], a;
  function o(l) {
    return l != null && l <= l ? i[oi(n, l, 0, r)] : a;
  }
  function u() {
    var l = -1;
    for (n = new Array(r); ++l < r; ) n[l] = ((l + 1) * t - (l - r) * e) / (r + 1);
    return o;
  }
  return o.domain = function(l) {
    return arguments.length ? ([e, t] = l, e = +e, t = +t, u()) : [e, t];
  }, o.range = function(l) {
    return arguments.length ? (r = (i = Array.from(l)).length - 1, u()) : i.slice();
  }, o.invertExtent = function(l) {
    var c = i.indexOf(l);
    return c < 0 ? [NaN, NaN] : c < 1 ? [e, n[0]] : c >= r ? [n[r - 1], t] : [n[c - 1], n[c]];
  }, o.unknown = function(l) {
    return arguments.length && (a = l), o;
  }, o.thresholds = function() {
    return n.slice();
  }, o.copy = function() {
    return qp().domain([e, t]).range(i).unknown(a);
  }, ft.apply(dr(o), arguments);
}
function Xp() {
  var e = [0.5], t = [0, 1], r, n = 1;
  function i(a) {
    return a != null && a <= a ? t[oi(e, a, 0, n)] : r;
  }
  return i.domain = function(a) {
    return arguments.length ? (e = Array.from(a), n = Math.min(e.length, t.length - 1), i) : e.slice();
  }, i.range = function(a) {
    return arguments.length ? (t = Array.from(a), n = Math.min(e.length, t.length - 1), i) : t.slice();
  }, i.invertExtent = function(a) {
    var o = t.indexOf(a);
    return [e[o - 1], e[o]];
  }, i.unknown = function(a) {
    return arguments.length ? (r = a, i) : r;
  }, i.copy = function() {
    return Xp().domain(e).range(t).unknown(r);
  }, ft.apply(i, arguments);
}
const jo = /* @__PURE__ */ new Date(), $o = /* @__PURE__ */ new Date();
function Ae(e, t, r, n) {
  function i(a) {
    return e(a = arguments.length === 0 ? /* @__PURE__ */ new Date() : /* @__PURE__ */ new Date(+a)), a;
  }
  return i.floor = (a) => (e(a = /* @__PURE__ */ new Date(+a)), a), i.ceil = (a) => (e(a = new Date(a - 1)), t(a, 1), e(a), a), i.round = (a) => {
    const o = i(a), u = i.ceil(a);
    return a - o < u - a ? o : u;
  }, i.offset = (a, o) => (t(a = /* @__PURE__ */ new Date(+a), o == null ? 1 : Math.floor(o)), a), i.range = (a, o, u) => {
    const l = [];
    if (a = i.ceil(a), u = u == null ? 1 : Math.floor(u), !(a < o) || !(u > 0)) return l;
    let c;
    do
      l.push(c = /* @__PURE__ */ new Date(+a)), t(a, u), e(a);
    while (c < a && a < o);
    return l;
  }, i.filter = (a) => Ae((o) => {
    if (o >= o) for (; e(o), !a(o); ) o.setTime(o - 1);
  }, (o, u) => {
    if (o >= o)
      if (u < 0) for (; ++u <= 0; )
        for (; t(o, -1), !a(o); )
          ;
      else for (; --u >= 0; )
        for (; t(o, 1), !a(o); )
          ;
  }), r && (i.count = (a, o) => (jo.setTime(+a), $o.setTime(+o), e(jo), e($o), Math.floor(r(jo, $o))), i.every = (a) => (a = Math.floor(a), !isFinite(a) || !(a > 0) ? null : a > 1 ? i.filter(n ? (o) => n(o) % a === 0 : (o) => i.count(0, o) % a === 0) : i)), i;
}
const ma = Ae(() => {
}, (e, t) => {
  e.setTime(+e + t);
}, (e, t) => t - e);
ma.every = (e) => (e = Math.floor(e), !isFinite(e) || !(e > 0) ? null : e > 1 ? Ae((t) => {
  t.setTime(Math.floor(t / e) * e);
}, (t, r) => {
  t.setTime(+t + r * e);
}, (t, r) => (r - t) / e) : ma);
ma.range;
const Rt = 1e3, ut = Rt * 60, Lt = ut * 60, Kt = Lt * 24, Tl = Kt * 7, zf = Kt * 30, Ro = Kt * 365, Sr = Ae((e) => {
  e.setTime(e - e.getMilliseconds());
}, (e, t) => {
  e.setTime(+e + t * Rt);
}, (e, t) => (t - e) / Rt, (e) => e.getUTCSeconds());
Sr.range;
const Ml = Ae((e) => {
  e.setTime(e - e.getMilliseconds() - e.getSeconds() * Rt);
}, (e, t) => {
  e.setTime(+e + t * ut);
}, (e, t) => (t - e) / ut, (e) => e.getMinutes());
Ml.range;
const Dl = Ae((e) => {
  e.setUTCSeconds(0, 0);
}, (e, t) => {
  e.setTime(+e + t * ut);
}, (e, t) => (t - e) / ut, (e) => e.getUTCMinutes());
Dl.range;
const Nl = Ae((e) => {
  e.setTime(e - e.getMilliseconds() - e.getSeconds() * Rt - e.getMinutes() * ut);
}, (e, t) => {
  e.setTime(+e + t * Lt);
}, (e, t) => (t - e) / Lt, (e) => e.getHours());
Nl.range;
const jl = Ae((e) => {
  e.setUTCMinutes(0, 0, 0);
}, (e, t) => {
  e.setTime(+e + t * Lt);
}, (e, t) => (t - e) / Lt, (e) => e.getUTCHours());
jl.range;
const ci = Ae(
  (e) => e.setHours(0, 0, 0, 0),
  (e, t) => e.setDate(e.getDate() + t),
  (e, t) => (t - e - (t.getTimezoneOffset() - e.getTimezoneOffset()) * ut) / Kt,
  (e) => e.getDate() - 1
);
ci.range;
const io = Ae((e) => {
  e.setUTCHours(0, 0, 0, 0);
}, (e, t) => {
  e.setUTCDate(e.getUTCDate() + t);
}, (e, t) => (t - e) / Kt, (e) => e.getUTCDate() - 1);
io.range;
const Zp = Ae((e) => {
  e.setUTCHours(0, 0, 0, 0);
}, (e, t) => {
  e.setUTCDate(e.getUTCDate() + t);
}, (e, t) => (t - e) / Kt, (e) => Math.floor(e / Kt));
Zp.range;
function Rr(e) {
  return Ae((t) => {
    t.setDate(t.getDate() - (t.getDay() + 7 - e) % 7), t.setHours(0, 0, 0, 0);
  }, (t, r) => {
    t.setDate(t.getDate() + r * 7);
  }, (t, r) => (r - t - (r.getTimezoneOffset() - t.getTimezoneOffset()) * ut) / Tl);
}
const ao = Rr(0), ya = Rr(1), oS = Rr(2), uS = Rr(3), rn = Rr(4), lS = Rr(5), cS = Rr(6);
ao.range;
ya.range;
oS.range;
uS.range;
rn.range;
lS.range;
cS.range;
function Lr(e) {
  return Ae((t) => {
    t.setUTCDate(t.getUTCDate() - (t.getUTCDay() + 7 - e) % 7), t.setUTCHours(0, 0, 0, 0);
  }, (t, r) => {
    t.setUTCDate(t.getUTCDate() + r * 7);
  }, (t, r) => (r - t) / Tl);
}
const oo = Lr(0), ga = Lr(1), sS = Lr(2), fS = Lr(3), nn = Lr(4), dS = Lr(5), vS = Lr(6);
oo.range;
ga.range;
sS.range;
fS.range;
nn.range;
dS.range;
vS.range;
const $l = Ae((e) => {
  e.setDate(1), e.setHours(0, 0, 0, 0);
}, (e, t) => {
  e.setMonth(e.getMonth() + t);
}, (e, t) => t.getMonth() - e.getMonth() + (t.getFullYear() - e.getFullYear()) * 12, (e) => e.getMonth());
$l.range;
const Rl = Ae((e) => {
  e.setUTCDate(1), e.setUTCHours(0, 0, 0, 0);
}, (e, t) => {
  e.setUTCMonth(e.getUTCMonth() + t);
}, (e, t) => t.getUTCMonth() - e.getUTCMonth() + (t.getUTCFullYear() - e.getUTCFullYear()) * 12, (e) => e.getUTCMonth());
Rl.range;
const Ht = Ae((e) => {
  e.setMonth(0, 1), e.setHours(0, 0, 0, 0);
}, (e, t) => {
  e.setFullYear(e.getFullYear() + t);
}, (e, t) => t.getFullYear() - e.getFullYear(), (e) => e.getFullYear());
Ht.every = (e) => !isFinite(e = Math.floor(e)) || !(e > 0) ? null : Ae((t) => {
  t.setFullYear(Math.floor(t.getFullYear() / e) * e), t.setMonth(0, 1), t.setHours(0, 0, 0, 0);
}, (t, r) => {
  t.setFullYear(t.getFullYear() + r * e);
});
Ht.range;
const Yt = Ae((e) => {
  e.setUTCMonth(0, 1), e.setUTCHours(0, 0, 0, 0);
}, (e, t) => {
  e.setUTCFullYear(e.getUTCFullYear() + t);
}, (e, t) => t.getUTCFullYear() - e.getUTCFullYear(), (e) => e.getUTCFullYear());
Yt.every = (e) => !isFinite(e = Math.floor(e)) || !(e > 0) ? null : Ae((t) => {
  t.setUTCFullYear(Math.floor(t.getUTCFullYear() / e) * e), t.setUTCMonth(0, 1), t.setUTCHours(0, 0, 0, 0);
}, (t, r) => {
  t.setUTCFullYear(t.getUTCFullYear() + r * e);
});
Yt.range;
function Qp(e, t, r, n, i, a) {
  const o = [
    [Sr, 1, Rt],
    [Sr, 5, 5 * Rt],
    [Sr, 15, 15 * Rt],
    [Sr, 30, 30 * Rt],
    [a, 1, ut],
    [a, 5, 5 * ut],
    [a, 15, 15 * ut],
    [a, 30, 30 * ut],
    [i, 1, Lt],
    [i, 3, 3 * Lt],
    [i, 6, 6 * Lt],
    [i, 12, 12 * Lt],
    [n, 1, Kt],
    [n, 2, 2 * Kt],
    [r, 1, Tl],
    [t, 1, zf],
    [t, 3, 3 * zf],
    [e, 1, Ro]
  ];
  function u(c, s, f) {
    const d = s < c;
    d && ([c, s] = [s, c]);
    const h = f && typeof f.range == "function" ? f : l(c, s, f), p = h ? h.range(c, +s + 1) : [];
    return d ? p.reverse() : p;
  }
  function l(c, s, f) {
    const d = Math.abs(s - c) / f, h = gl(([, , m]) => m).right(o, d);
    if (h === o.length) return e.every(wu(c / Ro, s / Ro, f));
    if (h === 0) return ma.every(Math.max(wu(c, s, f), 1));
    const [p, v] = o[d / o[h - 1][2] < o[h][2] / d ? h - 1 : h];
    return p.every(v);
  }
  return [u, l];
}
const [hS, pS] = Qp(Yt, Rl, oo, Zp, jl, Dl), [mS, yS] = Qp(Ht, $l, ao, ci, Nl, Ml);
function Lo(e) {
  if (0 <= e.y && e.y < 100) {
    var t = new Date(-1, e.m, e.d, e.H, e.M, e.S, e.L);
    return t.setFullYear(e.y), t;
  }
  return new Date(e.y, e.m, e.d, e.H, e.M, e.S, e.L);
}
function zo(e) {
  if (0 <= e.y && e.y < 100) {
    var t = new Date(Date.UTC(-1, e.m, e.d, e.H, e.M, e.S, e.L));
    return t.setUTCFullYear(e.y), t;
  }
  return new Date(Date.UTC(e.y, e.m, e.d, e.H, e.M, e.S, e.L));
}
function kn(e, t, r) {
  return { y: e, m: t, d: r, H: 0, M: 0, S: 0, L: 0 };
}
function gS(e) {
  var t = e.dateTime, r = e.date, n = e.time, i = e.periods, a = e.days, o = e.shortDays, u = e.months, l = e.shortMonths, c = Cn(i), s = Tn(i), f = Cn(a), d = Tn(a), h = Cn(o), p = Tn(o), v = Cn(u), m = Tn(u), y = Cn(l), b = Tn(l), x = {
    a: K,
    A: L,
    b: G,
    B: F,
    c: null,
    d: Kf,
    e: Kf,
    f: FS,
    g: ZS,
    G: JS,
    H: LS,
    I: zS,
    j: BS,
    L: Jp,
    m: WS,
    M: US,
    p: he,
    q: pe,
    Q: Gf,
    s: qf,
    S: VS,
    u: KS,
    U: HS,
    V: YS,
    w: GS,
    W: qS,
    x: null,
    X: null,
    y: XS,
    Y: QS,
    Z: e_,
    "%": Yf
  }, w = {
    a: le,
    A: We,
    b: Qe,
    B: vt,
    c: null,
    d: Hf,
    e: Hf,
    f: i_,
    g: h_,
    G: m_,
    H: t_,
    I: r_,
    j: n_,
    L: tm,
    m: a_,
    M: o_,
    p: ht,
    q: On,
    Q: Gf,
    s: qf,
    S: u_,
    u: l_,
    U: c_,
    V: s_,
    w: f_,
    W: d_,
    x: null,
    X: null,
    y: v_,
    Y: p_,
    Z: y_,
    "%": Yf
  }, O = {
    a: C,
    A: T,
    b: _,
    B: z,
    c: $,
    d: Uf,
    e: Uf,
    f: NS,
    g: Wf,
    G: Ff,
    H: Vf,
    I: Vf,
    j: CS,
    L: DS,
    m: kS,
    M: TS,
    p: I,
    q: IS,
    Q: $S,
    s: RS,
    S: MS,
    u: OS,
    U: ES,
    V: SS,
    w: AS,
    W: _S,
    x: U,
    X: V,
    y: Wf,
    Y: Ff,
    Z: PS,
    "%": jS
  };
  x.x = g(r, x), x.X = g(n, x), x.c = g(t, x), w.x = g(r, w), w.X = g(n, w), w.c = g(t, w);
  function g(M, W) {
    return function(B) {
      var k = [], je = -1, J = 0, R = M.length, Je, hr, yc;
      for (B instanceof Date || (B = /* @__PURE__ */ new Date(+B)); ++je < R; )
        M.charCodeAt(je) === 37 && (k.push(M.slice(J, je)), (hr = Bf[Je = M.charAt(++je)]) != null ? Je = M.charAt(++je) : hr = Je === "e" ? " " : "0", (yc = W[Je]) && (Je = yc(B, hr)), k.push(Je), J = je + 1);
      return k.push(M.slice(J, je)), k.join("");
    };
  }
  function S(M, W) {
    return function(B) {
      var k = kn(1900, void 0, 1), je = P(k, M, B += "", 0), J, R;
      if (je != B.length) return null;
      if ("Q" in k) return new Date(k.Q);
      if ("s" in k) return new Date(k.s * 1e3 + ("L" in k ? k.L : 0));
      if (W && !("Z" in k) && (k.Z = 0), "p" in k && (k.H = k.H % 12 + k.p * 12), k.m === void 0 && (k.m = "q" in k ? k.q : 0), "V" in k) {
        if (k.V < 1 || k.V > 53) return null;
        "w" in k || (k.w = 1), "Z" in k ? (J = zo(kn(k.y, 0, 1)), R = J.getUTCDay(), J = R > 4 || R === 0 ? ga.ceil(J) : ga(J), J = io.offset(J, (k.V - 1) * 7), k.y = J.getUTCFullYear(), k.m = J.getUTCMonth(), k.d = J.getUTCDate() + (k.w + 6) % 7) : (J = Lo(kn(k.y, 0, 1)), R = J.getDay(), J = R > 4 || R === 0 ? ya.ceil(J) : ya(J), J = ci.offset(J, (k.V - 1) * 7), k.y = J.getFullYear(), k.m = J.getMonth(), k.d = J.getDate() + (k.w + 6) % 7);
      } else ("W" in k || "U" in k) && ("w" in k || (k.w = "u" in k ? k.u % 7 : "W" in k ? 1 : 0), R = "Z" in k ? zo(kn(k.y, 0, 1)).getUTCDay() : Lo(kn(k.y, 0, 1)).getDay(), k.m = 0, k.d = "W" in k ? (k.w + 6) % 7 + k.W * 7 - (R + 5) % 7 : k.w + k.U * 7 - (R + 6) % 7);
      return "Z" in k ? (k.H += k.Z / 100 | 0, k.M += k.Z % 100, zo(k)) : Lo(k);
    };
  }
  function P(M, W, B, k) {
    for (var je = 0, J = W.length, R = B.length, Je, hr; je < J; ) {
      if (k >= R) return -1;
      if (Je = W.charCodeAt(je++), Je === 37) {
        if (Je = W.charAt(je++), hr = O[Je in Bf ? W.charAt(je++) : Je], !hr || (k = hr(M, B, k)) < 0) return -1;
      } else if (Je != B.charCodeAt(k++))
        return -1;
    }
    return k;
  }
  function I(M, W, B) {
    var k = c.exec(W.slice(B));
    return k ? (M.p = s.get(k[0].toLowerCase()), B + k[0].length) : -1;
  }
  function C(M, W, B) {
    var k = h.exec(W.slice(B));
    return k ? (M.w = p.get(k[0].toLowerCase()), B + k[0].length) : -1;
  }
  function T(M, W, B) {
    var k = f.exec(W.slice(B));
    return k ? (M.w = d.get(k[0].toLowerCase()), B + k[0].length) : -1;
  }
  function _(M, W, B) {
    var k = y.exec(W.slice(B));
    return k ? (M.m = b.get(k[0].toLowerCase()), B + k[0].length) : -1;
  }
  function z(M, W, B) {
    var k = v.exec(W.slice(B));
    return k ? (M.m = m.get(k[0].toLowerCase()), B + k[0].length) : -1;
  }
  function $(M, W, B) {
    return P(M, t, W, B);
  }
  function U(M, W, B) {
    return P(M, r, W, B);
  }
  function V(M, W, B) {
    return P(M, n, W, B);
  }
  function K(M) {
    return o[M.getDay()];
  }
  function L(M) {
    return a[M.getDay()];
  }
  function G(M) {
    return l[M.getMonth()];
  }
  function F(M) {
    return u[M.getMonth()];
  }
  function he(M) {
    return i[+(M.getHours() >= 12)];
  }
  function pe(M) {
    return 1 + ~~(M.getMonth() / 3);
  }
  function le(M) {
    return o[M.getUTCDay()];
  }
  function We(M) {
    return a[M.getUTCDay()];
  }
  function Qe(M) {
    return l[M.getUTCMonth()];
  }
  function vt(M) {
    return u[M.getUTCMonth()];
  }
  function ht(M) {
    return i[+(M.getUTCHours() >= 12)];
  }
  function On(M) {
    return 1 + ~~(M.getUTCMonth() / 3);
  }
  return {
    format: function(M) {
      var W = g(M += "", x);
      return W.toString = function() {
        return M;
      }, W;
    },
    parse: function(M) {
      var W = S(M += "", !1);
      return W.toString = function() {
        return M;
      }, W;
    },
    utcFormat: function(M) {
      var W = g(M += "", w);
      return W.toString = function() {
        return M;
      }, W;
    },
    utcParse: function(M) {
      var W = S(M += "", !0);
      return W.toString = function() {
        return M;
      }, W;
    }
  };
}
var Bf = { "-": "", _: " ", 0: "0" }, Ce = /^\s*\d+/, bS = /^%/, xS = /[\\^$*+?|[\]().{}]/g;
function X(e, t, r) {
  var n = e < 0 ? "-" : "", i = (n ? -e : e) + "", a = i.length;
  return n + (a < r ? new Array(r - a + 1).join(t) + i : i);
}
function wS(e) {
  return e.replace(xS, "\\$&");
}
function Cn(e) {
  return new RegExp("^(?:" + e.map(wS).join("|") + ")", "i");
}
function Tn(e) {
  return new Map(e.map((t, r) => [t.toLowerCase(), r]));
}
function AS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 1));
  return n ? (e.w = +n[0], r + n[0].length) : -1;
}
function OS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 1));
  return n ? (e.u = +n[0], r + n[0].length) : -1;
}
function ES(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 2));
  return n ? (e.U = +n[0], r + n[0].length) : -1;
}
function SS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 2));
  return n ? (e.V = +n[0], r + n[0].length) : -1;
}
function _S(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 2));
  return n ? (e.W = +n[0], r + n[0].length) : -1;
}
function Ff(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 4));
  return n ? (e.y = +n[0], r + n[0].length) : -1;
}
function Wf(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 2));
  return n ? (e.y = +n[0] + (+n[0] > 68 ? 1900 : 2e3), r + n[0].length) : -1;
}
function PS(e, t, r) {
  var n = /^(Z)|([+-]\d\d)(?::?(\d\d))?/.exec(t.slice(r, r + 6));
  return n ? (e.Z = n[1] ? 0 : -(n[2] + (n[3] || "00")), r + n[0].length) : -1;
}
function IS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 1));
  return n ? (e.q = n[0] * 3 - 3, r + n[0].length) : -1;
}
function kS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 2));
  return n ? (e.m = n[0] - 1, r + n[0].length) : -1;
}
function Uf(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 2));
  return n ? (e.d = +n[0], r + n[0].length) : -1;
}
function CS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 3));
  return n ? (e.m = 0, e.d = +n[0], r + n[0].length) : -1;
}
function Vf(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 2));
  return n ? (e.H = +n[0], r + n[0].length) : -1;
}
function TS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 2));
  return n ? (e.M = +n[0], r + n[0].length) : -1;
}
function MS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 2));
  return n ? (e.S = +n[0], r + n[0].length) : -1;
}
function DS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 3));
  return n ? (e.L = +n[0], r + n[0].length) : -1;
}
function NS(e, t, r) {
  var n = Ce.exec(t.slice(r, r + 6));
  return n ? (e.L = Math.floor(n[0] / 1e3), r + n[0].length) : -1;
}
function jS(e, t, r) {
  var n = bS.exec(t.slice(r, r + 1));
  return n ? r + n[0].length : -1;
}
function $S(e, t, r) {
  var n = Ce.exec(t.slice(r));
  return n ? (e.Q = +n[0], r + n[0].length) : -1;
}
function RS(e, t, r) {
  var n = Ce.exec(t.slice(r));
  return n ? (e.s = +n[0], r + n[0].length) : -1;
}
function Kf(e, t) {
  return X(e.getDate(), t, 2);
}
function LS(e, t) {
  return X(e.getHours(), t, 2);
}
function zS(e, t) {
  return X(e.getHours() % 12 || 12, t, 2);
}
function BS(e, t) {
  return X(1 + ci.count(Ht(e), e), t, 3);
}
function Jp(e, t) {
  return X(e.getMilliseconds(), t, 3);
}
function FS(e, t) {
  return Jp(e, t) + "000";
}
function WS(e, t) {
  return X(e.getMonth() + 1, t, 2);
}
function US(e, t) {
  return X(e.getMinutes(), t, 2);
}
function VS(e, t) {
  return X(e.getSeconds(), t, 2);
}
function KS(e) {
  var t = e.getDay();
  return t === 0 ? 7 : t;
}
function HS(e, t) {
  return X(ao.count(Ht(e) - 1, e), t, 2);
}
function em(e) {
  var t = e.getDay();
  return t >= 4 || t === 0 ? rn(e) : rn.ceil(e);
}
function YS(e, t) {
  return e = em(e), X(rn.count(Ht(e), e) + (Ht(e).getDay() === 4), t, 2);
}
function GS(e) {
  return e.getDay();
}
function qS(e, t) {
  return X(ya.count(Ht(e) - 1, e), t, 2);
}
function XS(e, t) {
  return X(e.getFullYear() % 100, t, 2);
}
function ZS(e, t) {
  return e = em(e), X(e.getFullYear() % 100, t, 2);
}
function QS(e, t) {
  return X(e.getFullYear() % 1e4, t, 4);
}
function JS(e, t) {
  var r = e.getDay();
  return e = r >= 4 || r === 0 ? rn(e) : rn.ceil(e), X(e.getFullYear() % 1e4, t, 4);
}
function e_(e) {
  var t = e.getTimezoneOffset();
  return (t > 0 ? "-" : (t *= -1, "+")) + X(t / 60 | 0, "0", 2) + X(t % 60, "0", 2);
}
function Hf(e, t) {
  return X(e.getUTCDate(), t, 2);
}
function t_(e, t) {
  return X(e.getUTCHours(), t, 2);
}
function r_(e, t) {
  return X(e.getUTCHours() % 12 || 12, t, 2);
}
function n_(e, t) {
  return X(1 + io.count(Yt(e), e), t, 3);
}
function tm(e, t) {
  return X(e.getUTCMilliseconds(), t, 3);
}
function i_(e, t) {
  return tm(e, t) + "000";
}
function a_(e, t) {
  return X(e.getUTCMonth() + 1, t, 2);
}
function o_(e, t) {
  return X(e.getUTCMinutes(), t, 2);
}
function u_(e, t) {
  return X(e.getUTCSeconds(), t, 2);
}
function l_(e) {
  var t = e.getUTCDay();
  return t === 0 ? 7 : t;
}
function c_(e, t) {
  return X(oo.count(Yt(e) - 1, e), t, 2);
}
function rm(e) {
  var t = e.getUTCDay();
  return t >= 4 || t === 0 ? nn(e) : nn.ceil(e);
}
function s_(e, t) {
  return e = rm(e), X(nn.count(Yt(e), e) + (Yt(e).getUTCDay() === 4), t, 2);
}
function f_(e) {
  return e.getUTCDay();
}
function d_(e, t) {
  return X(ga.count(Yt(e) - 1, e), t, 2);
}
function v_(e, t) {
  return X(e.getUTCFullYear() % 100, t, 2);
}
function h_(e, t) {
  return e = rm(e), X(e.getUTCFullYear() % 100, t, 2);
}
function p_(e, t) {
  return X(e.getUTCFullYear() % 1e4, t, 4);
}
function m_(e, t) {
  var r = e.getUTCDay();
  return e = r >= 4 || r === 0 ? nn(e) : nn.ceil(e), X(e.getUTCFullYear() % 1e4, t, 4);
}
function y_() {
  return "+0000";
}
function Yf() {
  return "%";
}
function Gf(e) {
  return +e;
}
function qf(e) {
  return Math.floor(+e / 1e3);
}
var Vr, nm, im;
g_({
  dateTime: "%x, %X",
  date: "%-m/%-d/%Y",
  time: "%-I:%M:%S %p",
  periods: ["AM", "PM"],
  days: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
  shortDays: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
  months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
  shortMonths: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
});
function g_(e) {
  return Vr = gS(e), nm = Vr.format, Vr.parse, im = Vr.utcFormat, Vr.utcParse, Vr;
}
function b_(e) {
  return new Date(e);
}
function x_(e) {
  return e instanceof Date ? +e : +/* @__PURE__ */ new Date(+e);
}
function Ll(e, t, r, n, i, a, o, u, l, c) {
  var s = El(), f = s.invert, d = s.domain, h = c(".%L"), p = c(":%S"), v = c("%I:%M"), m = c("%I %p"), y = c("%a %d"), b = c("%b %d"), x = c("%B"), w = c("%Y");
  function O(g) {
    return (l(g) < g ? h : u(g) < g ? p : o(g) < g ? v : a(g) < g ? m : n(g) < g ? i(g) < g ? y : b : r(g) < g ? x : w)(g);
  }
  return s.invert = function(g) {
    return new Date(f(g));
  }, s.domain = function(g) {
    return arguments.length ? d(Array.from(g, x_)) : d().map(b_);
  }, s.ticks = function(g) {
    var S = d();
    return e(S[0], S[S.length - 1], g ?? 10);
  }, s.tickFormat = function(g, S) {
    return S == null ? O : c(S);
  }, s.nice = function(g) {
    var S = d();
    return (!g || typeof g.range != "function") && (g = t(S[0], S[S.length - 1], g ?? 10)), g ? d(Vp(S, g)) : s;
  }, s.copy = function() {
    return li(s, Ll(e, t, r, n, i, a, o, u, l, c));
  }, s;
}
function w_() {
  return ft.apply(Ll(mS, yS, Ht, $l, ao, ci, Nl, Ml, Sr, nm).domain([new Date(2e3, 0, 1), new Date(2e3, 0, 2)]), arguments);
}
function A_() {
  return ft.apply(Ll(hS, pS, Yt, Rl, oo, io, jl, Dl, Sr, im).domain([Date.UTC(2e3, 0, 1), Date.UTC(2e3, 0, 2)]), arguments);
}
function uo() {
  var e = 0, t = 1, r, n, i, a, o = Ve, u = !1, l;
  function c(f) {
    return f == null || isNaN(f = +f) ? l : o(i === 0 ? 0.5 : (f = (a(f) - r) * i, u ? Math.max(0, Math.min(1, f)) : f));
  }
  c.domain = function(f) {
    return arguments.length ? ([e, t] = f, r = a(e = +e), n = a(t = +t), i = r === n ? 0 : 1 / (n - r), c) : [e, t];
  }, c.clamp = function(f) {
    return arguments.length ? (u = !!f, c) : u;
  }, c.interpolator = function(f) {
    return arguments.length ? (o = f, c) : o;
  };
  function s(f) {
    return function(d) {
      var h, p;
      return arguments.length ? ([h, p] = d, o = f(h, p), c) : [o(0), o(1)];
    };
  }
  return c.range = s(mn), c.rangeRound = s(Ol), c.unknown = function(f) {
    return arguments.length ? (l = f, c) : l;
  }, function(f) {
    return a = f, r = f(e), n = f(t), i = r === n ? 0 : 1 / (n - r), c;
  };
}
function vr(e, t) {
  return t.domain(e.domain()).interpolator(e.interpolator()).clamp(e.clamp()).unknown(e.unknown());
}
function am() {
  var e = dr(uo()(Ve));
  return e.copy = function() {
    return vr(e, am());
  }, Xt.apply(e, arguments);
}
function om() {
  var e = Pl(uo()).domain([1, 10]);
  return e.copy = function() {
    return vr(e, om()).base(e.base());
  }, Xt.apply(e, arguments);
}
function um() {
  var e = Il(uo());
  return e.copy = function() {
    return vr(e, um()).constant(e.constant());
  }, Xt.apply(e, arguments);
}
function zl() {
  var e = kl(uo());
  return e.copy = function() {
    return vr(e, zl()).exponent(e.exponent());
  }, Xt.apply(e, arguments);
}
function O_() {
  return zl.apply(null, arguments).exponent(0.5);
}
function lm() {
  var e = [], t = Ve;
  function r(n) {
    if (n != null && !isNaN(n = +n)) return t((oi(e, n, 1) - 1) / (e.length - 1));
  }
  return r.domain = function(n) {
    if (!arguments.length) return e.slice();
    e = [];
    for (let i of n) i != null && !isNaN(i = +i) && e.push(i);
    return e.sort(ur), r;
  }, r.interpolator = function(n) {
    return arguments.length ? (t = n, r) : t;
  }, r.range = function() {
    return e.map((n, i) => t(i / (e.length - 1)));
  }, r.quantiles = function(n) {
    return Array.from({ length: n + 1 }, (i, a) => lE(e, a / n));
  }, r.copy = function() {
    return lm(t).domain(e);
  }, Xt.apply(r, arguments);
}
function lo() {
  var e = 0, t = 0.5, r = 1, n = 1, i, a, o, u, l, c = Ve, s, f = !1, d;
  function h(v) {
    return isNaN(v = +v) ? d : (v = 0.5 + ((v = +s(v)) - a) * (n * v < n * a ? u : l), c(f ? Math.max(0, Math.min(1, v)) : v));
  }
  h.domain = function(v) {
    return arguments.length ? ([e, t, r] = v, i = s(e = +e), a = s(t = +t), o = s(r = +r), u = i === a ? 0 : 0.5 / (a - i), l = a === o ? 0 : 0.5 / (o - a), n = a < i ? -1 : 1, h) : [e, t, r];
  }, h.clamp = function(v) {
    return arguments.length ? (f = !!v, h) : f;
  }, h.interpolator = function(v) {
    return arguments.length ? (c = v, h) : c;
  };
  function p(v) {
    return function(m) {
      var y, b, x;
      return arguments.length ? ([y, b, x] = m, c = jE(v, [y, b, x]), h) : [c(0), c(0.5), c(1)];
    };
  }
  return h.range = p(mn), h.rangeRound = p(Ol), h.unknown = function(v) {
    return arguments.length ? (d = v, h) : d;
  }, function(v) {
    return s = v, i = v(e), a = v(t), o = v(r), u = i === a ? 0 : 0.5 / (a - i), l = a === o ? 0 : 0.5 / (o - a), n = a < i ? -1 : 1, h;
  };
}
function cm() {
  var e = dr(lo()(Ve));
  return e.copy = function() {
    return vr(e, cm());
  }, Xt.apply(e, arguments);
}
function sm() {
  var e = Pl(lo()).domain([0.1, 1, 10]);
  return e.copy = function() {
    return vr(e, sm()).base(e.base());
  }, Xt.apply(e, arguments);
}
function fm() {
  var e = Il(lo());
  return e.copy = function() {
    return vr(e, fm()).constant(e.constant());
  }, Xt.apply(e, arguments);
}
function Bl() {
  var e = kl(lo());
  return e.copy = function() {
    return vr(e, Bl()).exponent(e.exponent());
  }, Xt.apply(e, arguments);
}
function E_() {
  return Bl.apply(null, arguments).exponent(0.5);
}
const dm = /* @__PURE__ */ Object.freeze(/* @__PURE__ */ Object.defineProperty({
  __proto__: null,
  scaleBand: xl,
  scaleDiverging: cm,
  scaleDivergingLog: sm,
  scaleDivergingPow: Bl,
  scaleDivergingSqrt: E_,
  scaleDivergingSymlog: fm,
  scaleIdentity: Up,
  scaleImplicit: Au,
  scaleLinear: Wp,
  scaleLog: Kp,
  scaleOrdinal: bl,
  scalePoint: fE,
  scalePow: Cl,
  scaleQuantile: Gp,
  scaleQuantize: qp,
  scaleRadial: Yp,
  scaleSequential: am,
  scaleSequentialLog: om,
  scaleSequentialPow: zl,
  scaleSequentialQuantile: lm,
  scaleSequentialSqrt: O_,
  scaleSequentialSymlog: um,
  scaleSqrt: iS,
  scaleSymlog: Hp,
  scaleThreshold: Xp,
  scaleTime: w_,
  scaleUtc: A_,
  tickFormat: Fp
}, Symbol.toStringTag, { value: "Module" }));
function S_(e) {
  var t = dm;
  if (e in t && typeof t[e] == "function")
    return t[e]();
  var r = "scale".concat(Hu(e));
  if (r in t && typeof t[r] == "function")
    return t[r]();
}
function Xf(e, t, r) {
  if (typeof e == "function")
    return e.copy().domain(t).range(r);
  if (e != null) {
    var n = S_(e);
    if (n != null)
      return n.domain(t).range(r), n;
  }
}
function Fl(e, t, r, n) {
  if (!(r == null || n == null))
    return typeof e.scale == "function" ? Xf(e.scale, r, n) : Xf(t, r, n);
}
function __(e) {
  return "scale".concat(Hu(e));
}
function P_(e) {
  return __(e) in dm;
}
var vm = (e, t, r) => {
  if (e != null) {
    var n = e.scale, i = e.type;
    if (n === "auto")
      return i === "category" && r && (r.indexOf("LineChart") >= 0 || r.indexOf("AreaChart") >= 0 || r.indexOf("ComposedChart") >= 0 && !t) ? "point" : i === "category" ? "band" : "linear";
    if (typeof n == "string")
      return P_(n) ? n : "point";
  }
};
function I_(e, t) {
  for (var r = 0, n = e.length, i = e[0] < e[e.length - 1]; r < n; ) {
    var a = Math.floor((r + n) / 2);
    (i ? e[a] < t : e[a] > t) ? r = a + 1 : n = a;
  }
  return r;
}
function hm(e, t) {
  if (e) {
    var r = t ?? e.domain(), n = r.map((a) => {
      var o;
      return (o = e(a)) !== null && o !== void 0 ? o : 0;
    }), i = e.range();
    if (!(r.length === 0 || i.length < 2))
      return (a) => {
        var o, u, l = I_(n, a);
        if (l <= 0)
          return r[0];
        if (l >= r.length)
          return r[r.length - 1];
        var c = (o = n[l - 1]) !== null && o !== void 0 ? o : 0, s = (u = n[l]) !== null && u !== void 0 ? u : 0;
        return Math.abs(a - c) <= Math.abs(a - s) ? r[l - 1] : r[l];
      };
  }
}
function k_(e) {
  if (e != null)
    return "invert" in e && typeof e.invert == "function" ? e.invert.bind(e) : hm(e, void 0);
}
function Zf(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function ba(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Zf(Object(r), !0).forEach(function(n) {
      C_(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Zf(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function C_(e, t, r) {
  return (t = T_(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function T_(e) {
  var t = M_(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function M_(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function pm(e, t) {
  return $_(e) || j_(e, t) || N_(e, t) || D_();
}
function D_() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function N_(e, t) {
  if (e) {
    if (typeof e == "string") return Qf(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Qf(e, t) : void 0;
  }
}
function Qf(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function j_(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function $_(e) {
  if (Array.isArray(e)) return e;
}
var _u = [0, "auto"], ge = {
  allowDataOverflow: !1,
  allowDecimals: !0,
  allowDuplicatedCategory: !0,
  angle: 0,
  dataKey: void 0,
  domain: void 0,
  height: 30,
  hide: !0,
  id: 0,
  includeHidden: !1,
  interval: "preserveEnd",
  minTickGap: 5,
  mirror: !1,
  name: void 0,
  orientation: "bottom",
  padding: {
    left: 0,
    right: 0
  },
  reversed: !1,
  scale: "auto",
  tick: !0,
  tickCount: 5,
  tickFormatter: void 0,
  ticks: void 0,
  type: "category",
  unit: void 0,
  niceTicks: "auto"
}, mm = (e, t) => e.cartesianAxis.xAxis[t], Zt = (e, t) => {
  var r = mm(e, t);
  return r ?? ge;
}, be = {
  allowDataOverflow: !1,
  allowDecimals: !0,
  allowDuplicatedCategory: !0,
  angle: 0,
  dataKey: void 0,
  domain: _u,
  hide: !0,
  id: 0,
  includeHidden: !1,
  interval: "preserveEnd",
  minTickGap: 5,
  mirror: !1,
  name: void 0,
  orientation: "left",
  padding: {
    top: 0,
    bottom: 0
  },
  reversed: !1,
  scale: "auto",
  tick: !0,
  tickCount: 5,
  tickFormatter: void 0,
  ticks: void 0,
  type: "number",
  unit: void 0,
  niceTicks: "auto",
  width: ti
}, ym = (e, t) => e.cartesianAxis.yAxis[t], Qt = (e, t) => {
  var r = ym(e, t);
  return r ?? be;
}, R_ = {
  domain: [0, "auto"],
  includeHidden: !1,
  reversed: !1,
  allowDataOverflow: !1,
  allowDuplicatedCategory: !1,
  dataKey: void 0,
  id: 0,
  name: "",
  range: [64, 64],
  scale: "auto",
  type: "number",
  unit: ""
}, Wl = (e, t) => {
  var r = e.cartesianAxis.zAxis[t];
  return r ?? R_;
}, Fe = (e, t, r) => {
  switch (t) {
    case "xAxis":
      return Zt(e, r);
    case "yAxis":
      return Qt(e, r);
    case "zAxis":
      return Wl(e, r);
    case "angleAxis":
      return vl(e, r);
    case "radiusAxis":
      return hl(e, r);
    default:
      throw new Error("Unexpected axis type: ".concat(t));
  }
}, L_ = (e, t, r) => {
  switch (t) {
    case "xAxis":
      return Zt(e, r);
    case "yAxis":
      return Qt(e, r);
    default:
      throw new Error("Unexpected axis type: ".concat(t));
  }
}, si = (e, t, r) => {
  switch (t) {
    case "xAxis":
      return Zt(e, r);
    case "yAxis":
      return Qt(e, r);
    case "angleAxis":
      return vl(e, r);
    case "radiusAxis":
      return hl(e, r);
    default:
      throw new Error("Unexpected axis type: ".concat(t));
  }
}, gm = (e) => e.graphicalItems.cartesianItems.some((t) => t.type === "bar") || e.graphicalItems.polarItems.some((t) => t.type === "radialBar");
function bm(e, t) {
  return (r) => {
    switch (e) {
      case "xAxis":
        return "xAxisId" in r && r.xAxisId === t;
      case "yAxis":
        return "yAxisId" in r && r.yAxisId === t;
      case "zAxis":
        return "zAxisId" in r && r.zAxisId === t;
      case "angleAxis":
        return "angleAxisId" in r && r.angleAxisId === t;
      case "radiusAxis":
        return "radiusAxisId" in r && r.radiusAxisId === t;
      default:
        return !1;
    }
  };
}
var Ul = (e) => e.graphicalItems.cartesianItems, z_ = E([Ie, eo], bm), xm = (e, t, r) => e.filter(r).filter((n) => (t == null ? void 0 : t.includeHidden) === !0 ? !0 : !n.hide), yn = E([Ul, Fe, z_], xm, {
  memoizeOptions: {
    resultEqualityCheck: ro
  }
}), wm = E([yn], (e) => e.filter((t) => t.type === "area" || t.type === "bar").filter(to)), Am = (e) => e.filter((t) => !("stackId" in t) || t.stackId === void 0), B_ = E([yn], Am), Om = (e) => e.map((t) => t.data).filter(Boolean).flat(1), F_ = E([yn], (e) => e.some((t) => !t.data)), Em = E([yn], Om, {
  memoizeOptions: {
    resultEqualityCheck: ro
  }
}), Sm = (e, t) => {
  var r = t.chartData, n = r === void 0 ? [] : r, i = t.dataStartIndex, a = t.dataEndIndex;
  return e.length > 0 ? e : n.slice(i, a + 1);
}, Vl = E([Em, qa], Sm), W_ = (e, t, r) => (t == null ? void 0 : t.dataKey) != null ? e.map((n) => ({
  value: Se(n, t.dataKey)
})) : r.length > 0 ? r.map((n) => n.dataKey).flatMap((n) => e.map((i) => ({
  value: Se(i, n)
}))) : e.map((n) => ({
  value: n
})), _m = (e, t, r, n, i, a) => {
  var o = n.chartData, u = o === void 0 ? [] : o, l = n.dataStartIndex, c = n.dataEndIndex, s = W_(e, t, r);
  if (i && (t == null ? void 0 : t.dataKey) != null && a.length > 0) {
    var f = u.slice(l, c + 1), d = f.map((h) => ({
      value: Se(h, t.dataKey)
    })).filter((h) => h.value != null);
    return [...d, ...s];
  }
  return s;
}, fi = E([Vl, Fe, yn, qa, F_, Em], _m);
function Jr(e) {
  if (Mt(e) || e instanceof Date) {
    var t = Number(e);
    if (Y(t))
      return t;
  }
}
function Jf(e) {
  if (Array.isArray(e)) {
    var t = [Jr(e[0]), Jr(e[1])];
    return It(t) ? t : void 0;
  }
  var r = Jr(e);
  if (r != null)
    return [r, r];
}
function bt(e) {
  return e.map(Jr).filter(Ye);
}
function U_(e, t) {
  var r = Jr(e), n = Jr(t);
  return r == null && n == null ? 0 : r == null ? -1 : n == null ? 1 : r - n;
}
var V_ = E([fi], (e) => e == null ? void 0 : e.map((t) => t.value).sort(U_));
function Pm(e, t) {
  switch (e) {
    case "xAxis":
      return t.direction === "x";
    case "yAxis":
      return t.direction === "y";
    default:
      return !1;
  }
}
function K_(e, t, r) {
  if (!r)
    return [];
  if (!r.length)
    return [];
  var n;
  if (typeof t == "number" && !Tt(t))
    n = t;
  else if (Array.isArray(t)) {
    var i = bt(t);
    i.length > 0 && (n = Math.max(...i));
  }
  return n == null ? [] : bt(r.flatMap((a) => {
    var o = Se(e, a.dataKey), u, l;
    if (Array.isArray(o)) {
      var c = pm(o, 2);
      u = c[0], l = c[1];
    } else
      u = l = o;
    if (!(!Y(u) || !Y(l)))
      return [n - u, n + l];
  }));
}
var Oe = (e) => {
  var t = ke(e), r = pn(e);
  return si(e, t, r);
}, an = E([Oe], (e) => e == null ? void 0 : e.dataKey), H_ = E([wm, qa, Oe], Mp), Im = (e, t, r, n) => {
  var i = {}, a = t.reduce((o, u) => {
    if (u.stackId == null)
      return o;
    var l = o[u.stackId];
    return l == null && (l = []), l.push(u), o[u.stackId] = l, o;
  }, i);
  return Object.fromEntries(Object.entries(a).map((o) => {
    var u = pm(o, 2), l = u[0], c = u[1], s = n ? [...c].reverse() : c, f = s.map(ml);
    return [l, {
      // @ts-expect-error getStackedData requires that the input is array of objects, Recharts does not test for that
      stackedData: xw(e, f, r),
      graphicalItems: s
    }];
  }));
}, Pu = E([H_, wm, Xa, Sp], Im), km = (e, t, r, n) => {
  var i = t.dataStartIndex, a = t.dataEndIndex;
  if (n == null && r !== "zAxis")
    return Sw(e, i, a);
}, Y_ = E([Fe], (e) => e.allowDataOverflow), Kl = (e) => {
  var t;
  if (e == null || !("domain" in e))
    return _u;
  if (e.domain != null)
    return e.domain;
  if ("ticks" in e && e.ticks != null) {
    if (e.type === "number") {
      var r = bt(e.ticks);
      return [Math.min(...r), Math.max(...r)];
    }
    if (e.type === "category")
      return e.ticks.map(String);
  }
  return (t = e == null ? void 0 : e.domain) !== null && t !== void 0 ? t : _u;
}, Cm = E([Fe], Kl), Tm = E([Cm, Y_], dp), G_ = E([Pu, xt, Ie, Tm], km, {
  memoizeOptions: {
    resultEqualityCheck: ai
  }
}), Hl = (e) => e.errorBars, q_ = (e, t, r) => e.flatMap((n) => t[n.id]).filter(Boolean).filter((n) => Pm(r, n)), xa = function() {
  for (var t = arguments.length, r = new Array(t), n = 0; n < t; n++)
    r[n] = arguments[n];
  var i = r.filter(Boolean);
  if (i.length !== 0) {
    var a = i.flat(), o = Math.min(...a), u = Math.max(...a);
    return [o, u];
  }
}, Mm = function(t, r, n, i, a) {
  var o = arguments.length > 5 && arguments[5] !== void 0 ? arguments[5] : [], u, l;
  if (n.length > 0 && n.forEach((c) => {
    var s, f = c.data != null ? [...c.data] : o, d = (s = i[c.id]) === null || s === void 0 ? void 0 : s.filter((h) => Pm(a, h));
    f.forEach((h) => {
      var p, v = Se(h, (p = r.dataKey) !== null && p !== void 0 ? p : c.dataKey), m = K_(h, v, d);
      if (m.length >= 2) {
        var y = Math.min(...m), b = Math.max(...m);
        (u == null || y < u) && (u = y), (l == null || b > l) && (l = b);
      }
      var x = Jf(v);
      x != null && (u = u == null ? x[0] : Math.min(u, x[0]), l = l == null ? x[1] : Math.max(l, x[1]));
    });
  }), (r == null ? void 0 : r.dataKey) != null && n.length === 0 && t.forEach((c) => {
    var s = Jf(Se(c, r.dataKey));
    s != null && (u = u == null ? s[0] : Math.min(u, s[0]), l = l == null ? s[1] : Math.max(l, s[1]));
  }), Y(u) && Y(l))
    return [u, l];
}, X_ = E([Vl, Fe, B_, Hl, Ie, _1], Mm, {
  memoizeOptions: {
    resultEqualityCheck: ai
  }
});
function Z_(e) {
  var t = e.value;
  if (Mt(t) || t instanceof Date)
    return t;
}
var Q_ = (e, t, r) => {
  var n = e.map(Z_).filter((i) => i != null);
  return r && (t.dataKey == null || t.allowDuplicatedCategory && Vv(n)) ? fp(0, e.length) : t.allowDuplicatedCategory ? n : Array.from(new Set(n));
}, Dm = (e) => e.referenceElements.dots, gn = (e, t, r) => e.filter((n) => n.ifOverflow === "extendDomain").filter((n) => t === "xAxis" ? n.xAxisId === r : n.yAxisId === r), J_ = E([Dm, Ie, eo], gn), Nm = (e) => e.referenceElements.areas, eP = E([Nm, Ie, eo], gn), jm = (e) => e.referenceElements.lines, tP = E([jm, Ie, eo], gn), $m = (e, t) => {
  if (e != null) {
    var r = bt(e.map((n) => t === "xAxis" ? n.x : n.y));
    if (r.length !== 0)
      return [Math.min(...r), Math.max(...r)];
  }
}, rP = E(J_, Ie, $m), Rm = (e, t) => {
  if (e != null) {
    var r = bt(e.flatMap((n) => [t === "xAxis" ? n.x1 : n.y1, t === "xAxis" ? n.x2 : n.y2]));
    if (r.length !== 0)
      return [Math.min(...r), Math.max(...r)];
  }
}, nP = E([eP, Ie], Rm);
function iP(e) {
  var t;
  if (e.x != null)
    return bt([e.x]);
  var r = (t = e.segment) === null || t === void 0 ? void 0 : t.map((n) => n.x);
  return r == null || r.length === 0 ? [] : bt(r);
}
function aP(e) {
  var t;
  if (e.y != null)
    return bt([e.y]);
  var r = (t = e.segment) === null || t === void 0 ? void 0 : t.map((n) => n.y);
  return r == null || r.length === 0 ? [] : bt(r);
}
var Lm = (e, t) => {
  if (e != null) {
    var r = e.flatMap((n) => t === "xAxis" ? iP(n) : aP(n));
    if (r.length !== 0)
      return [Math.min(...r), Math.max(...r)];
  }
}, oP = E([tP, Ie], Lm), uP = E(rP, oP, nP, (e, t, r) => xa(e, r, t)), zm = (e, t, r, n, i, a, o, u, l) => {
  if (r != null)
    return r;
  var c = o === "vertical" && u === "xAxis" || o === "horizontal" && u === "yAxis", s = c ? xa(n, a, i) : xa(a, i), f = M1(t, s, e.allowDataOverflow);
  return f ?? (e.allowDataOverflow && s == null && l != null ? l : f);
}, lP = (e) => {
  if (!(e == null || e.type !== "number" || !("ticks" in e) || e.ticks == null)) {
    var t = bt(e.ticks);
    if (t.length !== 0)
      return [Math.min(...t), Math.max(...t)];
  }
}, cP = E([Fe], lP, {
  memoizeOptions: {
    resultEqualityCheck: ai
  }
}), sP = E([Fe, Cm, Tm, G_, X_, uP, ne, Ie, cP], zm, {
  memoizeOptions: {
    resultEqualityCheck: ai
  }
}), fP = [0, 1], Bm = (e, t, r, n, i, a, o) => {
  if (!((e == null || r == null || r.length === 0) && o === void 0)) {
    var u = e.dataKey, l = e.type, c = fr(t, a);
    if (c && u == null) {
      var s;
      return fp(0, (s = r == null ? void 0 : r.length) !== null && s !== void 0 ? s : 0);
    }
    return l === "category" ? Q_(n, e, c) : i === "expand" && !c ? fP : o;
  }
}, Yl = E([Fe, ne, Vl, fi, Xa, Ie, sP], Bm), bn = E([Fe, gm, sl], vm), Fm = (e, t, r) => {
  var n = t.niceTicks;
  if (n !== "none") {
    var i = Kl(t), a = Array.isArray(i) && (i[0] === "auto" || i[1] === "auto");
    if ((n === "snap125" || n === "adaptive") && t != null && t.tickCount && It(e)) {
      if (a)
        return lf(e, t.tickCount, t.allowDecimals, n);
      if (t.type === "number")
        return cf(e, t.tickCount, t.allowDecimals, n);
    }
    if (n === "auto" && r === "linear" && t != null && t.tickCount) {
      if (a && It(e))
        return lf(e, t.tickCount, t.allowDecimals, "adaptive");
      if (t.type === "number" && It(e))
        return cf(e, t.tickCount, t.allowDecimals, "adaptive");
    }
  }
}, Gl = E([Yl, si, bn], Fm), Wm = (e, t, r, n) => {
  if (
    /*
     * Angle axis for some reason uses nice ticks when rendering axis tick labels,
     * but doesn't use nice ticks for extending domain like all the other axes do.
     * Not really sure why? Is there a good reason,
     * or is it just because someone added support for nice ticks to the other axes and forgot this one?
     */
    n !== "angleAxis" && (e == null ? void 0 : e.type) === "number" && It(t) && Array.isArray(r) && r.length > 0
  ) {
    var i, a, o = t[0], u = (i = r[0]) !== null && i !== void 0 ? i : 0, l = t[1], c = (a = r[r.length - 1]) !== null && a !== void 0 ? a : 0;
    return [Math.min(o, u), Math.max(l, c)];
  }
  return t;
}, dP = E([Fe, Yl, Gl, Ie], Wm), vP = E(fi, Fe, (e, t) => {
  if (!(!t || t.type !== "number")) {
    var r = 1 / 0, n = Array.from(bt(e.map((f) => f.value))).sort((f, d) => f - d), i = n[0], a = n[n.length - 1];
    if (i == null || a == null)
      return 1 / 0;
    var o = a - i;
    if (o === 0)
      return 1 / 0;
    for (var u = 0; u < n.length - 1; u++) {
      var l = n[u], c = n[u + 1];
      if (!(l == null || c == null)) {
        var s = c - l;
        r = Math.min(r, s);
      }
    }
    return r / o;
  }
}), Um = E(vP, ne, Ep, Pe, (e, t, r, n, i) => i, (e, t, r, n, i) => {
  if (!Y(e))
    return 0;
  var a = t === "vertical" ? n.height : n.width;
  if (i === "gap")
    return e * a / 2;
  if (i === "no-gap") {
    var o = yt(r, e * a), u = e * a / 2;
    return u - o - (u - o) / a * o;
  }
  return 0;
}), hP = (e, t, r) => {
  var n = Zt(e, t);
  return n == null || typeof n.padding != "string" ? 0 : Um(e, "xAxis", t, r, n.padding);
}, pP = (e, t, r) => {
  var n = Qt(e, t);
  return n == null || typeof n.padding != "string" ? 0 : Um(e, "yAxis", t, r, n.padding);
}, mP = E(Zt, hP, (e, t) => {
  var r, n;
  if (e == null)
    return {
      left: 0,
      right: 0
    };
  var i = e.padding;
  return typeof i == "string" ? {
    left: t,
    right: t
  } : {
    left: ((r = i.left) !== null && r !== void 0 ? r : 0) + t,
    right: ((n = i.right) !== null && n !== void 0 ? n : 0) + t
  };
}), yP = E(Qt, pP, (e, t) => {
  var r, n;
  if (e == null)
    return {
      top: 0,
      bottom: 0
    };
  var i = e.padding;
  return typeof i == "string" ? {
    top: t,
    bottom: t
  } : {
    top: ((r = i.top) !== null && r !== void 0 ? r : 0) + t,
    bottom: ((n = i.bottom) !== null && n !== void 0 ? n : 0) + t
  };
}), Vm = E([Pe, mP, Ha, Ka, (e, t, r) => r], (e, t, r, n, i) => {
  var a = n.padding;
  return i ? [a.left, r.width - a.right] : [e.left + t.left, e.left + e.width - t.right];
}), Km = E([Pe, ne, yP, Ha, Ka, (e, t, r) => r], (e, t, r, n, i, a) => {
  var o = i.padding;
  return a ? [n.height - o.bottom, o.top] : t === "horizontal" ? [e.top + e.height - r.bottom, e.top + r.top] : [e.top + r.top, e.top + e.height - r.bottom];
}), di = (e, t, r, n) => {
  var i;
  switch (t) {
    case "xAxis":
      return Vm(e, r, n);
    case "yAxis":
      return Km(e, r, n);
    case "zAxis":
      return (i = Wl(e, r)) === null || i === void 0 ? void 0 : i.range;
    case "angleAxis":
      return kp(e);
    case "radiusAxis":
      return Cp(e, r);
    default:
      return;
  }
}, Hm = E([Fe, di], Za), gP = E([bn, dP], X1), ql = E([Fe, bn, gP, Hm], Fl), Ym = (e, t, r, n) => {
  if (!(r == null || r.dataKey == null)) {
    var i = r.type, a = r.scale, o = fr(e, n);
    if (o && (i === "number" || a !== "auto"))
      return t.map((u) => u.value);
  }
}, Xl = E([ne, fi, si, Ie], Ym), co = E([ql], yl);
E([ql], k_);
E([ql, V_], hm);
E([yn, Hl, Ie], q_);
function Gm(e, t) {
  return e.id < t.id ? -1 : e.id > t.id ? 1 : 0;
}
var so = (e, t) => t, fo = (e, t, r) => r, bP = E(Ua, so, fo, (e, t, r) => e.filter((n) => n.orientation === t).filter((n) => n.mirror === r).sort(Gm)), xP = E(Va, so, fo, (e, t, r) => e.filter((n) => n.orientation === t).filter((n) => n.mirror === r).sort(Gm)), qm = (e, t) => ({
  width: e.width,
  height: t.height
}), wP = (e, t) => {
  var r = typeof t.width == "number" ? t.width : ti;
  return {
    width: r,
    height: e.height
  };
}, Xm = E(Pe, Zt, qm), AP = (e, t, r) => {
  switch (t) {
    case "top":
      return e.top;
    case "bottom":
      return r - e.bottom;
    default:
      return 0;
  }
}, OP = (e, t, r) => {
  switch (t) {
    case "left":
      return e.left;
    case "right":
      return r - e.right;
    default:
      return 0;
  }
}, EP = E(qt, Pe, bP, so, fo, (e, t, r, n, i) => {
  var a = {}, o;
  return r.forEach((u) => {
    var l = qm(t, u);
    o == null && (o = AP(t, n, e));
    var c = n === "top" && !i || n === "bottom" && i;
    a[u.id] = o - Number(c) * l.height, o += (c ? -1 : 1) * l.height;
  }), a;
}), SP = E(Gt, Pe, xP, so, fo, (e, t, r, n, i) => {
  var a = {}, o;
  return r.forEach((u) => {
    var l = wP(t, u);
    o == null && (o = OP(t, n, e));
    var c = n === "left" && !i || n === "right" && i;
    a[u.id] = o - Number(c) * l.width, o += (c ? -1 : 1) * l.width;
  }), a;
}), _P = (e, t) => {
  var r = Zt(e, t);
  if (r != null)
    return EP(e, r.orientation, r.mirror);
}, PP = E([Pe, Zt, _P, (e, t) => t], (e, t, r, n) => {
  if (t != null) {
    var i = r == null ? void 0 : r[n];
    return i == null ? {
      x: e.left,
      y: 0
    } : {
      x: e.left,
      y: i
    };
  }
}), IP = (e, t) => {
  var r = Qt(e, t);
  if (r != null)
    return SP(e, r.orientation, r.mirror);
}, kP = E([Pe, Qt, IP, (e, t) => t], (e, t, r, n) => {
  if (t != null) {
    var i = r == null ? void 0 : r[n];
    return i == null ? {
      x: 0,
      y: e.top
    } : {
      x: i,
      y: e.top
    };
  }
}), Zm = E(Pe, Qt, (e, t) => {
  var r = typeof t.width == "number" ? t.width : ti;
  return {
    width: r,
    height: e.height
  };
}), ed = (e, t, r) => {
  switch (t) {
    case "xAxis":
      return Xm(e, r).width;
    case "yAxis":
      return Zm(e, r).height;
    default:
      return;
  }
}, Qm = (e, t, r, n) => {
  if (r != null) {
    var i = r.allowDuplicatedCategory, a = r.type, o = r.dataKey, u = fr(e, n), l = t.map((s) => s.value), c = l.filter((s) => s != null);
    if (o && u && a === "category" && i && Vv(c))
      return l;
  }
}, Zl = E([ne, fi, Fe, Ie], Qm), td = E([ne, L_, bn, co, Zl, Xl, di, Gl, Ie], (e, t, r, n, i, a, o, u, l) => {
  if (t != null) {
    var c = fr(e, l);
    return {
      angle: t.angle,
      interval: t.interval,
      minTickGap: t.minTickGap,
      orientation: t.orientation,
      tick: t.tick,
      tickCount: t.tickCount,
      tickFormatter: t.tickFormatter,
      ticks: t.ticks,
      type: t.type,
      unit: t.unit,
      axisType: l,
      categoricalDomain: a,
      duplicateDomain: i,
      isCategorical: c,
      niceTicks: u,
      range: o,
      realScaleType: r,
      scale: n
    };
  }
}), CP = (e, t, r, n, i, a, o, u, l) => {
  if (!(t == null || n == null)) {
    var c = fr(e, l), s = t.type, f = t.ticks, d = t.tickCount, h = (
      // @ts-expect-error This is testing for `scaleBand` but for band axis the type is reported as `band` so this looks like a dead code with a workaround elsewhere?
      r === "scaleBand" && typeof n.bandwidth == "function" ? n.bandwidth() / 2 : 2
    ), p = s === "category" && n.bandwidth ? n.bandwidth() / h : 0;
    p = l === "angleAxis" && a != null && a.length >= 2 ? He(a[0] - a[1]) * 2 * p : p;
    var v = f || i;
    return v ? v.map((m, y) => {
      var b = o ? o.indexOf(m) : m, x = n.map(b);
      return Y(x) ? {
        index: y,
        coordinate: x + p,
        value: m,
        offset: p
      } : null;
    }).filter(Ye) : c && u ? u.map((m, y) => {
      var b = n.map(m);
      return Y(b) ? {
        coordinate: b + p,
        value: m,
        index: y,
        offset: p
      } : null;
    }).filter(Ye) : n.ticks ? n.ticks(d).map((m, y) => {
      var b = n.map(m);
      return Y(b) ? {
        coordinate: b + p,
        value: m,
        index: y,
        offset: p
      } : null;
    }).filter(Ye) : n.domain().map((m, y) => {
      var b = n.map(m);
      return Y(b) ? {
        coordinate: b + p,
        // @ts-expect-error can't use Date as index
        value: o ? o[m] : m,
        index: y,
        offset: p
      } : null;
    }).filter(Ye);
  }
}, Jm = E([ne, si, bn, co, Gl, di, Zl, Xl, Ie], CP), TP = (e, t, r, n, i, a, o) => {
  if (!(t == null || r == null || n == null || n[0] === n[1])) {
    var u = fr(e, o), l = t.tickCount, c = 0;
    return c = o === "angleAxis" && (n == null ? void 0 : n.length) >= 2 ? He(n[0] - n[1]) * 2 * c : c, u && a ? a.map((s, f) => {
      var d = r.map(s);
      return Y(d) ? {
        coordinate: d + c,
        value: s,
        index: f,
        offset: c
      } : null;
    }).filter(Ye) : r.ticks ? r.ticks(l).map((s, f) => {
      var d = r.map(s);
      return Y(d) ? {
        coordinate: d + c,
        value: s,
        index: f,
        offset: c
      } : null;
    }).filter(Ye) : r.domain().map((s, f) => {
      var d = r.map(s);
      return Y(d) ? {
        coordinate: d + c,
        // @ts-expect-error can't use unknown as index
        value: i ? i[s] : s,
        index: f,
        offset: c
      } : null;
    }).filter(Ye);
  }
}, on = E([ne, si, co, di, Zl, Xl, Ie], TP), un = E(Fe, co, (e, t) => {
  if (!(e == null || t == null))
    return ba(ba({}, e), {}, {
      scale: t
    });
}), MP = E([Fe, bn, Yl, Hm], Fl), DP = E([MP], yl);
E((e, t, r) => Wl(e, r), DP, (e, t) => {
  if (!(e == null || t == null))
    return ba(ba({}, e), {}, {
      scale: t
    });
});
var NP = E([ne, Ua, Va], (e, t, r) => {
  switch (e) {
    case "horizontal":
      return t.some((n) => n.reversed) ? "right-to-left" : "left-to-right";
    case "vertical":
      return r.some((n) => n.reversed) ? "bottom-to-top" : "top-to-bottom";
    case "centric":
    case "radial":
      return "left-to-right";
    default:
      return;
  }
}), jP = (e, t, r) => {
  var n;
  return (n = e.renderedTicks[t]) === null || n === void 0 ? void 0 : n[r];
};
E([jP], (e) => {
  if (!(!e || e.length === 0))
    return (t) => {
      var r, n = 1 / 0, i = e[0];
      for (var a of e) {
        var o = Math.abs(a.coordinate - t);
        o < n && (n = o, i = a);
      }
      return (r = i) === null || r === void 0 ? void 0 : r.value;
    };
});
var ey = (e) => e.options.defaultTooltipEventType, ty = (e) => e.options.validateTooltipEventTypes;
function ry(e, t, r) {
  if (e == null)
    return t;
  var n = e ? "axis" : "item";
  return r == null ? t : r.includes(n) ? n : t;
}
function vi(e, t) {
  var r = ey(e), n = ty(e);
  return ry(t, r, n);
}
function $P(e) {
  return j((t) => vi(t, e));
}
var ny = (e, t) => {
  var r, n = Number(t);
  if (!(Tt(n) || t == null))
    return n >= 0 ? e == null || (r = e[n]) === null || r === void 0 ? void 0 : r.value : void 0;
}, RP = (e) => e.tooltip.settings, ar = {
  active: !1,
  index: null,
  dataKey: void 0,
  graphicalItemId: void 0,
  coordinate: void 0
}, LP = {
  itemInteraction: {
    click: ar,
    hover: ar
  },
  axisInteraction: {
    click: ar,
    hover: ar
  },
  keyboardInteraction: ar,
  syncInteraction: {
    active: !1,
    index: null,
    dataKey: void 0,
    label: void 0,
    coordinate: void 0,
    sourceViewBox: void 0,
    graphicalItemId: void 0
  },
  tooltipItemPayloads: [],
  settings: {
    shared: void 0,
    trigger: "hover",
    axisId: 0,
    active: !1,
    defaultIndex: void 0
  }
}, iy = Be({
  name: "tooltip",
  initialState: LP,
  reducers: {
    addTooltipEntrySettings: {
      reducer(e, t) {
        e.tooltipItemPayloads.push(q(t.payload));
      },
      prepare: ae()
    },
    replaceTooltipEntrySettings: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next, a = ot(e).tooltipItemPayloads.indexOf(q(n));
        a > -1 && (e.tooltipItemPayloads[a] = q(i));
      },
      prepare: ae()
    },
    removeTooltipEntrySettings: {
      reducer(e, t) {
        var r = ot(e).tooltipItemPayloads.indexOf(q(t.payload));
        r > -1 && e.tooltipItemPayloads.splice(r, 1);
      },
      prepare: ae()
    },
    setTooltipSettingsState(e, t) {
      e.settings = t.payload;
    },
    setActiveMouseOverItemIndex(e, t) {
      e.syncInteraction.active = !1, e.syncInteraction.sourceViewBox = void 0, e.keyboardInteraction.active = !1, e.itemInteraction.hover.active = !0, e.itemInteraction.hover.index = t.payload.activeIndex, e.itemInteraction.hover.dataKey = t.payload.activeDataKey, e.itemInteraction.hover.graphicalItemId = t.payload.activeGraphicalItemId, e.itemInteraction.hover.coordinate = t.payload.activeCoordinate;
    },
    mouseLeaveChart(e) {
      e.itemInteraction.hover.active = !1, e.axisInteraction.hover.active = !1;
    },
    mouseLeaveItem(e) {
      e.itemInteraction.hover.active = !1;
    },
    setActiveClickItemIndex(e, t) {
      e.syncInteraction.active = !1, e.syncInteraction.sourceViewBox = void 0, e.itemInteraction.click.active = !0, e.keyboardInteraction.active = !1, e.itemInteraction.click.index = t.payload.activeIndex, e.itemInteraction.click.dataKey = t.payload.activeDataKey, e.itemInteraction.click.graphicalItemId = t.payload.activeGraphicalItemId, e.itemInteraction.click.coordinate = t.payload.activeCoordinate;
    },
    setMouseOverAxisIndex(e, t) {
      e.syncInteraction.active = !1, e.syncInteraction.sourceViewBox = void 0, e.axisInteraction.hover.active = !0, e.keyboardInteraction.active = !1, e.axisInteraction.hover.index = t.payload.activeIndex, e.axisInteraction.hover.dataKey = t.payload.activeDataKey, e.axisInteraction.hover.coordinate = t.payload.activeCoordinate;
    },
    setMouseClickAxisIndex(e, t) {
      e.syncInteraction.active = !1, e.syncInteraction.sourceViewBox = void 0, e.keyboardInteraction.active = !1, e.axisInteraction.click.active = !0, e.axisInteraction.click.index = t.payload.activeIndex, e.axisInteraction.click.dataKey = t.payload.activeDataKey, e.axisInteraction.click.coordinate = t.payload.activeCoordinate;
    },
    setSyncInteraction(e, t) {
      e.syncInteraction = t.payload;
    },
    setKeyboardInteraction(e, t) {
      e.keyboardInteraction.active = t.payload.active, e.keyboardInteraction.index = t.payload.activeIndex, e.keyboardInteraction.coordinate = t.payload.activeCoordinate;
    }
  }
}), dt = iy.actions, zP = dt.addTooltipEntrySettings, BP = dt.replaceTooltipEntrySettings, FP = dt.removeTooltipEntrySettings, WP = dt.setTooltipSettingsState, ay = dt.setActiveMouseOverItemIndex, UP = dt.mouseLeaveItem, oy = dt.mouseLeaveChart, VP = dt.setActiveClickItemIndex, uy = dt.setMouseOverAxisIndex, KP = dt.setMouseClickAxisIndex, jn = dt.setSyncInteraction, wa = dt.setKeyboardInteraction, HP = iy.reducer;
function rd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Ti(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? rd(Object(r), !0).forEach(function(n) {
      YP(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : rd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function YP(e, t, r) {
  return (t = GP(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function GP(e) {
  var t = qP(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function qP(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function XP(e, t, r) {
  return t === "axis" ? r === "click" ? e.axisInteraction.click : e.axisInteraction.hover : r === "click" ? e.itemInteraction.click : e.itemInteraction.hover;
}
function ZP(e) {
  return e.index != null;
}
var ly = (e, t, r, n) => {
  if (t == null)
    return ar;
  var i = XP(e, t, r);
  if (i == null)
    return ar;
  if (i.active)
    return i;
  if (e.keyboardInteraction.active)
    return e.keyboardInteraction;
  if (e.syncInteraction.active && e.syncInteraction.index != null)
    return e.syncInteraction;
  var a = e.settings.active === !0;
  if (ZP(i)) {
    if (a)
      return Ti(Ti({}, i), {}, {
        active: !0
      });
  } else if (n != null)
    return {
      active: !0,
      coordinate: void 0,
      dataKey: void 0,
      index: n,
      graphicalItemId: void 0
    };
  return Ti(Ti({}, ar), {}, {
    coordinate: i.coordinate
  });
};
function QP(e) {
  if (typeof e == "number")
    return Number.isFinite(e) ? e : void 0;
  if (e instanceof Date) {
    var t = e.valueOf();
    return Number.isFinite(t) ? t : void 0;
  }
  var r = Number(e);
  return Number.isFinite(r) ? r : void 0;
}
function JP(e, t) {
  var r = QP(e), n = t[0], i = t[1];
  if (r === void 0)
    return !1;
  var a = Math.min(n, i), o = Math.max(n, i);
  return r >= a && r <= o;
}
function eI(e, t, r) {
  if (r == null || t == null)
    return !0;
  var n = Se(e, t);
  return n == null || !It(r) ? !0 : JP(n, r);
}
var Rn = (e, t, r, n) => {
  var i = e == null ? void 0 : e.index;
  if (i == null)
    return null;
  var a = Number(i);
  if (!Y(a))
    return i;
  var o = 0, u = 1 / 0;
  t.length > 0 && (u = t.length - 1);
  var l = Math.max(o, Math.min(a, u)), c = t[l];
  return c == null || eI(c, r, n) ? String(l) : null;
}, cy = (e, t, r, n, i, a, o) => {
  if (a != null) {
    var u = o[0], l = u == null ? void 0 : u.getPosition(a);
    if (l != null)
      return l;
    var c = i == null ? void 0 : i[Number(a)];
    if (c)
      switch (r) {
        case "horizontal":
          return {
            x: c.coordinate,
            y: (n.top + t) / 2
          };
        default:
          return {
            x: (n.left + e) / 2,
            y: c.coordinate
          };
      }
  }
}, sy = (e, t, r, n) => {
  if (t === "axis")
    return e.tooltipItemPayloads;
  if (e.tooltipItemPayloads.length === 0)
    return [];
  var i;
  if (r === "hover" ? i = e.itemInteraction.hover.graphicalItemId : i = e.itemInteraction.click.graphicalItemId, e.syncInteraction.active && i == null)
    return e.tooltipItemPayloads;
  if (i == null && (n != null || e.keyboardInteraction.active)) {
    var a = e.tooltipItemPayloads[0];
    return a != null ? [a] : [];
  }
  return e.tooltipItemPayloads.filter((o) => {
    var u;
    return ((u = o.settings) === null || u === void 0 ? void 0 : u.graphicalItemId) === i;
  });
}, fy = (e) => e.options.tooltipPayloadSearcher, xn = (e) => e.tooltip;
function nd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function id(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? nd(Object(r), !0).forEach(function(n) {
      tI(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : nd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function tI(e, t, r) {
  return (t = rI(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function rI(e) {
  var t = nI(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function nI(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function iI(e) {
  if (typeof e == "string" || typeof e == "number")
    return e;
}
function aI(e) {
  if (typeof e == "string" || typeof e == "number" || typeof e == "boolean")
    return e;
}
function oI(e) {
  if (typeof e == "string" || typeof e == "number")
    return e;
  if (typeof e == "function")
    return (t) => e(t);
}
function ad(e) {
  if (typeof e == "string")
    return e;
}
function uI(e) {
  if (!(e == null || typeof e != "object")) {
    var t = "name" in e ? iI(e.name) : void 0, r = "unit" in e ? aI(e.unit) : void 0, n = "dataKey" in e ? oI(e.dataKey) : void 0, i = "payload" in e ? e.payload : void 0, a = "color" in e ? ad(e.color) : void 0, o = "fill" in e ? ad(e.fill) : void 0;
    return {
      name: t,
      unit: r,
      dataKey: n,
      payload: i,
      color: a,
      fill: o
    };
  }
}
function lI(e, t) {
  return e ?? t;
}
var dy = (e, t, r, n, i, a, o) => {
  if (!(t == null || a == null)) {
    var u = r.chartData, l = r.computedData, c = r.dataStartIndex, s = r.dataEndIndex, f = [];
    return e.reduce((d, h) => {
      var p, v = h.dataDefinedOnItem, m = h.settings, y = lI(v, u), b = Array.isArray(y) ? Fh(y, c, s) : y, x = (p = m == null ? void 0 : m.dataKey) !== null && p !== void 0 ? p : n, w = m == null ? void 0 : m.nameKey, O;
      if (n && Array.isArray(b) && /*
       * findEntryInArray won't work for Scatter because Scatter provides an array of arrays
       * as tooltip payloads and findEntryInArray is not prepared to handle that.
       * Sad but also ScatterChart only allows 'item' tooltipEventType
       * and also this is only a problem if there are multiple Scatters and each has its own data array
       * so let's fix that some other time.
       */
      !Array.isArray(b[0]) && /*
       * If the tooltipEventType is 'axis', we should search for the dataKey in the sliced data
       * because thanks to allowDuplicatedCategory=false, the order of elements in the array
       * no longer matches the order of elements in the original data
       * and so we need to search by the active dataKey + label rather than by index.
       *
       * The same happens if multiple graphical items are present in the chart
       * and each of them has its own data array. Those arrays get concatenated
       * and again the tooltip index no longer matches the original data.
       *
       * On the other hand the tooltipEventType 'item' should always search by index
       * because we get the index from interacting over the individual elements
       * which is always accurate, irrespective of the allowDuplicatedCategory setting.
       */
      o === "axis" ? O = O0(b, n, i) : O = a(b, t, l, w), Array.isArray(O))
        O.forEach((S) => {
          var P, I, C = uI(S), T = C == null ? void 0 : C.name, _ = C == null ? void 0 : C.dataKey, z = C == null ? void 0 : C.payload, $ = id(id({}, m), {}, {
            name: T,
            unit: C == null ? void 0 : C.unit,
            // Preserve item-level color/fill from graphical items.
            color: (P = C == null ? void 0 : C.color) !== null && P !== void 0 ? P : m == null ? void 0 : m.color,
            fill: (I = C == null ? void 0 : C.fill) !== null && I !== void 0 ? I : m == null ? void 0 : m.fill
          });
          d.push(ns({
            tooltipEntrySettings: $,
            dataKey: _,
            payload: z,
            value: Se(z, _),
            name: T == null ? void 0 : String(T)
          }));
        });
      else {
        var g;
        d.push(ns({
          tooltipEntrySettings: m,
          dataKey: x,
          payload: O,
          // getValueByDataKey does not validate the output type
          value: Se(O, x),
          // getValueByDataKey does not validate the output type
          name: (g = Se(O, w)) !== null && g !== void 0 ? g : m == null ? void 0 : m.name
        }));
      }
      return d;
    }, f);
  }
}, Ql = E([Oe, gm, sl], vm), cI = E([(e) => e.graphicalItems.cartesianItems, (e) => e.graphicalItems.polarItems], (e, t) => [...e, ...t]), sI = E([ke, pn], bm), zr = E([cI, Oe, sI], xm, {
  memoizeOptions: {
    resultEqualityCheck: ro
  }
}), fI = E([zr], (e) => e.filter(to)), vy = E([zr], Om, {
  memoizeOptions: {
    resultEqualityCheck: ro
  }
}), dI = E([zr], (e) => e.some((t) => !t.data)), jr = E([vy, xt], Sm), vI = E([fI, xt, Oe], Mp), Jl = E([jr, Oe, zr, xt, dI, vy], _m), hy = E([Oe], Kl), hI = E([Oe], (e) => e.allowDataOverflow), py = E([hy, hI], dp), pI = E([zr], (e) => e.filter(to)), mI = E([vI, pI, Xa, Sp], Im), yI = E([mI, xt, ke, py], km), gI = E([zr], Am), bI = E([jr, Oe, gI, Hl, ke, P1], Mm, {
  memoizeOptions: {
    resultEqualityCheck: ai
  }
}), xI = E([Dm, ke, pn], gn), wI = E([xI, ke], $m), AI = E([Nm, ke, pn], gn), OI = E([AI, ke], Rm), EI = E([jm, ke, pn], gn), SI = E([EI, ke], Lm), _I = E([wI, SI, OI], xa), PI = E([Oe, hy, py, yI, bI, _I, ne, ke], zm), ln = E([Oe, ne, jr, Jl, Xa, ke, PI], Bm), II = E([ln, Oe, Ql], Fm), kI = E([Oe, ln, II, ke], Wm), my = (e) => {
  var t = ke(e), r = pn(e), n = !1;
  return di(e, t, r, n);
}, yy = E([Oe, my], Za), CI = E([Oe, Ql, kI, yy], Fl), gy = E([CI], yl), TI = E([ne, Jl, Oe, ke], Qm), MI = E([ne, Jl, Oe, ke], Ym), DI = (e, t, r, n, i, a, o, u) => {
  if (t) {
    var l = t.type, c = fr(e, u);
    if (n) {
      var s = r === "scaleBand" && n.bandwidth ? n.bandwidth() / 2 : 2, f = l === "category" && n.bandwidth ? n.bandwidth() / s : 0;
      return f = u === "angleAxis" && i != null && (i == null ? void 0 : i.length) >= 2 ? He(i[0] - i[1]) * 2 * f : f, c && o ? o.map((d, h) => {
        var p = n.map(d);
        return Y(p) ? {
          coordinate: p + f,
          value: d,
          index: h,
          offset: f
        } : null;
      }).filter(Ye) : n.domain().map((d, h) => {
        var p = n.map(d);
        return Y(p) ? {
          coordinate: p + f,
          // @ts-expect-error can't use Date as an index
          value: a ? a[d] : d,
          index: h,
          offset: f
        } : null;
      }).filter(Ye);
    }
  }
}, Jt = E([ne, Oe, Ql, gy, my, TI, MI, ke], DI), ec = E([ey, ty, RP], (e, t, r) => ry(r.shared, e, t)), by = (e) => e.tooltip.settings.trigger, tc = (e) => e.tooltip.settings.defaultIndex, hi = E([xn, ec, by, tc], ly), cn = E([hi, jr, an, ln], Rn), xy = E([Jt, cn], ny), wy = E([hi], (e) => {
  if (e)
    return e.dataKey;
}), NI = E([hi], (e) => {
  if (e)
    return e.graphicalItemId;
}), Ay = E([xn, ec, by, tc], sy), jI = E([Gt, qt, ne, Pe, Jt, tc, Ay], cy), $I = E([hi, jI], (e, t) => e != null && e.coordinate ? e.coordinate : t), RI = E([hi], (e) => {
  var t;
  return (t = e == null ? void 0 : e.active) !== null && t !== void 0 ? t : !1;
}), LI = E([Ay, cn, xt, an, xy, fy, ec], dy);
E([LI], (e) => {
  if (e != null) {
    var t = e.map((r) => r.payload).filter((r) => r != null);
    return Array.from(new Set(t));
  }
});
function od(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function ud(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? od(Object(r), !0).forEach(function(n) {
      zI(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : od(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function zI(e, t, r) {
  return (t = BI(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function BI(e) {
  var t = FI(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function FI(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var WI = () => j(Oe), UI = () => {
  var e = WI(), t = j(Jt), r = j(gy);
  return ea(!e || !r ? void 0 : ud(ud({}, e), {}, {
    scale: r
  }), t);
};
function ld(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Kr(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? ld(Object(r), !0).forEach(function(n) {
      VI(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : ld(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function VI(e, t, r) {
  return (t = KI(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function KI(e) {
  var t = HI(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function HI(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var YI = (e, t, r, n) => {
  var i = t.find((a) => a && a.index === r);
  if (i) {
    if (e === "horizontal")
      return {
        x: i.coordinate,
        y: n.relativeY
      };
    if (e === "vertical")
      return {
        x: n.relativeX,
        y: i.coordinate
      };
  }
  return {
    x: 0,
    y: 0
  };
}, GI = (e, t, r, n) => {
  var i = t.find((c) => c && c.index === r);
  if (i) {
    if (e === "centric") {
      var a = i.coordinate, o = n.radius;
      return Kr(Kr(Kr({}, n), Ne(n.cx, n.cy, o, a)), {}, {
        angle: a,
        radius: o
      });
    }
    var u = i.coordinate, l = n.angle;
    return Kr(Kr(Kr({}, n), Ne(n.cx, n.cy, u, l)), {}, {
      angle: l,
      radius: u
    });
  }
  return {
    angle: 0,
    clockWise: !1,
    cx: 0,
    cy: 0,
    endAngle: 0,
    innerRadius: 0,
    outerRadius: 0,
    radius: 0,
    startAngle: 0,
    x: 0,
    y: 0
  };
};
function qI(e, t) {
  var r = e.relativeX, n = e.relativeY;
  return r >= t.left && r <= t.left + t.width && n >= t.top && n <= t.top + t.height;
}
var Oy = (e, t, r, n, i) => {
  var a, o = (a = t == null ? void 0 : t.length) !== null && a !== void 0 ? a : 0;
  if (o <= 1 || e == null)
    return 0;
  if (n === "angleAxis" && i != null && Math.abs(Math.abs(i[1] - i[0]) - 360) <= 1e-6)
    for (var u = 0; u < o; u++) {
      var l, c, s, f, d, h = u > 0 ? (l = r[u - 1]) === null || l === void 0 ? void 0 : l.coordinate : (c = r[o - 1]) === null || c === void 0 ? void 0 : c.coordinate, p = (s = r[u]) === null || s === void 0 ? void 0 : s.coordinate, v = u >= o - 1 ? (f = r[0]) === null || f === void 0 ? void 0 : f.coordinate : (d = r[u + 1]) === null || d === void 0 ? void 0 : d.coordinate, m = void 0;
      if (!(h == null || p == null || v == null))
        if (He(p - h) !== He(v - p)) {
          var y = [];
          if (He(v - p) === He(i[1] - i[0])) {
            m = v;
            var b = p + i[1] - i[0];
            y[0] = Math.min(b, (b + h) / 2), y[1] = Math.max(b, (b + h) / 2);
          } else {
            m = h;
            var x = v + i[1] - i[0];
            y[0] = Math.min(p, (x + p) / 2), y[1] = Math.max(p, (x + p) / 2);
          }
          var w = [Math.min(p, (m + p) / 2), Math.max(p, (m + p) / 2)];
          if (e > w[0] && e <= w[1] || e >= y[0] && e <= y[1]) {
            var O;
            return (O = r[u]) === null || O === void 0 ? void 0 : O.index;
          }
        } else {
          var g = Math.min(h, v), S = Math.max(h, v);
          if (e > (g + p) / 2 && e <= (S + p) / 2) {
            var P;
            return (P = r[u]) === null || P === void 0 ? void 0 : P.index;
          }
        }
    }
  else if (t)
    for (var I = 0; I < o; I++) {
      var C = t[I];
      if (C != null) {
        var T = t[I + 1], _ = t[I - 1];
        if (I === 0 && T != null && e <= (C.coordinate + T.coordinate) / 2 || I === o - 1 && _ != null && e > (C.coordinate + _.coordinate) / 2 || I > 0 && I < o - 1 && _ != null && T != null && e > (C.coordinate + _.coordinate) / 2 && e <= (C.coordinate + T.coordinate) / 2)
          return C.index;
      }
    }
  return -1;
}, XI = () => j(sl), rc = (e, t) => t, Ey = (e, t, r) => r, nc = (e, t, r, n) => n, ZI = E(Jt, (e) => Da(e, (t) => t.coordinate)), ic = E([xn, rc, Ey, nc], ly), ac = E([ic, jr, an, ln], Rn), QI = (e, t, r) => {
  if (t != null) {
    var n = xn(e);
    return t === "axis" ? r === "hover" ? n.axisInteraction.hover.dataKey : n.axisInteraction.click.dataKey : r === "hover" ? n.itemInteraction.hover.dataKey : n.itemInteraction.click.dataKey;
  }
}, Sy = E([xn, rc, Ey, nc], sy), Aa = E([Gt, qt, ne, Pe, Jt, nc, Sy], cy), JI = E([ic, Aa], (e, t) => {
  var r;
  return (r = e.coordinate) !== null && r !== void 0 ? r : t;
}), _y = E([Jt, ac], ny), ek = E([Sy, ac, xt, an, _y, fy, rc], dy), tk = E([ic, ac], (e, t) => ({
  isActive: e.active && t != null,
  activeIndex: t
})), rk = (e, t, r, n, i, a, o) => {
  if (!(!e || !r || !n || !i) && qI(e, o)) {
    var u = _w(e, t), l = Oy(u, a, i, r, n), c = YI(t, i, l, e);
    return {
      activeIndex: String(l),
      activeCoordinate: c
    };
  }
}, nk = (e, t, r, n, i, a, o) => {
  if (!(!e || !n || !i || !a || !r)) {
    var u = g1(e, r);
    if (u) {
      var l = Pw(u, t), c = Oy(l, o, a, n, i), s = GI(t, a, c, u);
      return {
        activeIndex: String(c),
        activeCoordinate: s
      };
    }
  }
}, ik = (e, t, r, n, i, a, o, u) => {
  if (!(!e || !t || !n || !i || !a))
    return t === "horizontal" || t === "vertical" ? rk(e, t, n, i, a, o, u) : nk(e, t, r, n, i, a, o);
}, ak = E((e) => e.zIndex.zIndexMap, (e, t) => t, (e, t, r) => r, (e, t, r) => {
  if (t != null) {
    var n = e[t];
    if (n != null)
      return r ? n.panoramaElement : n.element;
  }
}), ok = E((e) => e.zIndex.zIndexMap, (e) => {
  var t = Object.keys(e).map((n) => parseInt(n, 10)).concat(Object.values(Ue)), r = Array.from(new Set(t));
  return r.sort((n, i) => n - i);
}, {
  memoizeOptions: {
    resultEqualityCheck: q1
  }
});
function cd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function sd(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? cd(Object(r), !0).forEach(function(n) {
      uk(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : cd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function uk(e, t, r) {
  return (t = lk(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function lk(e) {
  var t = ck(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function ck(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var sk = {}, fk = {
  zIndexMap: Object.values(Ue).reduce((e, t) => sd(sd({}, e), {}, {
    [t]: {
      element: void 0,
      panoramaElement: void 0,
      consumers: 0
    }
  }), sk)
}, dk = new Set(Object.values(Ue));
function vk(e) {
  return dk.has(e);
}
var Py = Be({
  name: "zIndex",
  initialState: fk,
  reducers: {
    registerZIndexPortal: {
      reducer: (e, t) => {
        var r = t.payload.zIndex;
        e.zIndexMap[r] ? e.zIndexMap[r].consumers += 1 : e.zIndexMap[r] = {
          consumers: 1,
          element: void 0,
          panoramaElement: void 0
        };
      },
      prepare: ae()
    },
    unregisterZIndexPortal: {
      reducer: (e, t) => {
        var r = t.payload.zIndex;
        e.zIndexMap[r] && (e.zIndexMap[r].consumers -= 1, e.zIndexMap[r].consumers <= 0 && !vk(r) && delete e.zIndexMap[r]);
      },
      prepare: ae()
    },
    registerZIndexPortalElement: {
      reducer: (e, t) => {
        var r = t.payload, n = r.zIndex, i = r.element, a = r.isPanorama;
        e.zIndexMap[n] ? a ? e.zIndexMap[n].panoramaElement = q(i) : e.zIndexMap[n].element = q(i) : e.zIndexMap[n] = {
          consumers: 0,
          element: a ? void 0 : q(i),
          panoramaElement: a ? q(i) : void 0
        };
      },
      prepare: ae()
    },
    unregisterZIndexPortalElement: {
      reducer: (e, t) => {
        var r = t.payload.zIndex;
        e.zIndexMap[r] && (t.payload.isPanorama ? e.zIndexMap[r].panoramaElement = void 0 : e.zIndexMap[r].element = void 0);
      },
      prepare: ae()
    }
  }
}), vo = Py.actions, hk = vo.registerZIndexPortal, Bo = vo.unregisterZIndexPortal, pk = vo.registerZIndexPortalElement, mk = vo.unregisterZIndexPortalElement, yk = Py.reducer;
function er(e) {
  var t = e.zIndex, r = e.children, n = fA(), i = n && t !== void 0 && t !== 0, a = Ze(), o = Q(void 0), u = Q(/* @__PURE__ */ new Set()), l = se(), c = j((f) => ak(f, t, a));
  if (qe(() => {
    if (!i) {
      var f = u.current;
      f.forEach((h) => {
        l(Bo({
          zIndex: h
        }));
      }), f.clear(), o.current = void 0;
      return;
    }
    if (u.current.has(t) || (l(hk({
      zIndex: t
    })), u.current.add(t)), c) {
      o.current = c;
      var d = u.current;
      d.forEach((h) => {
        h !== t && (l(Bo({
          zIndex: h
        })), d.delete(h));
      });
    }
  }, [l, t, i, c]), qe(() => {
    var f = u.current;
    return () => {
      f.forEach((d) => {
        l(Bo({
          zIndex: d
        }));
      }), f.clear();
    };
  }, [l]), !i)
    return r;
  var s = c ?? o.current;
  return s ? /* @__PURE__ */ Ov(r, s) : null;
}
function Iu() {
  return Iu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Iu.apply(null, arguments);
}
function fd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Mi(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? fd(Object(r), !0).forEach(function(n) {
      gk(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : fd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function gk(e, t, r) {
  return (t = bk(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function bk(e) {
  var t = xk(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function xk(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function wk(e) {
  var t = e.cursor, r = e.cursorComp, n = e.cursorProps;
  return /* @__PURE__ */ Pt(t) ? /* @__PURE__ */ Ca(t, n) : /* @__PURE__ */ Av(r, n);
}
function Ak(e) {
  var t, r = e.coordinate, n = e.payload, i = e.index, a = e.offset, o = e.tooltipAxisBandSize, u = e.layout, l = e.cursor, c = e.tooltipEventType, s = e.chartName, f = r, d = n, h = i;
  if (!l || !f || s !== "ScatterChart" && c !== "axis")
    return null;
  var p, v, m;
  if (s === "ScatterChart")
    p = f, v = PO, m = Ue.cursorLine;
  else if (s === "BarChart")
    p = IO(u, f, a, o), v = lp, m = Ue.cursorRectangle;
  else if (u === "radial" && Kv(f)) {
    var y = cp(f), b = y.cx, x = y.cy, w = y.radius, O = y.startAngle, g = y.endAngle;
    p = {
      cx: b,
      cy: x,
      startAngle: O,
      endAngle: g,
      innerRadius: w,
      outerRadius: w
    }, v = A1, m = Ue.cursorLine;
  } else
    p = {
      points: O1(u, f, a)
    }, v = gO, m = Ue.cursorLine;
  var S = typeof l == "object" && "className" in l ? l.className : void 0, P = Mi(Mi(Mi(Mi({
    stroke: "#ccc",
    pointerEvents: "none"
  }, a), p), Wu(l)), {}, {
    payload: d,
    payloadIndex: h,
    className: ce("recharts-tooltip-cursor", S)
  });
  return /* @__PURE__ */ A.createElement(er, {
    zIndex: (t = e.zIndex) !== null && t !== void 0 ? t : m
  }, /* @__PURE__ */ A.createElement(wk, {
    cursor: l,
    cursorComp: v,
    cursorProps: P
  }));
}
function Ok(e) {
  var t = UI(), r = qh(), n = dn(), i = XI();
  return t == null || r == null || n == null || i == null ? null : /* @__PURE__ */ A.createElement(Ak, Iu({}, e, {
    offset: r,
    layout: n,
    tooltipAxisBandSize: t,
    chartName: i
  }));
}
var Iy = /* @__PURE__ */ Xe(null), Ek = () => ct(Iy), ky = { exports: {} };
(function(e) {
  var t = Object.prototype.hasOwnProperty, r = "~";
  function n() {
  }
  Object.create && (n.prototype = /* @__PURE__ */ Object.create(null), new n().__proto__ || (r = !1));
  function i(l, c, s) {
    this.fn = l, this.context = c, this.once = s || !1;
  }
  function a(l, c, s, f, d) {
    if (typeof s != "function")
      throw new TypeError("The listener must be a function");
    var h = new i(s, f || l, d), p = r ? r + c : c;
    return l._events[p] ? l._events[p].fn ? l._events[p] = [l._events[p], h] : l._events[p].push(h) : (l._events[p] = h, l._eventsCount++), l;
  }
  function o(l, c) {
    --l._eventsCount === 0 ? l._events = new n() : delete l._events[c];
  }
  function u() {
    this._events = new n(), this._eventsCount = 0;
  }
  u.prototype.eventNames = function() {
    var c = [], s, f;
    if (this._eventsCount === 0) return c;
    for (f in s = this._events)
      t.call(s, f) && c.push(r ? f.slice(1) : f);
    return Object.getOwnPropertySymbols ? c.concat(Object.getOwnPropertySymbols(s)) : c;
  }, u.prototype.listeners = function(c) {
    var s = r ? r + c : c, f = this._events[s];
    if (!f) return [];
    if (f.fn) return [f.fn];
    for (var d = 0, h = f.length, p = new Array(h); d < h; d++)
      p[d] = f[d].fn;
    return p;
  }, u.prototype.listenerCount = function(c) {
    var s = r ? r + c : c, f = this._events[s];
    return f ? f.fn ? 1 : f.length : 0;
  }, u.prototype.emit = function(c, s, f, d, h, p) {
    var v = r ? r + c : c;
    if (!this._events[v]) return !1;
    var m = this._events[v], y = arguments.length, b, x;
    if (m.fn) {
      switch (m.once && this.removeListener(c, m.fn, void 0, !0), y) {
        case 1:
          return m.fn.call(m.context), !0;
        case 2:
          return m.fn.call(m.context, s), !0;
        case 3:
          return m.fn.call(m.context, s, f), !0;
        case 4:
          return m.fn.call(m.context, s, f, d), !0;
        case 5:
          return m.fn.call(m.context, s, f, d, h), !0;
        case 6:
          return m.fn.call(m.context, s, f, d, h, p), !0;
      }
      for (x = 1, b = new Array(y - 1); x < y; x++)
        b[x - 1] = arguments[x];
      m.fn.apply(m.context, b);
    } else {
      var w = m.length, O;
      for (x = 0; x < w; x++)
        switch (m[x].once && this.removeListener(c, m[x].fn, void 0, !0), y) {
          case 1:
            m[x].fn.call(m[x].context);
            break;
          case 2:
            m[x].fn.call(m[x].context, s);
            break;
          case 3:
            m[x].fn.call(m[x].context, s, f);
            break;
          case 4:
            m[x].fn.call(m[x].context, s, f, d);
            break;
          default:
            if (!b) for (O = 1, b = new Array(y - 1); O < y; O++)
              b[O - 1] = arguments[O];
            m[x].fn.apply(m[x].context, b);
        }
    }
    return !0;
  }, u.prototype.on = function(c, s, f) {
    return a(this, c, s, f, !1);
  }, u.prototype.once = function(c, s, f) {
    return a(this, c, s, f, !0);
  }, u.prototype.removeListener = function(c, s, f, d) {
    var h = r ? r + c : c;
    if (!this._events[h]) return this;
    if (!s)
      return o(this, h), this;
    var p = this._events[h];
    if (p.fn)
      p.fn === s && (!d || p.once) && (!f || p.context === f) && o(this, h);
    else {
      for (var v = 0, m = [], y = p.length; v < y; v++)
        (p[v].fn !== s || d && !p[v].once || f && p[v].context !== f) && m.push(p[v]);
      m.length ? this._events[h] = m.length === 1 ? m[0] : m : o(this, h);
    }
    return this;
  }, u.prototype.removeAllListeners = function(c) {
    var s;
    return c ? (s = r ? r + c : c, this._events[s] && o(this, s)) : (this._events = new n(), this._eventsCount = 0), this;
  }, u.prototype.off = u.prototype.removeListener, u.prototype.addListener = u.prototype.on, u.prefixed = r, u.EventEmitter = u, e.exports = u;
})(ky);
var Sk = ky.exports;
const _k = /* @__PURE__ */ Wg(Sk);
var qn = new _k(), ku = "recharts.syncEvent.tooltip", dd = "recharts.syncEvent.brush", Pk = (e, t) => {
  if (t && Array.isArray(e)) {
    var r = Number.parseInt(t, 10);
    if (!Tt(r))
      return e[r];
  }
}, Ik = {
  chartName: "",
  tooltipPayloadSearcher: () => {
  },
  eventEmitter: void 0,
  defaultTooltipEventType: "axis"
}, Cy = Be({
  name: "options",
  initialState: Ik,
  reducers: {
    createEventEmitter: (e) => {
      e.eventEmitter == null && (e.eventEmitter = Symbol("rechartsEventEmitter"));
    }
  }
}), kk = Cy.reducer, Ck = Cy.actions.createEventEmitter;
function Tk(e) {
  return e.tooltip.syncInteraction;
}
var Mk = {
  chartData: void 0,
  computedData: void 0,
  dataStartIndex: 0,
  dataEndIndex: 0
}, Ty = Be({
  name: "chartData",
  initialState: Mk,
  reducers: {
    setChartData(e, t) {
      if (e.chartData = q(t.payload), t.payload == null) {
        e.dataStartIndex = 0, e.dataEndIndex = 0;
        return;
      }
      t.payload.length > 0 && e.dataEndIndex !== t.payload.length - 1 && (e.dataEndIndex = t.payload.length - 1);
    },
    setComputedData(e, t) {
      e.computedData = t.payload;
    },
    setDataStartEndIndexes(e, t) {
      var r = t.payload, n = r.startIndex, i = r.endIndex;
      n != null && (e.dataStartIndex = n), i != null && (e.dataEndIndex = i);
    }
  }
}), oc = Ty.actions, vd = oc.setChartData, Dk = oc.setDataStartEndIndexes;
oc.setComputedData;
var Nk = Ty.reducer, jk = ["x", "y"];
function hd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Hr(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? hd(Object(r), !0).forEach(function(n) {
      $k(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : hd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function $k(e, t, r) {
  return (t = Rk(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function Rk(e) {
  var t = Lk(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Lk(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function zk(e, t) {
  if (e == null) return {};
  var r, n, i = Bk(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function Bk(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Fk() {
  var e = j(fl), t = j(dl), r = se(), n = j(_p), i = j(Jt), a = dn(), o = Ya(), u = j((l) => l.rootProps.className);
  ve(() => {
    if (e == null)
      return fn;
    var l = (c, s, f) => {
      if (t !== f && e === c) {
        if (s.payload.active === !1) {
          r(jn({
            active: !1,
            coordinate: void 0,
            dataKey: void 0,
            index: null,
            label: void 0,
            sourceViewBox: void 0,
            graphicalItemId: void 0
          }));
          return;
        }
        if (n === "index") {
          var d;
          if (o && s !== null && s !== void 0 && (d = s.payload) !== null && d !== void 0 && d.coordinate && s.payload.sourceViewBox) {
            var h = s.payload.coordinate, p = h.x, v = h.y, m = zk(h, jk), y = s.payload.sourceViewBox, b = y.x, x = y.y, w = y.width, O = y.height, g = Hr(Hr({}, m), {}, {
              x: o.x + (w ? (p - b) / w : 0) * o.width,
              y: o.y + (O ? (v - x) / O : 0) * o.height
            });
            r(Hr(Hr({}, s), {}, {
              payload: Hr(Hr({}, s.payload), {}, {
                coordinate: g
              })
            }));
          } else
            r(s);
          return;
        }
        if (i != null) {
          var S;
          if (typeof n == "function") {
            var P = {
              activeTooltipIndex: s.payload.index == null ? void 0 : Number(s.payload.index),
              isTooltipActive: s.payload.active,
              activeIndex: s.payload.index == null ? void 0 : Number(s.payload.index),
              activeLabel: s.payload.label,
              activeDataKey: s.payload.dataKey,
              activeCoordinate: s.payload.coordinate
            }, I = n(i, P);
            S = i[I];
          } else n === "value" && (S = i.find((K) => String(K.value) === s.payload.label));
          var C = s.payload.coordinate;
          if (C == null || o == null) {
            r(jn({
              active: !1,
              coordinate: void 0,
              dataKey: void 0,
              index: null,
              label: void 0,
              sourceViewBox: void 0,
              graphicalItemId: void 0
            }));
            return;
          }
          if (S == null) {
            r(jn({
              active: !1,
              coordinate: void 0,
              dataKey: void 0,
              index: null,
              label: void 0,
              sourceViewBox: s.payload.sourceViewBox,
              graphicalItemId: void 0
            }));
            return;
          }
          var T = C.x, _ = C.y, z = Math.min(T, o.x + o.width), $ = Math.min(_, o.y + o.height), U = {
            x: a === "horizontal" ? S.coordinate : z,
            y: a === "horizontal" ? $ : S.coordinate
          }, V = jn({
            active: s.payload.active,
            coordinate: U,
            dataKey: s.payload.dataKey,
            index: String(S.index),
            label: s.payload.label,
            sourceViewBox: s.payload.sourceViewBox,
            graphicalItemId: s.payload.graphicalItemId
          });
          r(V);
        }
      }
    };
    return qn.on(ku, l), () => {
      qn.off(ku, l);
    };
  }, [u, r, t, e, n, i, a, o]);
}
function Wk() {
  var e = j(fl), t = j(dl), r = se();
  ve(() => {
    if (e == null)
      return fn;
    var n = (i, a, o) => {
      t !== o && e === i && r(Dk(a));
    };
    return qn.on(dd, n), () => {
      qn.off(dd, n);
    };
  }, [r, t, e]);
}
function Uk() {
  var e = se();
  ve(() => {
    e(Ck());
  }, [e]), Fk(), Wk();
}
function Vk(e, t, r, n, i, a) {
  var o = j((p) => QI(p, e, t)), u = j(NI), l = j(dl), c = j(fl), s = j(_p), f = j(Tk), d = (f == null ? void 0 : f.sourceViewBox) != null, h = Ya();
  ve(() => {
    if (!d && c != null && l != null) {
      var p = jn({
        active: a,
        coordinate: r,
        dataKey: o,
        index: i,
        label: typeof n == "number" ? String(n) : n,
        sourceViewBox: h,
        graphicalItemId: u
      });
      qn.emit(ku, c, p, l);
    }
  }, [d, r, o, u, i, n, l, c, s, a, h]);
}
function pd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function md(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? pd(Object(r), !0).forEach(function(n) {
      Kk(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : pd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function Kk(e, t, r) {
  return (t = Hk(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function Hk(e) {
  var t = Yk(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Yk(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function Gk(e, t) {
  return Qk(e) || Zk(e, t) || Xk(e, t) || qk();
}
function qk() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function Xk(e, t) {
  if (e) {
    if (typeof e == "string") return yd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? yd(e, t) : void 0;
  }
}
function yd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function Zk(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function Qk(e) {
  if (Array.isArray(e)) return e;
}
function Jk(e) {
  return e.dataKey;
}
function eC(e, t) {
  return /* @__PURE__ */ A.isValidElement(e) ? /* @__PURE__ */ A.cloneElement(e, t) : typeof e == "function" ? /* @__PURE__ */ A.createElement(e, t) : /* @__PURE__ */ A.createElement(KA, t);
}
var gd = [], tC = {
  allowEscapeViewBox: {
    x: !1,
    y: !1
  },
  animationDuration: 400,
  animationEasing: "ease",
  axisId: 0,
  contentStyle: {},
  cursor: !0,
  filterNull: !0,
  includeHidden: !1,
  isAnimationActive: "auto",
  itemSorter: "name",
  itemStyle: {},
  labelStyle: {},
  offset: 10,
  reverseDirection: {
    x: !1,
    y: !1
  },
  separator: " : ",
  trigger: "hover",
  useTranslate3d: !1,
  wrapperStyle: {}
};
function rC(e) {
  var t, r, n = st(e, tC), i = n.active, a = n.allowEscapeViewBox, o = n.animationDuration, u = n.animationEasing, l = n.content, c = n.filterNull, s = n.isAnimationActive, f = n.offset, d = n.payloadUniqBy, h = n.position, p = n.reverseDirection, v = n.useTranslate3d, m = n.wrapperStyle, y = n.cursor, b = n.shared, x = n.trigger, w = n.defaultIndex, O = n.portal, g = n.axisId, S = se(), P = typeof w == "number" ? String(w) : w;
  ve(() => {
    S(WP({
      shared: b,
      trigger: x,
      axisId: g,
      active: i,
      defaultIndex: P
    }));
  }, [S, b, x, g, i, P]);
  var I = Ya(), C = rp(), T = $P(b), _ = (t = j((B) => tk(B, T, x, P))) !== null && t !== void 0 ? t : {}, z = _.activeIndex, $ = _.isActive, U = j((B) => ek(B, T, x, P)), V = j((B) => _y(B, T, x, P)), K = j((B) => JI(B, T, x, P)), L = U, G = Ek(), F = (r = i ?? $) !== null && r !== void 0 ? r : !1, he = Zb([L, F]), pe = Gk(he, 2), le = pe[0], We = pe[1], Qe = T === "axis" ? V : void 0;
  Vk(T, x, K, Qe, z, F);
  var vt = O ?? G;
  if (vt == null || I == null || T == null)
    return null;
  var ht = L ?? gd;
  F || (ht = gd), c && ht.length && (ht = yb(ht.filter((B) => B.value != null && (B.hide !== !0 || n.includeHidden)), d, Jk));
  var On = ht.length > 0, M = md(md({}, n), {}, {
    payload: ht,
    label: Qe,
    active: F,
    activeIndex: z,
    coordinate: K,
    accessibilityLayer: C
  }), W = /* @__PURE__ */ A.createElement(dO, {
    allowEscapeViewBox: a,
    animationDuration: o,
    animationEasing: u,
    isAnimationActive: s,
    active: F,
    coordinate: K,
    hasPayload: On,
    offset: f,
    position: h,
    reverseDirection: p,
    useTranslate3d: v,
    viewBox: I,
    wrapperStyle: m,
    lastBoundingBox: le,
    innerRef: We,
    hasPortalFromProps: !!O
  }, eC(l, M));
  return /* @__PURE__ */ A.createElement(A.Fragment, null, /* @__PURE__ */ Ov(W, vt), F && /* @__PURE__ */ A.createElement(Ok, {
    cursor: y,
    tooltipEventType: T,
    coordinate: K,
    payload: ht,
    index: z
  }));
}
var My = (e) => null;
My.displayName = "Cell";
function nC(e, t, r) {
  return (t = iC(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function iC(e) {
  var t = aC(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function aC(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
class oC {
  constructor(t) {
    nC(this, "cache", /* @__PURE__ */ new Map()), this.maxSize = t;
  }
  get(t) {
    var r = this.cache.get(t);
    return r !== void 0 && (this.cache.delete(t), this.cache.set(t, r)), r;
  }
  set(t, r) {
    if (this.cache.has(t))
      this.cache.delete(t);
    else if (this.cache.size >= this.maxSize) {
      var n = this.cache.keys().next().value;
      n != null && this.cache.delete(n);
    }
    this.cache.set(t, r);
  }
  clear() {
    this.cache.clear();
  }
  size() {
    return this.cache.size;
  }
}
function bd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function uC(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? bd(Object(r), !0).forEach(function(n) {
      lC(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : bd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function lC(e, t, r) {
  return (t = cC(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function cC(e) {
  var t = sC(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function sC(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var fC = {
  cacheSize: 2e3,
  enableCache: !0
}, Dy = uC({}, fC), xd = new oC(Dy.cacheSize), dC = {
  position: "absolute",
  top: "-20000px",
  left: 0,
  padding: 0,
  margin: 0,
  border: "none",
  whiteSpace: "pre"
}, wd = "recharts_measurement_span";
function vC(e, t) {
  var r = t.fontSize || "", n = t.fontFamily || "", i = t.fontWeight || "", a = t.fontStyle || "", o = t.letterSpacing || "", u = t.textTransform || "";
  return "".concat(e, "|").concat(r, "|").concat(n, "|").concat(i, "|").concat(a, "|").concat(o, "|").concat(u);
}
var Ad = (e, t) => {
  try {
    var r = document.getElementById(wd);
    r || (r = document.createElement("span"), r.setAttribute("id", wd), r.setAttribute("aria-hidden", "true"), document.body.appendChild(r)), Object.assign(r.style, dC, t), r.textContent = "".concat(e);
    var n = r.getBoundingClientRect();
    return {
      width: n.width,
      height: n.height
    };
  } catch {
    return {
      width: 0,
      height: 0
    };
  }
}, Ln = function(t) {
  var r = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : {};
  if (t == null || ii.isSsr)
    return {
      width: 0,
      height: 0
    };
  if (!Dy.enableCache)
    return Ad(t, r);
  var n = vC(t, r), i = xd.get(n);
  if (i)
    return i;
  var a = Ad(t, r);
  return xd.set(n, a), a;
}, Ny;
function Oa(e, t) {
  return yC(e) || mC(e, t) || pC(e, t) || hC();
}
function hC() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function pC(e, t) {
  if (e) {
    if (typeof e == "string") return Od(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Od(e, t) : void 0;
  }
}
function Od(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function mC(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t === 0) {
        if (Object(r) !== r) return;
        l = !1;
      } else for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function yC(e) {
  if (Array.isArray(e)) return e;
}
function gC(e, t, r) {
  return (t = bC(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function bC(e) {
  var t = xC(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function xC(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var Ed = /(-?\d+(?:\.\d+)?[a-zA-Z%]*)([*/])(-?\d+(?:\.\d+)?[a-zA-Z%]*)/, Sd = /(-?\d+(?:\.\d+)?[a-zA-Z%]*)([+-])(-?\d+(?:\.\d+)?[a-zA-Z%]*)/, wC = /^(px|cm|vh|vw|em|rem|%|mm|in|pt|pc|ex|ch|vmin|vmax|Q)$/, AC = /(-?\d+(?:\.\d+)?)([a-zA-Z%]+)?/, OC = {
  cm: 96 / 2.54,
  mm: 96 / 25.4,
  pt: 96 / 72,
  pc: 96 / 6,
  in: 96,
  Q: 96 / (2.54 * 40),
  px: 1
}, EC = ["cm", "mm", "pt", "pc", "in", "Q", "px"];
function SC(e) {
  return EC.includes(e);
}
var Xr = "NaN";
function _C(e, t) {
  return e * OC[t];
}
class De {
  static parse(t) {
    var r, n = (r = AC.exec(t)) !== null && r !== void 0 ? r : [], i = Oa(n, 3), a = i[1], o = i[2];
    return a == null ? De.NaN : new De(parseFloat(a), o ?? "");
  }
  constructor(t, r) {
    this.num = t, this.unit = r, this.num = t, this.unit = r, Tt(t) && (this.unit = ""), r !== "" && !wC.test(r) && (this.num = NaN, this.unit = ""), SC(r) && (this.num = _C(t, r), this.unit = "px");
  }
  add(t) {
    return this.unit !== t.unit ? new De(NaN, "") : new De(this.num + t.num, this.unit);
  }
  subtract(t) {
    return this.unit !== t.unit ? new De(NaN, "") : new De(this.num - t.num, this.unit);
  }
  multiply(t) {
    return this.unit !== "" && t.unit !== "" && this.unit !== t.unit ? new De(NaN, "") : new De(this.num * t.num, this.unit || t.unit);
  }
  divide(t) {
    return this.unit !== "" && t.unit !== "" && this.unit !== t.unit ? new De(NaN, "") : new De(this.num / t.num, this.unit || t.unit);
  }
  toString() {
    return "".concat(this.num).concat(this.unit);
  }
  isNaN() {
    return Tt(this.num);
  }
}
Ny = De;
gC(De, "NaN", new Ny(NaN, ""));
function jy(e) {
  if (e == null || e.includes(Xr))
    return Xr;
  for (var t = e; t.includes("*") || t.includes("/"); ) {
    var r, n = (r = Ed.exec(t)) !== null && r !== void 0 ? r : [], i = Oa(n, 4), a = i[1], o = i[2], u = i[3], l = De.parse(a ?? ""), c = De.parse(u ?? ""), s = o === "*" ? l.multiply(c) : l.divide(c);
    if (s.isNaN())
      return Xr;
    t = t.replace(Ed, s.toString());
  }
  for (; t.includes("+") || /.-\d+(?:\.\d+)?/.test(t); ) {
    var f, d = (f = Sd.exec(t)) !== null && f !== void 0 ? f : [], h = Oa(d, 4), p = h[1], v = h[2], m = h[3], y = De.parse(p ?? ""), b = De.parse(m ?? ""), x = v === "+" ? y.add(b) : y.subtract(b);
    if (x.isNaN())
      return Xr;
    t = t.replace(Sd, x.toString());
  }
  return t;
}
var _d = /\(([^()]*)\)/;
function PC(e) {
  for (var t = e, r; (r = _d.exec(t)) != null; ) {
    var n = r, i = Oa(n, 2), a = i[1];
    t = t.replace(_d, jy(a));
  }
  return t;
}
function IC(e) {
  var t = e.replace(/\s+/g, "");
  return t = PC(t), t = jy(t), t;
}
function kC(e) {
  try {
    return IC(e);
  } catch {
    return Xr;
  }
}
function Fo(e) {
  var t = kC(e.slice(5, -1));
  return t === Xr ? "" : t;
}
var CC = ["x", "y", "lineHeight", "capHeight", "fill", "scaleToFit", "textAnchor", "verticalAnchor"], TC = ["dx", "dy", "angle", "className", "breakAll"];
function Cu() {
  return Cu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Cu.apply(null, arguments);
}
function Pd(e, t) {
  if (e == null) return {};
  var r, n, i = MC(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function MC(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Id(e, t) {
  return $C(e) || jC(e, t) || NC(e, t) || DC();
}
function DC() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function NC(e, t) {
  if (e) {
    if (typeof e == "string") return kd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? kd(e, t) : void 0;
  }
}
function kd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function jC(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t === 0) {
        if (Object(r) !== r) return;
        l = !1;
      } else for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function $C(e) {
  if (Array.isArray(e)) return e;
}
var $y = /[ \f\n\r\t\v\u2028\u2029]+/, Ry = (e) => {
  var t = e.children, r = e.breakAll, n = e.style;
  try {
    var i = [];
    we(t) || (r ? i = t.toString().split("") : i = t.toString().split($y));
    var a = i.map((u) => ({
      word: u,
      width: Ln(u, n).width
    })), o = r ? 0 : Ln(" ", n).width;
    return {
      wordsWithComputedWidth: a,
      spaceWidth: o
    };
  } catch {
    return null;
  }
};
function Ly(e) {
  return e === "start" || e === "middle" || e === "end" || e === "inherit";
}
function RC(e) {
  return we(e) || typeof e == "string" || typeof e == "number" || typeof e == "boolean";
}
var zy = (e, t, r, n) => e.reduce((i, a) => {
  var o = a.word, u = a.width, l = i[i.length - 1];
  if (l && u != null && (t == null || n || l.width + u + r < Number(t)))
    l.words.push(o), l.width += u + r;
  else {
    var c = {
      words: [o],
      width: u
    };
    i.push(c);
  }
  return i;
}, []), By = (e) => e.reduce((t, r) => t.width > r.width ? t : r), LC = "…", Cd = (e, t, r, n, i, a, o, u) => {
  var l = e.slice(0, t), c = Ry({
    breakAll: r,
    style: n,
    children: l + LC
  });
  if (!c)
    return [!1, []];
  var s = zy(c.wordsWithComputedWidth, a, o, u), f = s.length > i || By(s).width > Number(a);
  return [f, s];
}, zC = (e, t, r, n, i) => {
  var a = e.maxLines, o = e.children, u = e.style, l = e.breakAll, c = N(a), s = String(o), f = zy(t, n, r, i);
  if (!c || i)
    return f;
  var d = f.length > a || By(f).width > Number(n);
  if (!d)
    return f;
  for (var h = 0, p = s.length - 1, v = 0, m; h <= p && v <= s.length - 1; ) {
    var y = Math.floor((h + p) / 2), b = y - 1, x = Cd(s, b, l, u, a, n, r, i), w = Id(x, 2), O = w[0], g = w[1], S = Cd(s, y, l, u, a, n, r, i), P = Id(S, 1), I = P[0];
    if (!O && !I && (h = y + 1), O && I && (p = y - 1), !O && I) {
      m = g;
      break;
    }
    v++;
  }
  return m || f;
}, Td = (e) => {
  var t = we(e) ? [] : e.toString().split($y);
  return [{
    words: t,
    width: void 0
  }];
}, BC = (e) => {
  var t = e.width, r = e.scaleToFit, n = e.children, i = e.style, a = e.breakAll, o = e.maxLines;
  if ((t || r) && !ii.isSsr) {
    var u, l, c = Ry({
      breakAll: a,
      children: n,
      style: i
    });
    if (c) {
      var s = c.wordsWithComputedWidth, f = c.spaceWidth;
      u = s, l = f;
    } else
      return Td(n);
    return zC({
      breakAll: a,
      children: n,
      maxLines: o,
      style: i
    }, u, l, t, !!r);
  }
  return Td(n);
}, Fy = "#808080", FC = {
  angle: 0,
  breakAll: !1,
  // Magic number from d3
  capHeight: "0.71em",
  fill: Fy,
  lineHeight: "1em",
  scaleToFit: !1,
  textAnchor: "start",
  // Maintain compat with existing charts / default SVG behavior
  verticalAnchor: "end",
  x: 0,
  y: 0
}, uc = /* @__PURE__ */ ze((e, t) => {
  var r = st(e, FC), n = r.x, i = r.y, a = r.lineHeight, o = r.capHeight, u = r.fill, l = r.scaleToFit, c = r.textAnchor, s = r.verticalAnchor, f = Pd(r, CC), d = sr(() => BC({
    breakAll: f.breakAll,
    children: f.children,
    maxLines: f.maxLines,
    scaleToFit: l,
    style: f.style,
    width: f.width
  }), [f.breakAll, f.children, f.maxLines, l, f.style, f.width]), h = f.dx, p = f.dy, v = f.angle, m = f.className, y = f.breakAll, b = Pd(f, TC);
  if (!Mt(n) || !Mt(i) || d.length === 0)
    return null;
  var x = Number(n) + (N(h) ? h : 0), w = Number(i) + (N(p) ? p : 0);
  if (!Y(x) || !Y(w))
    return null;
  var O;
  switch (s) {
    case "start":
      O = Fo("calc(".concat(o, ")"));
      break;
    case "middle":
      O = Fo("calc(".concat((d.length - 1) / 2, " * -").concat(a, " + (").concat(o, " / 2))"));
      break;
    default:
      O = Fo("calc(".concat(d.length - 1, " * -").concat(a, ")"));
      break;
  }
  var g = [], S = d[0];
  if (l && S != null) {
    var P = S.width, I = f.width;
    g.push("scale(".concat(N(I) && N(P) ? I / P : 1, ")"));
  }
  return v && g.push("rotate(".concat(v, ", ").concat(x, ", ").concat(w, ")")), g.length && (b.transform = g.join(" ")), /* @__PURE__ */ A.createElement("text", Cu({}, Wt(b), {
    ref: t,
    x,
    y: w,
    className: ce("recharts-text", m),
    textAnchor: c,
    fill: u.includes("url") ? Fy : u
  }), d.map((C, T) => {
    var _ = C.words.join(y ? "" : " ");
    return (
      // duplicate words will cause duplicate keys which is why we add the array index here
      /* @__PURE__ */ A.createElement("tspan", {
        x,
        dy: T === 0 ? O : a,
        key: "".concat(_, "-").concat(T)
      }, _)
    );
  }));
});
uc.displayName = "Text";
function Md(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Ot(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Md(Object(r), !0).forEach(function(n) {
      WC(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Md(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function WC(e, t, r) {
  return (t = UC(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function UC(e) {
  var t = VC(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function VC(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var KC = (e) => {
  var t = e.viewBox, r = e.position, n = e.offset, i = n === void 0 ? 0 : n, a = e.parentViewBox, o = il(t), u = o.x, l = o.y, c = o.height, s = o.upperWidth, f = o.lowerWidth, d = u, h = u + (s - f) / 2, p = (d + h) / 2, v = (s + f) / 2, m = d + s / 2, y = c >= 0 ? 1 : -1, b = y * i, x = y > 0 ? "end" : "start", w = y > 0 ? "start" : "end", O = s >= 0 ? 1 : -1, g = O * i, S = O > 0 ? "end" : "start", P = O > 0 ? "start" : "end", I = a;
  if (r === "top") {
    var C = {
      x: d + s / 2,
      y: l - b,
      horizontalAnchor: "middle",
      verticalAnchor: x
    };
    return I && (C.height = Math.max(l - I.y, 0), C.width = s), C;
  }
  if (r === "bottom") {
    var T = {
      x: h + f / 2,
      y: l + c + b,
      horizontalAnchor: "middle",
      verticalAnchor: w
    };
    return I && (T.height = Math.max(I.y + I.height - (l + c), 0), T.width = f), T;
  }
  if (r === "left") {
    var _ = {
      x: p - g,
      y: l + c / 2,
      horizontalAnchor: S,
      verticalAnchor: "middle"
    };
    return I && (_.width = Math.max(_.x - I.x, 0), _.height = c), _;
  }
  if (r === "right") {
    var z = {
      x: p + v + g,
      y: l + c / 2,
      horizontalAnchor: P,
      verticalAnchor: "middle"
    };
    return I && (z.width = Math.max(I.x + I.width - z.x, 0), z.height = c), z;
  }
  var $ = I ? {
    width: v,
    height: c
  } : {};
  return r === "insideLeft" ? Ot({
    x: p + g,
    y: l + c / 2,
    horizontalAnchor: P,
    verticalAnchor: "middle"
  }, $) : r === "insideRight" ? Ot({
    x: p + v - g,
    y: l + c / 2,
    horizontalAnchor: S,
    verticalAnchor: "middle"
  }, $) : r === "insideTop" ? Ot({
    x: d + s / 2,
    y: l + b,
    horizontalAnchor: "middle",
    verticalAnchor: w
  }, $) : r === "insideBottom" ? Ot({
    x: h + f / 2,
    y: l + c - b,
    horizontalAnchor: "middle",
    verticalAnchor: x
  }, $) : r === "insideTopLeft" ? Ot({
    x: d + g,
    y: l + b,
    horizontalAnchor: P,
    verticalAnchor: w
  }, $) : r === "insideTopRight" ? Ot({
    x: d + s - g,
    y: l + b,
    horizontalAnchor: S,
    verticalAnchor: w
  }, $) : r === "insideBottomLeft" ? Ot({
    x: h + g,
    y: l + c - b,
    horizontalAnchor: P,
    verticalAnchor: x
  }, $) : r === "insideBottomRight" ? Ot({
    x: h + f - g,
    y: l + c - b,
    horizontalAnchor: S,
    verticalAnchor: x
  }, $) : r && typeof r == "object" && (N(r.x) || Mr(r.x)) && (N(r.y) || Mr(r.y)) ? Ot({
    x: u + yt(r.x, v),
    y: l + yt(r.y, c),
    horizontalAnchor: "end",
    verticalAnchor: "end"
  }, $) : Ot({
    x: m,
    y: l + c / 2,
    horizontalAnchor: "middle",
    verticalAnchor: "middle"
  }, $);
}, HC = ["labelRef"], YC = ["content"];
function Dd(e, t) {
  if (e == null) return {};
  var r, n, i = GC(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function GC(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Nd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function $n(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Nd(Object(r), !0).forEach(function(n) {
      qC(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Nd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function qC(e, t, r) {
  return (t = XC(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function XC(e) {
  var t = ZC(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function ZC(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function $t() {
  return $t = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, $t.apply(null, arguments);
}
var Wy = /* @__PURE__ */ Xe(null), QC = (e) => {
  var t = e.x, r = e.y, n = e.upperWidth, i = e.lowerWidth, a = e.width, o = e.height, u = e.children, l = sr(() => ({
    x: t,
    y: r,
    upperWidth: n,
    lowerWidth: i,
    width: a,
    height: o
  }), [t, r, n, i, a, o]);
  return /* @__PURE__ */ A.createElement(Wy.Provider, {
    value: l
  }, u);
}, Uy = () => {
  var e = ct(Wy), t = Ya();
  return e || (t ? il(t) : void 0);
}, JC = /* @__PURE__ */ Xe(null), eT = () => {
  var e = ct(JC), t = j(Tp);
  return e || t;
}, tT = (e) => {
  var t = e.value, r = e.formatter, n = we(e.children) ? t : e.children;
  return typeof r == "function" ? r(n) : n;
}, lc = (e) => e != null && typeof e == "function", rT = (e, t) => {
  var r = He(t - e), n = Math.min(Math.abs(t - e), 360);
  return r * n;
}, nT = (e, t, r, n, i) => {
  var a = e.offset, o = e.className, u = i.cx, l = i.cy, c = i.innerRadius, s = i.outerRadius, f = i.startAngle, d = i.endAngle, h = i.clockWise, p = (c + s) / 2, v = rT(f, d), m = v >= 0 ? 1 : -1, y, b;
  switch (t) {
    case "insideStart":
      y = f + m * a, b = h;
      break;
    case "insideEnd":
      y = d - m * a, b = !h;
      break;
    case "end":
      y = d + m * a, b = h;
      break;
    default:
      throw new Error("Unsupported position ".concat(t));
  }
  b = v <= 0 ? b : !b;
  var x = Ne(u, l, p, y), w = Ne(u, l, p, y + (b ? 1 : -1) * 359), O = "M".concat(x.x, ",").concat(x.y, `
    A`).concat(p, ",").concat(p, ",0,1,").concat(b ? 0 : 1, `,
    `).concat(w.x, ",").concat(w.y), g = we(e.id) ? zn("recharts-radial-line-") : e.id;
  return /* @__PURE__ */ A.createElement("text", $t({}, n, {
    dominantBaseline: "central",
    className: ce("recharts-radial-bar-label", o)
  }), /* @__PURE__ */ A.createElement("defs", null, /* @__PURE__ */ A.createElement("path", {
    id: g,
    d: O
  })), /* @__PURE__ */ A.createElement("textPath", {
    xlinkHref: "#".concat(g)
  }, r));
}, iT = (e, t, r) => {
  var n = e.cx, i = e.cy, a = e.innerRadius, o = e.outerRadius, u = e.startAngle, l = e.endAngle, c = (u + l) / 2;
  if (r === "outside") {
    var s = Ne(n, i, o + t, c), f = s.x, d = s.y;
    return {
      x: f,
      y: d,
      textAnchor: f >= n ? "start" : "end",
      verticalAnchor: "middle"
    };
  }
  if (r === "center")
    return {
      x: n,
      y: i,
      textAnchor: "middle",
      verticalAnchor: "middle"
    };
  if (r === "centerTop")
    return {
      x: n,
      y: i,
      textAnchor: "middle",
      verticalAnchor: "start"
    };
  if (r === "centerBottom")
    return {
      x: n,
      y: i,
      textAnchor: "middle",
      verticalAnchor: "end"
    };
  var h = (a + o) / 2, p = Ne(n, i, h, c), v = p.x, m = p.y;
  return {
    x: v,
    y: m,
    textAnchor: "middle",
    verticalAnchor: "middle"
  };
}, Bi = (e) => e != null && "cx" in e && N(e.cx), aT = {
  angle: 0,
  offset: 5,
  zIndex: Ue.label,
  position: "middle",
  textBreakAll: !1
};
function oT(e) {
  if (!Bi(e))
    return e;
  var t = e.cx, r = e.cy, n = e.outerRadius, i = n * 2;
  return {
    x: t - n,
    y: r - n,
    width: i,
    upperWidth: i,
    lowerWidth: i,
    height: i
  };
}
function ir(e) {
  var t = st(e, aT), r = t.viewBox, n = t.parentViewBox, i = t.position, a = t.value, o = t.children, u = t.content, l = t.className, c = l === void 0 ? "" : l, s = t.textBreakAll, f = t.labelRef, d = eT(), h = Uy(), p = i === "center" ? h : d ?? h, v, m, y;
  r == null ? v = p : Bi(r) ? v = r : v = il(r);
  var b = oT(v);
  if (!v || we(a) && we(o) && !/* @__PURE__ */ Pt(u) && typeof u != "function")
    return null;
  var x = $n($n({}, t), {}, {
    viewBox: v
  });
  if (/* @__PURE__ */ Pt(u)) {
    x.labelRef;
    var w = Dd(x, HC);
    return /* @__PURE__ */ Ca(u, w);
  }
  if (typeof u == "function") {
    x.content;
    var O = Dd(x, YC);
    if (m = /* @__PURE__ */ Av(u, O), /* @__PURE__ */ Pt(m))
      return m;
  } else
    m = tT(t);
  var g = Wt(t);
  if (Bi(v)) {
    if (i === "insideStart" || i === "insideEnd" || i === "end")
      return nT(t, i, m, g, v);
    y = iT(v, t.offset, t.position);
  } else {
    if (!b)
      return null;
    var S = KC({
      viewBox: b,
      position: i,
      offset: t.offset,
      parentViewBox: Bi(n) ? void 0 : n
    });
    y = $n($n({
      x: S.x,
      y: S.y,
      textAnchor: S.horizontalAnchor,
      verticalAnchor: S.verticalAnchor
    }, S.width !== void 0 ? {
      width: S.width
    } : {}), S.height !== void 0 ? {
      height: S.height
    } : {});
  }
  return /* @__PURE__ */ A.createElement(er, {
    zIndex: t.zIndex
  }, /* @__PURE__ */ A.createElement(uc, $t({
    ref: f,
    className: ce("recharts-label", c)
  }, g, y, {
    /*
     * textAnchor is decided by default based on the `position`
     * but we allow overriding via props for precise control.
     */
    textAnchor: Ly(g.textAnchor) ? g.textAnchor : y.textAnchor,
    breakAll: s
  }), m));
}
ir.displayName = "Label";
var uT = (e, t, r) => {
  if (!e)
    return null;
  var n = {
    viewBox: t,
    labelRef: r
  };
  return e === !0 ? /* @__PURE__ */ A.createElement(ir, $t({
    key: "label-implicit"
  }, n)) : Mt(e) ? /* @__PURE__ */ A.createElement(ir, $t({
    key: "label-implicit",
    value: e
  }, n)) : /* @__PURE__ */ Pt(e) ? e.type === ir ? /* @__PURE__ */ Ca(e, $n({
    key: "label-implicit"
  }, n)) : /* @__PURE__ */ A.createElement(ir, $t({
    key: "label-implicit",
    content: e
  }, n)) : lc(e) ? /* @__PURE__ */ A.createElement(ir, $t({
    key: "label-implicit",
    content: e
  }, n)) : e && typeof e == "object" ? /* @__PURE__ */ A.createElement(ir, $t({}, e, {
    key: "label-implicit"
  }, n)) : null;
};
function lT(e) {
  var t = e.label, r = e.labelRef, n = Uy();
  return uT(t, n, r) || null;
}
var cT = ["valueAccessor"], sT = ["dataKey", "clockWise", "id", "textBreakAll", "zIndex"];
function Ea() {
  return Ea = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Ea.apply(null, arguments);
}
function jd(e, t) {
  if (e == null) return {};
  var r, n, i = fT(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function fT(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var dT = (e) => {
  var t = Array.isArray(e.value) ? e.value[e.value.length - 1] : e.value;
  if (RC(t))
    return t;
}, Vy = /* @__PURE__ */ Xe(void 0), vT = Vy.Provider, Ky = /* @__PURE__ */ Xe(void 0);
Ky.Provider;
function hT() {
  return ct(Vy);
}
function pT() {
  return ct(Ky);
}
function Fi(e) {
  var t = e.valueAccessor, r = t === void 0 ? dT : t, n = jd(e, cT), i = n.dataKey;
  n.clockWise;
  var a = n.id, o = n.textBreakAll, u = n.zIndex, l = jd(n, sT), c = hT(), s = pT(), f = c || s;
  return !f || !f.length ? null : /* @__PURE__ */ A.createElement(er, {
    zIndex: u ?? Ue.label
  }, /* @__PURE__ */ A.createElement(Ct, {
    className: "recharts-label-list"
  }, f.map((d, h) => {
    var p, v = we(i) ? r(d, h) : Se(d.payload, i), m = we(a) ? {} : {
      id: "".concat(a, "-").concat(h)
    };
    return /* @__PURE__ */ A.createElement(ir, Ea({
      key: "label-".concat(h)
    }, Wt(d), l, m, {
      /*
       * Prefer to use the explicit fill from LabelList props.
       * Only in an absence of that, fall back to the fill of the entry.
       * The entry fill can be quite difficult to see especially in Bar, Pie, RadialBar in inside positions.
       * On the other hand it's quite convenient in Scatter, Line, or when the position is outside the Bar, Pie filled shapes.
       */
      fill: (p = n.fill) !== null && p !== void 0 ? p : d.fill,
      parentViewBox: d.parentViewBox,
      value: v,
      textBreakAll: o,
      viewBox: d.viewBox,
      index: h,
      zIndex: 0
    }));
  })));
}
Fi.displayName = "LabelList";
function mT(e) {
  var t = e.label;
  return t ? t === !0 ? /* @__PURE__ */ A.createElement(Fi, {
    key: "labelList-implicit"
  }) : /* @__PURE__ */ A.isValidElement(t) || lc(t) ? /* @__PURE__ */ A.createElement(Fi, {
    key: "labelList-implicit",
    content: t
  }) : typeof t == "object" ? /* @__PURE__ */ A.createElement(Fi, Ea({
    key: "labelList-implicit"
  }, t, {
    type: String(t.type)
  })) : null : null;
}
var yT = {
  radiusAxis: {},
  angleAxis: {}
}, Hy = Be({
  name: "polarAxis",
  initialState: yT,
  reducers: {
    addRadiusAxis(e, t) {
      e.radiusAxis[t.payload.id] = q(t.payload);
    },
    removeRadiusAxis(e, t) {
      delete e.radiusAxis[t.payload.id];
    },
    addAngleAxis(e, t) {
      e.angleAxis[t.payload.id] = q(t.payload);
    },
    removeAngleAxis(e, t) {
      delete e.angleAxis[t.payload.id];
    }
  }
}), ho = Hy.actions;
ho.addRadiusAxis;
ho.removeRadiusAxis;
ho.addAngleAxis;
ho.removeAngleAxis;
var gT = Hy.reducer;
function bT(e) {
  return e && typeof e == "object" && "className" in e && typeof e.className == "string" ? e.className : "";
}
var Tu = { exports: {} }, ee = {};
/**
 * @license React
 * react-is.production.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var $d;
function xT() {
  if ($d) return ee;
  $d = 1;
  var e = Symbol.for("react.transitional.element"), t = Symbol.for("react.portal"), r = Symbol.for("react.fragment"), n = Symbol.for("react.strict_mode"), i = Symbol.for("react.profiler"), a = Symbol.for("react.consumer"), o = Symbol.for("react.context"), u = Symbol.for("react.forward_ref"), l = Symbol.for("react.suspense"), c = Symbol.for("react.suspense_list"), s = Symbol.for("react.memo"), f = Symbol.for("react.lazy"), d = Symbol.for("react.view_transition"), h = Symbol.for("react.client.reference");
  function p(v) {
    if (typeof v == "object" && v !== null) {
      var m = v.$$typeof;
      switch (m) {
        case e:
          switch (v = v.type, v) {
            case r:
            case i:
            case n:
            case l:
            case c:
            case d:
              return v;
            default:
              switch (v = v && v.$$typeof, v) {
                case o:
                case u:
                case f:
                case s:
                  return v;
                case a:
                  return v;
                default:
                  return m;
              }
          }
        case t:
          return m;
      }
    }
  }
  return ee.ContextConsumer = a, ee.ContextProvider = o, ee.Element = e, ee.ForwardRef = u, ee.Fragment = r, ee.Lazy = f, ee.Memo = s, ee.Portal = t, ee.Profiler = i, ee.StrictMode = n, ee.Suspense = l, ee.SuspenseList = c, ee.isContextConsumer = function(v) {
    return p(v) === a;
  }, ee.isContextProvider = function(v) {
    return p(v) === o;
  }, ee.isElement = function(v) {
    return typeof v == "object" && v !== null && v.$$typeof === e;
  }, ee.isForwardRef = function(v) {
    return p(v) === u;
  }, ee.isFragment = function(v) {
    return p(v) === r;
  }, ee.isLazy = function(v) {
    return p(v) === f;
  }, ee.isMemo = function(v) {
    return p(v) === s;
  }, ee.isPortal = function(v) {
    return p(v) === t;
  }, ee.isProfiler = function(v) {
    return p(v) === i;
  }, ee.isStrictMode = function(v) {
    return p(v) === n;
  }, ee.isSuspense = function(v) {
    return p(v) === l;
  }, ee.isSuspenseList = function(v) {
    return p(v) === c;
  }, ee.isValidElementType = function(v) {
    return typeof v == "string" || typeof v == "function" || v === r || v === i || v === n || v === l || v === c || typeof v == "object" && v !== null && (v.$$typeof === f || v.$$typeof === s || v.$$typeof === o || v.$$typeof === a || v.$$typeof === u || v.$$typeof === h || v.getModuleId !== void 0);
  }, ee.typeOf = p, ee;
}
var te = {};
/**
 * @license React
 * react-is.development.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var Rd;
function wT() {
  return Rd || (Rd = 1, process.env.NODE_ENV !== "production" && function() {
    function e(v) {
      if (typeof v == "object" && v !== null) {
        var m = v.$$typeof;
        switch (m) {
          case t:
            switch (v = v.type, v) {
              case n:
              case a:
              case i:
              case c:
              case s:
              case h:
                return v;
              default:
                switch (v = v && v.$$typeof, v) {
                  case u:
                  case l:
                  case d:
                  case f:
                    return v;
                  case o:
                    return v;
                  default:
                    return m;
                }
            }
          case r:
            return m;
        }
      }
    }
    var t = Symbol.for("react.transitional.element"), r = Symbol.for("react.portal"), n = Symbol.for("react.fragment"), i = Symbol.for("react.strict_mode"), a = Symbol.for("react.profiler"), o = Symbol.for("react.consumer"), u = Symbol.for("react.context"), l = Symbol.for("react.forward_ref"), c = Symbol.for("react.suspense"), s = Symbol.for("react.suspense_list"), f = Symbol.for("react.memo"), d = Symbol.for("react.lazy"), h = Symbol.for("react.view_transition"), p = Symbol.for("react.client.reference");
    te.ContextConsumer = o, te.ContextProvider = u, te.Element = t, te.ForwardRef = l, te.Fragment = n, te.Lazy = d, te.Memo = f, te.Portal = r, te.Profiler = a, te.StrictMode = i, te.Suspense = c, te.SuspenseList = s, te.isContextConsumer = function(v) {
      return e(v) === o;
    }, te.isContextProvider = function(v) {
      return e(v) === u;
    }, te.isElement = function(v) {
      return typeof v == "object" && v !== null && v.$$typeof === t;
    }, te.isForwardRef = function(v) {
      return e(v) === l;
    }, te.isFragment = function(v) {
      return e(v) === n;
    }, te.isLazy = function(v) {
      return e(v) === d;
    }, te.isMemo = function(v) {
      return e(v) === f;
    }, te.isPortal = function(v) {
      return e(v) === r;
    }, te.isProfiler = function(v) {
      return e(v) === a;
    }, te.isStrictMode = function(v) {
      return e(v) === i;
    }, te.isSuspense = function(v) {
      return e(v) === c;
    }, te.isSuspenseList = function(v) {
      return e(v) === s;
    }, te.isValidElementType = function(v) {
      return typeof v == "string" || typeof v == "function" || v === n || v === a || v === i || v === c || v === s || typeof v == "object" && v !== null && (v.$$typeof === d || v.$$typeof === f || v.$$typeof === u || v.$$typeof === o || v.$$typeof === l || v.$$typeof === p || v.getModuleId !== void 0);
    }, te.typeOf = e;
  }()), te;
}
process.env.NODE_ENV === "production" ? Tu.exports = xT() : Tu.exports = wT();
var AT = Tu.exports, Ld = (e) => typeof e == "string" ? e : e ? e.displayName || e.name || "Component" : "", zd = null, Wo = null, Yy = (e) => {
  if (e === zd && Array.isArray(Wo))
    return Wo;
  var t = [];
  return Lg.forEach(e, (r) => {
    we(r) || (AT.isFragment(r) ? t = t.concat(Yy(r.props.children)) : t.push(r));
  }), Wo = t, zd = e, t;
};
function OT(e, t) {
  var r = [], n = [];
  return Array.isArray(t) ? n = t.map((i) => Ld(i)) : n = [Ld(t)], Yy(e).forEach((i) => {
    var a = Ut(i, "type.displayName") || Ut(i, "type.name");
    a && n.indexOf(a) !== -1 && r.push(i);
  }), r;
}
function Bd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Fd(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Bd(Object(r), !0).forEach(function(n) {
      ET(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Bd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function ET(e, t, r) {
  return (t = ST(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function ST(e) {
  var t = _T(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function _T(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function Gy(e, t) {
  return Fd(Fd({}, t), e);
}
function PT(e) {
  return /* @__PURE__ */ Pt(e) ? e.props : e;
}
function IT(e, t) {
  return /* @__PURE__ */ Ca(e, Gy(PT(e), t));
}
function kT(e) {
  if ("index" in e) {
    var t = e.index;
    return typeof t == "number" || typeof t == "string" ? t : void 0;
  }
}
function CT(e) {
  return "isActive" in e && e.isActive === !0;
}
function TT(e) {
  var t = e.option, r = e.DefaultShape, n = e.shapeProps, i = e.activeClassName, a = i === void 0 ? "recharts-active-shape" : i, o = e.inActiveClassName, u = o === void 0 ? "recharts-shape" : o, l = kT(n), c;
  return /* @__PURE__ */ Pt(t) ? c = IT(t, n) : t === r ? c = /* @__PURE__ */ A.createElement(r, n) : typeof t == "function" ? c = t(n, l) : typeof t == "object" ? c = /* @__PURE__ */ A.createElement(r, Gy(t, n)) : c = /* @__PURE__ */ A.createElement(r, n), CT(n) ? /* @__PURE__ */ A.createElement(Ct, {
    className: a
  }, c) : /* @__PURE__ */ A.createElement(Ct, {
    className: u
  }, c);
}
var qy = (e, t, r) => {
  var n = se();
  return (i, a) => (o) => {
    e == null || e(i, a, o), n(ay({
      activeIndex: String(a),
      activeDataKey: t,
      activeCoordinate: i.tooltipPosition,
      activeGraphicalItemId: r
    }));
  };
}, Xy = (e) => {
  var t = se();
  return (r, n) => (i) => {
    e == null || e(r, n, i), t(UP());
  };
}, Zy = (e, t, r) => {
  var n = se();
  return (i, a) => (o) => {
    e == null || e(i, a, o), n(VP({
      activeIndex: String(a),
      activeDataKey: t,
      activeCoordinate: i.tooltipPosition,
      activeGraphicalItemId: r
    }));
  };
};
function MT(e) {
  var t = e.tooltipEntrySettings, r = se(), n = Ze(), i = Q(null);
  return qe(() => {
    n || (i.current === null ? r(zP(t)) : i.current !== t && r(BP({
      prev: i.current,
      next: t
    })), i.current = t);
  }, [t, r, n]), qe(() => () => {
    i.current && (r(FP(i.current)), i.current = null);
  }, [r]), null;
}
function DT(e) {
  var t = e.legendPayload, r = se(), n = Ze(), i = Q(null);
  return qe(() => {
    n || (i.current === null ? r(vA(t)) : i.current !== t && r(hA({
      prev: i.current,
      next: t
    })), i.current = t);
  }, [r, n, t]), qe(() => () => {
    i.current && (r(pA(i.current)), i.current = null);
  }, [r]), null;
}
function NT(e, t) {
  return LT(e) || RT(e, t) || $T(e, t) || jT();
}
function jT() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function $T(e, t) {
  if (e) {
    if (typeof e == "string") return Wd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Wd(e, t) : void 0;
  }
}
function Wd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function RT(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function LT(e) {
  if (Array.isArray(e)) return e;
}
var Qy = "index", Jy = "append";
function cc(e, t) {
  var r = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : [], n = [];
  for (var i of r)
    n.push({
      status: "removed",
      prev: i
    });
  for (var a = 0; a < t.length; a++) {
    var o = e[a], u = t[a];
    o != null ? n.push({
      status: "matched",
      prev: o,
      next: u
    }) : n.push({
      status: "added",
      next: u
    });
  }
  return n;
}
function zT(e, t) {
  var r = e.length / t.length, n = t.map((i, a) => e[Math.floor(a * r)]);
  return cc(n, t);
}
function BT(e, t) {
  var r = t.map((n, i) => e[i]);
  return cc(r, t);
}
function FT(e, t) {
  for (var r = /* @__PURE__ */ new Map(), n = 0; n < e.length; n++) {
    var i = e[n];
    if (i != null) {
      var a = t(i, n);
      a != null && !r.has(a) && r.set(a, i);
    }
  }
  return r;
}
function WT(e, t, r) {
  var n = FT(e, r), i = /* @__PURE__ */ new Set(), a = t.map((f, d) => {
    var h = r(f, d);
    if (h != null) {
      var p = n.get(h);
      if (p !== void 0)
        return i.add(h), p;
    }
  }), o = [];
  for (var u of n) {
    var l = NT(u, 2), c = l[0], s = l[1];
    i.has(c) || o.push(s);
  }
  return cc(a, t, o);
}
function UT(e, t, r) {
  return t == null ? null : e == null ? t.map((n) => ({
    status: "added",
    next: n
  })) : r === Qy ? zT(e, t) : r === Jy ? BT(e, t) : WT(e, t, r);
}
function VT(e, t) {
  var r = Q(e), n = Q(t.current), i = Q(!0);
  r.current !== e && (r.current = e, n.current = t.current, i.current = !1);
  var a = ie(function(o, u) {
    var l = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : !0;
    if (u === 0) {
      i.current = !0;
      return;
    }
    u === 1 && (n.current = o), u > 0 && i.current && l && (t.current = o);
  }, [t]);
  return {
    startValue: n.current,
    syncStepValue: a
  };
}
function KT(e, t) {
  return qT(e) || GT(e, t) || YT(e, t) || HT();
}
function HT() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function YT(e, t) {
  if (e) {
    if (typeof e == "string") return Ud(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Ud(e, t) : void 0;
  }
}
function Ud(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function GT(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function qT(e) {
  if (Array.isArray(e)) return e;
}
function XT(e, t) {
  var r = _e(!1), n = KT(r, 2), i = n[0], a = n[1], o = ie(() => {
    typeof e == "function" && e(), a(!0);
  }, [e]), u = ie(() => {
    typeof t == "function" && t(), a(!1);
  }, [t]);
  return {
    isAnimating: i,
    handleAnimationStart: o,
    handleAnimationEnd: u
  };
}
function ZT(e) {
  var t, r = e.animationInput, n = e.animationIdPrefix, i = e.items, a = e.previousItemsRef, o = e.isAnimationActive, u = e.animationBegin, l = e.animationDuration, c = e.animationEasing, s = e.onAnimationStart, f = e.onAnimationEnd, d = e.animationInterpolateFn, h = e.animationMatchBy, p = e.shouldUpdatePreviousRef, v = e.children, m = e.layout, y = up(r, n), b = VT(y, a), x = (t = b.startValue) !== null && t !== void 0 ? t : null, w = UT(x, i, h ?? Qy);
  return /* @__PURE__ */ A.createElement(op, {
    animationId: y,
    begin: u,
    duration: l,
    isActive: o,
    easing: c,
    onAnimationEnd: f,
    onAnimationStart: s,
    key: y
  }, (O) => {
    var g = x == null, S = i == null ? i : d(w, O, m), P = p ? p(O) : O > 0;
    return b.syncStepValue(S, O, P), S == null ? null : v(S, O, g);
  });
}
var Uo;
function QT(e, t) {
  return rM(e) || tM(e, t) || eM(e, t) || JT();
}
function JT() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function eM(e, t) {
  if (e) {
    if (typeof e == "string") return Vd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Vd(e, t) : void 0;
  }
}
function Vd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function tM(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function rM(e) {
  if (Array.isArray(e)) return e;
}
var nM = () => {
  var e = A.useState(() => zn("uid-")), t = QT(e, 1), r = t[0];
  return r;
}, iM = (Uo = A.useId) !== null && Uo !== void 0 ? Uo : nM;
function aM(e, t) {
  var r = iM();
  return t || (e ? "".concat(e, "-").concat(r) : r);
}
var oM = /* @__PURE__ */ Xe(void 0), uM = (e) => {
  var t = e.id, r = e.type, n = e.children, i = aM("recharts-".concat(r), t);
  return /* @__PURE__ */ A.createElement(oM.Provider, {
    value: i
  }, n(i));
}, lM = {
  cartesianItems: [],
  polarItems: []
}, eg = Be({
  name: "graphicalItems",
  initialState: lM,
  reducers: {
    addCartesianGraphicalItem: {
      reducer(e, t) {
        e.cartesianItems.push(q(t.payload));
      },
      prepare: ae()
    },
    replaceCartesianGraphicalItem: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next, a = ot(e).cartesianItems.indexOf(q(n));
        a > -1 && (e.cartesianItems[a] = q(i));
      },
      prepare: ae()
    },
    removeCartesianGraphicalItem: {
      reducer(e, t) {
        var r = ot(e).cartesianItems.indexOf(q(t.payload));
        r > -1 && e.cartesianItems.splice(r, 1);
      },
      prepare: ae()
    },
    addPolarGraphicalItem: {
      reducer(e, t) {
        e.polarItems.push(q(t.payload));
      },
      prepare: ae()
    },
    removePolarGraphicalItem: {
      reducer(e, t) {
        var r = ot(e).polarItems.indexOf(q(t.payload));
        r > -1 && e.polarItems.splice(r, 1);
      },
      prepare: ae()
    },
    replacePolarGraphicalItem: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next, a = ot(e).polarItems.indexOf(q(n));
        a > -1 && (e.polarItems[a] = q(i));
      },
      prepare: ae()
    }
  }
}), wn = eg.actions, cM = wn.addCartesianGraphicalItem, sM = wn.replaceCartesianGraphicalItem, fM = wn.removeCartesianGraphicalItem;
wn.addPolarGraphicalItem;
wn.removePolarGraphicalItem;
wn.replacePolarGraphicalItem;
var dM = eg.reducer, vM = (e) => {
  var t = se(), r = Q(null);
  return qe(() => {
    r.current === null ? t(cM(e)) : r.current !== e && t(sM({
      prev: r.current,
      next: e
    })), r.current = e;
  }, [t, e]), qe(() => () => {
    r.current && (t(fM(r.current)), r.current = null);
  }, [t]), null;
}, hM = /* @__PURE__ */ Bu(vM);
function Kd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Hd(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Kd(Object(r), !0).forEach(function(n) {
      pM(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Kd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function pM(e, t, r) {
  return (t = mM(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function mM(e) {
  var t = yM(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function yM(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var tg = 0, gM = {
  xAxis: {},
  yAxis: {},
  zAxis: {}
}, rg = Be({
  name: "cartesianAxis",
  initialState: gM,
  reducers: {
    addXAxis: {
      reducer(e, t) {
        e.xAxis[t.payload.id] = q(t.payload);
      },
      prepare: ae()
    },
    replaceXAxis: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next;
        e.xAxis[n.id] !== void 0 && (n.id !== i.id && delete e.xAxis[n.id], e.xAxis[i.id] = q(i));
      },
      prepare: ae()
    },
    removeXAxis: {
      reducer(e, t) {
        delete e.xAxis[t.payload.id];
      },
      prepare: ae()
    },
    addYAxis: {
      reducer(e, t) {
        e.yAxis[t.payload.id] = q(t.payload);
      },
      prepare: ae()
    },
    replaceYAxis: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next;
        e.yAxis[n.id] !== void 0 && (n.id !== i.id && delete e.yAxis[n.id], e.yAxis[i.id] = q(i));
      },
      prepare: ae()
    },
    removeYAxis: {
      reducer(e, t) {
        delete e.yAxis[t.payload.id];
      },
      prepare: ae()
    },
    addZAxis: {
      reducer(e, t) {
        e.zAxis[t.payload.id] = q(t.payload);
      },
      prepare: ae()
    },
    replaceZAxis: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next;
        e.zAxis[n.id] !== void 0 && (n.id !== i.id && delete e.zAxis[n.id], e.zAxis[i.id] = q(i));
      },
      prepare: ae()
    },
    removeZAxis: {
      reducer(e, t) {
        delete e.zAxis[t.payload.id];
      },
      prepare: ae()
    },
    updateYAxisWidth(e, t) {
      var r = t.payload, n = r.id, i = r.width, a = e.yAxis[n];
      if (a) {
        var o, u = a.widthHistory || [];
        if (u.length === 3 && u[0] === u[2] && i === u[1] && i !== a.width && Math.abs(i - ((o = u[0]) !== null && o !== void 0 ? o : 0)) <= 1)
          return;
        var l = [...u, i].slice(-3);
        e.yAxis[n] = Hd(Hd({}, a), {}, {
          width: i,
          widthHistory: l
        });
      }
    }
  }
}), jt = rg.actions, bM = jt.addXAxis, xM = jt.replaceXAxis, wM = jt.removeXAxis, AM = jt.addYAxis, OM = jt.replaceYAxis, EM = jt.removeYAxis;
jt.addZAxis;
jt.replaceZAxis;
jt.removeZAxis;
var SM = jt.updateYAxisWidth, _M = rg.reducer, PM = E([Pe], (e) => ({
  top: e.top,
  bottom: e.bottom,
  left: e.left,
  right: e.right
})), IM = E([PM, Gt, qt], (e, t, r) => {
  if (!(!e || t == null || r == null))
    return {
      x: e.left,
      y: e.top,
      width: Math.max(0, t - e.left - e.right),
      height: Math.max(0, r - e.top - e.bottom)
    };
}), ng = () => j(IM);
function kM(e, t) {
  return DM(e) || MM(e, t) || TM(e, t) || CM();
}
function CM() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function TM(e, t) {
  if (e) {
    if (typeof e == "string") return Yd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Yd(e, t) : void 0;
  }
}
function Yd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function MM(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function DM(e) {
  if (Array.isArray(e)) return e;
}
var Gd = (e, t, r) => {
  var n = r ?? e;
  if (!we(n))
    return yt(n, t, 0);
}, NM = (e, t, r) => {
  var n = {}, i = e.filter(to), a = e.filter((c) => c.stackId == null), o = i.reduce((c, s) => {
    var f = c[s.stackId];
    return f == null && (f = []), f.push(s), c[s.stackId] = f, c;
  }, n), u = Object.entries(o).map((c) => {
    var s, f = kM(c, 2), d = f[0], h = f[1], p = h.map((m) => m.dataKey), v = Gd(t, r, (s = h[0]) === null || s === void 0 ? void 0 : s.barSize);
    return {
      stackId: d,
      dataKeys: p,
      barSize: v
    };
  }), l = a.map((c) => {
    var s = [c.dataKey].filter((d) => d != null), f = Gd(t, r, c.barSize);
    return {
      stackId: void 0,
      dataKeys: s,
      barSize: f
    };
  });
  return [...u, ...l];
};
function qd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Di(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? qd(Object(r), !0).forEach(function(n) {
      jM(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : qd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function jM(e, t, r) {
  return (t = $M(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function $M(e) {
  var t = RM(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function RM(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function LM(e, t, r, n, i) {
  var a, o = n.length;
  if (!(o < 1)) {
    var u = yt(e, r, 0, !0), l, c = [];
    if (Y((a = n[0]) === null || a === void 0 ? void 0 : a.barSize)) {
      var s = !1, f = r / o, d = n.reduce((b, x) => b + (x.barSize || 0), 0);
      d += (o - 1) * u, d >= r && (d -= (o - 1) * u, u = 0), d >= r && f > 0 && (s = !0, f *= 0.9, d = o * f);
      var h = Math.round((r - d) / 2), p = {
        offset: h - u,
        size: 0
      };
      l = n.reduce((b, x) => {
        var w, O = {
          stackId: x.stackId,
          dataKeys: x.dataKeys,
          position: {
            offset: p.offset + p.size + u,
            size: s ? f : (w = x.barSize) !== null && w !== void 0 ? w : 0
          }
        }, g = [...b, O];
        return p = O.position, g;
      }, c);
    } else {
      var v = yt(t, r, 0, !0);
      r - 2 * v - (o - 1) * u <= 0 && (u = 0);
      var m = (r - 2 * v - (o - 1) * u) / o;
      m > 1 && (m = Math.round(m));
      var y = Y(i) ? Math.min(m, i) : m;
      l = n.reduce((b, x, w) => [...b, {
        stackId: x.stackId,
        dataKeys: x.dataKeys,
        position: {
          offset: v + (m + u) * w + (m - y) / 2,
          size: y
        }
      }], c);
    }
    return l;
  }
}
var zM = (e, t, r, n, i, a, o) => {
  var u = we(o) ? t : o, l = LM(r, n, i !== a ? i : a, e, u);
  return i !== a && l != null && (l = l.map((c) => Di(Di({}, c), {}, {
    position: Di(Di({}, c.position), {}, {
      offset: c.position.offset - i / 2
    })
  }))), l;
}, BM = (e, t) => {
  var r = ml(t);
  if (!(!e || r == null || t == null)) {
    var n = t.stackId;
    if (n != null) {
      var i = e[n];
      if (i) {
        var a = i.stackedData;
        if (a)
          return a.find((o) => o.key === r);
      }
    }
  }
}, FM = (e, t) => {
  if (!(e == null || t == null)) {
    var r = e.find((n) => n.stackId === t.stackId && t.dataKey != null && n.dataKeys.includes(t.dataKey));
    if (r != null)
      return r.position;
  }
};
function WM(e, t) {
  return e && typeof e == "object" && "zIndex" in e && typeof e.zIndex == "number" && Y(e.zIndex) ? e.zIndex : t;
}
var UM = (e) => {
  var t = e.chartData, r = se(), n = Ze();
  return ve(() => n ? () => {
  } : (r(vd(t)), () => {
    r(vd(void 0));
  }), [t, r, n]), null;
}, Xd = {
  x: 0,
  y: 0,
  width: 0,
  height: 0,
  padding: {
    top: 0,
    right: 0,
    bottom: 0,
    left: 0
  }
}, ig = Be({
  name: "brush",
  initialState: Xd,
  reducers: {
    setBrushSettings(e, t) {
      return t.payload == null ? Xd : t.payload;
    }
  }
});
ig.actions.setBrushSettings;
var VM = ig.reducer;
function KM(e) {
  return (e % 180 + 180) % 180;
}
var HM = function(t) {
  var r = t.width, n = t.height, i = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : 0, a = KM(i), o = a * Math.PI / 180, u = Math.atan(n / r), l = o > u && o < Math.PI - u ? n / Math.sin(o) : r / Math.cos(o);
  return Math.abs(l);
}, YM = {
  dots: [],
  areas: [],
  lines: []
}, ag = Be({
  name: "referenceElements",
  initialState: YM,
  reducers: {
    addDot: (e, t) => {
      e.dots.push(t.payload);
    },
    removeDot: (e, t) => {
      var r = ot(e).dots.findIndex((n) => n === t.payload);
      r !== -1 && e.dots.splice(r, 1);
    },
    addArea: (e, t) => {
      e.areas.push(t.payload);
    },
    removeArea: (e, t) => {
      var r = ot(e).areas.findIndex((n) => n === t.payload);
      r !== -1 && e.areas.splice(r, 1);
    },
    addLine: (e, t) => {
      e.lines.push(q(t.payload));
    },
    removeLine: (e, t) => {
      var r = ot(e).lines.findIndex((n) => n === t.payload);
      r !== -1 && e.lines.splice(r, 1);
    }
  }
}), An = ag.actions;
An.addDot;
An.removeDot;
An.addArea;
An.removeArea;
An.addLine;
An.removeLine;
var GM = ag.reducer;
function qM(e, t) {
  return JM(e) || QM(e, t) || ZM(e, t) || XM();
}
function XM() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function ZM(e, t) {
  if (e) {
    if (typeof e == "string") return Zd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Zd(e, t) : void 0;
  }
}
function Zd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function QM(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function JM(e) {
  if (Array.isArray(e)) return e;
}
var eD = /* @__PURE__ */ Xe(void 0), tD = (e) => {
  var t = e.children, r = _e("".concat(zn("recharts"), "-clip")), n = qM(r, 1), i = n[0], a = ng();
  if (a == null)
    return null;
  var o = a.x, u = a.y, l = a.width, c = a.height;
  return /* @__PURE__ */ A.createElement(eD.Provider, {
    value: i
  }, /* @__PURE__ */ A.createElement("defs", null, /* @__PURE__ */ A.createElement("clipPath", {
    id: i
  }, /* @__PURE__ */ A.createElement("rect", {
    x: o,
    y: u,
    height: c,
    width: l
  }))), t);
};
function og(e, t) {
  if (t < 1)
    return [];
  if (t === 1)
    return e;
  for (var r = [], n = 0; n < e.length; n += t) {
    var i = e[n];
    i !== void 0 && r.push(i);
  }
  return r;
}
function rD(e, t, r) {
  var n = {
    width: e.width + t.width,
    height: e.height + t.height
  };
  return HM(n, r);
}
function nD(e, t, r) {
  var n = r === "width", i = e.x, a = e.y, o = e.width, u = e.height;
  return t === 1 ? {
    start: n ? i : a,
    end: n ? i + o : a + u
  } : {
    start: n ? i + o : a + u,
    end: n ? i : a
  };
}
function Xn(e, t, r, n, i) {
  if (e * t < e * n || e * t > e * i)
    return !1;
  var a = r();
  return e * (t - e * a / 2 - n) >= 0 && e * (t + e * a / 2 - i) <= 0;
}
function iD(e, t) {
  return og(e, t + 1);
}
function aD(e, t, r, n, i) {
  for (var a = (n || []).slice(), o = t.start, u = t.end, l = 0, c = 1, s = o, f = function() {
    var p = n == null ? void 0 : n[l];
    if (p === void 0)
      return {
        v: og(n, c)
      };
    var v = l, m, y = () => (m === void 0 && (m = r(p, v)), m), b = p.coordinate, x = l === 0 || Xn(e, b, y, s, u);
    x || (l = 0, s = o, c += 1), x && (s = b + e * (y() / 2 + i), l += c);
  }, d; c <= a.length; )
    if (d = f(), d) return d.v;
  return [];
}
function oD(e, t, r, n, i) {
  var a = (n || []).slice(), o = a.length;
  if (o === 0)
    return [];
  for (var u = t.start, l = t.end, c = 1; c <= o; c++) {
    for (var s = (o - 1) % c, f = u, d = !0, h = function() {
      var w = n[v];
      if (w == null)
        return 0;
      var O = v, g, S = () => (g === void 0 && (g = r(w, O)), g), P = w.coordinate, I = v === s || Xn(e, P, S, f, l);
      if (!I)
        return d = !1, 1;
      I && (f = P + e * (S() / 2 + i));
    }, p, v = s; v < o && (p = h(), !(p !== 0 && p === 1)); v += c)
      ;
    if (d) {
      for (var m = [], y = s; y < o; y += c) {
        var b = n[y];
        b != null && m.push(b);
      }
      return m;
    }
  }
  return [];
}
function Qd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Re(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Qd(Object(r), !0).forEach(function(n) {
      uD(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Qd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function uD(e, t, r) {
  return (t = lD(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function lD(e) {
  var t = cD(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function cD(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function sD(e, t, r, n, i) {
  for (var a = (n || []).slice(), o = a.length, u = t.start, l = t.end, c = function(d) {
    var h = a[d];
    if (h == null)
      return 1;
    var p = h, v, m = () => (v === void 0 && (v = r(h, d)), v);
    if (d === o - 1) {
      var y = e * (p.coordinate + e * m() / 2 - l);
      a[d] = p = Re(Re({}, p), {}, {
        tickCoord: y > 0 ? p.coordinate - y * e : p.coordinate
      });
    } else
      a[d] = p = Re(Re({}, p), {}, {
        tickCoord: p.coordinate
      });
    if (p.tickCoord != null) {
      var b = Xn(e, p.tickCoord, m, u, l);
      b && (l = p.tickCoord - e * (m() / 2 + i), a[d] = Re(Re({}, p), {}, {
        isShow: !0
      }));
    }
  }, s = o - 1; s >= 0; s--)
    c(s);
  return a;
}
function fD(e, t, r, n, i, a) {
  var o = (n || []).slice(), u = o.length, l = t.start, c = t.end;
  if (a) {
    var s = n[u - 1];
    if (s != null) {
      var f = r(s, u - 1), d = e * (s.coordinate + e * f / 2 - c);
      if (o[u - 1] = s = Re(Re({}, s), {}, {
        tickCoord: d > 0 ? s.coordinate - d * e : s.coordinate
      }), s.tickCoord != null) {
        var h = Xn(e, s.tickCoord, () => f, l, c);
        h && (c = s.tickCoord - e * (f / 2 + i), o[u - 1] = Re(Re({}, s), {}, {
          isShow: !0
        }));
      }
    }
  }
  for (var p = a ? u - 1 : u, v = function(b) {
    var x = o[b];
    if (x == null)
      return 1;
    var w = x, O, g = () => (O === void 0 && (O = r(x, b)), O);
    if (b === 0) {
      var S = e * (w.coordinate - e * g() / 2 - l);
      o[b] = w = Re(Re({}, w), {}, {
        tickCoord: S < 0 ? w.coordinate - S * e : w.coordinate
      });
    } else
      o[b] = w = Re(Re({}, w), {}, {
        tickCoord: w.coordinate
      });
    if (w.tickCoord != null) {
      var P = Xn(e, w.tickCoord, g, l, c);
      P && (l = w.tickCoord + e * (g() / 2 + i), o[b] = Re(Re({}, w), {}, {
        isShow: !0
      }));
    }
  }, m = 0; m < p; m++)
    v(m);
  return o;
}
function sc(e, t, r) {
  var n = e.tick, i = e.ticks, a = e.viewBox, o = e.minTickGap, u = e.orientation, l = e.interval, c = e.tickFormatter, s = e.unit, f = e.angle;
  if (!i || !i.length || !n)
    return [];
  if (N(l) || ii.isSsr) {
    var d;
    return (d = iD(i, N(l) ? l : 0)) !== null && d !== void 0 ? d : [];
  }
  var h = [], p = u === "top" || u === "bottom" ? "width" : "height", v = s && p === "width" ? Ln(s, {
    fontSize: t,
    letterSpacing: r
  }) : {
    width: 0,
    height: 0
  }, m = (O, g) => {
    var S = typeof c == "function" ? c(O.value, g) : O.value;
    return p === "width" ? rD(Ln(S, {
      fontSize: t,
      letterSpacing: r
    }), v, f) : Ln(S, {
      fontSize: t,
      letterSpacing: r
    })[p];
  }, y = i[0], b = i[1], x = i.length >= 2 && y != null && b != null ? He(b.coordinate - y.coordinate) : 1, w = nD(a, x, p);
  return l === "equidistantPreserveStart" ? aD(x, w, m, i, o) : l === "equidistantPreserveEnd" ? oD(x, w, m, i, o) : (l === "preserveStart" || l === "preserveStartEnd" ? h = fD(x, w, m, i, o, l === "preserveStartEnd") : h = sD(x, w, m, i, o), h.filter((O) => O.isShow));
}
var dD = (e) => {
  var t = e.ticks, r = e.label, n = e.labelGapWithTick, i = n, a = e.tickSize, o = a === void 0 ? 0 : a, u = e.tickMargin, l = u === void 0 ? 0 : u, c = 0;
  if (t) {
    Array.from(t).forEach((h) => {
      if (h) {
        var p = h.getBoundingClientRect();
        p.width > c && (c = p.width);
      }
    });
    var s = r ? r.getBoundingClientRect().width : 0, f = o + l, d = c + f + s + (r ? i : 0);
    return Math.round(d);
  }
  return 0;
}, vD = {
  xAxis: {},
  yAxis: {}
}, ug = Be({
  name: "renderedTicks",
  initialState: vD,
  reducers: {
    setRenderedTicks: (e, t) => {
      var r = t.payload, n = r.axisType, i = r.axisId, a = r.ticks;
      e[n][i] = q(a);
    },
    removeRenderedTicks: (e, t) => {
      var r = t.payload, n = r.axisType, i = r.axisId;
      delete e[n][i];
    }
  }
}), lg = ug.actions, hD = lg.setRenderedTicks, pD = lg.removeRenderedTicks, mD = ug.reducer, yD = ["axisLine", "width", "height", "className", "hide", "ticks", "axisType", "axisId"];
function Jd(e, t) {
  return wD(e) || xD(e, t) || bD(e, t) || gD();
}
function gD() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function bD(e, t) {
  if (e) {
    if (typeof e == "string") return ev(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? ev(e, t) : void 0;
  }
}
function ev(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function xD(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function wD(e) {
  if (Array.isArray(e)) return e;
}
function AD(e, t) {
  if (e == null) return {};
  var r, n, i = OD(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function OD(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function $r() {
  return $r = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, $r.apply(null, arguments);
}
function tv(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function de(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? tv(Object(r), !0).forEach(function(n) {
      ED(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : tv(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function ED(e, t, r) {
  return (t = SD(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function SD(e) {
  var t = _D(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function _D(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var Bt = {
  x: 0,
  y: 0,
  width: 0,
  height: 0,
  viewBox: {
    x: 0,
    y: 0,
    width: 0,
    height: 0
  },
  // The orientation of axis
  orientation: "bottom",
  // The ticks
  ticks: [],
  stroke: "#666",
  tickLine: !0,
  axisLine: !0,
  tick: !0,
  mirror: !1,
  minTickGap: 5,
  // The width or height of tick
  tickSize: 6,
  tickMargin: 2,
  interval: "preserveEnd",
  zIndex: Ue.axis
};
function PD(e) {
  var t = e.x, r = e.y, n = e.width, i = e.height, a = e.orientation, o = e.mirror, u = e.axisLine, l = e.otherSvgProps;
  if (!u)
    return null;
  var c = de(de(de({}, l), Ft(u)), {}, {
    fill: "none"
  });
  if (a === "top" || a === "bottom") {
    var s = +(a === "top" && !o || a === "bottom" && o);
    c = de(de({}, c), {}, {
      x1: t,
      y1: r + s * i,
      x2: t + n,
      y2: r + s * i
    });
  } else {
    var f = +(a === "left" && !o || a === "right" && o);
    c = de(de({}, c), {}, {
      x1: t + f * n,
      y1: r,
      x2: t + f * n,
      y2: r + i
    });
  }
  return /* @__PURE__ */ A.createElement("line", $r({}, c, {
    className: ce("recharts-cartesian-axis-line", Ut(u, "className"))
  }));
}
function ID(e, t, r, n, i, a, o, u, l) {
  var c, s, f, d, h, p, v = u ? -1 : 1, m = e.tickSize || o, y = N(e.tickCoord) ? e.tickCoord : e.coordinate;
  switch (a) {
    case "top":
      c = s = e.coordinate, d = r + +!u * i, f = d - v * m, p = f - v * l, h = y;
      break;
    case "left":
      f = d = e.coordinate, s = t + +!u * n, c = s - v * m, h = c - v * l, p = y;
      break;
    case "right":
      f = d = e.coordinate, s = t + +u * n, c = s + v * m, h = c + v * l, p = y;
      break;
    default:
      c = s = e.coordinate, d = r + +u * i, f = d + v * m, p = f + v * l, h = y;
      break;
  }
  return {
    line: {
      x1: c,
      y1: f,
      x2: s,
      y2: d
    },
    tick: {
      x: h,
      y: p
    }
  };
}
function kD(e, t) {
  switch (e) {
    case "left":
      return t ? "start" : "end";
    case "right":
      return t ? "end" : "start";
    default:
      return "middle";
  }
}
function CD(e, t) {
  switch (e) {
    case "left":
    case "right":
      return "middle";
    case "top":
      return t ? "start" : "end";
    default:
      return t ? "end" : "start";
  }
}
function TD(e) {
  var t = e.option, r = e.tickProps, n = e.value, i, a = ce(r.className, "recharts-cartesian-axis-tick-value");
  if (/* @__PURE__ */ A.isValidElement(t))
    i = /* @__PURE__ */ A.cloneElement(t, de(de({}, r), {}, {
      className: a
    }));
  else if (typeof t == "function")
    i = t(de(de({}, r), {}, {
      className: a
    }));
  else {
    var o = "recharts-cartesian-axis-tick-value";
    typeof t != "boolean" && (o = ce(o, bT(t))), i = /* @__PURE__ */ A.createElement(uc, $r({}, r, {
      className: o
    }), n);
  }
  return i;
}
function MD(e) {
  var t = e.ticks, r = e.axisType, n = e.axisId, i = se();
  return ve(() => {
    if (n == null || r == null)
      return fn;
    var a = t.map((o) => ({
      value: o.value,
      coordinate: o.coordinate,
      offset: o.offset,
      index: o.index
    }));
    return i(hD({
      ticks: a,
      axisId: n,
      axisType: r
    })), () => {
      i(pD({
        axisId: n,
        axisType: r
      }));
    };
  }, [i, t, n, r]), null;
}
var DD = /* @__PURE__ */ ze((e, t) => {
  var r = e.ticks, n = r === void 0 ? [] : r, i = e.tick, a = e.tickLine, o = e.stroke, u = e.tickFormatter, l = e.unit, c = e.padding, s = e.tickTextProps, f = e.orientation, d = e.mirror, h = e.x, p = e.y, v = e.width, m = e.height, y = e.tickSize, b = e.tickMargin, x = e.fontSize, w = e.letterSpacing, O = e.getTicksConfig, g = e.events, S = e.axisType, P = e.axisId, I = sc(de(de({}, O), {}, {
    ticks: n
  }), x, w), C = Ft(O), T = Wu(i), _ = Ly(C.textAnchor) ? C.textAnchor : kD(f, d), z = CD(f, d), $ = {};
  typeof a == "object" && ($ = a);
  var U = de(de({}, C), {}, {
    fill: "none"
  }, $), V = I.map((G) => de({
    entry: G
  }, ID(G, h, p, v, m, f, y, d, b))), K = V.map((G) => {
    var F = G.entry, he = G.line;
    return /* @__PURE__ */ A.createElement(Ct, {
      className: "recharts-cartesian-axis-tick",
      key: "tick-".concat(F.value, "-").concat(F.coordinate, "-").concat(F.tickCoord)
    }, a && /* @__PURE__ */ A.createElement("line", $r({}, U, he, {
      className: ce("recharts-cartesian-axis-tick-line", Ut(a, "className"))
    })));
  }), L = V.map((G, F) => {
    var he, pe, le = G.entry, We = G.tick, Qe = de(de(de(de({
      verticalAnchor: z
    }, C), {}, {
      textAnchor: _,
      stroke: "none",
      fill: o
    }, We), {}, {
      index: F,
      payload: le,
      visibleTicksCount: I.length,
      tickFormatter: u,
      padding: c
    }, s), {}, {
      angle: (he = (pe = s == null ? void 0 : s.angle) !== null && pe !== void 0 ? pe : C.angle) !== null && he !== void 0 ? he : 0
    }), vt = de(de({}, Qe), T);
    return /* @__PURE__ */ A.createElement(Ct, $r({
      className: "recharts-cartesian-axis-tick-label",
      key: "tick-label-".concat(le.value, "-").concat(le.coordinate, "-").concat(le.tickCoord)
    }, Yu(g, le, F)), i && /* @__PURE__ */ A.createElement(TD, {
      option: i,
      tickProps: vt,
      value: "".concat(typeof u == "function" ? u(le.value, F) : le.value).concat(l || "")
    }));
  });
  return /* @__PURE__ */ A.createElement("g", {
    className: "recharts-cartesian-axis-ticks recharts-".concat(S, "-ticks")
  }, /* @__PURE__ */ A.createElement(MD, {
    ticks: I,
    axisId: P,
    axisType: S
  }), L.length > 0 && /* @__PURE__ */ A.createElement(er, {
    zIndex: Ue.label
  }, /* @__PURE__ */ A.createElement("g", {
    className: "recharts-cartesian-axis-tick-labels recharts-".concat(S, "-tick-labels"),
    ref: t
  }, L)), K.length > 0 && /* @__PURE__ */ A.createElement("g", {
    className: "recharts-cartesian-axis-tick-lines recharts-".concat(S, "-tick-lines")
  }, K));
}), ND = /* @__PURE__ */ ze((e, t) => {
  var r = e.axisLine, n = e.width, i = e.height, a = e.className, o = e.hide, u = e.ticks, l = e.axisType, c = e.axisId, s = AD(e, yD), f = _e(""), d = Jd(f, 2), h = d[0], p = d[1], v = _e(""), m = Jd(v, 2), y = m[0], b = m[1], x = Q(null);
  wv(t, () => ({
    getCalculatedWidth: () => {
      var O;
      return dD({
        ticks: x.current,
        label: (O = e.labelRef) === null || O === void 0 ? void 0 : O.current,
        labelGapWithTick: 5,
        tickSize: e.tickSize,
        tickMargin: e.tickMargin
      });
    }
  }));
  var w = ie((O) => {
    if (O) {
      var g = O.getElementsByClassName("recharts-cartesian-axis-tick-value");
      x.current = g;
      var S = g[0];
      if (S) {
        var P = window.getComputedStyle(S), I = P.fontSize, C = P.letterSpacing;
        (I !== h || C !== y) && (p(I), b(C));
      }
    }
  }, [h, y]);
  return o || n != null && n <= 0 || i != null && i <= 0 ? null : /* @__PURE__ */ A.createElement(er, {
    zIndex: e.zIndex
  }, /* @__PURE__ */ A.createElement(Ct, {
    className: ce("recharts-cartesian-axis", a)
  }, /* @__PURE__ */ A.createElement(PD, {
    x: e.x,
    y: e.y,
    width: n,
    height: i,
    orientation: e.orientation,
    mirror: e.mirror,
    axisLine: r,
    otherSvgProps: Ft(e)
  }), /* @__PURE__ */ A.createElement(DD, {
    ref: w,
    axisType: l,
    events: s,
    fontSize: h,
    getTicksConfig: e,
    height: e.height,
    letterSpacing: y,
    mirror: e.mirror,
    orientation: e.orientation,
    padding: e.padding,
    stroke: e.stroke,
    tick: e.tick,
    tickFormatter: e.tickFormatter,
    tickLine: e.tickLine,
    tickMargin: e.tickMargin,
    tickSize: e.tickSize,
    tickTextProps: e.tickTextProps,
    ticks: u,
    unit: e.unit,
    width: e.width,
    x: e.x,
    y: e.y,
    axisId: c
  }), /* @__PURE__ */ A.createElement(QC, {
    x: e.x,
    y: e.y,
    width: e.width,
    height: e.height,
    lowerWidth: e.width,
    upperWidth: e.width
  }, /* @__PURE__ */ A.createElement(lT, {
    label: e.label,
    labelRef: e.labelRef
  }), e.children)));
}), fc = /* @__PURE__ */ A.forwardRef((e, t) => {
  var r = st(e, Bt);
  return /* @__PURE__ */ A.createElement(ND, $r({}, r, {
    ref: t
  }));
});
fc.displayName = "CartesianAxis";
var jD = ["x1", "y1", "x2", "y2", "key"], $D = ["offset"], RD = ["xAxisId", "yAxisId"], LD = ["xAxisId", "yAxisId"];
function rv(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Le(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? rv(Object(r), !0).forEach(function(n) {
      zD(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : rv(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function zD(e, t, r) {
  return (t = BD(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function BD(e) {
  var t = FD(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function FD(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function _r() {
  return _r = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, _r.apply(null, arguments);
}
function Sa(e, t) {
  if (e == null) return {};
  var r, n, i = WD(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function WD(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var UD = (e) => {
  var t = e.fill;
  if (!t || t === "none")
    return null;
  var r = e.fillOpacity, n = e.x, i = e.y, a = e.width, o = e.height, u = e.ry;
  return /* @__PURE__ */ A.createElement("rect", {
    x: n,
    y: i,
    ry: u,
    width: a,
    height: o,
    stroke: "none",
    fill: t,
    fillOpacity: r,
    className: "recharts-cartesian-grid-bg"
  });
};
function cg(e) {
  var t = e.option, r = e.lineItemProps, n;
  if (/* @__PURE__ */ A.isValidElement(t))
    n = /* @__PURE__ */ A.cloneElement(t, r);
  else if (typeof t == "function")
    n = t(r);
  else {
    var i, a = r.x1, o = r.y1, u = r.x2, l = r.y2, c = r.key, s = Sa(r, jD), f = (i = Ft(s)) !== null && i !== void 0 ? i : {};
    f.offset;
    var d = Sa(f, $D);
    n = /* @__PURE__ */ A.createElement("line", _r({}, d, {
      x1: a,
      y1: o,
      x2: u,
      y2: l,
      fill: "none",
      key: c
    }));
  }
  return n;
}
function VD(e) {
  var t = e.x, r = e.width, n = e.horizontal, i = n === void 0 ? !0 : n, a = e.horizontalPoints;
  if (!i || !a || !a.length)
    return null;
  e.xAxisId, e.yAxisId;
  var o = Sa(e, RD), u = a.map((l, c) => {
    var s = Le(Le({}, o), {}, {
      x1: t,
      y1: l,
      x2: t + r,
      y2: l,
      key: "line-".concat(c),
      index: c
    });
    return /* @__PURE__ */ A.createElement(cg, {
      key: "line-".concat(c),
      option: i,
      lineItemProps: s
    });
  });
  return /* @__PURE__ */ A.createElement("g", {
    className: "recharts-cartesian-grid-horizontal"
  }, u);
}
function KD(e) {
  var t = e.y, r = e.height, n = e.vertical, i = n === void 0 ? !0 : n, a = e.verticalPoints;
  if (!i || !a || !a.length)
    return null;
  e.xAxisId, e.yAxisId;
  var o = Sa(e, LD), u = a.map((l, c) => {
    var s = Le(Le({}, o), {}, {
      x1: l,
      y1: t,
      x2: l,
      y2: t + r,
      key: "line-".concat(c),
      index: c
    });
    return /* @__PURE__ */ A.createElement(cg, {
      option: i,
      lineItemProps: s,
      key: "line-".concat(c)
    });
  });
  return /* @__PURE__ */ A.createElement("g", {
    className: "recharts-cartesian-grid-vertical"
  }, u);
}
function HD(e) {
  var t = e.horizontalFill, r = e.fillOpacity, n = e.x, i = e.y, a = e.width, o = e.height, u = e.horizontalPoints, l = e.horizontal, c = l === void 0 ? !0 : l;
  if (!c || !t || !t.length || u == null)
    return null;
  var s = u.map((d) => Math.round(d + i - i)).sort((d, h) => d - h);
  i !== s[0] && s.unshift(0);
  var f = s.map((d, h) => {
    var p = s[h + 1], v = p == null, m = v ? i + o - d : p - d;
    if (m <= 0)
      return null;
    var y = h % t.length;
    return /* @__PURE__ */ A.createElement("rect", {
      key: "react-".concat(h),
      y: d,
      x: n,
      height: m,
      width: a,
      stroke: "none",
      fill: t[y],
      fillOpacity: r,
      className: "recharts-cartesian-grid-bg"
    });
  });
  return /* @__PURE__ */ A.createElement("g", {
    className: "recharts-cartesian-gridstripes-horizontal"
  }, f);
}
function YD(e) {
  var t = e.vertical, r = t === void 0 ? !0 : t, n = e.verticalFill, i = e.fillOpacity, a = e.x, o = e.y, u = e.width, l = e.height, c = e.verticalPoints;
  if (!r || !n || !n.length)
    return null;
  var s = c.map((d) => Math.round(d + a - a)).sort((d, h) => d - h);
  a !== s[0] && s.unshift(0);
  var f = s.map((d, h) => {
    var p = s[h + 1], v = p == null, m = v ? a + u - d : p - d;
    if (m <= 0)
      return null;
    var y = h % n.length;
    return /* @__PURE__ */ A.createElement("rect", {
      key: "react-".concat(h),
      x: d,
      y: o,
      width: m,
      height: l,
      stroke: "none",
      fill: n[y],
      fillOpacity: i,
      className: "recharts-cartesian-grid-bg"
    });
  });
  return /* @__PURE__ */ A.createElement("g", {
    className: "recharts-cartesian-gridstripes-vertical"
  }, f);
}
var GD = (e, t) => {
  var r = e.xAxis, n = e.width, i = e.height, a = e.offset;
  return Wh(sc(Le(Le(Le({}, Bt), r), {}, {
    ticks: Uh(r),
    viewBox: {
      x: 0,
      y: 0,
      width: n,
      height: i
    }
  })), a.left, a.left + a.width, t);
}, qD = (e, t) => {
  var r = e.yAxis, n = e.width, i = e.height, a = e.offset;
  return Wh(sc(Le(Le(Le({}, Bt), r), {}, {
    ticks: Uh(r),
    viewBox: {
      x: 0,
      y: 0,
      width: n,
      height: i
    }
  })), a.top, a.top + a.height, t);
}, XD = {
  horizontal: !0,
  vertical: !0,
  // The ordinates of horizontal grid lines
  horizontalPoints: [],
  // The abscissas of vertical grid lines
  verticalPoints: [],
  stroke: "#ccc",
  fill: "none",
  // The fill of colors of grid lines
  verticalFill: [],
  horizontalFill: [],
  xAxisId: 0,
  yAxisId: 0,
  syncWithTicks: !1,
  zIndex: Ue.grid
};
function sg(e) {
  var t = Xh(), r = Zh(), n = qh(), i = Le(Le({}, st(e, XD)), {}, {
    x: N(e.x) ? e.x : n.left,
    y: N(e.y) ? e.y : n.top,
    width: N(e.width) ? e.width : n.width,
    height: N(e.height) ? e.height : n.height
  }), a = i.xAxisId, o = i.yAxisId, u = i.x, l = i.y, c = i.width, s = i.height, f = i.syncWithTicks, d = i.horizontalValues, h = i.verticalValues, p = Ze(), v = j((I) => td(I, "xAxis", a, p)), m = j((I) => td(I, "yAxis", o, p));
  if (!Dt(c) || !Dt(s) || !N(u) || !N(l))
    return null;
  var y = i.verticalCoordinatesGenerator || GD, b = i.horizontalCoordinatesGenerator || qD, x = i.horizontalPoints, w = i.verticalPoints;
  if ((!x || !x.length) && typeof b == "function") {
    var O = d && d.length, g = b({
      yAxis: m ? Le(Le({}, m), {}, {
        ticks: O ? d : m.ticks
      }) : void 0,
      width: t ?? c,
      height: r ?? s,
      offset: n
    }, O ? !0 : f);
    ta(Array.isArray(g), "horizontalCoordinatesGenerator should return Array but instead it returned [".concat(typeof g, "]")), Array.isArray(g) && (x = g);
  }
  if ((!w || !w.length) && typeof y == "function") {
    var S = h && h.length, P = y({
      xAxis: v ? Le(Le({}, v), {}, {
        ticks: S ? h : v.ticks
      }) : void 0,
      width: t ?? c,
      height: r ?? s,
      offset: n
    }, S ? !0 : f);
    ta(Array.isArray(P), "verticalCoordinatesGenerator should return Array but instead it returned [".concat(typeof P, "]")), Array.isArray(P) && (w = P);
  }
  return /* @__PURE__ */ A.createElement(er, {
    zIndex: i.zIndex
  }, /* @__PURE__ */ A.createElement("g", {
    className: "recharts-cartesian-grid"
  }, /* @__PURE__ */ A.createElement(UD, {
    fill: i.fill,
    fillOpacity: i.fillOpacity,
    x: i.x,
    y: i.y,
    width: i.width,
    height: i.height,
    ry: i.ry
  }), /* @__PURE__ */ A.createElement(HD, _r({}, i, {
    horizontalPoints: x
  })), /* @__PURE__ */ A.createElement(YD, _r({}, i, {
    verticalPoints: w
  })), /* @__PURE__ */ A.createElement(VD, _r({}, i, {
    offset: n,
    horizontalPoints: x,
    xAxis: v,
    yAxis: m
  })), /* @__PURE__ */ A.createElement(KD, _r({}, i, {
    offset: n,
    verticalPoints: w,
    xAxis: v,
    yAxis: m
  }))));
}
sg.displayName = "CartesianGrid";
var ZD = {}, fg = Be({
  name: "errorBars",
  initialState: ZD,
  reducers: {
    addErrorBar: (e, t) => {
      var r = t.payload, n = r.itemId, i = r.errorBar;
      e[n] || (e[n] = []), e[n].push(i);
    },
    replaceErrorBar: (e, t) => {
      var r = t.payload, n = r.itemId, i = r.prev, a = r.next;
      e[n] && (e[n] = e[n].map((o) => o.dataKey === i.dataKey && o.direction === i.direction ? a : o));
    },
    removeErrorBar: (e, t) => {
      var r = t.payload, n = r.itemId, i = r.errorBar;
      e[n] && (e[n] = e[n].filter((a) => a.dataKey !== i.dataKey || a.direction !== i.direction));
    }
  }
}), dc = fg.actions;
dc.addErrorBar;
dc.replaceErrorBar;
dc.removeErrorBar;
var QD = fg.reducer, JD = ["children"];
function eN(e, t) {
  if (e == null) return {};
  var r, n, i = tN(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function tN(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var rN = {
  data: [],
  xAxisId: "xAxis-0",
  yAxisId: "yAxis-0",
  dataPointFormatter: () => ({
    x: 0,
    y: 0,
    value: 0
  }),
  errorBarOffset: 0
}, nN = /* @__PURE__ */ Xe(rN);
function iN(e) {
  var t = e.children, r = eN(e, JD);
  return /* @__PURE__ */ A.createElement(nN.Provider, {
    value: r
  }, t);
}
function dg(e, t) {
  var r, n, i = j((c) => Zt(c, e)), a = j((c) => Qt(c, t)), o = (r = i == null ? void 0 : i.allowDataOverflow) !== null && r !== void 0 ? r : ge.allowDataOverflow, u = (n = a == null ? void 0 : a.allowDataOverflow) !== null && n !== void 0 ? n : be.allowDataOverflow, l = o || u;
  return {
    needClip: l,
    needClipX: o,
    needClipY: u
  };
}
function aN(e) {
  var t = e.xAxisId, r = e.yAxisId, n = e.clipPathId, i = ng(), a = dg(t, r), o = a.needClipX, u = a.needClipY, l = a.needClip, c = j((x) => Vm(x, t, !1)), s = j((x) => Km(x, r, !1));
  if (!l || !i)
    return null;
  var f = i.x, d = i.y, h = i.width, p = i.height, v = o && c ? Math.min(c[0], c[1]) : f - h / 2, m = u && s ? Math.min(s[0], s[1]) : d - p / 2, y = o && c ? Math.abs(c[1] - c[0]) : h * 2, b = u && s ? Math.abs(s[1] - s[0]) : p * 2;
  return /* @__PURE__ */ A.createElement("clipPath", {
    id: "clipPath-".concat(n)
  }, /* @__PURE__ */ A.createElement("rect", {
    x: v,
    y: m,
    width: y,
    height: b
  }));
}
function Br(e, t) {
  var r, n;
  return (r = (n = e.graphicalItems.cartesianItems.find((i) => i.id === t)) === null || n === void 0 ? void 0 : n.xAxisId) !== null && r !== void 0 ? r : tg;
}
function Fr(e, t) {
  var r, n;
  return (r = (n = e.graphicalItems.cartesianItems.find((i) => i.id === t)) === null || n === void 0 ? void 0 : n.yAxisId) !== null && r !== void 0 ? r : tg;
}
var oN = process.env.NODE_ENV === "production", Vo = "Invariant failed";
function uN(e, t) {
  if (oN)
    throw new Error(Vo);
  var r = typeof t == "function" ? t() : t, n = r ? "".concat(Vo, ": ").concat(r) : Vo;
  throw new Error(n);
}
var lN = ["option"];
function cN(e, t) {
  if (e == null) return {};
  var r, n, i = sN(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function sN(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var vc = lp;
function hc(e) {
  var t = e.option, r = cN(e, lN);
  return /* @__PURE__ */ A.createElement(TT, {
    option: t,
    DefaultShape: vc,
    shapeProps: r,
    activeClassName: "recharts-active-bar",
    inActiveClassName: "recharts-inactive-bar"
  });
}
var fN = function(t) {
  var r = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : 0;
  return (n, i) => {
    if (N(t)) return t;
    var a = N(n) || we(n);
    return a ? t(n, i) : (a || uN(!1, "minPointSize callback function received a value with type of ".concat(typeof n, ". Currently only numbers or null/undefined are supported.")), r);
  };
}, dN = (e, t, r) => r, vN = (e, t) => t, pi = E([Ul, vN], (e, t) => e.filter((r) => r.type === "bar").find((r) => r.id === t)), hN = E([pi], (e) => e == null ? void 0 : e.maxBarSize), pN = (e, t, r, n) => n, mN = E([ne, Ul, Br, Fr, dN], (e, t, r, n, i) => t.filter((a) => e === "horizontal" ? a.xAxisId === r : a.yAxisId === n).filter((a) => a.isPanorama === i).filter((a) => a.hide === !1).filter((a) => a.type === "bar")), yN = (e, t, r) => {
  var n = ne(e), i = Br(e, t), a = Fr(e, t);
  if (!(i == null || a == null))
    return n === "horizontal" ? Pu(e, "yAxis", a, r) : Pu(e, "xAxis", i, r);
}, gN = (e, t) => {
  var r = ne(e), n = Br(e, t), i = Fr(e, t);
  if (!(n == null || i == null))
    return r === "horizontal" ? ed(e, "xAxis", n) : ed(e, "yAxis", i);
}, bN = E([mN, W1, gN], NM), xN = (e, t, r) => {
  var n, i, a = pi(e, t);
  if (a == null)
    return 0;
  var o = Br(e, t), u = Fr(e, t);
  if (o == null || u == null)
    return 0;
  var l = ne(e), c = Op(e), s = a.maxBarSize, f = we(s) ? c : s, d, h;
  return l === "horizontal" ? (d = un(e, "xAxis", o, r), h = on(e, "xAxis", o, r)) : (d = un(e, "yAxis", u, r), h = on(e, "yAxis", u, r)), (n = (i = ea(d, h, !0)) !== null && i !== void 0 ? i : f) !== null && n !== void 0 ? n : 0;
}, vg = (e, t, r) => {
  var n = ne(e), i = Br(e, t), a = Fr(e, t);
  if (!(i == null || a == null)) {
    var o, u;
    return n === "horizontal" ? (o = un(e, "xAxis", i, r), u = on(e, "xAxis", i, r)) : (o = un(e, "yAxis", a, r), u = on(e, "yAxis", a, r)), ea(o, u);
  }
}, wN = E([bN, Op, F1, Ep, xN, vg, hN], zM), AN = (e, t, r) => {
  var n = Br(e, t);
  if (n != null)
    return un(e, "xAxis", n, r);
}, ON = (e, t, r) => {
  var n = Fr(e, t);
  if (n != null)
    return un(e, "yAxis", n, r);
}, EN = (e, t, r) => {
  var n = Br(e, t);
  if (n != null)
    return on(e, "xAxis", n, r);
}, SN = (e, t, r) => {
  var n = Fr(e, t);
  if (n != null)
    return on(e, "yAxis", n, r);
}, _N = E([wN, pi], FM), PN = E([yN, pi], BM), IN = E([Pe, rl, AN, ON, EN, SN, _N, ne, S1, vg, PN, pi, pN], (e, t, r, n, i, a, o, u, l, c, s, f, d) => {
  var h = l.chartData, p = l.dataStartIndex, v = l.dataEndIndex;
  if (!(f == null || o == null || t == null || u !== "horizontal" && u !== "vertical" || r == null || n == null || i == null || a == null || c == null)) {
    var m = f.data, y;
    if (m != null && m.length > 0 ? y = m : y = h == null ? void 0 : h.slice(p, v + 1), y != null)
      return uj({
        layout: u,
        barSettings: f,
        pos: o,
        parentViewBox: t,
        bandSize: c,
        xAxis: r,
        yAxis: n,
        xAxisTicks: i,
        yAxisTicks: a,
        stackedData: s,
        displayedData: y,
        offset: e,
        cells: d,
        dataStartIndex: p
      });
  }
}), kN = ["index"];
function Mu() {
  return Mu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Mu.apply(null, arguments);
}
function CN(e, t) {
  if (e == null) return {};
  var r, n, i = TN(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function TN(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var hg = /* @__PURE__ */ Xe(void 0), MN = (e) => {
  var t = ct(hg);
  if (t != null)
    return t.stackId;
  if (e != null)
    return ww(e);
}, DN = (e, t) => "recharts-bar-stack-clip-path-".concat(e, "-").concat(t), NN = (e) => {
  var t = ct(hg);
  if (t != null) {
    var r = t.stackId;
    return "url(#".concat(DN(r, e), ")");
  }
}, pg = (e) => {
  var t = e.index, r = CN(e, kN), n = NN(t);
  return /* @__PURE__ */ A.createElement(Ct, Mu({
    className: "recharts-bar-stack-layer",
    clipPath: n
  }, r));
}, jN = ["onMouseEnter", "onMouseLeave", "onClick"], $N = ["value", "background", "tooltipPosition"], RN = ["id"], LN = ["onMouseEnter", "onClick", "onMouseLeave"];
function nv(e, t) {
  return WN(e) || FN(e, t) || BN(e, t) || zN();
}
function zN() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function BN(e, t) {
  if (e) {
    if (typeof e == "string") return iv(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? iv(e, t) : void 0;
  }
}
function iv(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function FN(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function WN(e) {
  if (Array.isArray(e)) return e;
}
function cr() {
  return cr = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, cr.apply(null, arguments);
}
function av(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function xe(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? av(Object(r), !0).forEach(function(n) {
      UN(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : av(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function UN(e, t, r) {
  return (t = VN(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function VN(e) {
  var t = KN(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function KN(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function _a(e, t) {
  if (e == null) return {};
  var r, n, i = HN(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function HN(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var YN = (e) => {
  var t = e.dataKey, r = e.name, n = e.fill, i = e.legendType, a = e.hide;
  return [{
    inactive: a,
    dataKey: t,
    type: i,
    color: n,
    value: Vh(r, t),
    payload: e
  }];
}, GN = /* @__PURE__ */ A.memo((e) => {
  var t = e.dataKey, r = e.stroke, n = e.strokeWidth, i = e.fill, a = e.name, o = e.hide, u = e.unit, l = e.formatter, c = e.tooltipType, s = e.id, f = {
    dataDefinedOnItem: void 0,
    getPosition: fn,
    settings: {
      stroke: r,
      strokeWidth: n,
      fill: i,
      dataKey: t,
      nameKey: void 0,
      name: Vh(a, t),
      hide: o,
      type: c,
      color: i,
      unit: u,
      formatter: l,
      graphicalItemId: s
    }
  };
  return /* @__PURE__ */ A.createElement(MT, {
    tooltipEntrySettings: f
  });
});
function qN(e) {
  var t = j(cn), r = e.data, n = e.dataKey, i = e.background, a = e.allOtherBarProps, o = a.onMouseEnter, u = a.onMouseLeave, l = a.onClick, c = _a(a, jN), s = qy(o, n, a.id), f = Xy(u), d = Zy(l, n, a.id);
  if (!i || r == null)
    return null;
  var h = Wu(i);
  return /* @__PURE__ */ A.createElement(er, {
    zIndex: WM(i, Ue.barBackground)
  }, r.map((p, v) => {
    p.value;
    var m = p.background;
    p.tooltipPosition;
    var y = _a(p, $N);
    if (!m)
      return null;
    var b = s(p, p.originalDataIndex), x = f(p, p.originalDataIndex), w = d(p, p.originalDataIndex), O = xe(xe(xe(xe(xe({
      option: i,
      isActive: String(p.originalDataIndex) === t
    }, y), {}, {
      // @ts-expect-error backgroundProps is contributing unknown props
      fill: "#eee"
    }, m), h), Yu(c, p, v)), {}, {
      onMouseEnter: b,
      onMouseLeave: x,
      onClick: w,
      dataKey: n,
      index: v,
      className: "recharts-bar-background-rectangle"
    });
    return /* @__PURE__ */ A.createElement(hc, cr({
      key: "background-bar-".concat(v)
    }, O));
  }));
}
function XN(e) {
  var t = e.showLabels, r = e.children, n = e.rects, i = n == null ? void 0 : n.map((a) => {
    var o = {
      x: a.x,
      y: a.y,
      width: a.width,
      lowerWidth: a.width,
      upperWidth: a.width,
      height: a.height
    };
    return xe(xe({}, o), {}, {
      value: a.value,
      payload: a.payload,
      parentViewBox: a.parentViewBox,
      viewBox: o,
      fill: a.fill
    });
  });
  return /* @__PURE__ */ A.createElement(vT, {
    value: t ? i : void 0
  }, r);
}
function ZN(e) {
  var t = e.shape, r = e.activeBar, n = e.baseProps, i = e.entry, a = e.index, o = e.dataKey, u = j(cn), l = j(wy), c = r && String(i.originalDataIndex) === u && (l == null || o === l), s = _e(!1), f = nv(s, 2), d = f[0], h = f[1], p = _e(!1), v = nv(p, 2), m = v[0], y = v[1];
  ve(() => {
    var S;
    return c ? (h(!0), S = requestAnimationFrame(() => {
      y(!0);
    })) : y(!1), () => {
      cancelAnimationFrame(S);
    };
  }, [c]);
  var b = ie(() => {
    c || h(!1);
  }, [c]), x = c && m, w = c || d, O;
  c ? r === !0 ? O = t : O = r : O = t;
  var g = /* @__PURE__ */ A.createElement(hc, cr({}, n, {
    name: String(n.name)
  }, i, {
    isActive: x,
    option: O,
    index: a,
    dataKey: o,
    animationElapsedTime: e.animationElapsedTime,
    isAnimating: e.isAnimating,
    isEntrance: e.isEntrance,
    onTransitionEnd: b
  }));
  return w ? /* @__PURE__ */ A.createElement(er, {
    zIndex: Ue.activeBar
  }, /* @__PURE__ */ A.createElement(pg, {
    index: i.originalDataIndex
  }, g)) : g;
}
function QN(e) {
  var t = e.shape, r = e.baseProps, n = e.entry, i = e.index, a = e.dataKey;
  return /* @__PURE__ */ A.createElement(hc, cr({}, r, {
    name: String(r.name)
  }, n, {
    isActive: !1,
    option: t,
    index: i,
    dataKey: a,
    animationElapsedTime: e.animationElapsedTime,
    isAnimating: e.isAnimating,
    isEntrance: e.isEntrance
  }));
}
function JN(e) {
  var t, r = e.data, n = e.props, i = e.animationElapsedTime, a = e.isAnimating, o = e.isEntrance, u = (t = Ft(n)) !== null && t !== void 0 ? t : {}, l = u.id, c = _a(u, RN), s = n.shape, f = n.dataKey, d = n.activeBar, h = n.onMouseEnter, p = n.onClick, v = n.onMouseLeave, m = _a(n, LN), y = qy(h, f, l), b = Xy(v), x = Zy(p, f, l);
  return r ? /* @__PURE__ */ A.createElement(A.Fragment, null, r.map((w, O) => /* @__PURE__ */ A.createElement(pg, cr({
    index: w.originalDataIndex,
    key: "rectangle-".concat(w == null ? void 0 : w.x, "-").concat(w == null ? void 0 : w.y, "-").concat(w == null ? void 0 : w.value, "-").concat(O),
    className: "recharts-bar-rectangle"
  }, Yu(m, w, O), {
    onMouseEnter: y(w, w.originalDataIndex),
    onMouseLeave: b(w, w.originalDataIndex),
    onClick: x(w, w.originalDataIndex)
  }), d ? /* @__PURE__ */ A.createElement(ZN, {
    shape: s,
    activeBar: d,
    baseProps: c,
    entry: w,
    index: O,
    dataKey: f,
    animationElapsedTime: i,
    isAnimating: a,
    isEntrance: o
  }) : (
    /*
     * If the `activeBar` prop is falsy, then let's call the variant without hooks.
     * Using the `selectActiveTooltipIndex` selector is usually fast
     * but in charts with large-ish amount of data even the few nanoseconds add up to a noticeable jank.
     * If the activeBar is false then we don't need to know which index is active - because we won't use it anyway.
     * So let's just skip the hooks altogether. That way, React can skip rendering the component,
     * and can skip the tree reconciliation for its children too.
     * Because we can't call hooks conditionally, we need to have a separate component for that.
     */
    /* @__PURE__ */ A.createElement(QN, {
      shape: s,
      baseProps: c,
      entry: w,
      index: O,
      dataKey: f,
      animationElapsedTime: i,
      isAnimating: a,
      isEntrance: o
    })
  )))) : null;
}
var ej = (e, t, r) => e == null ? [] : t === 1 ? e.flatMap((n) => n.status === "removed" ? [] : [n.next]) : e.flatMap((n) => {
  if (n.status === "removed")
    return r === "horizontal" ? [xe(xe({}, n.prev), {}, {
      height: $e(n.prev.height, 0, t),
      y: $e(n.prev.y, n.prev.y + n.prev.height, t)
    })] : [xe(xe({}, n.prev), {}, {
      width: $e(n.prev.width, 0, t)
    })];
  if (n.status === "matched")
    return [xe(xe({}, n.next), {}, {
      x: $e(n.prev.x, n.next.x, t),
      y: $e(n.prev.y, n.next.y, t),
      width: $e(n.prev.width, n.next.width, t),
      height: $e(n.prev.height, n.next.height, t)
    })];
  var i = n.next;
  return r === "horizontal" ? [xe(xe({}, i), {}, {
    height: $e(0, i.height, t),
    y: $e(i.stackedBarStart, i.y, t)
  })] : [xe(xe({}, i), {}, {
    width: $e(0, i.width, t),
    x: $e(i.stackedBarStart, i.x, t)
  })];
});
function tj(e) {
  var t = e.props, r = e.previousRectanglesRef, n = t.data, i = t.isAnimationActive, a = t.animationBegin, o = t.animationDuration, u = t.animationEasing, l = t.animationInterpolateFn, c = t.layout, s = XT(t.onAnimationStart, t.onAnimationEnd), f = s.isAnimating, d = s.handleAnimationStart, h = s.handleAnimationEnd;
  return /* @__PURE__ */ A.createElement(XN, {
    showLabels: !f,
    rects: n
  }, /* @__PURE__ */ A.createElement(ZT, {
    animationInput: n,
    animationIdPrefix: "recharts-bar-",
    items: n,
    previousItemsRef: r,
    isAnimationActive: i,
    animationBegin: a,
    animationDuration: o,
    animationEasing: u,
    onAnimationStart: d,
    onAnimationEnd: h,
    animationInterpolateFn: l,
    animationMatchBy: t.animationMatchBy,
    layout: c
  }, (p, v, m) => /* @__PURE__ */ A.createElement(Ct, null, /* @__PURE__ */ A.createElement(JN, {
    props: t,
    data: p,
    animationElapsedTime: v,
    isAnimating: f || v < 1,
    isEntrance: m
  }))), /* @__PURE__ */ A.createElement(mT, {
    label: t.label
  }), t.children);
}
function rj(e) {
  var t = Q(null);
  return /* @__PURE__ */ A.createElement(tj, {
    previousRectanglesRef: t,
    props: e
  });
}
var mg = 0, nj = (e, t) => {
  var r = Array.isArray(e.value) ? e.value[1] : e.value;
  return {
    x: e.x,
    y: e.y,
    value: r,
    // getValueByDataKey does not validate the output type
    errorVal: Se(e, t)
  };
};
class ij extends zg {
  render() {
    var t = this.props, r = t.hide, n = t.data, i = t.dataKey, a = t.className, o = t.xAxisId, u = t.yAxisId, l = t.needClip, c = t.background, s = t.id;
    if (r || n == null)
      return null;
    var f = ce("recharts-bar", a), d = s;
    return /* @__PURE__ */ A.createElement(Ct, {
      className: f,
      id: s
    }, l && /* @__PURE__ */ A.createElement("defs", null, /* @__PURE__ */ A.createElement(aN, {
      clipPathId: d,
      xAxisId: o,
      yAxisId: u
    })), /* @__PURE__ */ A.createElement(Ct, {
      className: "recharts-bar-rectangles",
      clipPath: l ? "url(#clipPath-".concat(d, ")") : void 0
    }, /* @__PURE__ */ A.createElement(qN, {
      data: n,
      dataKey: i,
      background: c,
      allOtherBarProps: this.props
    }), /* @__PURE__ */ A.createElement(rj, this.props)));
  }
}
var aj = {
  activeBar: !1,
  animationBegin: 0,
  animationDuration: 400,
  animationEasing: "ease",
  animationInterpolateFn: ej,
  animationMatchBy: Jy,
  background: !1,
  hide: !1,
  isAnimationActive: "auto",
  label: !1,
  legendType: "rect",
  minPointSize: mg,
  shape: vc,
  xAxisId: 0,
  yAxisId: 0,
  zIndex: Ue.bar
};
function oj(e) {
  var t = e.xAxisId, r = e.yAxisId, n = e.hide, i = e.legendType, a = e.minPointSize, o = e.activeBar, u = e.animationBegin, l = e.animationDuration, c = e.animationEasing, s = e.isAnimationActive, f = dg(t, r), d = f.needClip, h = dn(), p = Ze(), v = OT(e.children, My), m = j((x) => IN(x, e.id, p, v));
  if (h !== "vertical" && h !== "horizontal")
    return null;
  var y, b = m == null ? void 0 : m[0];
  return b == null || b.height == null || b.width == null ? y = 0 : y = h === "vertical" ? b.height / 2 : b.width / 2, /* @__PURE__ */ A.createElement(iN, {
    xAxisId: t,
    yAxisId: r,
    data: m,
    dataPointFormatter: nj,
    errorBarOffset: y
  }, /* @__PURE__ */ A.createElement(ij, cr({}, e, {
    layout: h,
    needClip: d,
    data: m,
    xAxisId: t,
    yAxisId: r,
    hide: n,
    legendType: i,
    minPointSize: a,
    activeBar: o,
    animationBegin: u,
    animationDuration: l,
    animationEasing: c,
    isAnimationActive: s
  })));
}
function uj(e) {
  var t = e.layout, r = e.barSettings, n = r.dataKey, i = r.minPointSize, a = r.hasCustomShape, o = e.pos, u = e.bandSize, l = e.xAxis, c = e.yAxis, s = e.xAxisTicks, f = e.yAxisTicks, d = e.stackedData, h = e.displayedData, p = e.offset, v = e.cells, m = e.parentViewBox, y = e.dataStartIndex, b = t === "horizontal" ? c : l, x = d ? b.scale.domain() : null, w = Aw({
    numericAxis: b
  }), O = b.scale.map(w);
  return h.map((g, S) => {
    var P, I, C, T, _, z;
    if (d) {
      var $ = d[S + y];
      if ($ == null)
        return null;
      P = mw($, x);
    } else
      P = Se(g, n), Array.isArray(P) || (P = [w, P]);
    var U = fN(i, mg)(P[1], S);
    if (t === "horizontal") {
      var V, K = c.scale.map(P[0]), L = c.scale.map(P[1]);
      if (K == null || L == null)
        return null;
      I = es({
        axis: l,
        ticks: s,
        bandSize: u,
        offset: o.offset,
        entry: g,
        index: S
      }), C = (V = L ?? K) !== null && V !== void 0 ? V : void 0, T = o.size;
      var G = K - L;
      if (_ = Tt(G) ? 0 : G, z = {
        x: I,
        y: p.top,
        width: T,
        height: p.height
      }, Math.abs(U) > 0 && Math.abs(_) < Math.abs(U)) {
        var F = He(_ || U) * (Math.abs(U) - Math.abs(_));
        C -= F, _ += F;
      }
    } else {
      var he = l.scale.map(P[0]), pe = l.scale.map(P[1]);
      if (he == null || pe == null)
        return null;
      if (I = he, C = es({
        axis: c,
        ticks: f,
        bandSize: u,
        offset: o.offset,
        entry: g,
        index: S
      }), T = pe - he, _ = o.size, z = {
        x: p.left,
        y: C,
        width: p.width,
        height: _
      }, Math.abs(U) > 0 && Math.abs(T) < Math.abs(U)) {
        var le = He(T || U) * (Math.abs(U) - Math.abs(T));
        T += le;
      }
    }
    if (I == null || C == null || T == null || _ == null || !a && (T === 0 || _ === 0))
      return null;
    var We = xe(xe({}, g), {}, {
      stackedBarStart: O,
      x: I,
      y: C,
      width: T,
      height: _,
      value: d ? P : P[1],
      payload: g,
      background: z,
      tooltipPosition: {
        x: I + T / 2,
        y: C + _ / 2
      },
      parentViewBox: m,
      originalDataIndex: S
    }, v && v[S] && v[S].props);
    return We;
  }).filter(Boolean);
}
function lj(e) {
  var t = st(e, aj), r = MN(t.stackId), n = Ze();
  return /* @__PURE__ */ A.createElement(uM, {
    id: t.id,
    type: "bar"
  }, (i) => /* @__PURE__ */ A.createElement(A.Fragment, null, /* @__PURE__ */ A.createElement(DT, {
    legendPayload: YN(t)
  }), /* @__PURE__ */ A.createElement(GN, {
    dataKey: t.dataKey,
    stroke: t.stroke,
    strokeWidth: t.strokeWidth,
    fill: t.fill,
    name: t.name,
    hide: t.hide,
    unit: t.unit,
    formatter: t.formatter,
    tooltipType: t.tooltipType,
    id: i
  }), /* @__PURE__ */ A.createElement(hM, {
    type: "bar",
    id: i,
    data: void 0,
    xAxisId: t.xAxisId,
    yAxisId: t.yAxisId,
    zAxisId: 0,
    dataKey: t.dataKey,
    stackId: r,
    hide: t.hide,
    barSize: t.barSize,
    minPointSize: t.minPointSize,
    maxBarSize: t.maxBarSize,
    isPanorama: n,
    hasCustomShape: t.shape != null && t.shape !== vc
  }), /* @__PURE__ */ A.createElement(er, {
    zIndex: t.zIndex
  }, /* @__PURE__ */ A.createElement(oj, cr({}, t, {
    id: i
  })))));
}
var yg = /* @__PURE__ */ A.memo(lj, Ga);
yg.displayName = "Bar";
var cj = ["domain", "range"], sj = ["domain", "range"];
function ov(e, t) {
  if (e == null) return {};
  var r, n, i = fj(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function fj(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function uv(e, t) {
  return e === t ? !0 : Array.isArray(e) && e.length === 2 && Array.isArray(t) && t.length === 2 ? e[0] === t[0] && e[1] === t[1] : !1;
}
function gg(e, t) {
  if (e === t)
    return !0;
  var r = e.domain, n = e.range, i = ov(e, cj), a = t.domain, o = t.range, u = ov(t, sj);
  return !uv(r, a) || !uv(n, o) ? !1 : Ga(i, u);
}
var dj = ["type"], vj = ["dangerouslySetInnerHTML", "ticks", "scale"], hj = ["id", "scale"];
function Du() {
  return Du = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Du.apply(null, arguments);
}
function lv(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function cv(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? lv(Object(r), !0).forEach(function(n) {
      pj(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : lv(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function pj(e, t, r) {
  return (t = mj(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function mj(e) {
  var t = yj(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function yj(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function Nu(e, t) {
  if (e == null) return {};
  var r, n, i = gj(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function gj(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function bj(e) {
  var t = se(), r = Q(null), n = Qh(), i = e.type, a = Nu(e, dj), o = Qa(n, "xAxis", i), u = sr(() => {
    if (o != null)
      return cv(cv({}, a), {}, {
        type: o
      });
  }, [a, o]);
  return qe(() => {
    u != null && (r.current === null ? t(bM(u)) : r.current !== u && t(xM({
      prev: r.current,
      next: u
    })), r.current = u);
  }, [u, t]), qe(() => () => {
    r.current && (t(wM(r.current)), r.current = null);
  }, [t]), null;
}
var xj = (e) => {
  var t = e.xAxisId, r = e.className, n = j(rl), i = Ze(), a = "xAxis", o = j((d) => Jm(d, a, t, i)), u = j((d) => Xm(d, t)), l = j((d) => PP(d, t)), c = j((d) => mm(d, t));
  if (u == null || l == null || c == null)
    return null;
  e.dangerouslySetInnerHTML, e.ticks, e.scale;
  var s = Nu(e, vj);
  c.id, c.scale;
  var f = Nu(c, hj);
  return /* @__PURE__ */ A.createElement(fc, Du({}, s, f, {
    x: l.x,
    y: l.y,
    width: u.width,
    height: u.height,
    className: ce("recharts-".concat(a, " ").concat(a), r),
    viewBox: n,
    ticks: o,
    axisType: a,
    axisId: t
  }));
}, wj = {
  allowDataOverflow: ge.allowDataOverflow,
  allowDecimals: ge.allowDecimals,
  allowDuplicatedCategory: ge.allowDuplicatedCategory,
  angle: ge.angle,
  axisLine: Bt.axisLine,
  height: ge.height,
  hide: !1,
  includeHidden: ge.includeHidden,
  interval: ge.interval,
  label: !1,
  minTickGap: ge.minTickGap,
  mirror: ge.mirror,
  orientation: ge.orientation,
  padding: ge.padding,
  reversed: ge.reversed,
  scale: ge.scale,
  tick: ge.tick,
  tickCount: ge.tickCount,
  tickLine: Bt.tickLine,
  tickSize: Bt.tickSize,
  type: ge.type,
  niceTicks: ge.niceTicks,
  xAxisId: 0
}, Aj = (e) => {
  var t = st(e, wj);
  return /* @__PURE__ */ A.createElement(A.Fragment, null, /* @__PURE__ */ A.createElement(bj, {
    allowDataOverflow: t.allowDataOverflow,
    allowDecimals: t.allowDecimals,
    allowDuplicatedCategory: t.allowDuplicatedCategory,
    angle: t.angle,
    dataKey: t.dataKey,
    domain: t.domain,
    height: t.height,
    hide: t.hide,
    id: t.xAxisId,
    includeHidden: t.includeHidden,
    interval: t.interval,
    minTickGap: t.minTickGap,
    mirror: t.mirror,
    name: t.name,
    orientation: t.orientation,
    padding: t.padding,
    reversed: t.reversed,
    scale: t.scale,
    tick: t.tick,
    tickCount: t.tickCount,
    tickFormatter: t.tickFormatter,
    ticks: t.ticks,
    type: t.type,
    unit: t.unit,
    niceTicks: t.niceTicks
  }), /* @__PURE__ */ A.createElement(xj, t));
}, bg = /* @__PURE__ */ A.memo(Aj, gg);
bg.displayName = "XAxis";
var Oj = ["type"], Ej = ["dangerouslySetInnerHTML", "ticks", "scale"], Sj = ["id", "scale"];
function ju() {
  return ju = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, ju.apply(null, arguments);
}
function sv(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function fv(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? sv(Object(r), !0).forEach(function(n) {
      _j(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : sv(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function _j(e, t, r) {
  return (t = Pj(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function Pj(e) {
  var t = Ij(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Ij(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function $u(e, t) {
  if (e == null) return {};
  var r, n, i = kj(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function kj(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Cj(e) {
  var t = se(), r = Q(null), n = Qh(), i = e.type, a = $u(e, Oj), o = Qa(n, "yAxis", i), u = sr(() => {
    if (o != null)
      return fv(fv({}, a), {}, {
        type: o
      });
  }, [o, a]);
  return qe(() => {
    u != null && (r.current === null ? t(AM(u)) : r.current !== u && t(OM({
      prev: r.current,
      next: u
    })), r.current = u);
  }, [u, t]), qe(() => () => {
    r.current && (t(EM(r.current)), r.current = null);
  }, [t]), null;
}
function Tj(e) {
  var t = e.yAxisId, r = e.className, n = e.width, i = e.label, a = Q(null), o = Q(null), u = j(rl), l = Ze(), c = se(), s = "yAxis", f = j((y) => Zm(y, t)), d = j((y) => kP(y, t)), h = j((y) => Jm(y, s, t, l)), p = j((y) => ym(y, t));
  if (qe(() => {
    if (!(n !== "auto" || !f || lc(i) || /* @__PURE__ */ Pt(i) || p == null)) {
      var y = a.current;
      if (y) {
        var b = y.getCalculatedWidth();
        Math.round(f.width) !== Math.round(b) && c(SM({
          id: t,
          width: b
        }));
      }
    }
  }, [
    // The dependency on cartesianAxisRef.current is not needed because useLayoutEffect will run after every render.
    // The ref will be populated by then.
    // To re-run this effect when ticks change, we can depend on the ticks array from the store.
    h,
    f,
    c,
    i,
    t,
    n,
    p
  ]), f == null || d == null || p == null)
    return null;
  e.dangerouslySetInnerHTML, e.ticks, e.scale;
  var v = $u(e, Ej);
  p.id, p.scale;
  var m = $u(p, Sj);
  return /* @__PURE__ */ A.createElement(fc, ju({}, v, m, {
    ref: a,
    labelRef: o,
    x: d.x,
    y: d.y,
    tickTextProps: n === "auto" ? {
      width: void 0
    } : {
      width: n
    },
    width: f.width,
    height: f.height,
    className: ce("recharts-".concat(s, " ").concat(s), r),
    viewBox: u,
    ticks: h,
    axisType: s,
    axisId: t
  }));
}
var Mj = {
  allowDataOverflow: be.allowDataOverflow,
  allowDecimals: be.allowDecimals,
  allowDuplicatedCategory: be.allowDuplicatedCategory,
  angle: be.angle,
  axisLine: Bt.axisLine,
  hide: !1,
  includeHidden: be.includeHidden,
  interval: be.interval,
  label: !1,
  minTickGap: be.minTickGap,
  mirror: be.mirror,
  orientation: be.orientation,
  padding: be.padding,
  reversed: be.reversed,
  scale: be.scale,
  tick: be.tick,
  tickCount: be.tickCount,
  tickLine: Bt.tickLine,
  tickSize: Bt.tickSize,
  type: be.type,
  niceTicks: be.niceTicks,
  width: be.width,
  yAxisId: 0
}, Dj = (e) => {
  var t = st(e, Mj);
  return /* @__PURE__ */ A.createElement(A.Fragment, null, /* @__PURE__ */ A.createElement(Cj, {
    interval: t.interval,
    id: t.yAxisId,
    scale: t.scale,
    type: t.type,
    domain: t.domain,
    allowDataOverflow: t.allowDataOverflow,
    dataKey: t.dataKey,
    allowDuplicatedCategory: t.allowDuplicatedCategory,
    allowDecimals: t.allowDecimals,
    tickCount: t.tickCount,
    padding: t.padding,
    includeHidden: t.includeHidden,
    reversed: t.reversed,
    ticks: t.ticks,
    width: t.width,
    orientation: t.orientation,
    mirror: t.mirror,
    hide: t.hide,
    unit: t.unit,
    name: t.name,
    angle: t.angle,
    minTickGap: t.minTickGap,
    tick: t.tick,
    tickFormatter: t.tickFormatter,
    niceTicks: t.niceTicks
  }), /* @__PURE__ */ A.createElement(Tj, t));
}, xg = /* @__PURE__ */ A.memo(Dj, gg);
xg.displayName = "YAxis";
var Nj = (e, t) => t, pc = E([Nj, ne, Tp, ke, yy, Jt, ZI, Pe], ik);
function jj(e) {
  return "getBBox" in e.currentTarget && typeof e.currentTarget.getBBox == "function";
}
function mc(e) {
  var t = e.currentTarget.getBoundingClientRect(), r, n;
  if (jj(e)) {
    var i = e.currentTarget.getBBox();
    r = i.width > 0 ? t.width / i.width : 1, n = i.height > 0 ? t.height / i.height : 1;
  } else {
    var a = e.currentTarget;
    r = a.offsetWidth > 0 ? t.width / a.offsetWidth : 1, n = a.offsetHeight > 0 ? t.height / a.offsetHeight : 1;
  }
  var o = (u, l) => ({
    /*
     * Here it's important to use:
     * - event.clientX and event.clientY to get the mouse position relative to the viewport, including scroll.
     * - pageX and pageY are not used because they are relative to the whole document, and ignore scroll.
     * - rect.left and rect.top are used to get the position of the chart relative to the viewport.
     * - offsetX and offsetY are not used because they are relative to the offset parent
     *  which may or may not be the same as the clientX and clientY, depending on the position of the chart in the DOM
     *  and surrounding element styles. CSS position: relative, absolute, fixed, will change the offset parent.
     * - scaleX and scaleY are necessary for when the chart element is scaled using CSS `transform: scale(N)`.
     */
    relativeX: Math.round((u - t.left) / r),
    relativeY: Math.round((l - t.top) / n)
  });
  return "touches" in e ? Array.from(e.touches).map((u) => o(u.clientX, u.clientY)) : o(e.clientX, e.clientY);
}
var wg = nt("mouseClick"), Ag = ei();
Ag.startListening({
  actionCreator: wg,
  effect: (e, t) => {
    var r = e.payload, n = pc(t.getState(), mc(r));
    (n == null ? void 0 : n.activeIndex) != null && t.dispatch(KP({
      activeIndex: n.activeIndex,
      activeDataKey: void 0,
      activeCoordinate: n.activeCoordinate
    }));
  }
});
var Ru = nt("mouseMove"), Og = ei(), Yr = null, mr = null, Ko = null;
Og.startListening({
  actionCreator: Ru,
  effect: (e, t) => {
    var r = e.payload, n = t.getState(), i = n.eventSettings, a = i.throttleDelay, o = i.throttledEvents, u = o === "all" || (o == null ? void 0 : o.includes("mousemove"));
    Yr !== null && (cancelAnimationFrame(Yr), Yr = null), mr !== null && (typeof a != "number" || !u) && (clearTimeout(mr), mr = null), Ko = mc(r);
    var l = () => {
      var c = t.getState(), s = vi(c, c.tooltip.settings.shared);
      if (!Ko) {
        Yr = null, mr = null;
        return;
      }
      if (s === "axis") {
        var f = pc(c, Ko);
        (f == null ? void 0 : f.activeIndex) != null ? t.dispatch(uy({
          activeIndex: f.activeIndex,
          activeDataKey: void 0,
          activeCoordinate: f.activeCoordinate
        })) : t.dispatch(oy());
      }
      Yr = null, mr = null;
    };
    if (!u) {
      l();
      return;
    }
    a === "raf" ? Yr = requestAnimationFrame(l) : typeof a == "number" && mr === null && (mr = setTimeout(l, a));
  }
});
function $j(e, t) {
  return t instanceof HTMLElement ? "HTMLElement <".concat(t.tagName, ' class="').concat(t.className, '">') : t === window ? "global.window" : e === "children" && typeof t == "object" && t !== null ? "<<CHILDREN>>" : t;
}
var dv = {
  accessibilityLayer: !0,
  barCategoryGap: "10%",
  barGap: 4,
  barSize: void 0,
  className: void 0,
  maxBarSize: void 0,
  stackOffset: "none",
  syncId: void 0,
  syncMethod: "index",
  baseValue: void 0,
  reverseStackOrder: !1
}, Eg = Be({
  name: "rootProps",
  initialState: dv,
  reducers: {
    updateOptions: (e, t) => {
      var r;
      e.accessibilityLayer = t.payload.accessibilityLayer, e.barCategoryGap = t.payload.barCategoryGap, e.barGap = (r = t.payload.barGap) !== null && r !== void 0 ? r : dv.barGap, e.barSize = t.payload.barSize, e.maxBarSize = t.payload.maxBarSize, e.stackOffset = t.payload.stackOffset, e.syncId = t.payload.syncId, e.syncMethod = t.payload.syncMethod, e.className = t.payload.className, e.baseValue = t.payload.baseValue, e.reverseStackOrder = t.payload.reverseStackOrder;
    }
  }
}), Rj = Eg.reducer, Lj = Eg.actions.updateOptions, zj = null, Bj = {
  updatePolarOptions: (e, t) => e === null ? t.payload : (e.startAngle = t.payload.startAngle, e.endAngle = t.payload.endAngle, e.cx = t.payload.cx, e.cy = t.payload.cy, e.innerRadius = t.payload.innerRadius, e.outerRadius = t.payload.outerRadius, e)
}, Sg = Be({
  name: "polarOptions",
  initialState: zj,
  reducers: Bj
});
Sg.actions.updatePolarOptions;
var Fj = Sg.reducer, _g = nt("keyDown"), Pg = nt("focus"), Ig = nt("blur"), po = ei(), Gr = null, yr = null, Ni = null;
po.startListening({
  actionCreator: _g,
  effect: (e, t) => {
    Ni = e.payload, Gr !== null && (cancelAnimationFrame(Gr), Gr = null);
    var r = t.getState(), n = r.eventSettings, i = n.throttleDelay, a = n.throttledEvents, o = a === "all" || a.includes("keydown");
    yr !== null && (typeof i != "number" || !o) && (clearTimeout(yr), yr = null);
    var u = () => {
      try {
        var l = t.getState(), c = l.rootProps.accessibilityLayer !== !1;
        if (!c)
          return;
        var s = l.tooltip.keyboardInteraction, f = Ni;
        if (f !== "ArrowRight" && f !== "ArrowLeft" && f !== "Enter")
          return;
        var d = Rn(s, jr(l), an(l), ln(l)), h = d == null ? -1 : Number(d), p = !Number.isFinite(h) || h < 0, v = Jt(l), m = jr(l), y = vi(l, l.tooltip.settings.shared);
        if (f === "Enter") {
          if (p)
            return;
          var b = Aa(l, y, "hover", String(s.index));
          t.dispatch(wa({
            active: !s.active,
            activeIndex: s.index,
            activeCoordinate: b
          }));
          return;
        }
        var x = NP(l), w = x === "left-to-right" ? 1 : -1, O = f === "ArrowRight" ? 1 : -1, g;
        if (p) {
          var S = an(l), P = ln(l), I = O * w, C = (U) => ({
            active: !1,
            index: String(U),
            dataKey: void 0,
            graphicalItemId: void 0,
            coordinate: void 0
          });
          if (g = -1, I > 0) {
            for (var T = 0; T < m.length; T++)
              if (Rn(C(T), m, S, P) != null) {
                g = T;
                break;
              }
          } else
            for (var _ = m.length - 1; _ >= 0; _--)
              if (Rn(C(_), m, S, P) != null) {
                g = _;
                break;
              }
          if (g < 0)
            return;
        } else {
          g = h + O * w;
          var z = (v == null ? void 0 : v.length) || m.length;
          if (z === 0 || g >= z || g < 0)
            return;
        }
        var $ = Aa(l, y, "hover", String(g));
        t.dispatch(wa({
          active: !0,
          activeIndex: g.toString(),
          activeCoordinate: $
        }));
      } finally {
        Gr = null, yr = null;
      }
    };
    if (!o) {
      u();
      return;
    }
    i === "raf" ? Gr = requestAnimationFrame(u) : typeof i == "number" && yr === null && (u(), Ni = null, yr = setTimeout(() => {
      Ni ? u() : (yr = null, Gr = null);
    }, i));
  }
});
po.startListening({
  actionCreator: Pg,
  effect: (e, t) => {
    var r = t.getState(), n = r.rootProps.accessibilityLayer !== !1;
    if (n) {
      var i = r.tooltip.keyboardInteraction;
      if (!i.active && i.index == null) {
        var a = "0", o = vi(r, r.tooltip.settings.shared), u = Aa(r, o, "hover", String(a));
        t.dispatch(wa({
          active: !0,
          activeIndex: a,
          activeCoordinate: u
        }));
      }
    }
  }
});
po.startListening({
  actionCreator: Ig,
  effect: (e, t) => {
    var r = t.getState(), n = r.rootProps.accessibilityLayer !== !1;
    if (n) {
      var i = r.tooltip.keyboardInteraction;
      i.active && t.dispatch(wa({
        active: !1,
        activeIndex: i.index,
        activeCoordinate: i.coordinate
      }));
    }
  }
});
function kg(e) {
  e.persist();
  var t = e.currentTarget;
  return new Proxy(e, {
    get: (r, n) => {
      if (n === "currentTarget")
        return t;
      var i = Reflect.get(r, n);
      return typeof i == "function" ? i.bind(r) : i;
    }
  });
}
var at = nt("externalEvent"), Cg = ei(), ji = /* @__PURE__ */ new Map(), Mn = /* @__PURE__ */ new Map(), Ho = /* @__PURE__ */ new Map();
Cg.startListening({
  actionCreator: at,
  effect: (e, t) => {
    var r = e.payload, n = r.handler, i = r.reactEvent;
    if (n != null) {
      var a = i.type, o = kg(i);
      Ho.set(a, {
        handler: n,
        reactEvent: o
      });
      var u = ji.get(a);
      u !== void 0 && (cancelAnimationFrame(u), ji.delete(a));
      var l = t.getState(), c = l.eventSettings, s = c.throttleDelay, f = c.throttledEvents, d = f, h = d === "all" || (d == null ? void 0 : d.includes(a)), p = Mn.get(a);
      p !== void 0 && (typeof s != "number" || !h) && (clearTimeout(p), Mn.delete(a));
      var v = () => {
        var b = Ho.get(a);
        try {
          if (!b)
            return;
          var x = b.handler, w = b.reactEvent, O = t.getState(), g = {
            activeCoordinate: $I(O),
            activeDataKey: wy(O),
            activeIndex: cn(O),
            activeLabel: xy(O),
            activeTooltipIndex: cn(O),
            isTooltipActive: RI(O)
          };
          x && x(g, w);
        } finally {
          ji.delete(a), Mn.delete(a), Ho.delete(a);
        }
      };
      if (!h) {
        v();
        return;
      }
      if (s === "raf") {
        var m = requestAnimationFrame(v);
        ji.set(a, m);
      } else if (typeof s == "number") {
        if (!Mn.has(a)) {
          v();
          var y = setTimeout(v, s);
          Mn.set(a, y);
        }
      } else
        v();
    }
  }
});
var Wj = E([xn], (e) => e.tooltipItemPayloads), Uj = E([Wj, (e, t) => t, (e, t, r) => r], (e, t, r) => {
  if (t != null) {
    var n = e.find((a) => a.settings.graphicalItemId === r);
    if (n != null) {
      var i = n.getPosition;
      if (i != null)
        return i(t);
    }
  }
}), Tg = nt("touchMove"), Mg = ei(), gr = null, tr = null, vv = null, Dn = null;
Mg.startListening({
  actionCreator: Tg,
  effect: (e, t) => {
    var r = e.payload;
    if (!(r.touches == null || r.touches.length === 0)) {
      Dn = kg(r);
      var n = t.getState(), i = n.eventSettings, a = i.throttleDelay, o = i.throttledEvents, u = o === "all" || o.includes("touchmove");
      gr !== null && (cancelAnimationFrame(gr), gr = null), tr !== null && (typeof a != "number" || !u) && (clearTimeout(tr), tr = null), vv = Array.from(r.touches).map((c) => mc({
        clientX: c.clientX,
        clientY: c.clientY,
        currentTarget: r.currentTarget
      }));
      var l = () => {
        if (Dn != null) {
          var c = t.getState(), s = vi(c, c.tooltip.settings.shared);
          if (s === "axis") {
            var f, d = (f = vv) === null || f === void 0 ? void 0 : f[0];
            if (d == null) {
              gr = null, tr = null;
              return;
            }
            var h = pc(c, d);
            (h == null ? void 0 : h.activeIndex) != null && t.dispatch(uy({
              activeIndex: h.activeIndex,
              activeDataKey: void 0,
              activeCoordinate: h.activeCoordinate
            }));
          } else if (s === "item") {
            var p, v = Dn.touches[0];
            if (document.elementFromPoint == null || v == null)
              return;
            var m = document.elementFromPoint(v.clientX, v.clientY);
            if (!m || !m.getAttribute)
              return;
            var y = m.getAttribute(kw), b = (p = m.getAttribute(Cw)) !== null && p !== void 0 ? p : void 0, x = zr(c).find((g) => g.id === b);
            if (y == null || x == null || b == null)
              return;
            var w = x.dataKey, O = Uj(c, y, b);
            t.dispatch(ay({
              activeDataKey: w,
              activeIndex: y,
              activeCoordinate: O,
              activeGraphicalItemId: b
            }));
          }
          gr = null, tr = null;
        }
      };
      if (!u) {
        l();
        return;
      }
      a === "raf" ? gr = requestAnimationFrame(l) : typeof a == "number" && tr === null && (l(), Dn = null, tr = setTimeout(() => {
        Dn ? l() : (tr = null, gr = null);
      }, a));
    }
  }
});
var Dg = {
  throttleDelay: "raf",
  throttledEvents: ["mousemove", "touchmove", "pointermove", "scroll", "wheel"]
}, Ng = Be({
  name: "eventSettings",
  initialState: Dg,
  reducers: {
    setEventSettings: (e, t) => {
      t.payload.throttleDelay != null && (e.throttleDelay = t.payload.throttleDelay), t.payload.throttledEvents != null && (e.throttledEvents = q(t.payload.throttledEvents));
    }
  }
}), Vj = Ng.actions.setEventSettings, Kj = Ng.reducer, Hj = ch({
  brush: VM,
  cartesianAxis: _M,
  chartData: Nk,
  errorBars: QD,
  eventSettings: Kj,
  graphicalItems: dM,
  layout: fw,
  legend: mA,
  options: kk,
  polarAxis: gT,
  polarOptions: Fj,
  referenceElements: GM,
  renderedTicks: mD,
  rootProps: Rj,
  tooltip: HP,
  zIndex: yk
}), Yj = function(t) {
  var r = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : "Chart";
  return $x({
    reducer: Hj,
    // redux-toolkit v1 types are unhappy with the preloadedState type. Remove the `as any` when bumping to v2
    preloadedState: t,
    // @ts-expect-error redux-toolkit v1 types are unhappy with the middleware array. Remove this comment when bumping to v2
    middleware: (n) => {
      var i;
      return n({
        serializableCheck: !1,
        immutableCheck: !["commonjs", "es6", "production"].includes((i = "es6") !== null && i !== void 0 ? i : "")
      }).concat([Ag.middleware, Og.middleware, po.middleware, Cg.middleware, Mg.middleware]);
    },
    /*
     * I can't find out how to satisfy typescript here.
     * We return `EnhancerArray<[StoreEnhancer<{}, {}>, StoreEnhancer]>` from this function,
     * but the types say we should return `EnhancerArray<StoreEnhancer<{}, {}>`.
     * Looks like it's badly inferred generics, but it won't allow me to provide the correct type manually either.
     * So let's just ignore the error for now.
     */
    // @ts-expect-error mismatched generics
    enhancers: (n) => {
      var i = n;
      return typeof n == "function" && (i = n()), i.concat(Ih({
        type: "raf"
      }));
    },
    devTools: {
      serialize: {
        replacer: $j
      },
      name: "recharts-".concat(r)
    }
  });
};
function Gj(e) {
  var t = e.preloadedState, r = e.children, n = e.reduxStoreName, i = Ze(), a = Q(null);
  if (i)
    return r;
  a.current == null && (a.current = Yj(t, n));
  var o = qu;
  return /* @__PURE__ */ A.createElement(MA, {
    context: o,
    store: a.current
  }, r);
}
function qj(e) {
  var t = e.layout, r = e.margin, n = se(), i = Ze();
  return ve(() => {
    i || (n(lw(t)), n(uw(r)));
  }, [n, i, t, r]), null;
}
var Xj = /* @__PURE__ */ Bu(qj, Ga);
function Zj(e) {
  var t = se();
  return ve(() => {
    t(Lj(e));
  }, [t, e]), null;
}
var Qj = (e) => {
  var t = se();
  return ve(() => {
    t(Vj(e));
  }, [t, e]), null;
}, Jj = /* @__PURE__ */ Bu(Qj, Ga);
function hv(e) {
  var t = e.zIndex, r = e.isPanorama, n = Q(null), i = se();
  return qe(() => (n.current && i(pk({
    zIndex: t,
    element: n.current,
    isPanorama: r
  })), () => {
    i(mk({
      zIndex: t,
      isPanorama: r
    }));
  }), [i, t, r]), /* @__PURE__ */ A.createElement("g", {
    tabIndex: -1,
    ref: n,
    className: "recharts-zIndex-layer_".concat(t)
  });
}
function pv(e) {
  var t = e.children, r = e.isPanorama, n = j(ok);
  if (!n || n.length === 0)
    return t;
  var i = n.filter((o) => o < 0), a = n.filter((o) => o > 0);
  return /* @__PURE__ */ A.createElement(A.Fragment, null, i.map((o) => /* @__PURE__ */ A.createElement(hv, {
    key: o,
    zIndex: o,
    isPanorama: r
  })), t, a.map((o) => /* @__PURE__ */ A.createElement(hv, {
    key: o,
    zIndex: o,
    isPanorama: r
  })));
}
var e$ = ["children"];
function t$(e, t) {
  if (e == null) return {};
  var r, n, i = r$(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function r$(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Pa() {
  return Pa = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Pa.apply(null, arguments);
}
var n$ = {
  width: "100%",
  height: "100%",
  /*
   * display: block is necessary here because the default for an SVG is display: inline,
   * which in some browsers (Chrome) adds a little bit of extra space above and below the SVG
   * to make space for the descender of letters like "g" and "y". This throws off the height calculation
   * and causes the container to grow indefinitely on each render with responsive=true.
   * Display: block removes that extra space.
   *
   * Interestingly, Firefox does not have this problem, but it doesn't hurt to add the style anyway.
   */
  display: "block"
}, i$ = /* @__PURE__ */ ze((e, t) => {
  var r = Xh(), n = Zh(), i = rp();
  if (!Dt(r) || !Dt(n))
    return null;
  var a = e.children, o = e.otherAttributes, u = e.title, l = e.desc, c, s;
  return o != null && (typeof o.tabIndex == "number" ? c = o.tabIndex : c = i ? 0 : void 0, typeof o.role == "string" ? s = o.role : s = i ? "application" : void 0), /* @__PURE__ */ A.createElement(Pv, Pa({}, o, {
    title: u,
    desc: l,
    role: s,
    tabIndex: c,
    width: r,
    height: n,
    style: n$,
    ref: t
  }), a);
}), a$ = (e) => {
  var t = e.children, r = j(Ha);
  if (!r)
    return null;
  var n = r.width, i = r.height, a = r.y, o = r.x;
  return /* @__PURE__ */ A.createElement(Pv, {
    width: n,
    height: i,
    x: o,
    y: a
  }, t);
}, mv = /* @__PURE__ */ ze((e, t) => {
  var r = e.children, n = t$(e, e$), i = Ze();
  return i ? /* @__PURE__ */ A.createElement(a$, null, /* @__PURE__ */ A.createElement(pv, {
    isPanorama: !0
  }, r)) : /* @__PURE__ */ A.createElement(i$, Pa({
    ref: t
  }, n), /* @__PURE__ */ A.createElement(pv, {
    isPanorama: !1
  }, r));
});
function o$(e, t) {
  return s$(e) || c$(e, t) || l$(e, t) || u$();
}
function u$() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function l$(e, t) {
  if (e) {
    if (typeof e == "string") return yv(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? yv(e, t) : void 0;
  }
}
function yv(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function c$(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function s$(e) {
  if (Array.isArray(e)) return e;
}
function f$() {
  var e = se(), t = _e(null), r = o$(t, 2), n = r[0], i = r[1], a = j(Iw);
  return ve(() => {
    if (n != null) {
      var o = n.getBoundingClientRect(), u = o.width / n.offsetWidth;
      Y(u) && u !== a && e(sw(u));
    }
  }, [n, e, a]), i;
}
function gv(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function d$(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? gv(Object(r), !0).forEach(function(n) {
      v$(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : gv(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function v$(e, t, r) {
  return (t = h$(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function h$(e) {
  var t = p$(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function p$(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function lr() {
  return lr = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, lr.apply(null, arguments);
}
function Ia(e, t) {
  return b$(e) || g$(e, t) || y$(e, t) || m$();
}
function m$() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function y$(e, t) {
  if (e) {
    if (typeof e == "string") return bv(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? bv(e, t) : void 0;
  }
}
function bv(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function g$(e, t) {
  var r = e == null ? null : typeof Symbol < "u" && e[Symbol.iterator] || e["@@iterator"];
  if (r != null) {
    var n, i, a, o, u = [], l = !0, c = !1;
    try {
      if (a = (r = r.call(e)).next, t !== 0) for (; !(l = (n = a.call(r)).done) && (u.push(n.value), u.length !== t); l = !0) ;
    } catch (s) {
      c = !0, i = s;
    } finally {
      try {
        if (!l && r.return != null && (o = r.return(), Object(o) !== o)) return;
      } finally {
        if (c) throw i;
      }
    }
    return u;
  }
}
function b$(e) {
  if (Array.isArray(e)) return e;
}
var x$ = () => (Uk(), null);
function ka(e) {
  if (typeof e == "number")
    return e;
  if (typeof e == "string") {
    var t = parseFloat(e);
    if (!Number.isNaN(t))
      return t;
  }
  return 0;
}
var w$ = /* @__PURE__ */ ze((e, t) => {
  var r, n, i = Q(null), a = _e({
    containerWidth: ka((r = e.style) === null || r === void 0 ? void 0 : r.width),
    containerHeight: ka((n = e.style) === null || n === void 0 ? void 0 : n.height)
  }), o = Ia(a, 2), u = o[0], l = o[1], c = ie((f, d) => {
    l((h) => {
      var p = Math.round(f), v = Math.round(d);
      return h.containerWidth === p && h.containerHeight === v ? h : {
        containerWidth: p,
        containerHeight: v
      };
    });
  }, []), s = ie((f) => {
    if (typeof t == "function" && t(f), i.current != null && (i.current.disconnect(), i.current = null), f != null && typeof ResizeObserver < "u") {
      var d = f.getBoundingClientRect(), h = d.width, p = d.height;
      c(h, p);
      var v = (y) => {
        var b = y[0];
        if (b != null) {
          var x = b.contentRect, w = x.width, O = x.height;
          c(w, O);
        }
      }, m = new ResizeObserver(v);
      m.observe(f), i.current = m;
    }
  }, [t, c]);
  return ve(() => () => {
    var f = i.current;
    f != null && f.disconnect();
  }, [c]), /* @__PURE__ */ A.createElement(A.Fragment, null, /* @__PURE__ */ A.createElement(ri, {
    width: u.containerWidth,
    height: u.containerHeight
  }), /* @__PURE__ */ A.createElement("div", lr({
    ref: s
  }, e)));
}), A$ = /* @__PURE__ */ ze((e, t) => {
  var r = e.width, n = e.height, i = _e({
    containerWidth: ka(r),
    containerHeight: ka(n)
  }), a = Ia(i, 2), o = a[0], u = a[1], l = ie((s, f) => {
    u((d) => {
      var h = Math.round(s), p = Math.round(f);
      return d.containerWidth === h && d.containerHeight === p ? d : {
        containerWidth: h,
        containerHeight: p
      };
    });
  }, []), c = ie((s) => {
    if (typeof t == "function" && t(s), s != null) {
      var f = s.getBoundingClientRect(), d = f.width, h = f.height;
      l(d, h);
    }
  }, [t, l]);
  return /* @__PURE__ */ A.createElement(A.Fragment, null, /* @__PURE__ */ A.createElement(ri, {
    width: o.containerWidth,
    height: o.containerHeight
  }), /* @__PURE__ */ A.createElement("div", lr({
    ref: c
  }, e)));
}), O$ = /* @__PURE__ */ ze((e, t) => {
  var r = e.width, n = e.height;
  return /* @__PURE__ */ A.createElement(A.Fragment, null, /* @__PURE__ */ A.createElement(ri, {
    width: r,
    height: n
  }), /* @__PURE__ */ A.createElement("div", lr({
    ref: t
  }, e)));
}), E$ = /* @__PURE__ */ ze((e, t) => {
  var r = e.width, n = e.height;
  return typeof r == "string" || typeof n == "string" ? /* @__PURE__ */ A.createElement(A$, lr({}, e, {
    ref: t
  })) : typeof r == "number" && typeof n == "number" ? /* @__PURE__ */ A.createElement(O$, lr({}, e, {
    width: r,
    height: n,
    ref: t
  })) : /* @__PURE__ */ A.createElement(A.Fragment, null, /* @__PURE__ */ A.createElement(ri, {
    width: r,
    height: n
  }), /* @__PURE__ */ A.createElement("div", lr({
    ref: t
  }, e)));
});
function S$(e) {
  return e ? w$ : E$;
}
var _$ = /* @__PURE__ */ ze((e, t) => {
  var r = e.children, n = e.className, i = e.height, a = e.onClick, o = e.onContextMenu, u = e.onDoubleClick, l = e.onMouseDown, c = e.onMouseEnter, s = e.onMouseLeave, f = e.onMouseMove, d = e.onMouseUp, h = e.onTouchEnd, p = e.onTouchMove, v = e.onTouchStart, m = e.style, y = e.width, b = e.responsive, x = e.dispatchTouchEvents, w = x === void 0 ? !0 : x, O = Q(null), g = se(), S = _e(null), P = Ia(S, 2), I = P[0], C = P[1], T = _e(null), _ = Ia(T, 2), z = _[0], $ = _[1], U = f$(), V = nl(), K = (V == null ? void 0 : V.width) > 0 ? V.width : y, L = (V == null ? void 0 : V.height) > 0 ? V.height : i, G = ie((R) => {
    U(R), typeof t == "function" && t(R), C(R), $(R), R != null && (O.current = R);
  }, [U, t, C, $]), F = ie((R) => {
    g(wg(R)), g(at({
      handler: a,
      reactEvent: R
    }));
  }, [g, a]), he = ie((R) => {
    g(Ru(R)), g(at({
      handler: c,
      reactEvent: R
    }));
  }, [g, c]), pe = ie((R) => {
    g(oy()), g(at({
      handler: s,
      reactEvent: R
    }));
  }, [g, s]), le = ie((R) => {
    g(Ru(R)), g(at({
      handler: f,
      reactEvent: R
    }));
  }, [g, f]), We = ie(() => {
    g(Pg());
  }, [g]), Qe = ie(() => {
    g(Ig());
  }, [g]), vt = ie((R) => {
    g(_g(R.key));
  }, [g]), ht = ie((R) => {
    g(at({
      handler: o,
      reactEvent: R
    }));
  }, [g, o]), On = ie((R) => {
    g(at({
      handler: u,
      reactEvent: R
    }));
  }, [g, u]), M = ie((R) => {
    g(at({
      handler: l,
      reactEvent: R
    }));
  }, [g, l]), W = ie((R) => {
    g(at({
      handler: d,
      reactEvent: R
    }));
  }, [g, d]), B = ie((R) => {
    g(at({
      handler: v,
      reactEvent: R
    }));
  }, [g, v]), k = ie((R) => {
    w && g(Tg(R)), g(at({
      handler: p,
      reactEvent: R
    }));
  }, [g, w, p]), je = ie((R) => {
    g(at({
      handler: h,
      reactEvent: R
    }));
  }, [g, h]), J = S$(b);
  return /* @__PURE__ */ A.createElement(Iy.Provider, {
    value: I
  }, /* @__PURE__ */ A.createElement(Qg.Provider, {
    value: z
  }, /* @__PURE__ */ A.createElement(J, {
    width: K ?? (m == null ? void 0 : m.width),
    height: L ?? (m == null ? void 0 : m.height),
    className: ce("recharts-wrapper", n),
    style: d$({
      position: "relative",
      cursor: "default",
      width: K,
      height: L
    }, m),
    onClick: F,
    onContextMenu: ht,
    onDoubleClick: On,
    onFocus: We,
    onBlur: Qe,
    onKeyDown: vt,
    onMouseDown: M,
    onMouseEnter: he,
    onMouseLeave: pe,
    onMouseMove: le,
    onMouseUp: W,
    onTouchEnd: je,
    onTouchMove: k,
    onTouchStart: B,
    ref: G
  }, /* @__PURE__ */ A.createElement(x$, null), r)));
}), P$ = ["width", "height", "responsive", "children", "className", "style", "compact", "title", "desc"];
function I$(e, t) {
  if (e == null) return {};
  var r, n, i = k$(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function k$(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var C$ = /* @__PURE__ */ ze((e, t) => {
  var r = e.width, n = e.height, i = e.responsive, a = e.children, o = e.className, u = e.style, l = e.compact, c = e.title, s = e.desc, f = I$(e, P$), d = Ft(f);
  return l ? /* @__PURE__ */ A.createElement(A.Fragment, null, /* @__PURE__ */ A.createElement(ri, {
    width: r,
    height: n
  }), /* @__PURE__ */ A.createElement(mv, {
    otherAttributes: d,
    title: c,
    desc: s
  }, a)) : /* @__PURE__ */ A.createElement(_$, {
    className: o,
    style: u,
    width: r,
    height: n,
    responsive: i ?? !1,
    onClick: e.onClick,
    onMouseLeave: e.onMouseLeave,
    onMouseEnter: e.onMouseEnter,
    onMouseMove: e.onMouseMove,
    onMouseDown: e.onMouseDown,
    onMouseUp: e.onMouseUp,
    onContextMenu: e.onContextMenu,
    onDoubleClick: e.onDoubleClick,
    onTouchStart: e.onTouchStart,
    onTouchMove: e.onTouchMove,
    onTouchEnd: e.onTouchEnd
  }, /* @__PURE__ */ A.createElement(mv, {
    otherAttributes: d,
    title: c,
    desc: s,
    ref: t
  }, /* @__PURE__ */ A.createElement(tD, null, a)));
});
function Lu() {
  return Lu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Lu.apply(null, arguments);
}
function xv(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function T$(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? xv(Object(r), !0).forEach(function(n) {
      M$(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : xv(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function M$(e, t, r) {
  return (t = D$(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function D$(e) {
  var t = N$(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function N$(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var j$ = {
  top: 5,
  right: 5,
  bottom: 5,
  left: 5
}, $$ = T$({
  accessibilityLayer: !0,
  barCategoryGap: "10%",
  barGap: 4,
  layout: "horizontal",
  margin: j$,
  responsive: !1,
  reverseStackOrder: !1,
  stackOffset: "none",
  syncMethod: "index"
}, Dg), R$ = /* @__PURE__ */ ze(function(t, r) {
  var n, i = st(t.categoricalChartProps, $$), a = t.chartName, o = t.defaultTooltipEventType, u = t.validateTooltipEventTypes, l = t.tooltipPayloadSearcher, c = t.categoricalChartProps, s = {
    chartName: a,
    defaultTooltipEventType: o,
    validateTooltipEventTypes: u,
    tooltipPayloadSearcher: l,
    eventEmitter: void 0
  };
  return /* @__PURE__ */ A.createElement(Gj, {
    preloadedState: {
      options: s
    },
    reduxStoreName: (n = c.id) !== null && n !== void 0 ? n : a
  }, /* @__PURE__ */ A.createElement(UM, {
    chartData: c.data
  }), /* @__PURE__ */ A.createElement(Xj, {
    layout: i.layout,
    margin: i.margin
  }), /* @__PURE__ */ A.createElement(Jj, {
    throttleDelay: i.throttleDelay,
    throttledEvents: i.throttledEvents
  }), /* @__PURE__ */ A.createElement(Zj, {
    baseValue: i.baseValue,
    accessibilityLayer: i.accessibilityLayer,
    barCategoryGap: i.barCategoryGap,
    maxBarSize: i.maxBarSize,
    stackOffset: i.stackOffset,
    barGap: i.barGap,
    barSize: i.barSize,
    syncId: i.syncId,
    syncMethod: i.syncMethod,
    className: i.className,
    reverseStackOrder: i.reverseStackOrder
  }), /* @__PURE__ */ A.createElement(C$, Lu({}, i, {
    ref: r
  })));
}), L$ = ["axis", "item"], z$ = /* @__PURE__ */ ze((e, t) => /* @__PURE__ */ A.createElement(R$, {
  chartName: "BarChart",
  defaultTooltipEventType: "axis",
  validateTooltipEventTypes: L$,
  tooltipPayloadSearcher: Pk,
  categoricalChartProps: e,
  ref: t
}));
let zu = null;
function B$(e) {
  zu = e;
}
const F$ = {
  call: (e, t) => {
    if (!zu) throw new Error("bridge not set — mount was not called with a bridge");
    return zu.call(e, t);
  }
};
function W$() {
  const [e, t] = _e(null), [r, n] = _e(null);
  ve(() => {
    F$.call("federation.query", {
      source: "demo-buildings",
      sql: "SELECT s.name AS site, ROUND(SUM(pr.value),1) AS kwh FROM point_reading pr JOIN point p ON p.id = pr.point_id JOIN meter m ON m.id = p.meter_id JOIN site s ON s.id = m.site_id WHERE p.name LIKE '%Energy kWh%' GROUP BY s.name ORDER BY kwh DESC LIMIT 8"
    }).then((o) => {
      const u = (o.rows ?? []).map((l) => ({
        site: String(l[0]),
        kwh: Number(l[1])
      }));
      t(u);
    }).catch((o) => n(o == null ? String(o) : o.message ?? String(o)));
  }, []);
  const i = e ? e.reduce((a, o) => a + o.kwh, 0) : 0;
  return /* @__PURE__ */ it("section", { style: { padding: 24, display: "flex", flexDirection: "column", gap: 16 }, children: [
    /* @__PURE__ */ it("header", { children: [
      /* @__PURE__ */ me("h2", { style: { margin: 0, fontSize: 18, fontWeight: 600, color: "hsl(var(--lbx-fg))" }, children: "Energy dashboard" }),
      /* @__PURE__ */ it("p", { style: { margin: "4px 0 0", fontSize: 13, color: "hsl(var(--lbx-muted))" }, children: [
        "Total kWh by site — live from ",
        /* @__PURE__ */ me("code", { children: "demo-buildings" }),
        " via federation.query."
      ] })
    ] }),
    r && /* @__PURE__ */ it("div", { style: { color: "hsl(var(--lbx-destructive))", fontSize: 13 }, children: [
      "query failed: ",
      r
    ] }),
    e && /* @__PURE__ */ it(Rg, { children: [
      /* @__PURE__ */ it(
        "div",
        {
          style: {
            background: "hsl(var(--lbx-card))",
            border: "1px solid hsl(var(--lbx-border))",
            borderRadius: "calc(var(--lbx-radius))",
            padding: 16
          },
          children: [
            /* @__PURE__ */ me("h3", { style: { margin: "0 0 12px", fontSize: 14, fontWeight: 500, color: "hsl(var(--lbx-card-foreground))" }, children: "kWh by site" }),
            /* @__PURE__ */ me("div", { style: { height: 280, width: "100%" }, children: /* @__PURE__ */ me(cA, { children: /* @__PURE__ */ it(z$, { data: e, margin: { top: 8, right: 8, bottom: 8, left: 8 }, children: [
              /* @__PURE__ */ me(sg, { strokeDasharray: "3 3", stroke: "hsl(var(--lbx-border))" }),
              /* @__PURE__ */ me(
                bg,
                {
                  dataKey: "site",
                  stroke: "hsl(var(--lbx-muted))",
                  tickLine: !1,
                  axisLine: !1,
                  fontSize: 11,
                  angle: -20,
                  textAnchor: "end",
                  height: 70,
                  interval: 0
                }
              ),
              /* @__PURE__ */ me(
                xg,
                {
                  stroke: "hsl(var(--lbx-muted))",
                  tickLine: !1,
                  axisLine: !1,
                  fontSize: 12
                }
              ),
              /* @__PURE__ */ me(
                rC,
                {
                  contentStyle: {
                    background: "hsl(var(--lbx-card))",
                    border: "1px solid hsl(var(--lbx-border))",
                    borderRadius: "calc(var(--lbx-radius))",
                    color: "hsl(var(--lbx-fg))",
                    fontSize: 12
                  },
                  cursor: { fill: "hsl(var(--lbx-muted) / 0.15)" }
                }
              ),
              /* @__PURE__ */ me(yg, { dataKey: "kwh", fill: "hsl(var(--lbx-chart-1))", radius: [4, 4, 0, 0] })
            ] }) }) })
          ]
        }
      ),
      /* @__PURE__ */ it(
        "div",
        {
          style: {
            background: "hsl(var(--lbx-card))",
            border: "1px solid hsl(var(--lbx-border))",
            borderRadius: "calc(var(--lbx-radius))",
            padding: 16
          },
          children: [
            /* @__PURE__ */ it("h3", { style: { margin: "0 0 12px", fontSize: 14, fontWeight: 500, color: "hsl(var(--lbx-card-foreground))" }, children: [
              "Sites — total ",
              i.toLocaleString(void 0, { maximumFractionDigits: 1 }),
              " kWh"
            ] }),
            /* @__PURE__ */ it("table", { style: { width: "100%", borderCollapse: "collapse", fontSize: 13 }, children: [
              /* @__PURE__ */ me("thead", { children: /* @__PURE__ */ it("tr", { style: { color: "hsl(var(--lbx-muted))", textAlign: "left" }, children: [
                /* @__PURE__ */ me("th", { style: { padding: "6px 8px", borderBottom: "1px solid hsl(var(--lbx-border))" }, children: "Site" }),
                /* @__PURE__ */ me("th", { style: { padding: "6px 8px", borderBottom: "1px solid hsl(var(--lbx-border))", textAlign: "right" }, children: "kWh" })
              ] }) }),
              /* @__PURE__ */ me("tbody", { children: e.map((a) => /* @__PURE__ */ it("tr", { style: { color: "hsl(var(--lbx-card-foreground))" }, children: [
                /* @__PURE__ */ me("td", { style: { padding: "6px 8px", borderBottom: "1px solid hsl(var(--lbx-border))" }, children: a.site }),
                /* @__PURE__ */ me("td", { style: { padding: "6px 8px", borderBottom: "1px solid hsl(var(--lbx-border))", textAlign: "right" }, children: a.kwh.toLocaleString() })
              ] }, a.site)) })
            ] })
          ]
        }
      )
    ] }),
    !e && !r && /* @__PURE__ */ me("div", { style: { color: "hsl(var(--lbx-muted))", fontSize: 13 }, children: "loading…" })
  ] });
}
function Y$(e, t, r) {
  B$(r);
  const n = Yo(e);
  return n.render(
    /* @__PURE__ */ me(Bg, { children: /* @__PURE__ */ me("div", { className: "lbx-energy-dashboard", children: /* @__PURE__ */ me(W$, {}) }) })
  ), () => n.unmount();
}
export {
  Y$ as mount
};
