var vg = Object.defineProperty;
var pg = (e, t, r) => t in e ? vg(e, t, { enumerable: !0, configurable: !0, writable: !0, value: r }) : e[t] = r;
var si = (e, t, r) => pg(e, typeof t != "symbol" ? t + "" : t, r);
import { jsx as oe, jsxs as bt } from "react/jsx-runtime";
import * as w from "react";
import tn, { isValidElement as Ze, forwardRef as Me, createContext as Je, useContext as kt, useMemo as Kt, useState as fe, useRef as V, useCallback as ee, useEffect as he, useImperativeHandle as ah, useLayoutEffect as Ue, cloneElement as rn, createElement as oh, memo as ju, Component as mg, StrictMode as yg } from "react";
import gg, { createPortal as uh } from "react-dom";
function bg(e) {
  return e && e.__esModule && Object.prototype.hasOwnProperty.call(e, "default") ? e.default : e;
}
var Fo, fi = gg;
if (process.env.NODE_ENV === "production")
  Fo = fi.createRoot, fi.hydrateRoot;
else {
  var cc = fi.__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED;
  Fo = function(e, t) {
    cc.usingClientEntryPoint = !0;
    try {
      return fi.createRoot(e, t);
    } finally {
      cc.usingClientEntryPoint = !1;
    }
  };
}
function lh(e) {
  var t, r, n = "";
  if (typeof e == "string" || typeof e == "number") n += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var i = e.length;
    for (t = 0; t < i; t++) e[t] && (r = lh(e[t])) && (n && (n += " "), n += r);
  } else for (r in e) e[r] && (n && (n += " "), n += r);
  return n;
}
function ie() {
  for (var e, t, r = 0, n = "", i = arguments.length; r < i; r++) (e = arguments[r]) && (t = lh(e)) && (n && (n += " "), n += t);
  return n;
}
var wg = ["dangerouslySetInnerHTML", "onCopy", "onCopyCapture", "onCut", "onCutCapture", "onPaste", "onPasteCapture", "onCompositionEnd", "onCompositionEndCapture", "onCompositionStart", "onCompositionStartCapture", "onCompositionUpdate", "onCompositionUpdateCapture", "onFocus", "onFocusCapture", "onBlur", "onBlurCapture", "onChange", "onChangeCapture", "onBeforeInput", "onBeforeInputCapture", "onInput", "onInputCapture", "onReset", "onResetCapture", "onSubmit", "onSubmitCapture", "onInvalid", "onInvalidCapture", "onLoad", "onLoadCapture", "onError", "onErrorCapture", "onKeyDown", "onKeyDownCapture", "onKeyPress", "onKeyPressCapture", "onKeyUp", "onKeyUpCapture", "onAbort", "onAbortCapture", "onCanPlay", "onCanPlayCapture", "onCanPlayThrough", "onCanPlayThroughCapture", "onDurationChange", "onDurationChangeCapture", "onEmptied", "onEmptiedCapture", "onEncrypted", "onEncryptedCapture", "onEnded", "onEndedCapture", "onLoadedData", "onLoadedDataCapture", "onLoadedMetadata", "onLoadedMetadataCapture", "onLoadStart", "onLoadStartCapture", "onPause", "onPauseCapture", "onPlay", "onPlayCapture", "onPlaying", "onPlayingCapture", "onProgress", "onProgressCapture", "onRateChange", "onRateChangeCapture", "onSeeked", "onSeekedCapture", "onSeeking", "onSeekingCapture", "onStalled", "onStalledCapture", "onSuspend", "onSuspendCapture", "onTimeUpdate", "onTimeUpdateCapture", "onVolumeChange", "onVolumeChangeCapture", "onWaiting", "onWaitingCapture", "onAuxClick", "onAuxClickCapture", "onClick", "onClickCapture", "onContextMenu", "onContextMenuCapture", "onDoubleClick", "onDoubleClickCapture", "onDrag", "onDragCapture", "onDragEnd", "onDragEndCapture", "onDragEnter", "onDragEnterCapture", "onDragExit", "onDragExitCapture", "onDragLeave", "onDragLeaveCapture", "onDragOver", "onDragOverCapture", "onDragStart", "onDragStartCapture", "onDrop", "onDropCapture", "onMouseDown", "onMouseDownCapture", "onMouseEnter", "onMouseLeave", "onMouseMove", "onMouseMoveCapture", "onMouseOut", "onMouseOutCapture", "onMouseOver", "onMouseOverCapture", "onMouseUp", "onMouseUpCapture", "onSelect", "onSelectCapture", "onTouchCancel", "onTouchCancelCapture", "onTouchEnd", "onTouchEndCapture", "onTouchMove", "onTouchMoveCapture", "onTouchStart", "onTouchStartCapture", "onPointerDown", "onPointerDownCapture", "onPointerMove", "onPointerMoveCapture", "onPointerUp", "onPointerUpCapture", "onPointerCancel", "onPointerCancelCapture", "onPointerEnter", "onPointerEnterCapture", "onPointerLeave", "onPointerLeaveCapture", "onPointerOver", "onPointerOverCapture", "onPointerOut", "onPointerOutCapture", "onGotPointerCapture", "onGotPointerCaptureCapture", "onLostPointerCapture", "onLostPointerCaptureCapture", "onScroll", "onScrollCapture", "onWheel", "onWheelCapture", "onAnimationStart", "onAnimationStartCapture", "onAnimationEnd", "onAnimationEndCapture", "onAnimationIteration", "onAnimationIterationCapture", "onTransitionEnd", "onTransitionEndCapture"];
function Nu(e) {
  if (typeof e != "string")
    return !1;
  var t = wg;
  return t.includes(e);
}
var xg = [
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
], Og = new Set(xg);
function ch(e) {
  return typeof e != "string" ? !1 : Og.has(e);
}
function sh(e) {
  return typeof e == "string" && e.startsWith("data-");
}
function Et(e) {
  if (typeof e != "object" || e === null)
    return {};
  var t = {};
  for (var r in e)
    Object.prototype.hasOwnProperty.call(e, r) && (ch(r) || sh(r)) && (t[r] = e[r]);
  return t;
}
function Ea(e) {
  if (e == null)
    return null;
  if (/* @__PURE__ */ Ze(e) && typeof e.props == "object" && e.props !== null) {
    var t = e.props;
    return Et(t);
  }
  return typeof e == "object" && !Array.isArray(e) ? Et(e) : null;
}
function at(e) {
  var t = {};
  for (var r in e)
    Object.prototype.hasOwnProperty.call(e, r) && (ch(r) || sh(r) || Nu(r)) && (t[r] = e[r]);
  return t;
}
function Ag(e) {
  return e == null ? null : /* @__PURE__ */ Ze(e) ? at(e.props) : typeof e == "object" && !Array.isArray(e) ? at(e) : null;
}
var Sg = ["children", "width", "height", "viewBox", "className", "style", "title", "desc"];
function Wo() {
  return Wo = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Wo.apply(null, arguments);
}
function Eg(e, t) {
  if (e == null) return {};
  var r, n, i = Pg(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function Pg(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var fh = /* @__PURE__ */ Me((e, t) => {
  var r = e.children, n = e.width, i = e.height, a = e.viewBox, o = e.className, u = e.style, l = e.title, c = e.desc, s = Eg(e, Sg), f = a || {
    width: n,
    height: i,
    x: 0,
    y: 0
  }, d = ie("recharts-surface", o);
  return /* @__PURE__ */ w.createElement("svg", Wo({}, at(s), {
    className: d,
    width: n,
    height: i,
    style: u,
    viewBox: "".concat(f.x, " ").concat(f.y, " ").concat(f.width, " ").concat(f.height),
    ref: t
  }), /* @__PURE__ */ w.createElement("title", null, l), /* @__PURE__ */ w.createElement("desc", null, c), r);
}), _g = ["children", "className"];
function Uo() {
  return Uo = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Uo.apply(null, arguments);
}
function Ig(e, t) {
  if (e == null) return {};
  var r, n, i = kg(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function kg(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var zt = /* @__PURE__ */ w.forwardRef((e, t) => {
  var r = e.children, n = e.className, i = Ig(e, _g), a = ie("recharts-layer", n);
  return /* @__PURE__ */ w.createElement("g", Uo({
    className: a
  }, at(i), {
    ref: t
  }), r);
}), Cg = /* @__PURE__ */ Je(null);
function le(e) {
  return function() {
    return e;
  };
}
const Vo = Math.PI, Ko = 2 * Vo, pr = 1e-6, Tg = Ko - pr;
function dh(e) {
  this._ += e[0];
  for (let t = 1, r = e.length; t < r; ++t)
    this._ += arguments[t] + e[t];
}
function Dg(e) {
  let t = Math.floor(e);
  if (!(t >= 0)) throw new Error(`invalid digits: ${e}`);
  if (t > 15) return dh;
  const r = 10 ** t;
  return function(n) {
    this._ += n[0];
    for (let i = 1, a = n.length; i < a; ++i)
      this._ += Math.round(arguments[i] * r) / r + n[i];
  };
}
class jg {
  constructor(t) {
    this._x0 = this._y0 = // start of current subpath
    this._x1 = this._y1 = null, this._ = "", this._append = t == null ? dh : Dg(t);
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
    else if (d > pr) if (!(Math.abs(f * l - c * s) > pr) || !a)
      this._append`L${this._x1 = t},${this._y1 = r}`;
    else {
      let h = n - o, v = i - u, p = l * l + c * c, m = h * h + v * v, y = Math.sqrt(p), b = Math.sqrt(d), x = a * Math.tan((Vo - Math.acos((p + d - m) / (2 * y * b))) / 2), O = x / b, A = x / y;
      Math.abs(O - 1) > pr && this._append`L${t + O * s},${r + O * f}`, this._append`A${a},${a},0,0,${+(f * h > s * v)},${this._x1 = t + A * l},${this._y1 = r + A * c}`;
    }
  }
  arc(t, r, n, i, a, o) {
    if (t = +t, r = +r, n = +n, o = !!o, n < 0) throw new Error(`negative radius: ${n}`);
    let u = n * Math.cos(i), l = n * Math.sin(i), c = t + u, s = r + l, f = 1 ^ o, d = o ? i - a : a - i;
    this._x1 === null ? this._append`M${c},${s}` : (Math.abs(this._x1 - c) > pr || Math.abs(this._y1 - s) > pr) && this._append`L${c},${s}`, n && (d < 0 && (d = d % Ko + Ko), d > Tg ? this._append`A${n},${n},0,1,${f},${t - u},${r - l}A${n},${n},0,1,${f},${this._x1 = c},${this._y1 = s}` : d > pr && this._append`A${n},${n},0,${+(d >= Vo)},${f},${this._x1 = t + n * Math.cos(a)},${this._y1 = r + n * Math.sin(a)}`);
  }
  rect(t, r, n, i) {
    this._append`M${this._x0 = this._x1 = +t},${this._y0 = this._y1 = +r}h${n = +n}v${+i}h${-n}Z`;
  }
  toString() {
    return this._;
  }
}
function hh(e) {
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
  }, () => new jg(t);
}
function Mu(e) {
  return typeof e == "object" && "length" in e ? e : Array.from(e);
}
function vh(e) {
  this._context = e;
}
vh.prototype = {
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
function Pa(e) {
  return new vh(e);
}
function ph(e) {
  return e[0];
}
function mh(e) {
  return e[1];
}
function yh(e, t) {
  var r = le(!0), n = null, i = Pa, a = null, o = hh(u);
  e = typeof e == "function" ? e : e === void 0 ? ph : le(e), t = typeof t == "function" ? t : t === void 0 ? mh : le(t);
  function u(l) {
    var c, s = (l = Mu(l)).length, f, d = !1, h;
    for (n == null && (a = i(h = o())), c = 0; c <= s; ++c)
      !(c < s && r(f = l[c], c, l)) === d && ((d = !d) ? a.lineStart() : a.lineEnd()), d && a.point(+e(f, c, l), +t(f, c, l));
    if (h) return a = null, h + "" || null;
  }
  return u.x = function(l) {
    return arguments.length ? (e = typeof l == "function" ? l : le(+l), u) : e;
  }, u.y = function(l) {
    return arguments.length ? (t = typeof l == "function" ? l : le(+l), u) : t;
  }, u.defined = function(l) {
    return arguments.length ? (r = typeof l == "function" ? l : le(!!l), u) : r;
  }, u.curve = function(l) {
    return arguments.length ? (i = l, n != null && (a = i(n)), u) : i;
  }, u.context = function(l) {
    return arguments.length ? (l == null ? n = a = null : a = i(n = l), u) : n;
  }, u;
}
function di(e, t, r) {
  var n = null, i = le(!0), a = null, o = Pa, u = null, l = hh(c);
  e = typeof e == "function" ? e : e === void 0 ? ph : le(+e), t = typeof t == "function" ? t : le(t === void 0 ? 0 : +t), r = typeof r == "function" ? r : r === void 0 ? mh : le(+r);
  function c(f) {
    var d, h, v, p = (f = Mu(f)).length, m, y = !1, b, x = new Array(p), O = new Array(p);
    for (a == null && (u = o(b = l())), d = 0; d <= p; ++d) {
      if (!(d < p && i(m = f[d], d, f)) === y)
        if (y = !y)
          h = d, u.areaStart(), u.lineStart();
        else {
          for (u.lineEnd(), u.lineStart(), v = d - 1; v >= h; --v)
            u.point(x[v], O[v]);
          u.lineEnd(), u.areaEnd();
        }
      y && (x[d] = +e(m, d, f), O[d] = +t(m, d, f), u.point(n ? +n(m, d, f) : x[d], r ? +r(m, d, f) : O[d]));
    }
    if (b) return u = null, b + "" || null;
  }
  function s() {
    return yh().defined(i).curve(o).context(a);
  }
  return c.x = function(f) {
    return arguments.length ? (e = typeof f == "function" ? f : le(+f), n = null, c) : e;
  }, c.x0 = function(f) {
    return arguments.length ? (e = typeof f == "function" ? f : le(+f), c) : e;
  }, c.x1 = function(f) {
    return arguments.length ? (n = f == null ? null : typeof f == "function" ? f : le(+f), c) : n;
  }, c.y = function(f) {
    return arguments.length ? (t = typeof f == "function" ? f : le(+f), r = null, c) : t;
  }, c.y0 = function(f) {
    return arguments.length ? (t = typeof f == "function" ? f : le(+f), c) : t;
  }, c.y1 = function(f) {
    return arguments.length ? (r = f == null ? null : typeof f == "function" ? f : le(+f), c) : r;
  }, c.lineX0 = c.lineY0 = function() {
    return s().x(e).y(t);
  }, c.lineY1 = function() {
    return s().x(e).y(r);
  }, c.lineX1 = function() {
    return s().x(n).y(t);
  }, c.defined = function(f) {
    return arguments.length ? (i = typeof f == "function" ? f : le(!!f), c) : i;
  }, c.curve = function(f) {
    return arguments.length ? (o = f, a != null && (u = o(a)), c) : o;
  }, c.context = function(f) {
    return arguments.length ? (f == null ? a = u = null : u = o(a = f), c) : a;
  }, c;
}
class gh {
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
function Ng(e) {
  return new gh(e, !0);
}
function Mg(e) {
  return new gh(e, !1);
}
function $i() {
}
function Li(e, t, r) {
  e._context.bezierCurveTo(
    (2 * e._x0 + e._x1) / 3,
    (2 * e._y0 + e._y1) / 3,
    (e._x0 + 2 * e._x1) / 3,
    (e._y0 + 2 * e._y1) / 3,
    (e._x0 + 4 * e._x1 + t) / 6,
    (e._y0 + 4 * e._y1 + r) / 6
  );
}
function bh(e) {
  this._context = e;
}
bh.prototype = {
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
        Li(this, this._x1, this._y1);
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
        Li(this, e, t);
        break;
    }
    this._x0 = this._x1, this._x1 = e, this._y0 = this._y1, this._y1 = t;
  }
};
function $g(e) {
  return new bh(e);
}
function wh(e) {
  this._context = e;
}
wh.prototype = {
  areaStart: $i,
  areaEnd: $i,
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
        Li(this, e, t);
        break;
    }
    this._x0 = this._x1, this._x1 = e, this._y0 = this._y1, this._y1 = t;
  }
};
function Lg(e) {
  return new wh(e);
}
function xh(e) {
  this._context = e;
}
xh.prototype = {
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
        Li(this, e, t);
        break;
    }
    this._x0 = this._x1, this._x1 = e, this._y0 = this._y1, this._y1 = t;
  }
};
function Rg(e) {
  return new xh(e);
}
function Oh(e) {
  this._context = e;
}
Oh.prototype = {
  areaStart: $i,
  areaEnd: $i,
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
function zg(e) {
  return new Oh(e);
}
function sc(e) {
  return e < 0 ? -1 : 1;
}
function fc(e, t, r) {
  var n = e._x1 - e._x0, i = t - e._x1, a = (e._y1 - e._y0) / (n || i < 0 && -0), o = (r - e._y1) / (i || n < 0 && -0), u = (a * i + o * n) / (n + i);
  return (sc(a) + sc(o)) * Math.min(Math.abs(a), Math.abs(o), 0.5 * Math.abs(u)) || 0;
}
function dc(e, t) {
  var r = e._x1 - e._x0;
  return r ? (3 * (e._y1 - e._y0) / r - t) / 2 : t;
}
function lo(e, t, r) {
  var n = e._x0, i = e._y0, a = e._x1, o = e._y1, u = (a - n) / 3;
  e._context.bezierCurveTo(n + u, i + u * t, a - u, o - u * r, a, o);
}
function Ri(e) {
  this._context = e;
}
Ri.prototype = {
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
        lo(this, this._t0, dc(this, this._t0));
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
          this._point = 3, lo(this, dc(this, r = fc(this, e, t)), r);
          break;
        default:
          lo(this, this._t0, r = fc(this, e, t));
          break;
      }
      this._x0 = this._x1, this._x1 = e, this._y0 = this._y1, this._y1 = t, this._t0 = r;
    }
  }
};
function Ah(e) {
  this._context = new Sh(e);
}
(Ah.prototype = Object.create(Ri.prototype)).point = function(e, t) {
  Ri.prototype.point.call(this, t, e);
};
function Sh(e) {
  this._context = e;
}
Sh.prototype = {
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
function Bg(e) {
  return new Ri(e);
}
function Fg(e) {
  return new Ah(e);
}
function Eh(e) {
  this._context = e;
}
Eh.prototype = {
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
        for (var n = hc(e), i = hc(t), a = 0, o = 1; o < r; ++a, ++o)
          this._context.bezierCurveTo(n[0][a], i[0][a], n[1][a], i[1][a], e[o], t[o]);
    (this._line || this._line !== 0 && r === 1) && this._context.closePath(), this._line = 1 - this._line, this._x = this._y = null;
  },
  point: function(e, t) {
    this._x.push(+e), this._y.push(+t);
  }
};
function hc(e) {
  var t, r = e.length - 1, n, i = new Array(r), a = new Array(r), o = new Array(r);
  for (i[0] = 0, a[0] = 2, o[0] = e[0] + 2 * e[1], t = 1; t < r - 1; ++t) i[t] = 1, a[t] = 4, o[t] = 4 * e[t] + 2 * e[t + 1];
  for (i[r - 1] = 2, a[r - 1] = 7, o[r - 1] = 8 * e[r - 1] + e[r], t = 1; t < r; ++t) n = i[t] / a[t - 1], a[t] -= n, o[t] -= n * o[t - 1];
  for (i[r - 1] = o[r - 1] / a[r - 1], t = r - 2; t >= 0; --t) i[t] = (o[t] - i[t + 1]) / a[t];
  for (a[r - 1] = (e[r] + i[r - 1]) / 2, t = 0; t < r - 1; ++t) a[t] = 2 * e[t + 1] - i[t + 1];
  return [i, a];
}
function Wg(e) {
  return new Eh(e);
}
function _a(e, t) {
  this._context = e, this._t = t;
}
_a.prototype = {
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
function Ug(e) {
  return new _a(e, 0.5);
}
function Vg(e) {
  return new _a(e, 0);
}
function Kg(e) {
  return new _a(e, 1);
}
function _r(e, t) {
  if ((o = e.length) > 1)
    for (var r = 1, n, i, a = e[t[0]], o, u = a.length; r < o; ++r)
      for (i = a, a = e[t[r]], n = 0; n < u; ++n)
        a[n][1] += a[n][0] = isNaN(i[n][1]) ? i[n][0] : i[n][1];
}
function Ho(e) {
  for (var t = e.length, r = new Array(t); --t >= 0; ) r[t] = t;
  return r;
}
function Hg(e, t) {
  return e[t];
}
function Gg(e) {
  const t = [];
  return t.key = e, t;
}
function Yg() {
  var e = le([]), t = Ho, r = _r, n = Hg;
  function i(a) {
    var o = Array.from(e.apply(this, arguments), Gg), u, l = o.length, c = -1, s;
    for (const f of a)
      for (u = 0, ++c; u < l; ++u)
        (o[u][c] = [0, +n(f, o[u].key, c, a)]).data = f;
    for (u = 0, s = Mu(t(o)); u < l; ++u)
      o[s[u]].index = u;
    return r(o, s), o;
  }
  return i.keys = function(a) {
    return arguments.length ? (e = typeof a == "function" ? a : le(Array.from(a)), i) : e;
  }, i.value = function(a) {
    return arguments.length ? (n = typeof a == "function" ? a : le(+a), i) : n;
  }, i.order = function(a) {
    return arguments.length ? (t = a == null ? Ho : typeof a == "function" ? a : le(Array.from(a)), i) : t;
  }, i.offset = function(a) {
    return arguments.length ? (r = a ?? _r, i) : r;
  }, i;
}
function qg(e, t) {
  if ((n = e.length) > 0) {
    for (var r, n, i = 0, a = e[0].length, o; i < a; ++i) {
      for (o = r = 0; r < n; ++r) o += e[r][i][1] || 0;
      if (o) for (r = 0; r < n; ++r) e[r][i][1] /= o;
    }
    _r(e, t);
  }
}
function Xg(e, t) {
  if ((i = e.length) > 0) {
    for (var r = 0, n = e[t[0]], i, a = n.length; r < a; ++r) {
      for (var o = 0, u = 0; o < i; ++o) u += e[o][r][1] || 0;
      n[r][1] += n[r][0] = -u / 2;
    }
    _r(e, t);
  }
}
function Zg(e, t) {
  if (!(!((o = e.length) > 0) || !((a = (i = e[t[0]]).length) > 0))) {
    for (var r = 0, n = 1, i, a, o; n < a; ++n) {
      for (var u = 0, l = 0, c = 0; u < o; ++u) {
        for (var s = e[t[u]], f = s[n][1] || 0, d = s[n - 1][1] || 0, h = (f - d) / 2, v = 0; v < u; ++v) {
          var p = e[t[v]], m = p[n][1] || 0, y = p[n - 1][1] || 0;
          h += m - y;
        }
        l += f, c += h * f;
      }
      i[n - 1][1] += i[n - 1][0] = r, l && (r -= c / l);
    }
    i[n - 1][1] += i[n - 1][0] = r, _r(e, t);
  }
}
function Go(e) {
  return e === "__proto__";
}
function Ph(e) {
  switch (typeof e) {
    case "number":
    case "symbol":
      return !1;
    case "string":
      return e.includes(".") || e.includes("[") || e.includes("]");
  }
}
function $u(e) {
  var t;
  return typeof e == "string" || typeof e == "symbol" ? e : Object.is((t = e == null ? void 0 : e.valueOf) == null ? void 0 : t.call(e), -0) ? "-0" : String(e);
}
function _h(e) {
  if (e == null) return "";
  if (typeof e == "string") return e;
  if (Array.isArray(e)) return e.map(_h).join(",");
  const t = String(e);
  return t === "0" && Object.is(Number(e), -0) ? "-0" : t;
}
function Lu(e) {
  if (Array.isArray(e)) return e.map($u);
  if (typeof e == "symbol") return [e];
  e = _h(e);
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
function jr(e, t, r) {
  if (e == null) return r;
  switch (typeof t) {
    case "string": {
      if (Go(t)) return r;
      const n = e[t];
      return n === void 0 ? Ph(t) && !Object.hasOwn(e, t) ? jr(e, Lu(t), r) : r : n;
    }
    case "number":
    case "symbol": {
      typeof t == "number" && (t = $u(t));
      const n = e[t];
      return n === void 0 ? r : n;
    }
    default: {
      if (Array.isArray(t)) return Qg(e, t, r);
      if (Object.is(t == null ? void 0 : t.valueOf(), -0) ? t = "-0" : t = String(t), Go(t)) return r;
      const n = e[t];
      return n === void 0 ? r : n;
    }
  }
}
function Qg(e, t, r) {
  if (t.length === 0) return r;
  let n = e;
  for (let i = 0; i < t.length; i++) {
    if (n == null || Go(t[i])) return r;
    n = n[t[i]];
  }
  return n === void 0 ? r : n;
}
var Jg = 4;
function jt(e) {
  var t = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : Jg, r = 10 ** t, n = Math.round(e * r) / r;
  return Object.is(n, -0) ? 0 : n;
}
function _e(e) {
  for (var t = arguments.length, r = new Array(t > 1 ? t - 1 : 0), n = 1; n < t; n++)
    r[n - 1] = arguments[n];
  return e.reduce((i, a, o) => {
    var u = r[o - 1];
    return typeof u == "string" ? i + u + a : u !== void 0 ? i + jt(u) + a : i + a;
  }, "");
}
var rt = (e) => e === 0 ? 0 : e > 0 ? 1 : -1, Bt = (e) => typeof e == "number" && e != +e, Ir = (e) => typeof e == "string" && e.length > 1 && e.indexOf("%") === e.length - 1, M = (e) => (typeof e == "number" || e instanceof Number) && !Bt(e), Pt = (e) => M(e) || typeof e == "string", e0 = 0, Dn = (e) => {
  var t = ++e0;
  return "".concat(e || "").concat(t);
}, ur = function(t, r) {
  var n = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : 0, i = arguments.length > 3 && arguments[3] !== void 0 ? arguments[3] : !1;
  if (!M(t) && typeof t != "string")
    return n;
  var a;
  if (Ir(t)) {
    if (r == null)
      return n;
    var o = t.indexOf("%");
    a = r * parseFloat(t.slice(0, o)) / 100;
  } else
    a = +t;
  return Bt(a) && (a = n), i && r != null && a > r && (a = r), a;
}, Ih = (e) => {
  if (!Array.isArray(e))
    return !1;
  for (var t = e.length, r = {}, n = 0; n < t; n++)
    if (!r[String(e[n])])
      r[String(e[n])] = !0;
    else
      return !0;
  return !1;
};
function Nt(e, t, r) {
  return M(e) && M(t) ? jt(e + r * (t - e)) : t;
}
function kh(e, t, r) {
  if (!(!e || !e.length))
    return e.find((n) => n && (typeof t == "function" ? t(n) : jr(n, t)) === r);
}
var xe = (e) => e === null || typeof e > "u", Ru = (e) => xe(e) ? e : "".concat(e.charAt(0).toUpperCase()).concat(e.slice(1));
function Fe(e) {
  return e != null;
}
function nn() {
}
var Ch = (e) => "radius" in e && "startAngle" in e && "endAngle" in e, zu = (e, t) => {
  if (!e || typeof e == "function" || typeof e == "boolean")
    return null;
  var r = e;
  if (/* @__PURE__ */ Ze(e) && (r = e.props), typeof r != "object" && typeof r != "function")
    return null;
  var n = {};
  return Object.keys(r).forEach((i) => {
    Nu(i) && typeof r[i] == "function" && (n[i] = (a) => r[i](r, a));
  }), n;
}, t0 = (e, t, r) => (n) => (e(t, r, n), null), r0 = (e, t, r) => {
  if (e === null || typeof e != "object" && typeof e != "function")
    return null;
  var n = null;
  return Object.keys(e).forEach((i) => {
    var a = e[i];
    Nu(i) && typeof a == "function" && (n || (n = {}), n[i] = t0(a, t, r));
  }), n;
};
function vc(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function n0(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? vc(Object(r), !0).forEach(function(n) {
      i0(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : vc(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function i0(e, t, r) {
  return (t = a0(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function a0(e) {
  var t = o0(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function o0(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function et(e, t) {
  var r = n0({}, e), n = t, i = Object.keys(t), a = i.reduce((o, u) => (o[u] === void 0 && n[u] !== void 0 && (o[u] = n[u]), o), r);
  return a;
}
function u0(e, t) {
  const r = /* @__PURE__ */ new Map();
  for (let n = 0; n < e.length; n++) {
    const i = e[n], a = t(i, n, e);
    r.has(a) || r.set(a, i);
  }
  return Array.from(r.values());
}
function l0(e, t) {
  return function(...r) {
    return e.apply(this, r.slice(0, t));
  };
}
function Th(e) {
  return e;
}
function c0(e) {
  return function(t) {
    return jr(t, e);
  };
}
function Yo(e) {
  return e == null || typeof e != "object" && typeof e != "function";
}
function s0(e) {
  return ArrayBuffer.isView(e) && !(e instanceof DataView);
}
function f0(e) {
  return Object.getOwnPropertySymbols(e).filter((t) => Object.prototype.propertyIsEnumerable.call(e, t));
}
function Bu(e) {
  return e == null ? e === void 0 ? "[object Undefined]" : "[object Null]" : Object.prototype.toString.call(e);
}
const d0 = "[object RegExp]", Dh = "[object String]", jh = "[object Number]", Nh = "[object Boolean]", Mh = "[object Arguments]", h0 = "[object Symbol]", v0 = "[object Date]", p0 = "[object Map]", m0 = "[object Set]", y0 = "[object Array]", g0 = "[object ArrayBuffer]", b0 = "[object Object]", w0 = "[object DataView]", x0 = "[object Uint8Array]", O0 = "[object Uint8ClampedArray]", A0 = "[object Uint16Array]", S0 = "[object Uint32Array]", E0 = "[object Int8Array]", P0 = "[object Int16Array]", _0 = "[object Int32Array]", I0 = "[object Float32Array]", k0 = "[object Float64Array]", pc = typeof globalThis == "object" && globalThis || typeof window == "object" && window || typeof self == "object" && self || typeof global == "object" && global || /* @__PURE__ */ function() {
  return this;
}();
function C0(e) {
  return typeof pc.Buffer < "u" && pc.Buffer.isBuffer(e);
}
function T0(e, t) {
  return gr(e, void 0, e, /* @__PURE__ */ new Map(), t);
}
function gr(e, t, r, n = /* @__PURE__ */ new Map(), i = void 0) {
  const a = i == null ? void 0 : i(e, t, r, n);
  if (a !== void 0) return a;
  if (Yo(e)) return e;
  if (n.has(e)) return n.get(e);
  if (Array.isArray(e)) {
    const o = new Array(e.length);
    n.set(e, o);
    for (let u = 0; u < e.length; u++) o[u] = gr(e[u], u, r, n, i);
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
    for (const [u, l] of e) o.set(u, gr(l, u, r, n, i));
    return o;
  }
  if (e instanceof Set) {
    const o = /* @__PURE__ */ new Set();
    n.set(e, o);
    for (const u of e) o.add(gr(u, void 0, r, n, i));
    return o;
  }
  if (C0(e)) return e.subarray();
  if (s0(e)) {
    const o = new (Object.getPrototypeOf(e)).constructor(e.length);
    n.set(e, o);
    for (let u = 0; u < e.length; u++) o[u] = gr(e[u], u, r, n, i);
    return o;
  }
  if (e instanceof ArrayBuffer || typeof SharedArrayBuffer < "u" && e instanceof SharedArrayBuffer) return e.slice(0);
  if (e instanceof DataView) {
    const o = new DataView(e.buffer.slice(0), e.byteOffset, e.byteLength);
    return n.set(e, o), ft(o, e, r, n, i), o;
  }
  if (typeof File < "u" && e instanceof File) {
    const o = new File([e], e.name, { type: e.type });
    return n.set(e, o), ft(o, e, r, n, i), o;
  }
  if (typeof Blob < "u" && e instanceof Blob) {
    const o = new Blob([e], { type: e.type });
    return n.set(e, o), ft(o, e, r, n, i), o;
  }
  if (e instanceof Error) {
    const o = structuredClone(e);
    return n.set(e, o), o.message = e.message, o.name = e.name, o.stack = e.stack, o.cause = e.cause, o.constructor = e.constructor, ft(o, e, r, n, i), o;
  }
  if (e instanceof Boolean) {
    const o = new Boolean(e.valueOf());
    return n.set(e, o), ft(o, e, r, n, i), o;
  }
  if (e instanceof Number) {
    const o = new Number(e.valueOf());
    return n.set(e, o), ft(o, e, r, n, i), o;
  }
  if (e instanceof String) {
    const o = new String(e.valueOf());
    return n.set(e, o), ft(o, e, r, n, i), o;
  }
  if (typeof e == "object" && D0(e)) {
    const o = Object.create(Object.getPrototypeOf(e));
    return n.set(e, o), ft(o, e, r, n, i), o;
  }
  return e;
}
function ft(e, t, r = e, n, i) {
  const a = [...Object.keys(t), ...f0(t)];
  for (let o = 0; o < a.length; o++) {
    const u = a[o], l = Object.getOwnPropertyDescriptor(e, u);
    (l == null || l.writable) && (e[u] = gr(t[u], u, r, n, i));
  }
}
function D0(e) {
  switch (Bu(e)) {
    case Mh:
    case y0:
    case g0:
    case w0:
    case Nh:
    case v0:
    case I0:
    case k0:
    case E0:
    case P0:
    case _0:
    case p0:
    case jh:
    case b0:
    case d0:
    case m0:
    case Dh:
    case h0:
    case x0:
    case O0:
    case A0:
    case S0:
      return !0;
    default:
      return !1;
  }
}
function j0(e) {
  return gr(e, void 0, e, /* @__PURE__ */ new Map(), void 0);
}
function Ci(e, t) {
  return e === t || Number.isNaN(e) && Number.isNaN(t);
}
function $h(e) {
  return e !== null && (typeof e == "object" || typeof e == "function");
}
function Lh(e, t, r) {
  return typeof r != "function" ? Lh(e, t, () => {
  }) : qo(e, t, function n(i, a, o, u, l, c) {
    const s = r(i, a, o, u, l, c);
    return s !== void 0 ? !!s : qo(i, a, n, c, !1);
  }, /* @__PURE__ */ new Map(), !0);
}
function qo(e, t, r, n, i = !1) {
  if (t === e) return !0;
  switch (typeof t) {
    case "object":
      return N0(e, t, r, n);
    case "function":
      return Object.keys(t).length > 0 ? qo(e, { ...t }, r, n, i) : Ci(e, t);
    default:
      return $h(e) && i ? typeof t == "string" ? t === "" : !0 : Ci(e, t);
  }
}
function N0(e, t, r, n) {
  if (t == null) return !0;
  if (Array.isArray(t)) return Rh(e, t, r, n);
  if (t instanceof Map) return M0(e, t, r, n);
  if (t instanceof Set) return $0(e, t, r, n);
  const i = Object.keys(t);
  if (e == null || Yo(e)) return i.length === 0;
  if (i.length === 0) return !0;
  if (n != null && n.has(t)) return n.get(t) === e;
  n == null || n.set(t, e);
  try {
    for (let a = 0; a < i.length; a++) {
      const o = i[a];
      if (!Yo(e) && !(o in e) || t[o] === void 0 && e[o] !== void 0 || t[o] === null && e[o] !== null || !r(e[o], t[o], o, e, t, n)) return !1;
    }
    return !0;
  } finally {
    n == null || n.delete(t);
  }
}
function M0(e, t, r, n) {
  if (t.size === 0) return !0;
  if (!(e instanceof Map)) return !1;
  for (const [i, a] of t.entries()) if (r(e.get(i), a, i, e, t, n) === !1) return !1;
  return !0;
}
function Rh(e, t, r, n) {
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
function $0(e, t, r, n) {
  return t.size === 0 ? !0 : e instanceof Set ? Rh([...e], [...t], r, n) : !1;
}
function zh(e, t) {
  return Lh(e, t, () => {
  });
}
function L0(e) {
  return e = j0(e), (t) => zh(t, e);
}
function R0(e, t) {
  return T0(e, (r, n, i, a) => {
    if (typeof e == "object") {
      if (Bu(e) === "[object Object]" && typeof e.constructor != "function") {
        const o = {};
        return a.set(e, o), ft(o, e, i, a), o;
      }
      switch (Object.prototype.toString.call(e)) {
        case jh:
        case Dh:
        case Nh: {
          const o = new e.constructor(e == null ? void 0 : e.valueOf());
          return ft(o, e), o;
        }
        case Mh: {
          const o = {};
          return ft(o, e), o.length = e.length, o[Symbol.iterator] = e[Symbol.iterator], o;
        }
        default:
          return;
      }
    }
  });
}
function z0(e) {
  return R0(e);
}
const B0 = /^(?:0|[1-9]\d*)$/;
function Bh(e, t = Number.MAX_SAFE_INTEGER) {
  switch (typeof e) {
    case "number":
      return Number.isInteger(e) && e >= 0 && e < t;
    case "symbol":
      return !1;
    case "string":
      return B0.test(e);
  }
}
function F0(e) {
  return e !== null && typeof e == "object" && Bu(e) === "[object Arguments]";
}
function W0(e, t) {
  let r;
  if (Array.isArray(t) ? r = t : typeof t == "string" && Ph(t) && (e == null ? void 0 : e[t]) == null ? r = Lu(t) : r = [t], r.length === 0) return !1;
  let n = e;
  for (let i = 0; i < r.length; i++) {
    const a = r[i];
    if ((n == null || !Object.hasOwn(n, a)) && !((Array.isArray(n) || F0(n)) && Bh(a) && a < n.length))
      return !1;
    n = n[a];
  }
  return !0;
}
function U0(e, t) {
  switch (typeof e) {
    case "object":
      Object.is(e == null ? void 0 : e.valueOf(), -0) && (e = "-0");
      break;
    case "number":
      e = $u(e);
      break;
  }
  return t = z0(t), function(r) {
    const n = jr(r, e);
    return n === void 0 ? W0(r, e) : t === void 0 ? n === void 0 : zh(n, t);
  };
}
function V0(e) {
  if (e == null) return Th;
  switch (typeof e) {
    case "function":
      return e;
    case "object":
      return Array.isArray(e) && e.length === 2 ? U0(e[0], e[1]) : L0(e);
    case "string":
    case "symbol":
    case "number":
      return c0(e);
  }
}
function K0(e) {
  return Number.isSafeInteger(e) && e >= 0;
}
function Fh(e) {
  return e != null && typeof e != "function" && K0(e.length);
}
function H0(e) {
  return typeof e == "object" && e !== null;
}
function G0(e) {
  return H0(e) && Fh(e);
}
function mc(e, t = Th) {
  return G0(e) ? u0(Array.from(e), l0(V0(t), 1)) : [];
}
function Y0(e, t, r) {
  return t === !0 ? mc(e, r) : typeof t == "function" ? mc(e, t) : e;
}
var Xo = { exports: {} }, co = {}, hi = { exports: {} }, so = {};
/**
 * @license React
 * use-sync-external-store-shim.production.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var yc;
function q0() {
  if (yc) return so;
  yc = 1;
  var e = tn;
  function t(f, d) {
    return f === d && (f !== 0 || 1 / f === 1 / d) || f !== f && d !== d;
  }
  var r = typeof Object.is == "function" ? Object.is : t, n = e.useState, i = e.useEffect, a = e.useLayoutEffect, o = e.useDebugValue;
  function u(f, d) {
    var h = d(), v = n({ inst: { value: h, getSnapshot: d } }), p = v[0].inst, m = v[1];
    return a(
      function() {
        p.value = h, p.getSnapshot = d, l(p) && m({ inst: p });
      },
      [f, h, d]
    ), i(
      function() {
        return l(p) && m({ inst: p }), f(function() {
          l(p) && m({ inst: p });
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
  return so.useSyncExternalStore = e.useSyncExternalStore !== void 0 ? e.useSyncExternalStore : s, so;
}
var fo = {};
/**
 * @license React
 * use-sync-external-store-shim.development.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var gc;
function X0() {
  return gc || (gc = 1, process.env.NODE_ENV !== "production" && function() {
    function e(h, v) {
      return h === v && (h !== 0 || 1 / h === 1 / v) || h !== h && v !== v;
    }
    function t(h, v) {
      s || i.startTransition === void 0 || (s = !0, console.error(
        "You are using an outdated, pre-release alpha of React 18 that does not support useSyncExternalStore. The use-sync-external-store shim will not work correctly. Upgrade to a newer pre-release."
      ));
      var p = v();
      if (!f) {
        var m = v();
        a(p, m) || (console.error(
          "The result of getSnapshot should be cached to avoid an infinite loop"
        ), f = !0);
      }
      m = o({
        inst: { value: p, getSnapshot: v }
      });
      var y = m[0].inst, b = m[1];
      return l(
        function() {
          y.value = p, y.getSnapshot = v, r(y) && b({ inst: y });
        },
        [h, p, v]
      ), u(
        function() {
          return r(y) && b({ inst: y }), h(function() {
            r(y) && b({ inst: y });
          });
        },
        [h]
      ), c(p), p;
    }
    function r(h) {
      var v = h.getSnapshot;
      h = h.value;
      try {
        var p = v();
        return !a(h, p);
      } catch {
        return !0;
      }
    }
    function n(h, v) {
      return v();
    }
    typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart(Error());
    var i = tn, a = typeof Object.is == "function" ? Object.is : e, o = i.useState, u = i.useEffect, l = i.useLayoutEffect, c = i.useDebugValue, s = !1, f = !1, d = typeof window > "u" || typeof window.document > "u" || typeof window.document.createElement > "u" ? n : t;
    fo.useSyncExternalStore = i.useSyncExternalStore !== void 0 ? i.useSyncExternalStore : d, typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop(Error());
  }()), fo;
}
var bc;
function Wh() {
  return bc || (bc = 1, process.env.NODE_ENV === "production" ? hi.exports = q0() : hi.exports = X0()), hi.exports;
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
var wc;
function Z0() {
  if (wc) return co;
  wc = 1;
  var e = tn, t = Wh();
  function r(c, s) {
    return c === s && (c !== 0 || 1 / c === 1 / s) || c !== c && s !== s;
  }
  var n = typeof Object.is == "function" ? Object.is : r, i = t.useSyncExternalStore, a = e.useRef, o = e.useEffect, u = e.useMemo, l = e.useDebugValue;
  return co.useSyncExternalStoreWithSelector = function(c, s, f, d, h) {
    var v = a(null);
    if (v.current === null) {
      var p = { hasValue: !1, value: null };
      v.current = p;
    } else p = v.current;
    v = u(
      function() {
        function y(g) {
          if (!b) {
            if (b = !0, x = g, g = d(g), h !== void 0 && p.hasValue) {
              var E = p.value;
              if (h(E, g))
                return O = E;
            }
            return O = g;
          }
          if (E = O, n(x, g)) return E;
          var _ = d(g);
          return h !== void 0 && h(E, _) ? (x = g, E) : (x = g, O = _);
        }
        var b = !1, x, O, A = f === void 0 ? null : f;
        return [
          function() {
            return y(s());
          },
          A === null ? void 0 : function() {
            return y(A());
          }
        ];
      },
      [s, f, d, h]
    );
    var m = i(c, v[0], v[1]);
    return o(
      function() {
        p.hasValue = !0, p.value = m;
      },
      [m]
    ), l(m), m;
  }, co;
}
var ho = {};
/**
 * @license React
 * use-sync-external-store-shim/with-selector.development.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var xc;
function Q0() {
  return xc || (xc = 1, process.env.NODE_ENV !== "production" && function() {
    function e(c, s) {
      return c === s && (c !== 0 || 1 / c === 1 / s) || c !== c && s !== s;
    }
    typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart(Error());
    var t = tn, r = Wh(), n = typeof Object.is == "function" ? Object.is : e, i = r.useSyncExternalStore, a = t.useRef, o = t.useEffect, u = t.useMemo, l = t.useDebugValue;
    ho.useSyncExternalStoreWithSelector = function(c, s, f, d, h) {
      var v = a(null);
      if (v.current === null) {
        var p = { hasValue: !1, value: null };
        v.current = p;
      } else p = v.current;
      v = u(
        function() {
          function y(g) {
            if (!b) {
              if (b = !0, x = g, g = d(g), h !== void 0 && p.hasValue) {
                var E = p.value;
                if (h(E, g))
                  return O = E;
              }
              return O = g;
            }
            if (E = O, n(x, g))
              return E;
            var _ = d(g);
            return h !== void 0 && h(E, _) ? (x = g, E) : (x = g, O = _);
          }
          var b = !1, x, O, A = f === void 0 ? null : f;
          return [
            function() {
              return y(s());
            },
            A === null ? void 0 : function() {
              return y(A());
            }
          ];
        },
        [s, f, d, h]
      );
      var m = i(c, v[0], v[1]);
      return o(
        function() {
          p.hasValue = !0, p.value = m;
        },
        [m]
      ), l(m), m;
    }, typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop(Error());
  }()), ho;
}
process.env.NODE_ENV === "production" ? Xo.exports = Z0() : Xo.exports = Q0();
var J0 = Xo.exports, Fu = /* @__PURE__ */ Je(null), eb = (e) => e, ve = () => {
  var e = kt(Fu);
  return e ? e.store.dispatch : eb;
}, Ti = () => {
}, tb = () => Ti, rb = (e, t) => e === t;
function N(e) {
  var t = kt(Fu), r = Kt(() => t ? (n) => {
    if (n != null)
      return e(n);
  } : Ti, [t, e]);
  return J0.useSyncExternalStoreWithSelector(t ? t.subscription.addNestedSub : tb, t ? t.store.getState : Ti, t ? t.store.getState : Ti, r, rb);
}
var nb = (e, t, r) => {
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
}, ib = (e, t, r) => {
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
}, ab = {
  inputStabilityCheck: "once",
  identityFunctionCheck: "once"
};
function ob(e, t = `expected a function, instead received ${typeof e}`) {
  if (typeof e != "function")
    throw new TypeError(t);
}
function ub(e, t = "expected all items to be functions, instead received the following types: ") {
  if (!e.every((r) => typeof r == "function")) {
    const r = e.map(
      (n) => typeof n == "function" ? `function ${n.name || "unnamed"}()` : typeof n
    ).join(", ");
    throw new TypeError(`${t}[${r}]`);
  }
}
var Oc = (e) => Array.isArray(e) ? e : [e];
function lb(e) {
  const t = Array.isArray(e[0]) ? e[0] : e;
  return ub(
    t,
    "createSelector expects all input-selectors to be functions, but received the following types: "
  ), t;
}
function Ac(e, t) {
  const r = [], { length: n } = e;
  for (let i = 0; i < n; i++)
    r.push(e[i].apply(null, t));
  return r;
}
var cb = (e, t) => {
  const { identityFunctionCheck: r, inputStabilityCheck: n } = {
    ...ab,
    ...t
  };
  return {
    identityFunctionCheck: {
      shouldRun: r === "always" || r === "once" && e,
      run: nb
    },
    inputStabilityCheck: {
      shouldRun: n === "always" || n === "once" && e,
      run: ib
    }
  };
}, sb = class {
  constructor(e) {
    this.value = e;
  }
  deref() {
    return this.value;
  }
}, fb = () => typeof WeakRef > "u" ? sb : WeakRef, Uh = /* @__PURE__ */ fb(), db = 0, Sc = 1;
function vi() {
  return {
    s: db,
    v: void 0,
    o: null,
    p: null
  };
}
function hb(e) {
  return e instanceof Uh ? e.deref() : e;
}
function Vh(e, t = {}) {
  let r = vi();
  const { resultEqualityCheck: n } = t;
  let i, a = 0;
  function o() {
    let u = r;
    const { length: l } = arguments;
    for (let f = 0, d = l; f < d; f++) {
      const h = arguments[f];
      if (typeof h == "function" || typeof h == "object" && h !== null) {
        let v = u.o;
        v === null && (u.o = v = /* @__PURE__ */ new WeakMap());
        const p = v.get(h);
        p === void 0 ? (u = vi(), v.set(h, u)) : u = p;
      } else {
        let v = u.p;
        v === null && (u.p = v = /* @__PURE__ */ new Map());
        const p = v.get(h);
        p === void 0 ? (u = vi(), v.set(h, u)) : u = p;
      }
    }
    const c = u;
    let s;
    if (u.s === Sc)
      s = u.v;
    else if (s = e.apply(null, arguments), a++, n) {
      const f = hb(i);
      f != null && n(f, s) && (s = f, a !== 0 && a--), i = typeof s == "object" && s !== null || typeof s == "function" ? /* @__PURE__ */ new Uh(s) : s;
    }
    return c.s = Sc, c.v = s, s;
  }
  return o.clearCache = () => {
    r = vi(), o.resetResultsCount();
  }, o.resultsCount = () => a, o.resetResultsCount = () => {
    a = 0;
  }, o;
}
function vb(e, ...t) {
  const r = typeof e == "function" ? {
    memoize: e,
    memoizeOptions: t
  } : e, n = (...i) => {
    let a = 0, o = 0, u, l = {}, c = i.pop();
    typeof c == "object" && (l = c, c = i.pop()), ob(
      c,
      `createSelector expects an output function after the inputs, but received: [${typeof c}]`
    );
    const s = {
      ...r,
      ...l
    }, {
      memoize: f,
      memoizeOptions: d = [],
      argsMemoize: h = Vh,
      argsMemoizeOptions: v = []
    } = s, p = Oc(d), m = Oc(v), y = lb(i), b = f(function() {
      return a++, c.apply(
        null,
        arguments
      );
    }, ...p);
    let x = !0;
    const O = h(function() {
      o++;
      const g = Ac(
        y,
        arguments
      );
      if (u = b.apply(null, g), process.env.NODE_ENV !== "production") {
        const { devModeChecks: E = {} } = s, { identityFunctionCheck: _, inputStabilityCheck: I } = cb(x, E);
        if (_.shouldRun && _.run(
          c,
          g,
          u
        ), I.shouldRun) {
          const C = Ac(
            y,
            arguments
          );
          I.run(
            { inputSelectorResults: g, inputSelectorResultsCopy: C },
            { memoize: f, memoizeOptions: p },
            arguments
          );
        }
        x && (x = !1);
      }
      return u;
    }, ...m);
    return Object.assign(O, {
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
var S = /* @__PURE__ */ vb(Vh);
function pb(e, t = 1) {
  const r = [], n = Math.floor(t), i = (a, o) => {
    for (let u = 0; u < a.length; u++) {
      const l = a[u];
      Array.isArray(l) && o < n ? i(l, o + 1) : r.push(l);
    }
  };
  return i(e, 0), r;
}
function Zo(e, t, r) {
  return $h(r) && (typeof t == "number" && Fh(r) && Bh(t) && t < r.length || typeof t == "string" && t in r) ? Ci(r[t], e) : !1;
}
function Ec(e) {
  return typeof e == "symbol" ? 1 : e === null ? 2 : e === void 0 ? 3 : e !== e ? 4 : 0;
}
const mb = (e, t, r) => {
  if (e !== t) {
    const n = Ec(e), i = Ec(t);
    if (n === i && n === 0) {
      if (e < t) return r === "desc" ? 1 : -1;
      if (e > t) return r === "desc" ? -1 : 1;
    }
    return r === "desc" ? i - n : n - i;
  }
  return 0;
};
function Kh(e) {
  return typeof e == "symbol" || e instanceof Symbol;
}
const yb = /\.|\[(?:[^[\]]*|(["'])(?:(?!\1)[^\\]|\\.)*?\1)\]/, gb = /^\w*$/;
function bb(e, t) {
  return Array.isArray(e) ? !1 : typeof e == "number" || typeof e == "boolean" || e == null || Kh(e) ? !0 : typeof e == "string" && (gb.test(e) || !yb.test(e)) || t != null;
}
function wb(e, t, r, n) {
  if (e == null) return [];
  r = r, Array.isArray(e) || (e = Object.values(e)), Array.isArray(t) || (t = t == null ? [null] : [t]), t.length === 0 && (t = [null]), Array.isArray(r) || (r = r == null ? [] : [r]), r = r.map((u) => String(u));
  const i = (u, l) => {
    let c = u;
    for (let s = 0; s < l.length && c != null; ++s) c = c[l[s]];
    return c;
  }, a = (u, l) => l == null || u == null ? l : typeof u == "object" && "key" in u ? Object.hasOwn(l, u.key) ? l[u.key] : i(l, u.path) : typeof u == "function" ? u(l) : Array.isArray(u) ? i(l, u) : typeof l == "object" ? l[u] : l, o = t.map((u) => (Array.isArray(u) && u.length === 1 && (u = u[0]), u == null || typeof u == "function" || Array.isArray(u) || bb(u) ? u : {
    key: u,
    path: Lu(u)
  }));
  return e.map((u) => ({
    original: u,
    criteria: o.map((l) => a(l, u))
  })).slice().sort((u, l) => {
    for (let c = 0; c < o.length; c++) {
      const s = mb(u.criteria[c], l.criteria[c], r[c]);
      if (s !== 0) return s;
    }
    return 0;
  }).map((u) => u.original);
}
function Ia(e, ...t) {
  const r = t.length;
  return r > 1 && Zo(e, t[0], t[1]) ? t = [] : r > 2 && Zo(t[0], t[1], t[2]) && (t = [t[0]]), wb(e, pb(t), ["asc"]);
}
var Hh = (e) => e.legend.settings, xb = (e) => e.legend.size, Ob = (e) => e.legend.payload;
S([Ob, Hh], (e, t) => {
  var r = t.itemSorter, n = e.flat(1);
  return r ? Ia(n, r) : n;
});
function Ab(e, t) {
  return _b(e) || Pb(e, t) || Eb(e, t) || Sb();
}
function Sb() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function Eb(e, t) {
  if (e) {
    if (typeof e == "string") return Pc(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Pc(e, t) : void 0;
  }
}
function Pc(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function Pb(e, t) {
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
function _b(e) {
  if (Array.isArray(e)) return e;
}
var pi = 1;
function _c(e, t) {
  return Math.abs(e.height - t.height) > pi || Math.abs(e.left - t.left) > pi || Math.abs(e.top - t.top) > pi || Math.abs(e.width - t.width) > pi;
}
function Ic(e) {
  var t = e.getBoundingClientRect();
  return {
    height: t.height,
    left: t.left,
    top: t.top,
    width: t.width
  };
}
function Ib() {
  var e = arguments.length > 0 && arguments[0] !== void 0 ? arguments[0] : [], t = fe({
    height: 0,
    left: 0,
    top: 0,
    width: 0
  }), r = Ab(t, 2), n = r[0], i = r[1], a = V(null), o = V(n);
  o.current = n;
  var u = ee(
    (l) => {
      if (a.current != null && (a.current.disconnect(), a.current = null), l != null) {
        var c = Ic(l);
        if (_c(c, o.current) && i(c), typeof ResizeObserver < "u") {
          var s = new ResizeObserver(() => {
            var f = Ic(l);
            _c(f, o.current) && i(f);
          });
          s.observe(l), a.current = s;
        }
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [...e]
  );
  return he(() => () => {
    var l;
    (l = a.current) === null || l === void 0 || l.disconnect();
  }, []), [n, u];
}
function Pe(e) {
  return `Minified Redux error #${e}; visit https://redux.js.org/Errors?code=${e} for the full message or use the non-minified dev environment for full errors. `;
}
var kb = typeof Symbol == "function" && Symbol.observable || "@@observable", kc = kb, vo = () => Math.random().toString(36).substring(7).split("").join("."), Cb = {
  INIT: `@@redux/INIT${/* @__PURE__ */ vo()}`,
  REPLACE: `@@redux/REPLACE${/* @__PURE__ */ vo()}`,
  PROBE_UNKNOWN_ACTION: () => `@@redux/PROBE_UNKNOWN_ACTION${vo()}`
}, Ar = Cb;
function Kn(e) {
  if (typeof e != "object" || e === null)
    return !1;
  let t = e;
  for (; Object.getPrototypeOf(t) !== null; )
    t = Object.getPrototypeOf(t);
  return Object.getPrototypeOf(e) === t || Object.getPrototypeOf(e) === null;
}
function Tb(e) {
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
  if (Nb(e))
    return "date";
  if (jb(e))
    return "error";
  const r = Db(e);
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
function Db(e) {
  return typeof e.constructor == "function" ? e.constructor.name : null;
}
function jb(e) {
  return e instanceof Error || typeof e.message == "string" && e.constructor && typeof e.constructor.stackTraceLimit == "number";
}
function Nb(e) {
  return e instanceof Date ? !0 : typeof e.toDateString == "function" && typeof e.getDate == "function" && typeof e.setDate == "function";
}
function tr(e) {
  let t = typeof e;
  return process.env.NODE_ENV !== "production" && (t = Tb(e)), t;
}
function Gh(e, t, r) {
  if (typeof e != "function")
    throw new Error(process.env.NODE_ENV === "production" ? Pe(2) : `Expected the root reducer to be a function. Instead, received: '${tr(e)}'`);
  if (typeof t == "function" && typeof r == "function" || typeof r == "function" && typeof arguments[3] == "function")
    throw new Error(process.env.NODE_ENV === "production" ? Pe(0) : "It looks like you are passing several store enhancers to createStore(). This is not supported. Instead, compose them together to a single function. See https://redux.js.org/tutorials/fundamentals/part-4-store#creating-a-store-with-enhancers for an example.");
  if (typeof t == "function" && typeof r > "u" && (r = t, t = void 0), typeof r < "u") {
    if (typeof r != "function")
      throw new Error(process.env.NODE_ENV === "production" ? Pe(1) : `Expected the enhancer to be a function. Instead, received: '${tr(r)}'`);
    return r(Gh)(e, t);
  }
  let n = e, i = t, a = /* @__PURE__ */ new Map(), o = a, u = 0, l = !1;
  function c() {
    o === a && (o = /* @__PURE__ */ new Map(), a.forEach((m, y) => {
      o.set(y, m);
    }));
  }
  function s() {
    if (l)
      throw new Error(process.env.NODE_ENV === "production" ? Pe(3) : "You may not call store.getState() while the reducer is executing. The reducer has already received the state as an argument. Pass it down from the top reducer instead of reading it from the store.");
    return i;
  }
  function f(m) {
    if (typeof m != "function")
      throw new Error(process.env.NODE_ENV === "production" ? Pe(4) : `Expected the listener to be a function. Instead, received: '${tr(m)}'`);
    if (l)
      throw new Error(process.env.NODE_ENV === "production" ? Pe(5) : "You may not call store.subscribe() while the reducer is executing. If you would like to be notified after the store has been updated, subscribe from a component and invoke store.getState() in the callback to access the latest state. See https://redux.js.org/api/store#subscribelistener for more details.");
    let y = !0;
    c();
    const b = u++;
    return o.set(b, m), function() {
      if (y) {
        if (l)
          throw new Error(process.env.NODE_ENV === "production" ? Pe(6) : "You may not unsubscribe from a store listener while the reducer is executing. See https://redux.js.org/api/store#subscribelistener for more details.");
        y = !1, c(), o.delete(b), a = null;
      }
    };
  }
  function d(m) {
    if (!Kn(m))
      throw new Error(process.env.NODE_ENV === "production" ? Pe(7) : `Actions must be plain objects. Instead, the actual type was: '${tr(m)}'. You may need to add middleware to your store setup to handle dispatching other values, such as 'redux-thunk' to handle dispatching functions. See https://redux.js.org/tutorials/fundamentals/part-4-store#middleware and https://redux.js.org/tutorials/fundamentals/part-6-async-logic#using-the-redux-thunk-middleware for examples.`);
    if (typeof m.type > "u")
      throw new Error(process.env.NODE_ENV === "production" ? Pe(8) : 'Actions may not have an undefined "type" property. You may have misspelled an action type string constant.');
    if (typeof m.type != "string")
      throw new Error(process.env.NODE_ENV === "production" ? Pe(17) : `Action "type" property must be a string. Instead, the actual type was: '${tr(m.type)}'. Value was: '${m.type}' (stringified)`);
    if (l)
      throw new Error(process.env.NODE_ENV === "production" ? Pe(9) : "Reducers may not dispatch actions.");
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
      throw new Error(process.env.NODE_ENV === "production" ? Pe(10) : `Expected the nextReducer to be a function. Instead, received: '${tr(m)}`);
    n = m, d({
      type: Ar.REPLACE
    });
  }
  function v() {
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
          throw new Error(process.env.NODE_ENV === "production" ? Pe(11) : `Expected the observer to be an object. Instead, received: '${tr(y)}'`);
        function b() {
          const O = y;
          O.next && O.next(s());
        }
        return b(), {
          unsubscribe: m(b)
        };
      },
      [kc]() {
        return this;
      }
    };
  }
  return d({
    type: Ar.INIT
  }), {
    dispatch: d,
    subscribe: f,
    getState: s,
    replaceReducer: h,
    [kc]: v
  };
}
function Cc(e) {
  typeof console < "u" && typeof console.error == "function" && console.error(e);
  try {
    throw new Error(e);
  } catch {
  }
}
function Mb(e, t, r, n) {
  const i = Object.keys(t), a = r && r.type === Ar.INIT ? "preloadedState argument passed to createStore" : "previous state received by the reducer";
  if (i.length === 0)
    return "Store does not have a valid reducer. Make sure the argument passed to combineReducers is an object whose values are reducers.";
  if (!Kn(e))
    return `The ${a} has unexpected type of "${tr(e)}". Expected argument to be an object with the following keys: "${i.join('", "')}"`;
  const o = Object.keys(e).filter((u) => !t.hasOwnProperty(u) && !n[u]);
  if (o.forEach((u) => {
    n[u] = !0;
  }), !(r && r.type === Ar.REPLACE) && o.length > 0)
    return `Unexpected ${o.length > 1 ? "keys" : "key"} "${o.join('", "')}" found in ${a}. Expected to find one of the known reducer keys instead: "${i.join('", "')}". Unexpected keys will be ignored.`;
}
function $b(e) {
  Object.keys(e).forEach((t) => {
    const r = e[t];
    if (typeof r(void 0, {
      type: Ar.INIT
    }) > "u")
      throw new Error(process.env.NODE_ENV === "production" ? Pe(12) : `The slice reducer for key "${t}" returned undefined during initialization. If the state passed to the reducer is undefined, you must explicitly return the initial state. The initial state may not be undefined. If you don't want to set a value for this reducer, you can use null instead of undefined.`);
    if (typeof r(void 0, {
      type: Ar.PROBE_UNKNOWN_ACTION()
    }) > "u")
      throw new Error(process.env.NODE_ENV === "production" ? Pe(13) : `The slice reducer for key "${t}" returned undefined when probed with a random type. Don't try to handle '${Ar.INIT}' or other actions in "redux/*" namespace. They are considered private. Instead, you must return the current state for any unknown actions, unless it is undefined, in which case you must return the initial state, regardless of the action type. The initial state may not be undefined, but can be null.`);
  });
}
function Yh(e) {
  const t = Object.keys(e), r = {};
  for (let o = 0; o < t.length; o++) {
    const u = t[o];
    process.env.NODE_ENV !== "production" && typeof e[u] > "u" && Cc(`No reducer provided for key "${u}"`), typeof e[u] == "function" && (r[u] = e[u]);
  }
  const n = Object.keys(r);
  let i;
  process.env.NODE_ENV !== "production" && (i = {});
  let a;
  try {
    $b(r);
  } catch (o) {
    a = o;
  }
  return function(u = {}, l) {
    if (a)
      throw a;
    if (process.env.NODE_ENV !== "production") {
      const f = Mb(u, r, l, i);
      f && Cc(f);
    }
    let c = !1;
    const s = {};
    for (let f = 0; f < n.length; f++) {
      const d = n[f], h = r[d], v = u[d], p = h(v, l);
      if (typeof p > "u") {
        const m = l && l.type;
        throw new Error(process.env.NODE_ENV === "production" ? Pe(14) : `When called with an action of type ${m ? `"${String(m)}"` : "(unknown type)"}, the slice reducer for key "${d}" returned undefined. To ignore an action, you must explicitly return the previous state. If you want this reducer to hold no value, you can return null instead of undefined.`);
      }
      s[d] = p, c = c || p !== v;
    }
    return c = c || n.length !== Object.keys(u).length, c ? s : u;
  };
}
function zi(...e) {
  return e.length === 0 ? (t) => t : e.length === 1 ? e[0] : e.reduce((t, r) => (...n) => t(r(...n)));
}
function Lb(...e) {
  return (t) => (r, n) => {
    const i = t(r, n);
    let a = () => {
      throw new Error(process.env.NODE_ENV === "production" ? Pe(15) : "Dispatching while constructing your middleware is not allowed. Other middleware would not be applied to this dispatch.");
    };
    const o = {
      getState: i.getState,
      dispatch: (l, ...c) => a(l, ...c)
    }, u = e.map((l) => l(o));
    return a = zi(...u)(i.dispatch), {
      ...i,
      dispatch: a
    };
  };
}
function Wu(e) {
  return Kn(e) && "type" in e && typeof e.type == "string";
}
var qh = Symbol.for("immer-nothing"), Tc = Symbol.for("immer-draftable"), Be = Symbol.for("immer-state"), Rb = process.env.NODE_ENV !== "production" ? [
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
function Ye(e, ...t) {
  if (process.env.NODE_ENV !== "production") {
    const r = Rb[e], n = yr(r) ? r.apply(null, t) : r;
    throw new Error(`[Immer] ${n}`);
  }
  throw new Error(
    `[Immer] minified error nr: ${e}. Full error at: https://bit.ly/3cXEKWf`
  );
}
var Xe = Object, qr = Xe.getPrototypeOf, Bi = "constructor", ka = "prototype", Qo = "configurable", Fi = "enumerable", Di = "writable", jn = "value", Ft = (e) => !!e && !!e[Be];
function ht(e) {
  var t;
  return e ? Xh(e) || Ta(e) || !!e[Tc] || !!((t = e[Bi]) != null && t[Tc]) || Da(e) || ja(e) : !1;
}
var zb = Xe[ka][Bi].toString(), Dc = /* @__PURE__ */ new WeakMap();
function Xh(e) {
  if (!e || !Uu(e))
    return !1;
  const t = qr(e);
  if (t === null || t === Xe[ka])
    return !0;
  const r = Xe.hasOwnProperty.call(t, Bi) && t[Bi];
  if (r === Object)
    return !0;
  if (!yr(r))
    return !1;
  let n = Dc.get(r);
  return n === void 0 && (n = Function.toString.call(r), Dc.set(r, n)), n === zb;
}
function Ca(e, t, r = !0) {
  Hn(e) === 0 ? (r ? Reflect.ownKeys(e) : Xe.keys(e)).forEach((i) => {
    t(i, e[i], e);
  }) : e.forEach((n, i) => t(i, n, e));
}
function Hn(e) {
  const t = e[Be];
  return t ? t.type_ : Ta(e) ? 1 : Da(e) ? 2 : ja(e) ? 3 : 0;
}
var po = (e, t, r = Hn(e)) => r === 2 ? e.has(t) : Xe[ka].hasOwnProperty.call(e, t), Jo = (e, t, r = Hn(e)) => (
  // @ts-ignore
  r === 2 ? e.get(t) : e[t]
), Wi = (e, t, r, n = Hn(e)) => {
  n === 2 ? e.set(t, r) : n === 3 ? e.add(r) : e[t] = r;
};
function Bb(e, t) {
  return e === t ? e !== 0 || 1 / e === 1 / t : e !== e && t !== t;
}
var Ta = Array.isArray, Da = (e) => e instanceof Map, ja = (e) => e instanceof Set, Uu = (e) => typeof e == "object", yr = (e) => typeof e == "function", mo = (e) => typeof e == "boolean";
function Fb(e) {
  const t = +e;
  return Number.isInteger(t) && String(t) === e;
}
var gt = (e) => e.copy_ || e.base_, Vu = (e) => e.modified_ ? e.copy_ : e.base_;
function eu(e, t) {
  if (Da(e))
    return new Map(e);
  if (ja(e))
    return new Set(e);
  if (Ta(e))
    return Array[ka].slice.call(e);
  const r = Xh(e);
  if (t === !0 || t === "class_only" && !r) {
    const n = Xe.getOwnPropertyDescriptors(e);
    delete n[Be];
    let i = Reflect.ownKeys(n);
    for (let a = 0; a < i.length; a++) {
      const o = i[a], u = n[o];
      u[Di] === !1 && (u[Di] = !0, u[Qo] = !0), (u.get || u.set) && (n[o] = {
        [Qo]: !0,
        [Di]: !0,
        // could live with !!desc.set as well here...
        [Fi]: u[Fi],
        [jn]: e[o]
      });
    }
    return Xe.create(qr(e), n);
  } else {
    const n = qr(e);
    if (n !== null && r)
      return { ...e };
    const i = Xe.create(n);
    return Xe.assign(i, e);
  }
}
function Ku(e, t = !1) {
  return Na(e) || Ft(e) || !ht(e) || (Hn(e) > 1 && Xe.defineProperties(e, {
    set: mi,
    add: mi,
    clear: mi,
    delete: mi
  }), Xe.freeze(e), t && Ca(
    e,
    (r, n) => {
      Ku(n, !0);
    },
    !1
  )), e;
}
function Wb() {
  Ye(2);
}
var mi = {
  [jn]: Wb
};
function Na(e) {
  return e === null || !Uu(e) ? !0 : Xe.isFrozen(e);
}
var Ui = "MapSet", tu = "Patches", jc = "ArrayMethods", Zh = {};
function kr(e) {
  const t = Zh[e];
  return t || Ye(0, e), t;
}
var Nc = (e) => !!Zh[e], Nn, Qh = () => Nn, Ub = (e, t) => ({
  drafts_: [],
  parent_: e,
  immer_: t,
  // Whenever the modified draft contains a draft from another scope, we
  // need to prevent auto-freezing so the unowned draft can be finalized.
  canAutoFreeze_: !0,
  unfinalizedDrafts_: 0,
  handledSet_: /* @__PURE__ */ new Set(),
  processedForPatches_: /* @__PURE__ */ new Set(),
  mapSetPlugin_: Nc(Ui) ? kr(Ui) : void 0,
  arrayMethodsPlugin_: Nc(jc) ? kr(jc) : void 0
});
function Mc(e, t) {
  t && (e.patchPlugin_ = kr(tu), e.patches_ = [], e.inversePatches_ = [], e.patchListener_ = t);
}
function ru(e) {
  nu(e), e.drafts_.forEach(Vb), e.drafts_ = null;
}
function nu(e) {
  e === Nn && (Nn = e.parent_);
}
var $c = (e) => Nn = Ub(Nn, e);
function Vb(e) {
  const t = e[Be];
  t.type_ === 0 || t.type_ === 1 ? t.revoke_() : t.revoked_ = !0;
}
function Lc(e, t) {
  t.unfinalizedDrafts_ = t.drafts_.length;
  const r = t.drafts_[0];
  if (e !== void 0 && e !== r) {
    r[Be].modified_ && (ru(t), Ye(4)), ht(e) && (e = Rc(t, e));
    const { patchPlugin_: i } = t;
    i && i.generateReplacementPatches_(
      r[Be].base_,
      e,
      t
    );
  } else
    e = Rc(t, r);
  return Kb(t, e, !0), ru(t), t.patches_ && t.patchListener_(t.patches_, t.inversePatches_), e !== qh ? e : void 0;
}
function Rc(e, t) {
  if (Na(t))
    return t;
  const r = t[Be];
  if (!r)
    return Vi(t, e.handledSet_, e);
  if (!Ma(r, e))
    return t;
  if (!r.modified_)
    return r.base_;
  if (!r.finalized_) {
    const { callbacks_: n } = r;
    if (n)
      for (; n.length > 0; )
        n.pop()(e);
    tv(r, e);
  }
  return r.copy_;
}
function Kb(e, t, r = !1) {
  !e.parent_ && e.immer_.autoFreeze_ && e.canAutoFreeze_ && Ku(t, r);
}
function Jh(e) {
  e.finalized_ = !0, e.scope_.unfinalizedDrafts_--;
}
var Ma = (e, t) => e.scope_ === t, Hb = [];
function ev(e, t, r, n) {
  const i = gt(e), a = e.type_;
  if (n !== void 0 && Jo(i, n, a) === t) {
    Wi(i, n, r, a);
    return;
  }
  if (!e.draftLocations_) {
    const u = e.draftLocations_ = /* @__PURE__ */ new Map();
    Ca(i, (l, c) => {
      if (Ft(c)) {
        const s = u.get(c) || [];
        s.push(l), u.set(c, s);
      }
    });
  }
  const o = e.draftLocations_.get(t) ?? Hb;
  for (const u of o)
    Wi(i, u, r, a);
}
function Gb(e, t, r) {
  e.callbacks_.push(function(i) {
    var u;
    const a = t;
    if (!a || !Ma(a, i))
      return;
    (u = i.mapSetPlugin_) == null || u.fixSetContents(a);
    const o = Vu(a);
    ev(e, a.draft_ ?? a, o, r), tv(a, i);
  });
}
function tv(e, t) {
  var n;
  if (e.modified_ && !e.finalized_ && (e.type_ === 3 || e.type_ === 1 && e.allIndicesReassigned_ || (((n = e.assigned_) == null ? void 0 : n.size) ?? 0) > 0)) {
    const { patchPlugin_: i } = t;
    if (i) {
      const a = i.getPath(e);
      a && i.generatePatches_(e, a, t);
    }
    Jh(e);
  }
}
function Yb(e, t, r) {
  const { scope_: n } = e;
  if (Ft(r)) {
    const i = r[Be];
    Ma(i, n) && i.callbacks_.push(function() {
      ji(e);
      const o = Vu(i);
      ev(e, r, o, t);
    });
  } else ht(r) && e.callbacks_.push(function() {
    const a = gt(e);
    e.type_ === 3 ? a.has(r) && Vi(r, n.handledSet_, n) : Jo(a, t, e.type_) === r && n.drafts_.length > 1 && (e.assigned_.get(t) ?? !1) === !0 && e.copy_ && Vi(
      Jo(e.copy_, t, e.type_),
      n.handledSet_,
      n
    );
  });
}
function Vi(e, t, r) {
  return !r.immer_.autoFreeze_ && r.unfinalizedDrafts_ < 1 || Ft(e) || t.has(e) || !ht(e) || Na(e) || (t.add(e), Ca(e, (n, i) => {
    if (Ft(i)) {
      const a = i[Be];
      if (Ma(a, r)) {
        const o = Vu(a);
        Wi(e, n, o, e.type_), Jh(a);
      }
    } else ht(i) && Vi(i, t, r);
  })), e;
}
function qb(e, t) {
  const r = Ta(e), n = {
    type_: r ? 1 : 0,
    // Track which produce call this is associated with.
    scope_: t ? t.scope_ : Qh(),
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
  let i = n, a = Ki;
  r && (i = [n], a = Mn);
  const { revoke: o, proxy: u } = Proxy.revocable(i, a);
  return n.draft_ = u, n.revoke_ = o, [u, n];
}
var Ki = {
  get(e, t) {
    if (t === Be)
      return e;
    if (t === "constructor" || t === "__proto__") {
      const u = gt(e)[t];
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
    const i = gt(e);
    if (!po(i, t, e.type_))
      return Xb(e, i, t);
    const a = i[t];
    if (e.finalized_ || !ht(a) || n && e.operationMethod && (r != null && r.isMutatingArrayMethod(
      e.operationMethod
    )) && Fb(t))
      return a;
    if (a === yo(e.base_, t)) {
      ji(e);
      const o = e.type_ === 1 ? +t : t, u = au(e.scope_, a, e, o);
      return e.copy_[o] = u;
    }
    return a;
  },
  has(e, t) {
    return t === "constructor" || t === "__proto__" || t === "prototype" ? !1 : t in gt(e);
  },
  ownKeys(e) {
    return Reflect.ownKeys(gt(e));
  },
  set(e, t, r) {
    if (t === "constructor" || t === "__proto__" || t === "prototype")
      return !0;
    const n = rv(gt(e), t);
    if (n != null && n.set)
      return n.set.call(e.draft_, r), !0;
    if (!e.modified_) {
      const i = yo(gt(e), t), a = i == null ? void 0 : i[Be];
      if (a && a.base_ === r)
        return e.copy_[t] = r, e.assigned_.set(t, !1), !0;
      if (Bb(r, i) && (r !== void 0 || po(e.base_, t, e.type_)))
        return !0;
      ji(e), iu(e);
    }
    return e.copy_[t] === r && // special case: handle new props with value 'undefined'
    (r !== void 0 || po(e.copy_, t, e.type_)) || // special case: NaN
    Number.isNaN(r) && Number.isNaN(e.copy_[t]) || (e.copy_[t] = r, e.assigned_.set(t, !0), Yb(e, t, r)), !0;
  },
  deleteProperty(e, t) {
    return ji(e), yo(e.base_, t) !== void 0 || t in e.base_ ? (e.assigned_.set(t, !1), iu(e)) : e.assigned_.delete(t), e.copy_ && delete e.copy_[t], !0;
  },
  // Note: We never coerce `desc.value` into an Immer draft, because we can't make
  // the same guarantee in ES5 mode.
  getOwnPropertyDescriptor(e, t) {
    const r = gt(e), n = Reflect.getOwnPropertyDescriptor(r, t);
    return n && {
      [Di]: !0,
      [Qo]: e.type_ !== 1 || t !== "length",
      [Fi]: n[Fi],
      [jn]: r[t]
    };
  },
  defineProperty() {
    Ye(11);
  },
  getPrototypeOf(e) {
    return qr(e.base_);
  },
  setPrototypeOf() {
    Ye(12);
  }
}, Mn = {};
for (let e in Ki) {
  let t = Ki[e];
  Mn[e] = function() {
    const r = arguments;
    return r[0] = r[0][0], t.apply(this, r);
  };
}
Mn.deleteProperty = function(e, t) {
  return process.env.NODE_ENV !== "production" && isNaN(parseInt(t)) && Ye(13), Mn.set.call(this, e, t, void 0);
};
Mn.set = function(e, t, r) {
  return process.env.NODE_ENV !== "production" && t !== "length" && isNaN(parseInt(t)) && Ye(14), Ki.set.call(this, e[0], t, r, e[0]);
};
function yo(e, t) {
  const r = e[Be];
  return (r ? gt(r) : e)[t];
}
function Xb(e, t, r) {
  var i;
  const n = rv(t, r);
  return n ? jn in n ? n[jn] : (
    // This is a very special case, if the prop is a getter defined by the
    // prototype, we should invoke it with the draft as context!
    (i = n.get) == null ? void 0 : i.call(e.draft_)
  ) : void 0;
}
function rv(e, t) {
  if (!(t in e))
    return;
  let r = qr(e);
  for (; r; ) {
    const n = Object.getOwnPropertyDescriptor(r, t);
    if (n)
      return n;
    r = qr(r);
  }
}
function iu(e) {
  e.modified_ || (e.modified_ = !0, e.parent_ && iu(e.parent_));
}
function ji(e) {
  e.copy_ || (e.assigned_ = /* @__PURE__ */ new Map(), e.copy_ = eu(
    e.base_,
    e.scope_.immer_.useStrictShallowCopy_
  ));
}
var Zb = class {
  constructor(e) {
    this.autoFreeze_ = !0, this.useStrictShallowCopy_ = !1, this.useStrictIteration_ = !1, this.produce = (t, r, n) => {
      if (yr(t) && !yr(r)) {
        const a = r;
        r = t;
        const o = this;
        return function(l = a, ...c) {
          return o.produce(l, (s) => r.call(this, s, ...c));
        };
      }
      yr(r) || Ye(6), n !== void 0 && !yr(n) && Ye(7);
      let i;
      if (ht(t)) {
        const a = $c(this), o = au(a, t, void 0);
        let u = !0;
        try {
          i = r(o), u = !1;
        } finally {
          u ? ru(a) : nu(a);
        }
        return Mc(a, n), Lc(i, a);
      } else if (!t || !Uu(t)) {
        if (i = r(t), i === void 0 && (i = t), i === qh && (i = void 0), this.autoFreeze_ && Ku(i, !0), n) {
          const a = [], o = [];
          kr(tu).generateReplacementPatches_(t, i, {
            patches_: a,
            inversePatches_: o
          }), n(a, o);
        }
        return i;
      } else
        Ye(1, t);
    }, this.produceWithPatches = (t, r) => {
      if (yr(t))
        return (o, ...u) => this.produceWithPatches(o, (l) => t(l, ...u));
      let n, i;
      return [this.produce(t, r, (o, u) => {
        n = o, i = u;
      }), n, i];
    }, mo(e == null ? void 0 : e.autoFreeze) && this.setAutoFreeze(e.autoFreeze), mo(e == null ? void 0 : e.useStrictShallowCopy) && this.setUseStrictShallowCopy(e.useStrictShallowCopy), mo(e == null ? void 0 : e.useStrictIteration) && this.setUseStrictIteration(e.useStrictIteration);
  }
  createDraft(e) {
    ht(e) || Ye(8), Ft(e) && (e = nt(e));
    const t = $c(this), r = au(t, e, void 0);
    return r[Be].isManual_ = !0, nu(t), r;
  }
  finishDraft(e, t) {
    const r = e && e[Be];
    (!r || !r.isManual_) && Ye(9);
    const { scope_: n } = r;
    return Mc(n, t), Lc(void 0, n);
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
    const n = kr(tu).applyPatches_;
    return Ft(e) ? n(e, t) : this.produce(
      e,
      (i) => n(i, t)
    );
  }
};
function au(e, t, r, n) {
  const [i, a] = Da(t) ? kr(Ui).proxyMap_(t, r) : ja(t) ? kr(Ui).proxySet_(t, r) : qb(t, r);
  return ((r == null ? void 0 : r.scope_) ?? Qh()).drafts_.push(i), a.callbacks_ = (r == null ? void 0 : r.callbacks_) ?? [], a.key_ = n, r && n !== void 0 ? Gb(r, a, n) : a.callbacks_.push(function(l) {
    var s;
    (s = l.mapSetPlugin_) == null || s.fixSetContents(a);
    const { patchPlugin_: c } = l;
    a.modified_ && c && c.generatePatches_(a, [], l);
  }), i;
}
function nt(e) {
  return Ft(e) || Ye(10, e), nv(e);
}
function nv(e) {
  if (!ht(e) || Na(e))
    return e;
  const t = e[Be];
  let r, n = !0;
  if (t) {
    if (!t.modified_)
      return t.base_;
    t.finalized_ = !0, r = eu(e, t.scope_.immer_.useStrictShallowCopy_), n = t.scope_.immer_.shouldUseStrictIteration();
  } else
    r = eu(e, !0);
  return Ca(
    r,
    (i, a) => {
      Wi(r, i, nv(a));
    },
    n
  ), t && (t.finalized_ = !1), r;
}
var Qb = new Zb(), iv = Qb.produce, G = (e) => e;
function av(e) {
  return ({ dispatch: r, getState: n }) => (i) => (a) => typeof a == "function" ? a(r, n, e) : i(a);
}
var Jb = av(), ew = av, tw = typeof window < "u" && window.__REDUX_DEVTOOLS_EXTENSION_COMPOSE__ ? window.__REDUX_DEVTOOLS_EXTENSION_COMPOSE__ : function() {
  if (arguments.length !== 0)
    return typeof arguments[0] == "object" ? zi : zi.apply(null, arguments);
}, rw = (e) => e && typeof e.match == "function";
function Qe(e, t) {
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
  return r.toString = () => `${e}`, r.type = e, r.match = (n) => Wu(n) && n.type === e, r;
}
function nw(e) {
  return typeof e == "function" && "type" in e && // hasMatchFunction only wants Matchers but I don't see the point in rewriting it
  rw(e);
}
function iw(e) {
  const t = e ? `${e}`.split("/") : [], r = t[t.length - 1] || "actionCreator";
  return `Detected an action creator with type "${e || "unknown"}" being dispatched.
Make sure you're calling the action creator before dispatching, i.e. \`dispatch(${r}())\` instead of \`dispatch(${r})\`. This is necessary even if the action has no payload.`;
}
function aw(e = {}) {
  if (process.env.NODE_ENV === "production")
    return () => (r) => (n) => r(n);
  const {
    isActionCreator: t = nw
  } = e;
  return () => (r) => (n) => (t(n) && console.warn(iw(n.type)), r(n));
}
function ov(e, t) {
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
var uv = class _n extends Array {
  constructor(...t) {
    super(...t), Object.setPrototypeOf(this, _n.prototype);
  }
  static get [Symbol.species]() {
    return _n;
  }
  concat(...t) {
    return super.concat.apply(this, t);
  }
  prepend(...t) {
    return t.length === 1 && Array.isArray(t[0]) ? new _n(...t[0].concat(this)) : new _n(...t.concat(this));
  }
};
function zc(e) {
  return ht(e) ? iv(e, () => {
  }) : e;
}
function yi(e, t, r) {
  return e.has(t) ? e.get(t) : e.set(t, r(t)).get(t);
}
function ow(e) {
  return typeof e != "object" || e == null || Object.isFrozen(e);
}
function uw(e, t, r) {
  const n = lv(e, t, r);
  return {
    detectMutations() {
      return cv(e, t, n, r);
    }
  };
}
function lv(e, t = [], r, n = "", i = /* @__PURE__ */ new Set()) {
  const a = {
    value: r
  };
  if (!e(r) && !i.has(r)) {
    i.add(r), a.children = {};
    const o = t.length > 0;
    for (const u in r) {
      const l = n ? n + "." + u : u;
      o && t.some((s) => s instanceof RegExp ? s.test(l) : l === s) || (a.children[u] = lv(e, t, r[u], l));
    }
  }
  return a;
}
function cv(e, t = [], r, n, i = !1, a = "") {
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
    if (c && t.some((v) => v instanceof RegExp ? v.test(f) : f === v))
      continue;
    const d = cv(e, t, r.children[s], n[s], u, f);
    if (d.wasMutated)
      return d;
  }
  return {
    wasMutated: !1
  };
}
function lw(e = {}) {
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
      isImmutable: n = ow,
      ignoredPaths: i,
      warnAfter: a = 32
    } = e;
    const o = uw.bind(null, n, i);
    return ({
      getState: u
    }) => {
      let l = u(), c = o(l), s;
      return (f) => (d) => {
        const h = ov(a, "ImmutableStateInvariantMiddleware");
        h.measureTime(() => {
          if (l = u(), s = c.detectMutations(), c = o(l), s.wasMutated)
            throw new Error(process.env.NODE_ENV === "production" ? Z(19) : `A state mutation was detected between dispatches, in the path '${s.path || ""}'.  This may cause incorrect behavior. (https://redux.js.org/style-guide/style-guide#do-not-mutate-state)`);
        });
        const v = f(d);
        return h.measureTime(() => {
          if (l = u(), s = c.detectMutations(), c = o(l), s.wasMutated)
            throw new Error(process.env.NODE_ENV === "production" ? Z(20) : `A state mutation was detected inside a dispatch, in the path: ${s.path || ""}. Take a look at the reducer(s) handling the action ${t(d)}. (https://redux.js.org/style-guide/style-guide#do-not-mutate-state)`);
        }), h.warnIfExceeded(), v;
      };
    };
  }
}
function sv(e) {
  const t = typeof e;
  return e == null || t === "string" || t === "boolean" || t === "number" || Array.isArray(e) || Kn(e);
}
function ou(e, t = "", r = sv, n, i = [], a) {
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
      if (typeof s == "object" && (o = ou(s, f, r, n, i, a), o))
        return o;
    }
  }
  return a && fv(e) && a.add(e), !1;
}
function fv(e) {
  if (!Object.isFrozen(e)) return !1;
  for (const t of Object.values(e))
    if (!(typeof t != "object" || t === null) && !fv(t))
      return !1;
  return !0;
}
function cw(e = {}) {
  if (process.env.NODE_ENV === "production")
    return () => (t) => (r) => t(r);
  {
    const {
      isSerializable: t = sv,
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
      if (!Wu(h))
        return d(h);
      const v = d(h), p = ov(o, "SerializableStateInvariantMiddleware");
      return !l && !(n.length && n.indexOf(h.type) !== -1) && p.measureTime(() => {
        const m = ou(h, "", t, r, i, s);
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
      }), u || (p.measureTime(() => {
        const m = f.getState(), y = ou(m, "", t, r, a, s);
        if (y) {
          const {
            keyPath: b,
            value: x
          } = y;
          console.error(`A non-serializable value was detected in the state, in the path: \`${b}\`. Value:`, x, `
Take a look at the reducer(s) handling this action type: ${h.type}.
(See https://redux.js.org/faq/organizing-state#can-i-put-functions-promises-or-other-non-serializable-items-in-my-store-state)`);
        }
      }), p.warnIfExceeded()), v;
    };
  }
}
function gi(e) {
  return typeof e == "boolean";
}
var sw = () => function(t) {
  const {
    thunk: r = !0,
    immutableCheck: n = !0,
    serializableCheck: i = !0,
    actionCreatorCheck: a = !0
  } = t ?? {};
  let o = new uv();
  if (r && (gi(r) ? o.push(Jb) : o.push(ew(r.extraArgument))), process.env.NODE_ENV !== "production") {
    if (n) {
      let u = {};
      gi(n) || (u = n), o.unshift(lw(u));
    }
    if (i) {
      let u = {};
      gi(i) || (u = i), o.push(cw(u));
    }
    if (a) {
      let u = {};
      gi(a) || (u = a), o.unshift(aw(u));
    }
  }
  return o;
}, dv = "RTK_autoBatch", re = () => (e) => ({
  payload: e,
  meta: {
    [dv]: !0
  }
}), Bc = (e) => (t) => {
  setTimeout(t, e);
}, fw = (e, t) => (r) => {
  let n = !1;
  const i = () => {
    n || (n = !0, cancelAnimationFrame(a), clearTimeout(o), r());
  }, a = e(i), o = setTimeout(i, t);
}, hv = (e = {
  type: "raf"
}) => (t) => (...r) => {
  const n = t(...r);
  let i = !0, a = !1, o = !1;
  const u = /* @__PURE__ */ new Set(), l = e.type === "tick" ? queueMicrotask : e.type === "raf" ? (
    // requestAnimationFrame won't exist in SSR environments. Fall back to a vague approximation just to keep from erroring.
    typeof window < "u" && window.requestAnimationFrame ? fw(window.requestAnimationFrame, 100) : Bc(10)
  ) : e.type === "callback" ? e.queueNotification : Bc(e.timeout), c = () => {
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
        return i = !((f = s == null ? void 0 : s.meta) != null && f[dv]), a = !i, a && (o || (o = !0, l(c))), n.dispatch(s);
      } finally {
        i = !0;
      }
    }
  });
}, dw = (e) => function(r) {
  const {
    autoBatch: n = !0
  } = r ?? {};
  let i = new uv(e);
  return n && i.push(hv(typeof n == "object" ? n : void 0)), i;
};
function hw(e) {
  const t = sw(), {
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
  else if (Kn(r))
    l = Yh(r);
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
  if (process.env.NODE_ENV !== "production" && c.some((p) => typeof p != "function"))
    throw new Error(process.env.NODE_ENV === "production" ? Z(4) : "each middleware provided to configureStore must be a function");
  if (process.env.NODE_ENV !== "production" && a) {
    let p = /* @__PURE__ */ new Set();
    c.forEach((m) => {
      if (p.has(m))
        throw new Error(process.env.NODE_ENV === "production" ? Z(42) : "Duplicate middleware references found when creating the store. Ensure that each middleware is only included once.");
      p.add(m);
    });
  }
  let s = zi;
  i && (s = tw({
    // Enable capture of stack traces for dispatched Redux actions
    trace: process.env.NODE_ENV !== "production",
    ...typeof i == "object" && i
  }));
  const f = Lb(...c), d = dw(f);
  if (process.env.NODE_ENV !== "production" && u && typeof u != "function")
    throw new Error(process.env.NODE_ENV === "production" ? Z(5) : "`enhancers` field must be a callback");
  let h = typeof u == "function" ? u(d) : d();
  if (process.env.NODE_ENV !== "production" && !Array.isArray(h))
    throw new Error(process.env.NODE_ENV === "production" ? Z(6) : "`enhancers` callback must return an array");
  if (process.env.NODE_ENV !== "production" && h.some((p) => typeof p != "function"))
    throw new Error(process.env.NODE_ENV === "production" ? Z(7) : "each enhancer provided to configureStore must be a function");
  process.env.NODE_ENV !== "production" && c.length && !h.includes(f) && console.error("middlewares were provided, but middleware enhancer was not included in final enhancers - make sure to call `getDefaultEnhancers`");
  const v = s(...h);
  return Gh(l, o, v);
}
function vv(e) {
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
function vw(e) {
  return typeof e == "function";
}
function pw(e, t) {
  if (process.env.NODE_ENV !== "production" && typeof t == "object")
    throw new Error(process.env.NODE_ENV === "production" ? Z(8) : "The object notation for `createReducer` has been removed. Please use the 'builder callback' notation instead: https://redux-toolkit.js.org/api/createReducer");
  let [r, n, i] = vv(t), a;
  if (vw(e))
    a = () => zc(e());
  else {
    const u = zc(e);
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
        if (Ft(s)) {
          const h = f(s, l);
          return h === void 0 ? s : h;
        } else {
          if (ht(s))
            return iv(s, (d) => f(d, l));
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
var mw = "ModuleSymbhasOwnPr-0123456789ABCDEFGHNRVfgctiUvz_KqYTJkLxpZXIjQW", yw = (e = 21) => {
  let t = "", r = e;
  for (; r--; )
    t += mw[Math.random() * 64 | 0];
  return t;
}, gw = /* @__PURE__ */ Symbol.for("rtk-slice-createasyncthunk");
function bw(e, t) {
  return `${e}/${t}`;
}
function ww({
  creators: e
} = {}) {
  var r;
  const t = (r = e == null ? void 0 : e.asyncThunk) == null ? void 0 : r[gw];
  return function(i) {
    const {
      name: a,
      reducerPath: o = a
    } = i;
    if (!a)
      throw new Error(process.env.NODE_ENV === "production" ? Z(11) : "`name` is a required option for createSlice");
    typeof process < "u" && process.env.NODE_ENV === "development" && i.initialState === void 0 && console.error("You must provide an `initialState` value that is not `undefined`. You may have misspelled `initialState`");
    const u = (typeof i.reducers == "function" ? i.reducers(Ow()) : i.reducers) || {}, l = Object.keys(u), c = {
      sliceCaseReducersByName: {},
      sliceCaseReducersByType: {},
      actionCreators: {},
      sliceMatchers: []
    }, s = {
      addCase(O, A) {
        const g = typeof O == "string" ? O : O.type;
        if (!g)
          throw new Error(process.env.NODE_ENV === "production" ? Z(12) : "`context.addCase` cannot be called with an empty action type");
        if (g in c.sliceCaseReducersByType)
          throw new Error(process.env.NODE_ENV === "production" ? Z(13) : "`context.addCase` cannot be called with two reducers for the same action type: " + g);
        return c.sliceCaseReducersByType[g] = A, s;
      },
      addMatcher(O, A) {
        return c.sliceMatchers.push({
          matcher: O,
          reducer: A
        }), s;
      },
      exposeAction(O, A) {
        return c.actionCreators[O] = A, s;
      },
      exposeCaseReducer(O, A) {
        return c.sliceCaseReducersByName[O] = A, s;
      }
    };
    l.forEach((O) => {
      const A = u[O], g = {
        reducerName: O,
        type: bw(a, O),
        createNotation: typeof i.reducers == "function"
      };
      Sw(A) ? Pw(g, A, s, t) : Aw(g, A, s);
    });
    function f() {
      if (process.env.NODE_ENV !== "production" && typeof i.extraReducers == "object")
        throw new Error(process.env.NODE_ENV === "production" ? Z(14) : "The object notation for `createSlice.extraReducers` has been removed. Please use the 'builder callback' notation instead: https://redux-toolkit.js.org/api/createSlice");
      const [O = {}, A = [], g = void 0] = typeof i.extraReducers == "function" ? vv(i.extraReducers) : [i.extraReducers], E = {
        ...O,
        ...c.sliceCaseReducersByType
      };
      return pw(i.initialState, (_) => {
        for (let I in E)
          _.addCase(I, E[I]);
        for (let I of c.sliceMatchers)
          _.addMatcher(I.matcher, I.reducer);
        for (let I of A)
          _.addMatcher(I.matcher, I.reducer);
        g && _.addDefaultCase(g);
      });
    }
    const d = (O) => O, h = /* @__PURE__ */ new Map(), v = /* @__PURE__ */ new WeakMap();
    let p;
    function m(O, A) {
      return p || (p = f()), p(O, A);
    }
    function y() {
      return p || (p = f()), p.getInitialState();
    }
    function b(O, A = !1) {
      function g(_) {
        let I = _[O];
        if (typeof I > "u") {
          if (A)
            I = yi(v, g, y);
          else if (process.env.NODE_ENV !== "production")
            throw new Error(process.env.NODE_ENV === "production" ? Z(15) : "selectSlice returned undefined for an uninjected slice reducer");
        }
        return I;
      }
      function E(_ = d) {
        const I = yi(h, A, () => /* @__PURE__ */ new WeakMap());
        return yi(I, _, () => {
          const C = {};
          for (const [T, P] of Object.entries(i.selectors ?? {}))
            C[T] = xw(P, _, () => yi(v, _, y), A);
          return C;
        });
      }
      return {
        reducerPath: O,
        getSelectors: E,
        get selectors() {
          return E(g);
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
      injectInto(O, {
        reducerPath: A,
        ...g
      } = {}) {
        const E = A ?? o;
        return O.inject({
          reducerPath: E,
          reducer: m
        }, g), {
          ...x,
          ...b(E, !0)
        };
      }
    };
    return x;
  };
}
function xw(e, t, r, n) {
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
var $e = /* @__PURE__ */ ww();
function Ow() {
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
function Aw({
  type: e,
  reducerName: t,
  createNotation: r
}, n, i) {
  let a, o;
  if ("reducer" in n) {
    if (r && !Ew(n))
      throw new Error(process.env.NODE_ENV === "production" ? Z(17) : "Please use the `create.preparedReducer` notation for prepared action creators with the `create` notation.");
    a = n.reducer, o = n.prepare;
  } else
    a = n;
  i.addCase(e, a).exposeCaseReducer(t, a).exposeAction(t, o ? Qe(e, o) : Qe(e));
}
function Sw(e) {
  return e._reducerDefinitionType === "asyncThunk";
}
function Ew(e) {
  return e._reducerDefinitionType === "reducerWithPrepare";
}
function Pw({
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
    fulfilled: o || bi,
    pending: u || bi,
    rejected: l || bi,
    settled: c || bi
  });
}
function bi() {
}
var _w = "task", pv = "listener", mv = "completed", Hu = "cancelled", Iw = `task-${Hu}`, kw = `task-${mv}`, uu = `${pv}-${Hu}`, Cw = `${pv}-${mv}`, $a = class {
  constructor(e) {
    si(this, "code");
    si(this, "name", "TaskAbortError");
    si(this, "message");
    this.code = e, this.message = `${_w} ${Hu} (reason: ${e})`;
  }
}, Gu = (e, t) => {
  if (typeof e != "function")
    throw new TypeError(process.env.NODE_ENV === "production" ? Z(32) : `${t} is not a function`);
}, Hi = () => {
}, yv = (e, t = Hi) => (e.catch(t), e), gv = (e, t) => (e.addEventListener("abort", t, {
  once: !0
}), () => e.removeEventListener("abort", t)), Sr = (e) => {
  if (e.aborted)
    throw new $a(e.reason);
};
function bv(e, t) {
  let r = Hi;
  return new Promise((n, i) => {
    const a = () => i(new $a(e.reason));
    if (e.aborted) {
      a();
      return;
    }
    r = gv(e, a), t.finally(() => r()).then(n, i);
  }).finally(() => {
    r = Hi;
  });
}
var Tw = async (e, t) => {
  try {
    return await Promise.resolve(), {
      status: "ok",
      value: await e()
    };
  } catch (r) {
    return {
      status: r instanceof $a ? "cancelled" : "rejected",
      error: r
    };
  } finally {
    t == null || t();
  }
}, Gi = (e) => (t) => yv(bv(e, t).then((r) => (Sr(e), r))), wv = (e) => {
  const t = Gi(e);
  return (r) => t(new Promise((n) => setTimeout(n, r)));
}, {
  assign: Hr
} = Object, Fc = {}, Gn = "listenerMiddleware", Dw = (e, t) => {
  const r = (n) => gv(e, () => n.abort(e.reason));
  return (n, i) => {
    Gu(n, "taskExecutor");
    const a = new AbortController();
    r(a);
    const o = Tw(async () => {
      Sr(e), Sr(a.signal);
      const u = await n({
        pause: Gi(a.signal),
        delay: wv(a.signal),
        signal: a.signal
      });
      return Sr(a.signal), u;
    }, () => a.abort(kw));
    return i != null && i.autoJoin && t.push(o.catch(Hi)), {
      result: Gi(e)(o),
      cancel() {
        a.abort(Iw);
      }
    };
  };
}, jw = (e, t) => {
  const r = async (n, i) => {
    Sr(t);
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
      const l = await bv(t, Promise.race(u));
      return Sr(t), l;
    } finally {
      a();
    }
  };
  return (n, i) => yv(r(n, i));
}, xv = (e) => {
  let {
    type: t,
    actionCreator: r,
    matcher: n,
    predicate: i,
    effect: a
  } = e;
  if (t)
    i = Qe(t).match;
  else if (r)
    t = r.type, i = r.match;
  else if (n)
    i = n;
  else if (!i) throw new Error(process.env.NODE_ENV === "production" ? Z(21) : "Creating or removing a listener requires one of the known fields for matching an action");
  return Gu(a, "options.listener"), {
    predicate: i,
    type: t,
    effect: a
  };
}, Ov = /* @__PURE__ */ Hr((e) => {
  const {
    type: t,
    predicate: r,
    effect: n
  } = xv(e);
  return {
    id: yw(),
    effect: n,
    type: t,
    predicate: r,
    pending: /* @__PURE__ */ new Set(),
    unsubscribe: () => {
      throw new Error(process.env.NODE_ENV === "production" ? Z(22) : "Unsubscribe not initialized");
    }
  };
}, {
  withTypes: () => Ov
}), Wc = (e, t) => {
  const {
    type: r,
    effect: n,
    predicate: i
  } = xv(t);
  return Array.from(e.values()).find((a) => (typeof r == "string" ? a.type === r : a.predicate === i) && a.effect === n);
}, lu = (e) => {
  e.pending.forEach((t) => {
    t.abort(uu);
  });
}, Nw = (e, t) => () => {
  for (const r of t.keys())
    lu(r);
  e.clear();
}, Uc = (e, t, r) => {
  try {
    e(t, r);
  } catch (n) {
    setTimeout(() => {
      throw n;
    }, 0);
  }
}, Av = /* @__PURE__ */ Hr(/* @__PURE__ */ Qe(`${Gn}/add`), {
  withTypes: () => Av
}), Mw = /* @__PURE__ */ Qe(`${Gn}/removeAll`), Sv = /* @__PURE__ */ Hr(/* @__PURE__ */ Qe(`${Gn}/remove`), {
  withTypes: () => Sv
}), $w = (...e) => {
  console.error(`${Gn}/error`, ...e);
}, Yn = (e = {}) => {
  const t = /* @__PURE__ */ new Map(), r = /* @__PURE__ */ new Map(), n = (h) => {
    const v = r.get(h) ?? 0;
    r.set(h, v + 1);
  }, i = (h) => {
    const v = r.get(h) ?? 1;
    v === 1 ? r.delete(h) : r.set(h, v - 1);
  }, {
    extra: a,
    onError: o = $w
  } = e;
  Gu(o, "onError");
  const u = (h) => (h.unsubscribe = () => t.delete(h.id), t.set(h.id, h), (v) => {
    h.unsubscribe(), v != null && v.cancelActive && lu(h);
  }), l = (h) => {
    const v = Wc(t, h) ?? Ov(h);
    return u(v);
  };
  Hr(l, {
    withTypes: () => l
  });
  const c = (h) => {
    const v = Wc(t, h);
    return v && (v.unsubscribe(), h.cancelActive && lu(v)), !!v;
  };
  Hr(c, {
    withTypes: () => c
  });
  const s = async (h, v, p, m) => {
    const y = new AbortController(), b = jw(l, y.signal), x = [];
    try {
      h.pending.add(y), n(h), await Promise.resolve(h.effect(
        v,
        // Use assign() rather than ... to avoid extra helper functions added to bundle
        Hr({}, p, {
          getOriginalState: m,
          condition: (O, A) => b(O, A).then(Boolean),
          take: b,
          delay: wv(y.signal),
          pause: Gi(y.signal),
          extra: a,
          signal: y.signal,
          fork: Dw(y.signal, x),
          unsubscribe: h.unsubscribe,
          subscribe: () => {
            t.set(h.id, h);
          },
          cancelActiveListeners: () => {
            h.pending.forEach((O, A, g) => {
              O !== y && (O.abort(uu), g.delete(O));
            });
          },
          cancel: () => {
            y.abort(uu), h.pending.delete(y);
          },
          throwIfCancelled: () => {
            Sr(y.signal);
          }
        })
      ));
    } catch (O) {
      O instanceof $a || Uc(o, O, {
        raisedBy: "effect"
      });
    } finally {
      await Promise.all(x), y.abort(Cw), i(h), h.pending.delete(y);
    }
  }, f = Nw(t, r);
  return {
    middleware: (h) => (v) => (p) => {
      if (!Wu(p))
        return v(p);
      if (Av.match(p))
        return l(p.payload);
      if (Mw.match(p)) {
        f();
        return;
      }
      if (Sv.match(p))
        return c(p.payload);
      let m = h.getState();
      const y = () => {
        if (m === Fc)
          throw new Error(process.env.NODE_ENV === "production" ? Z(23) : `${Gn}: getOriginalState can only be called synchronously`);
        return m;
      };
      let b;
      try {
        if (b = v(p), t.size > 0) {
          const x = h.getState(), O = Array.from(t.values());
          for (const A of O) {
            let g = !1;
            try {
              g = A.predicate(p, x, m);
            } catch (E) {
              g = !1, Uc(o, E, {
                raisedBy: "predicate"
              });
            }
            g && s(A, p, h, y);
          }
        }
      } finally {
        m = Fc;
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
var Lw = {
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
}, Ev = $e({
  name: "chartLayout",
  initialState: Lw,
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
}), La = Ev.actions, Rw = La.setMargin, zw = La.setLayout, Bw = La.setChartSize, Fw = La.setScale, Ww = Ev.reducer;
function Pv(e, t, r) {
  return Array.isArray(e) && e && t + r !== 0 ? e.slice(t, r + 1) : e;
}
function Y(e) {
  return Number.isFinite(e);
}
function _t(e) {
  return typeof e == "number" && e > 0 && Number.isFinite(e);
}
function Vc(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Vr(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Vc(Object(r), !0).forEach(function(n) {
      Uw(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Vc(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function Uw(e, t, r) {
  return (t = Vw(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function Vw(e) {
  var t = Kw(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Kw(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function we(e, t, r) {
  return xe(e) || xe(t) ? r : Pt(t) ? jr(e, t, r) : typeof t == "function" ? t(e) : r;
}
var Hw = (e, t, r) => {
  if (t && r) {
    var n = r.width, i = r.height, a = t.align, o = t.verticalAlign, u = t.layout;
    if ((u === "vertical" || u === "horizontal" && o === "middle") && a !== "center" && M(e[a]))
      return Vr(Vr({}, e), {}, {
        [a]: e[a] + (n || 0)
      });
    if ((u === "horizontal" || u === "vertical" && a === "center") && o !== "middle" && M(e[o]))
      return Vr(Vr({}, e), {}, {
        [o]: e[o] + (i || 0)
      });
  }
  return e;
}, Ht = (e, t) => e === "horizontal" && t === "xAxis" || e === "vertical" && t === "yAxis" || e === "centric" && t === "angleAxis" || e === "radial" && t === "radiusAxis", _v = (e, t, r, n) => {
  if (n)
    return e.map((u) => u.coordinate);
  var i, a, o = e.map((u) => (u.coordinate === t && (i = !0), u.coordinate === r && (a = !0), u.coordinate));
  return i || o.push(t), a || o.push(r), o;
}, Iv = (e, t, r) => {
  if (!e)
    return null;
  var n = e.duplicateDomain, i = e.type, a = e.range, o = e.scale, u = e.realScaleType, l = e.isCategorical, c = e.categoricalDomain, s = e.tickCount, f = e.ticks, d = e.niceTicks, h = e.axisType;
  if (!o)
    return null;
  var v = u === "scaleBand" && o.bandwidth ? o.bandwidth() / 2 : 2, p = i === "category" && o.bandwidth ? o.bandwidth() / v : 0;
  if (p = h === "angleAxis" && a && a.length >= 2 ? rt(a[0] - a[1]) * 2 * p : p, f || d) {
    var m = (f || d || []).map((y, b) => {
      var x = n ? n.indexOf(y) : y, O = o.map(x);
      return Y(O) ? {
        // If the scaleContent is not a number, the coordinate will be NaN.
        // That could be the case for example with a PointScale and a string as domain.
        coordinate: O + p,
        value: y,
        offset: p,
        index: b
      } : null;
    }).filter(Fe);
    return m;
  }
  return l && c ? c.map((y, b) => {
    var x = o.map(y);
    return Y(x) ? {
      coordinate: x + p,
      value: y,
      index: b,
      offset: p
    } : null;
  }).filter(Fe) : o.ticks && s != null ? o.ticks(s).map((y, b) => {
    var x = o.map(y);
    return Y(x) ? {
      coordinate: x + p,
      value: y,
      index: b,
      offset: p
    } : null;
  }).filter(Fe) : o.domain().map((y, b) => {
    var x = o.map(y);
    return Y(x) ? {
      coordinate: x + p,
      // @ts-expect-error can't use Date as an index
      value: n ? n[y] : y,
      index: b,
      offset: p
    } : null;
  }).filter(Fe);
}, Gw = (e) => {
  var t, r = e.length;
  if (!(r <= 0)) {
    var n = (t = e[0]) === null || t === void 0 ? void 0 : t.length;
    if (!(n == null || n <= 0))
      for (var i = 0; i < n; ++i)
        for (var a = 0, o = 0, u = 0; u < r; ++u) {
          var l = e[u], c = l == null ? void 0 : l[i];
          if (c != null) {
            var s = c[1], f = c[0], d = Bt(s) ? f : s;
            d >= 0 ? (c[0] = a, a += d, c[1] = a) : (c[0] = o, o += d, c[1] = o);
          }
        }
  }
}, Yw = (e) => {
  var t, r = e.length;
  if (!(r <= 0)) {
    var n = (t = e[0]) === null || t === void 0 ? void 0 : t.length;
    if (!(n == null || n <= 0))
      for (var i = 0; i < n; ++i)
        for (var a = 0, o = 0; o < r; ++o) {
          var u = e[o], l = u == null ? void 0 : u[i];
          if (l != null) {
            var c = Bt(l[1]) ? l[0] : l[1];
            c >= 0 ? (l[0] = a, a += c, l[1] = a) : (l[0] = 0, l[1] = 0);
          }
        }
  }
}, qw = {
  sign: Gw,
  // @ts-expect-error definitelytyped types are incorrect
  expand: qg,
  // @ts-expect-error definitelytyped types are incorrect
  none: _r,
  // @ts-expect-error definitelytyped types are incorrect
  silhouette: Xg,
  // @ts-expect-error definitelytyped types are incorrect
  wiggle: Zg,
  positive: Yw
}, Xw = (e, t, r) => {
  var n, i = (n = qw[r]) !== null && n !== void 0 ? n : _r, a = Yg().keys(t).value((u, l) => Number(we(u, l, 0))).order(Ho).offset(i), o = a(e);
  return o.forEach((u, l) => {
    u.forEach((c, s) => {
      var f = we(e[s], t[l], 0);
      Array.isArray(f) && f.length === 2 && M(f[0]) && M(f[1]) && (c[0] = f[0], c[1] = f[1]);
    });
  }), o;
};
function Kc(e) {
  var t = e.axis, r = e.ticks, n = e.bandSize, i = e.entry, a = e.index, o = e.dataKey;
  if (t.type === "category") {
    if (!t.allowDuplicatedCategory && t.dataKey && !xe(i[t.dataKey])) {
      var u = kh(r, "value", i[t.dataKey]);
      if (u)
        return u.coordinate + n / 2;
    }
    return r != null && r[a] ? r[a].coordinate + n / 2 : null;
  }
  var l = we(i, xe(o) ? t.dataKey : o), c = t.scale.map(l);
  return M(c) ? c : null;
}
var Zw = (e) => {
  var t = e.flat(2).filter(M);
  return [Math.min(...t), Math.max(...t)];
}, Qw = (e) => [e[0] === 1 / 0 ? 0 : e[0], e[1] === -1 / 0 ? 0 : e[1]], Jw = (e, t, r) => {
  if (!(e == null || Object.keys(e).length === 0))
    return Qw(Object.keys(e).reduce((n, i) => {
      var a = e[i];
      if (!a)
        return n;
      var o = a.stackedData, u = o.reduce((l, c) => {
        var s = Pv(c, t, r), f = Zw(s);
        return !Y(f[0]) || !Y(f[1]) ? l : [Math.min(l[0], f[0]), Math.max(l[1], f[1])];
      }, [1 / 0, -1 / 0]);
      return [Math.min(u[0], n[0]), Math.max(u[1], n[1])];
    }, [1 / 0, -1 / 0]));
}, Hc = /^dataMin[\s]*-[\s]*([0-9]+([.]{1}[0-9]+){0,1})$/, Gc = /^dataMax[\s]*\+[\s]*([0-9]+([.]{1}[0-9]+){0,1})$/, Yi = (e, t, r) => {
  if (e && e.scale && e.scale.bandwidth) {
    var n = e.scale.bandwidth();
    if (!r || n > 0)
      return n;
  }
  if (e && t && t.length >= 2) {
    for (var i = Ia(t, (s) => s.coordinate), a = 1 / 0, o = 1, u = i.length; o < u; o++) {
      var l = i[o], c = i[o - 1];
      a = Math.min(((l == null ? void 0 : l.coordinate) || 0) - ((c == null ? void 0 : c.coordinate) || 0), a);
    }
    return a === 1 / 0 ? 0 : a;
  }
  return r ? void 0 : 0;
};
function Yc(e) {
  var t = e.tooltipEntrySettings, r = e.dataKey, n = e.payload, i = e.value, a = e.name;
  return Vr(Vr({}, t), {}, {
    dataKey: r,
    payload: n,
    value: i,
    name: a
  });
}
function kv(e, t) {
  if (e != null)
    return String(e);
  if (typeof t == "string")
    return t;
}
var ex = (e, t) => {
  if (t === "horizontal")
    return e.relativeX;
  if (t === "vertical")
    return e.relativeY;
}, tx = (e, t) => t === "centric" ? e.angle : e.radius, Gt = (e) => e.layout.width, Yt = (e) => e.layout.height, rx = (e) => e.layout.scale, Cv = (e) => e.layout.margin, Ra = S((e) => e.cartesianAxis.xAxis, (e) => Object.values(e)), za = S((e) => e.cartesianAxis.yAxis, (e) => Object.values(e)), nx = "data-recharts-item-index", ix = "data-recharts-item-id", qn = 60;
function qc(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function wi(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? qc(Object(r), !0).forEach(function(n) {
      ax(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : qc(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function ax(e, t, r) {
  return (t = ox(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function ox(e) {
  var t = ux(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function ux(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var lx = (e) => e.brush.height;
function cx(e) {
  var t = za(e);
  return t.reduce((r, n) => {
    if (n.orientation === "left" && !n.mirror && !n.hide) {
      var i = typeof n.width == "number" ? n.width : qn;
      return r + i;
    }
    return r;
  }, 0);
}
function sx(e) {
  var t = za(e);
  return t.reduce((r, n) => {
    if (n.orientation === "right" && !n.mirror && !n.hide) {
      var i = typeof n.width == "number" ? n.width : qn;
      return r + i;
    }
    return r;
  }, 0);
}
function fx(e) {
  var t = Ra(e);
  return t.reduce((r, n) => n.orientation === "top" && !n.mirror && !n.hide ? r + n.height : r, 0);
}
function dx(e) {
  var t = Ra(e);
  return t.reduce((r, n) => n.orientation === "bottom" && !n.mirror && !n.hide ? r + n.height : r, 0);
}
var Ce = S([Gt, Yt, Cv, lx, cx, sx, fx, dx, Hh, xb], (e, t, r, n, i, a, o, u, l, c) => {
  var s = {
    left: (r.left || 0) + i,
    right: (r.right || 0) + a
  }, f = {
    top: (r.top || 0) + o,
    bottom: (r.bottom || 0) + u
  }, d = wi(wi({}, f), s), h = d.bottom;
  d.bottom += n, d = Hw(d, l, c);
  var v = e - d.left - d.right, p = t - d.top - d.bottom;
  return wi(wi({
    brushBottom: h
  }, d), {}, {
    // never return negative values for height and width
    width: Math.max(v, 0),
    height: Math.max(p, 0)
  });
}), hx = S(Ce, (e) => ({
  x: e.left,
  y: e.top,
  width: e.width,
  height: e.height
})), Tv = S(Gt, Yt, (e, t) => ({
  x: 0,
  y: 0,
  width: e,
  height: t
})), vx = /* @__PURE__ */ Je(null), Ve = () => kt(vx) != null, Ba = (e) => e.brush, Fa = S([Ba, Ce, Cv], (e, t, r) => ({
  height: e.height,
  x: M(e.x) ? e.x : t.left,
  y: M(e.y) ? e.y : t.top + t.height + t.brushBottom - ((r == null ? void 0 : r.bottom) || 0),
  width: M(e.width) ? e.width : t.width
}));
function px(e, t, { signal: r, edges: n } = {}) {
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
  }, v = () => {
    l();
  }, p = function(...m) {
    if (r != null && r.aborted) return;
    i = this, a = m;
    const y = s == null;
    f(), o && y && l();
  };
  return p.schedule = f, p.cancel = h, p.flush = v, r == null || r.addEventListener("abort", h, { once: !0 }), p;
}
function mx(e, t = 0, r = {}) {
  typeof r != "object" && (r = {});
  const { leading: n = !1, trailing: i = !0, maxWait: a } = r, o = Array(2);
  n && (o[0] = "leading"), i && (o[1] = "trailing");
  let u, l = null;
  const c = px(function(...d) {
    u = e.apply(this, d), l = null;
  }, t, { edges: o }), s = function(...d) {
    return a != null && (l === null && (l = Date.now()), Date.now() - l >= a) ? (u = e.apply(this, d), l = Date.now(), c.cancel(), c.schedule(), u) : (c.apply(this, d), u);
  }, f = () => (c.flush(), u);
  return s.cancel = c.cancel, s.flush = f, s;
}
function yx(e, t = 0, r = {}) {
  const { leading: n = !0, trailing: i = !0 } = r;
  return mx(e, t, {
    leading: n,
    maxWait: t,
    trailing: i
  });
}
var qi = function(t, r) {
  for (var n = arguments.length, i = new Array(n > 2 ? n - 2 : 0), a = 2; a < n; a++)
    i[a - 2] = arguments[a];
  if (typeof console < "u" && console.warn && (r === void 0 && console.warn("LogUtils requires an error message argument"), !t))
    if (r === void 0)
      console.warn("Minified exception occurred; use the non-minified dev environment for the full error message and additional helpful warnings.");
    else {
      var o = 0;
      console.warn(r.replace(/%s/g, () => i[o++]));
    }
}, xt = {
  width: "100%",
  height: "100%",
  debounce: 0,
  minWidth: 0,
  initialDimension: {
    width: -1,
    height: -1
  }
}, Dv = (e, t, r) => {
  var n = r.width, i = n === void 0 ? xt.width : n, a = r.height, o = a === void 0 ? xt.height : a, u = r.aspect, l = r.maxHeight, c = Ir(i) ? e : Number(i), s = Ir(o) ? t : Number(o);
  return u && u > 0 && (c ? s = c / u : s && (c = s * u), l && s != null && s > l && (s = l)), {
    calculatedWidth: c,
    calculatedHeight: s
  };
}, gx = {
  width: 0,
  height: 0,
  overflow: "visible"
}, bx = {
  width: 0,
  overflowX: "visible"
}, wx = {
  height: 0,
  overflowY: "visible"
}, xx = {}, Ox = (e) => {
  var t = e.width, r = e.height, n = Ir(t), i = Ir(r);
  return n && i ? gx : n ? bx : i ? wx : xx;
};
function Ax(e) {
  var t = e.width, r = e.height, n = e.aspect, i = t, a = r;
  return i === void 0 && a === void 0 ? (i = xt.width, a = xt.height) : i === void 0 ? i = n && n > 0 ? void 0 : xt.width : a === void 0 && (a = n && n > 0 ? void 0 : xt.height), {
    width: i,
    height: a
  };
}
var Sx = ["aspect", "initialDimension", "width", "height", "minWidth", "minHeight", "maxHeight", "children", "debounce", "id", "className", "onResize", "style"];
function Xi() {
  return Xi = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Xi.apply(null, arguments);
}
function Xc(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Zc(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Xc(Object(r), !0).forEach(function(n) {
      Ex(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Xc(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function Ex(e, t, r) {
  return (t = Px(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function Px(e) {
  var t = _x(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function _x(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function Ix(e, t) {
  return Dx(e) || Tx(e, t) || Cx(e, t) || kx();
}
function kx() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function Cx(e, t) {
  if (e) {
    if (typeof e == "string") return Qc(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Qc(e, t) : void 0;
  }
}
function Qc(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function Tx(e, t) {
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
function Dx(e) {
  if (Array.isArray(e)) return e;
}
function jx(e, t) {
  if (e == null) return {};
  var r, n, i = Nx(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function Nx(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var jv = /* @__PURE__ */ Je(xt.initialDimension);
function Mx(e) {
  return _t(e.width) && _t(e.height);
}
function Nv(e) {
  var t = e.children, r = e.width, n = e.height, i = Kt(() => ({
    width: r,
    height: n
  }), [r, n]);
  return Mx(i) ? /* @__PURE__ */ w.createElement(jv.Provider, {
    value: i
  }, t) : null;
}
var Yu = () => kt(jv), $x = /* @__PURE__ */ Me((e, t) => {
  var r = e.aspect, n = e.initialDimension, i = n === void 0 ? xt.initialDimension : n, a = e.width, o = e.height, u = e.minWidth, l = u === void 0 ? xt.minWidth : u, c = e.minHeight, s = e.maxHeight, f = e.children, d = e.debounce, h = d === void 0 ? xt.debounce : d, v = e.id, p = e.className, m = e.onResize, y = e.style, b = y === void 0 ? {} : y, x = jx(e, Sx), O = V(null), A = V();
  A.current = m, ah(t, () => O.current);
  var g = fe({
    containerWidth: i.width,
    containerHeight: i.height
  }), E = Ix(g, 2), _ = E[0], I = E[1], C = ee((K, H) => {
    I((B) => {
      var X = Math.round(K), W = Math.round(H);
      return B.containerWidth === X && B.containerHeight === W ? B : {
        containerWidth: X,
        containerHeight: W
      };
    });
  }, []);
  he(() => {
    if (O.current == null || typeof ResizeObserver > "u")
      return nn;
    var K = (Te) => {
      var Ee, se = Te[0];
      if (se != null) {
        var Ke = se.contentRect, He = Ke.width, ct = Ke.height;
        C(He, ct), (Ee = A.current) === null || Ee === void 0 || Ee.call(A, He, ct);
      }
    };
    h > 0 && (K = yx(K, h, {
      trailing: !0,
      leading: !1
    }));
    var H = new ResizeObserver(K), B = O.current.getBoundingClientRect(), X = B.width, W = B.height;
    return C(X, W), H.observe(O.current), () => {
      H.disconnect();
    };
  }, [C, h]);
  var T = _.containerWidth, P = _.containerHeight;
  qi(!r || r > 0, "The aspect(%s) must be greater than zero.", r);
  var z = Dv(T, P, {
    width: a,
    height: o,
    aspect: r,
    maxHeight: s
  }), $ = z.calculatedWidth, Q = z.calculatedHeight;
  return qi(T < 0 || P < 0 || $ != null && $ > 0 || Q != null && Q > 0, `The width(%s) and height(%s) of chart should be greater than 0,
       please check the style of container, or the props width(%s) and height(%s),
       or add a minWidth(%s) or minHeight(%s) or use aspect(%s) to control the
       height and width.`, $, Q, a, o, l, c, r), /* @__PURE__ */ w.createElement("div", Xi({
    id: v ? "".concat(v) : void 0,
    className: ie("recharts-responsive-container", p),
    style: Zc(Zc({}, b), {}, {
      width: a,
      height: o,
      minWidth: l,
      minHeight: c,
      maxHeight: s
    }),
    ref: O
  }, x), /* @__PURE__ */ w.createElement("div", {
    style: Ox({
      width: a,
      height: o
    })
  }, /* @__PURE__ */ w.createElement(Nv, {
    width: $,
    height: Q
  }, f)));
}), Lx = /* @__PURE__ */ Me((e, t) => {
  var r = Yu();
  if (_t(r.width) && _t(r.height))
    return e.children;
  var n = Ax({
    width: e.width,
    height: e.height,
    aspect: e.aspect
  }), i = n.width, a = n.height, o = Dv(void 0, void 0, {
    width: i,
    height: a,
    aspect: e.aspect,
    maxHeight: e.maxHeight
  }), u = o.calculatedWidth, l = o.calculatedHeight;
  return M(u) && M(l) ? /* @__PURE__ */ w.createElement(Nv, {
    width: u,
    height: l
  }, e.children) : /* @__PURE__ */ w.createElement($x, Xi({}, e, {
    width: i,
    height: a,
    ref: t
  }));
});
function qu(e) {
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
var Wa = () => {
  var e, t = Ve(), r = N(hx), n = N(Fa), i = (e = N(Ba)) === null || e === void 0 ? void 0 : e.padding;
  return !t || !n || !i ? r : {
    width: n.width - i.left - i.right,
    height: n.height - i.top - i.bottom,
    x: i.left,
    y: i.top
  };
}, Rx = {
  top: 0,
  bottom: 0,
  left: 0,
  right: 0,
  width: 0,
  height: 0,
  brushBottom: 0
}, Mv = () => {
  var e;
  return (e = N(Ce)) !== null && e !== void 0 ? e : Rx;
}, $v = () => N(Gt), Lv = () => N(Yt), ue = (e) => e.layout.layoutType, an = () => N(ue), Rv = () => {
  var e = an();
  if (e === "horizontal" || e === "vertical")
    return e;
}, zv = (e) => {
  var t = e.layout.layoutType;
  if (t === "centric" || t === "radial")
    return t;
}, zx = () => {
  var e = an();
  return e !== void 0;
}, Xn = (e) => {
  var t = ve(), r = Ve(), n = e.width, i = e.height, a = Yu(), o = n, u = i;
  return a && (o = a.width > 0 ? a.width : n, u = a.height > 0 ? a.height : i), he(() => {
    !r && _t(o) && _t(u) && t(Bw({
      width: o,
      height: u
    }));
  }, [t, r, o, u]), null;
}, Bx = {
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
}, Bv = $e({
  name: "legend",
  initialState: Bx,
  reducers: {
    setLegendSize(e, t) {
      e.size.width = t.payload.width, e.size.height = t.payload.height;
    },
    setLegendSettings(e, t) {
      e.settings.align = t.payload.align, e.settings.layout = t.payload.layout, e.settings.verticalAlign = t.payload.verticalAlign, e.settings.itemSorter = t.payload.itemSorter;
    },
    addLegendPayload: {
      reducer(e, t) {
        e.payload.push(G(t.payload));
      },
      prepare: re()
    },
    replaceLegendPayload: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next, a = nt(e).payload.indexOf(G(n));
        a > -1 && (e.payload[a] = G(i));
      },
      prepare: re()
    },
    removeLegendPayload: {
      reducer(e, t) {
        var r = nt(e).payload.indexOf(G(t.payload));
        r > -1 && e.payload.splice(r, 1);
      },
      prepare: re()
    }
  }
}), Zn = Bv.actions;
Zn.setLegendSize;
Zn.setLegendSettings;
var Fx = Zn.addLegendPayload, Wx = Zn.replaceLegendPayload, Ux = Zn.removeLegendPayload, Vx = Bv.reducer, go = {};
/**
 * @license React
 * use-sync-external-store-with-selector.production.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var Jc;
function Kx() {
  if (Jc) return go;
  Jc = 1;
  var e = tn;
  function t(l, c) {
    return l === c && (l !== 0 || 1 / l === 1 / c) || l !== l && c !== c;
  }
  var r = typeof Object.is == "function" ? Object.is : t, n = e.useSyncExternalStore, i = e.useRef, a = e.useEffect, o = e.useMemo, u = e.useDebugValue;
  return go.useSyncExternalStoreWithSelector = function(l, c, s, f, d) {
    var h = i(null);
    if (h.current === null) {
      var v = { hasValue: !1, value: null };
      h.current = v;
    } else v = h.current;
    h = o(
      function() {
        function m(A) {
          if (!y) {
            if (y = !0, b = A, A = f(A), d !== void 0 && v.hasValue) {
              var g = v.value;
              if (d(g, A))
                return x = g;
            }
            return x = A;
          }
          if (g = x, r(b, A)) return g;
          var E = f(A);
          return d !== void 0 && d(g, E) ? (b = A, g) : (b = A, x = E);
        }
        var y = !1, b, x, O = s === void 0 ? null : s;
        return [
          function() {
            return m(c());
          },
          O === null ? void 0 : function() {
            return m(O());
          }
        ];
      },
      [c, s, f, d]
    );
    var p = n(l, h[0], h[1]);
    return a(
      function() {
        v.hasValue = !0, v.value = p;
      },
      [p]
    ), u(p), p;
  }, go;
}
var bo = {};
/**
 * @license React
 * use-sync-external-store-with-selector.development.js
 *
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
var es;
function Hx() {
  return es || (es = 1, process.env.NODE_ENV !== "production" && function() {
    function e(l, c) {
      return l === c && (l !== 0 || 1 / l === 1 / c) || l !== l && c !== c;
    }
    typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart(Error());
    var t = tn, r = typeof Object.is == "function" ? Object.is : e, n = t.useSyncExternalStore, i = t.useRef, a = t.useEffect, o = t.useMemo, u = t.useDebugValue;
    bo.useSyncExternalStoreWithSelector = function(l, c, s, f, d) {
      var h = i(null);
      if (h.current === null) {
        var v = { hasValue: !1, value: null };
        h.current = v;
      } else v = h.current;
      h = o(
        function() {
          function m(A) {
            if (!y) {
              if (y = !0, b = A, A = f(A), d !== void 0 && v.hasValue) {
                var g = v.value;
                if (d(g, A))
                  return x = g;
              }
              return x = A;
            }
            if (g = x, r(b, A))
              return g;
            var E = f(A);
            return d !== void 0 && d(g, E) ? (b = A, g) : (b = A, x = E);
          }
          var y = !1, b, x, O = s === void 0 ? null : s;
          return [
            function() {
              return m(c());
            },
            O === null ? void 0 : function() {
              return m(O());
            }
          ];
        },
        [c, s, f, d]
      );
      var p = n(l, h[0], h[1]);
      return a(
        function() {
          v.hasValue = !0, v.value = p;
        },
        [p]
      ), u(p), p;
    }, typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ < "u" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop == "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop(Error());
  }()), bo;
}
process.env.NODE_ENV === "production" ? Kx() : Hx();
function Gx(e) {
  e();
}
function Yx() {
  let e = null, t = null;
  return {
    clear() {
      e = null, t = null;
    },
    notify() {
      Gx(() => {
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
var ts = {
  notify() {
  },
  get: () => []
};
function qx(e, t) {
  let r, n = ts, i = 0, a = !1;
  function o(p) {
    s();
    const m = n.subscribe(p);
    let y = !1;
    return () => {
      y || (y = !0, m(), f());
    };
  }
  function u() {
    n.notify();
  }
  function l() {
    v.onStateChange && v.onStateChange();
  }
  function c() {
    return a;
  }
  function s() {
    i++, r || (r = e.subscribe(l), n = Yx());
  }
  function f() {
    i--, r && i === 0 && (r(), r = void 0, n.clear(), n = ts);
  }
  function d() {
    a || (a = !0, s());
  }
  function h() {
    a && (a = !1, f());
  }
  const v = {
    addNestedSub: o,
    notifyNestedSubs: u,
    handleChangeWrapper: l,
    isSubscribed: c,
    trySubscribe: d,
    tryUnsubscribe: h,
    getListeners: () => n
  };
  return v;
}
var Xx = () => typeof window < "u" && typeof window.document < "u" && typeof window.document.createElement < "u", Zx = /* @__PURE__ */ Xx(), Qx = () => typeof navigator < "u" && navigator.product === "ReactNative", Jx = /* @__PURE__ */ Qx(), eO = () => Zx || Jx ? w.useLayoutEffect : w.useEffect, tO = /* @__PURE__ */ eO();
function rs(e, t) {
  return e === t ? e !== 0 || t !== 0 || 1 / e === 1 / t : e !== e && t !== t;
}
function rO(e, t) {
  if (rs(e, t)) return !0;
  if (typeof e != "object" || e === null || typeof t != "object" || t === null)
    return !1;
  const r = Object.keys(e), n = Object.keys(t);
  if (r.length !== n.length) return !1;
  for (let i = 0; i < r.length; i++)
    if (!Object.prototype.hasOwnProperty.call(t, r[i]) || !rs(e[r[i]], t[r[i]]))
      return !1;
  return !0;
}
var wo = /* @__PURE__ */ Symbol.for("react-redux-context"), xo = typeof globalThis < "u" ? globalThis : (
  /* fall back to a per-module scope (pre-8.1 behaviour) if `globalThis` is not available */
  {}
);
function nO() {
  if (!w.createContext) return {};
  const e = xo[wo] ?? (xo[wo] = /* @__PURE__ */ new Map());
  let t = e.get(w.createContext);
  return t || (t = w.createContext(
    null
  ), process.env.NODE_ENV !== "production" && (t.displayName = "ReactRedux"), e.set(w.createContext, t)), t;
}
var iO = /* @__PURE__ */ nO();
function aO(e) {
  const { children: t, context: r, serverState: n, store: i } = e, a = w.useMemo(() => {
    const l = qx(i), c = {
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
  }, [i, n]), o = w.useMemo(() => i.getState(), [i]);
  tO(() => {
    const { subscription: l } = a;
    return l.onStateChange = l.notifyNestedSubs, l.trySubscribe(), o !== i.getState() && l.notifyNestedSubs(), () => {
      l.tryUnsubscribe(), l.onStateChange = void 0;
    };
  }, [a, o]);
  const u = r || iO;
  return /* @__PURE__ */ w.createElement(u.Provider, { value: a }, t);
}
var oO = aO, uO = /* @__PURE__ */ new Set([
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
function lO(e, t) {
  return e == null && t == null ? !0 : typeof e == "number" && typeof t == "number" ? e === t || e !== e && t !== t : e === t;
}
function Ua(e, t) {
  var r = /* @__PURE__ */ new Set([...Object.keys(e), ...Object.keys(t)]);
  for (var n of r)
    if (uO.has(n)) {
      if (e[n] == null && t[n] == null)
        continue;
      if (!rO(e[n], t[n]))
        return !1;
    } else if (!lO(e[n], t[n]))
      return !1;
  return !0;
}
function cu() {
  return cu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, cu.apply(null, arguments);
}
function ns(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function yn(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? ns(Object(r), !0).forEach(function(n) {
      cO(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : ns(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function cO(e, t, r) {
  return (t = sO(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function sO(e) {
  var t = fO(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function fO(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function dO(e, t) {
  return mO(e) || pO(e, t) || vO(e, t) || hO();
}
function hO() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function vO(e, t) {
  if (e) {
    if (typeof e == "string") return is(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? is(e, t) : void 0;
  }
}
function is(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function pO(e, t) {
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
function mO(e) {
  if (Array.isArray(e)) return e;
}
function yO(e) {
  return Array.isArray(e) && Pt(e[0]) && Pt(e[1]) ? e.join(" ~ ") : e;
}
var Lr = {
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
function gO(e, t) {
  return t == null ? e : Ia(e, t);
}
var bO = (e) => {
  var t = e.separator, r = t === void 0 ? Lr.separator : t, n = e.contentStyle, i = e.itemStyle, a = e.labelStyle, o = a === void 0 ? Lr.labelStyle : a, u = e.payload, l = e.formatter, c = e.itemSorter, s = e.wrapperClassName, f = e.labelClassName, d = e.label, h = e.labelFormatter, v = e.accessibilityLayer, p = v === void 0 ? Lr.accessibilityLayer : v, m = () => {
    if (u && u.length) {
      var _ = {
        padding: 0,
        margin: 0
      }, I = gO(u, c), C = I.map((T, P) => {
        if (!T || T.type === "none")
          return null;
        var z = T.formatter || l || yO, $ = T.value, Q = T.name, K = $, H = Q;
        if (z) {
          var B = z($, Q, T, P, u);
          if (Array.isArray(B)) {
            var X = dO(B, 2);
            K = X[0], H = X[1];
          } else if (B != null)
            K = B;
          else
            return null;
        }
        var W = yn(yn({}, Lr.itemStyle), {}, {
          color: T.color || Lr.itemStyle.color
        }, i);
        return /* @__PURE__ */ w.createElement("li", {
          className: "recharts-tooltip-item",
          key: "tooltip-item-".concat(P),
          style: W
        }, Pt(H) ? /* @__PURE__ */ w.createElement("span", {
          className: "recharts-tooltip-item-name"
        }, H) : null, Pt(H) ? /* @__PURE__ */ w.createElement("span", {
          className: "recharts-tooltip-item-separator"
        }, r) : null, /* @__PURE__ */ w.createElement("span", {
          className: "recharts-tooltip-item-value"
        }, K), /* @__PURE__ */ w.createElement("span", {
          className: "recharts-tooltip-item-unit"
        }, T.unit || ""));
      });
      return /* @__PURE__ */ w.createElement("ul", {
        className: "recharts-tooltip-item-list",
        style: _
      }, C);
    }
    return null;
  }, y = yn(yn({}, Lr.contentStyle), n), b = yn({
    margin: 0
  }, o), x = !xe(d), O = x ? d : "", A = ie("recharts-default-tooltip", s), g = ie("recharts-tooltip-label", f);
  x && h && u !== void 0 && u !== null && (O = h(d, u));
  var E = p ? {
    role: "status",
    "aria-live": "assertive"
  } : {};
  return /* @__PURE__ */ w.createElement("div", cu({
    className: A,
    style: y
  }, E), /* @__PURE__ */ w.createElement("p", {
    className: g,
    style: b
  }, /* @__PURE__ */ w.isValidElement(O) ? O : "".concat(O)), m());
}, gn = "recharts-tooltip-wrapper", wO = {
  visibility: "hidden"
};
function xO(e) {
  var t = e.coordinate, r = e.translateX, n = e.translateY;
  return ie(gn, {
    ["".concat(gn, "-right")]: M(r) && t && M(t.x) && r >= t.x,
    ["".concat(gn, "-left")]: M(r) && t && M(t.x) && r < t.x,
    ["".concat(gn, "-bottom")]: M(n) && t && M(t.y) && n >= t.y,
    ["".concat(gn, "-top")]: M(n) && t && M(t.y) && n < t.y
  });
}
function as(e) {
  var t = e.allowEscapeViewBox, r = e.coordinate, n = e.key, i = e.offset, a = e.position, o = e.reverseDirection, u = e.tooltipDimension, l = e.viewBox, c = e.viewBoxDimension;
  if (a && M(a[n]))
    return a[n];
  var s = r[n] - u - (i > 0 ? i : 0), f = r[n] + i;
  if (t[n])
    return o[n] ? s : f;
  var d = l[n];
  if (d == null)
    return 0;
  if (o[n]) {
    var h = s, v = d;
    return h < v ? Math.max(f, d) : Math.max(s, d);
  }
  if (c == null)
    return 0;
  var p = f + u, m = d + c;
  return p > m ? Math.max(s, d) : Math.max(f, d);
}
function OO(e) {
  var t = e.translateX, r = e.translateY, n = e.useTranslate3d;
  return {
    transform: n ? "translate3d(".concat(t, "px, ").concat(r, "px, 0)") : "translate(".concat(t, "px, ").concat(r, "px)")
  };
}
function AO(e) {
  var t = e.allowEscapeViewBox, r = e.coordinate, n = e.offsetTop, i = e.offsetLeft, a = e.position, o = e.reverseDirection, u = e.tooltipBox, l = e.useTranslate3d, c = e.viewBox, s, f, d;
  return u.height > 0 && u.width > 0 && r ? (f = as({
    allowEscapeViewBox: t,
    coordinate: r,
    key: "x",
    offset: i,
    position: a,
    reverseDirection: o,
    tooltipDimension: u.width,
    viewBox: c,
    viewBoxDimension: c.width
  }), d = as({
    allowEscapeViewBox: t,
    coordinate: r,
    key: "y",
    offset: n,
    position: a,
    reverseDirection: o,
    tooltipDimension: u.height,
    viewBox: c,
    viewBoxDimension: c.height
  }), s = OO({
    translateX: f,
    translateY: d,
    useTranslate3d: l
  })) : s = wO, {
    cssProperties: s,
    cssClasses: xO({
      translateX: f,
      translateY: d,
      coordinate: r
    })
  };
}
var SO = () => !(typeof window < "u" && window.document && window.document.createElement && window.setTimeout), Qn = {
  isSsr: SO()
};
function EO(e, t) {
  return kO(e) || IO(e, t) || _O(e, t) || PO();
}
function PO() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function _O(e, t) {
  if (e) {
    if (typeof e == "string") return os(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? os(e, t) : void 0;
  }
}
function os(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function IO(e, t) {
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
function kO(e) {
  if (Array.isArray(e)) return e;
}
function Fv() {
  var e = fe(() => Qn.isSsr || !window.matchMedia ? !1 : window.matchMedia("(prefers-reduced-motion: reduce)").matches), t = EO(e, 2), r = t[0], n = t[1];
  return he(() => {
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
function us(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Rr(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? us(Object(r), !0).forEach(function(n) {
      CO(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : us(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function CO(e, t, r) {
  return (t = TO(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function TO(e) {
  var t = DO(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function DO(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function jO(e, t) {
  return LO(e) || $O(e, t) || MO(e, t) || NO();
}
function NO() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function MO(e, t) {
  if (e) {
    if (typeof e == "string") return ls(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? ls(e, t) : void 0;
  }
}
function ls(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function $O(e, t) {
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
function LO(e) {
  if (Array.isArray(e)) return e;
}
function RO(e) {
  if (!(e.prefersReducedMotion && e.isAnimationActive === "auto") && e.isAnimationActive && e.active) {
    var t = typeof e.animationEasing == "string" ? e.animationEasing : "ease";
    return "transform ".concat(e.animationDuration, "ms ").concat(t);
  }
}
function zO(e) {
  var t, r, n, i, a, o, u = Fv(), l = w.useState(() => ({
    dismissed: !1,
    dismissedAtCoordinate: {
      x: 0,
      y: 0
    }
  })), c = jO(l, 2), s = c[0], f = c[1];
  w.useEffect(() => {
    var y = (b) => {
      if (b.key === "Escape") {
        var x, O, A, g;
        f({
          dismissed: !0,
          dismissedAtCoordinate: {
            x: (x = (O = e.coordinate) === null || O === void 0 ? void 0 : O.x) !== null && x !== void 0 ? x : 0,
            y: (A = (g = e.coordinate) === null || g === void 0 ? void 0 : g.y) !== null && A !== void 0 ? A : 0
          }
        });
      }
    };
    return document.addEventListener("keydown", y), () => {
      document.removeEventListener("keydown", y);
    };
  }, [(t = e.coordinate) === null || t === void 0 ? void 0 : t.x, (r = e.coordinate) === null || r === void 0 ? void 0 : r.y]), s.dismissed && (((n = (i = e.coordinate) === null || i === void 0 ? void 0 : i.x) !== null && n !== void 0 ? n : 0) !== s.dismissedAtCoordinate.x || ((a = (o = e.coordinate) === null || o === void 0 ? void 0 : o.y) !== null && a !== void 0 ? a : 0) !== s.dismissedAtCoordinate.y) && f(Rr(Rr({}, s), {}, {
    dismissed: !1
  }));
  var d = AO({
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
  }), h = d.cssClasses, v = d.cssProperties, p = e.hasPortalFromProps ? {} : Rr(Rr({
    transition: RO({
      prefersReducedMotion: u,
      isAnimationActive: e.isAnimationActive,
      active: e.active,
      animationDuration: e.animationDuration,
      animationEasing: e.animationEasing
    })
  }, v), {}, {
    pointerEvents: "none",
    position: "absolute",
    top: 0,
    left: 0
  }), m = Rr(Rr({}, p), {}, {
    visibility: !s.dismissed && e.active && e.hasPayload ? "visible" : "hidden"
  }, e.wrapperStyle);
  return /* @__PURE__ */ w.createElement("div", {
    // @ts-expect-error typescript library does not recognize xmlns attribute, but it's required for an HTML chunk inside SVG.
    xmlns: "http://www.w3.org/1999/xhtml",
    tabIndex: -1,
    className: h,
    style: m,
    ref: e.innerRef
  }, e.children);
}
var BO = /* @__PURE__ */ w.memo(zO), Wv = () => {
  var e;
  return (e = N((t) => t.rootProps.accessibilityLayer)) !== null && e !== void 0 ? e : !0;
};
function su() {
  return su = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, su.apply(null, arguments);
}
function cs(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function ss(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? cs(Object(r), !0).forEach(function(n) {
      FO(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : cs(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function FO(e, t, r) {
  return (t = WO(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function WO(e) {
  var t = UO(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function UO(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var fs = {
  curveBasisClosed: Lg,
  curveBasisOpen: Rg,
  curveBasis: $g,
  curveBumpX: Ng,
  curveBumpY: Mg,
  curveLinearClosed: zg,
  curveLinear: Pa,
  curveMonotoneX: Bg,
  curveMonotoneY: Fg,
  curveNatural: Wg,
  curveStep: Ug,
  curveStepAfter: Kg,
  curveStepBefore: Vg
}, Zi = (e) => Y(e.x) && Y(e.y), ds = (e) => e.base != null && Zi(e.base) && Zi(e), bn = (e) => e.x, wn = (e) => e.y, VO = (e, t) => {
  if (typeof e == "function")
    return e;
  var r = "curve".concat(Ru(e));
  if ((r === "curveMonotone" || r === "curveBump") && t) {
    var n = fs["".concat(r).concat(t === "vertical" ? "Y" : "X")];
    if (n)
      return n;
  }
  return fs[r] || Pa;
}, hs = {
  connectNulls: !1,
  type: "linear"
}, KO = (e) => {
  var t = e.type, r = t === void 0 ? hs.type : t, n = e.points, i = n === void 0 ? [] : n, a = e.baseLine, o = e.layout, u = e.connectNulls, l = u === void 0 ? hs.connectNulls : u, c = VO(r, o), s = l ? i.filter(Zi) : i;
  if (Array.isArray(a)) {
    var f, d = i.map((y, b) => ss(ss({}, y), {}, {
      base: a[b]
    }));
    o === "vertical" ? f = di().y(wn).x1(bn).x0((y) => y.base.x) : f = di().x(bn).y1(wn).y0((y) => y.base.y);
    var h = f.defined(ds).curve(c), v = l ? d.filter(ds) : d;
    return h(v);
  }
  var p;
  o === "vertical" && M(a) ? p = di().y(wn).x1(bn).x0(a) : M(a) ? p = di().x(bn).y1(wn).y0(a) : p = yh().x(bn).y(wn);
  var m = p.defined(Zi).curve(c);
  return m(s);
}, Uv = (e) => {
  var t = e.className, r = e.points, n = e.path, i = e.pathRef, a = an();
  if ((!r || !r.length) && !n)
    return null;
  var o = {
    type: e.type,
    points: e.points,
    baseLine: e.baseLine,
    layout: e.layout || a,
    connectNulls: e.connectNulls
  }, u = r && r.length ? KO(o) : n;
  return /* @__PURE__ */ w.createElement("path", su({}, Et(e), zu(e), {
    className: ie("recharts-curve", t),
    d: u === null ? void 0 : u,
    ref: i
  }));
}, HO = ["x", "y", "top", "left", "width", "height", "className"];
function fu() {
  return fu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, fu.apply(null, arguments);
}
function vs(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function GO(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? vs(Object(r), !0).forEach(function(n) {
      YO(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : vs(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function YO(e, t, r) {
  return (t = qO(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function qO(e) {
  var t = XO(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function XO(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function ZO(e, t) {
  if (e == null) return {};
  var r, n, i = QO(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function QO(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var JO = (e, t, r, n, i, a) => "M".concat(e, ",").concat(i, "v").concat(n, "M").concat(a, ",").concat(t, "h").concat(r), e1 = (e) => {
  var t = e.x, r = t === void 0 ? 0 : t, n = e.y, i = n === void 0 ? 0 : n, a = e.top, o = a === void 0 ? 0 : a, u = e.left, l = u === void 0 ? 0 : u, c = e.width, s = c === void 0 ? 0 : c, f = e.height, d = f === void 0 ? 0 : f, h = e.className, v = ZO(e, HO), p = GO({
    x: r,
    y: i,
    top: o,
    left: l,
    width: s,
    height: d
  }, v);
  return !M(r) || !M(i) || !M(s) || !M(d) || !M(o) || !M(l) ? null : /* @__PURE__ */ w.createElement("path", fu({}, at(p), {
    className: ie("recharts-cross", h),
    d: JO(r, i, s, d, o, l)
  }));
};
function t1(e, t, r, n) {
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
var Qi = 1e-4, Vv = (e, t) => [0, 3 * e, 3 * t - 6 * e, 3 * e - 3 * t + 1], Kv = (e, t) => e.map((r, n) => r * t ** n).reduce((r, n) => r + n), ps = (e, t) => (r) => {
  var n = Vv(e, t);
  return Kv(n, r);
}, r1 = (e, t) => (r) => {
  var n = Vv(e, t), i = [...n.map((a, o) => a * o).slice(1), 0];
  return Kv(i, r);
}, n1 = (e) => {
  var t, r = e.split("(");
  if (r.length !== 2 || r[0] !== "cubic-bezier")
    return null;
  var n = (t = r[1]) === null || t === void 0 || (t = t.split(")")[0]) === null || t === void 0 ? void 0 : t.split(",");
  if (n == null || n.length !== 4)
    return null;
  var i = n.map((a) => parseFloat(a));
  return [i[0], i[1], i[2], i[3]];
}, i1 = function() {
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
        var i = n1(r[0]);
        if (i)
          return i;
      }
    }
  return r.length === 4 ? r : [0, 0, 1, 1];
}, a1 = (e, t, r, n) => {
  var i = ps(e, r), a = ps(t, n), o = r1(e, r), u = (c) => c > 1 ? 1 : c < 0 ? 0 : c, l = (c) => {
    for (var s = c > 1 ? 1 : c, f = s, d = 0; d < 8; ++d) {
      var h = i(f) - s, v = o(f);
      if (Math.abs(h - s) < Qi || v < Qi)
        return a(f);
      f = u(f - h / v);
    }
    return a(f);
  };
  return l.isStepper = !1, l;
}, ms = function() {
  return a1(...i1(...arguments));
}, o1 = function() {
  for (var t = arguments.length > 0 && arguments[0] !== void 0 ? arguments[0] : {}, r = t.stiff, n = r === void 0 ? 100 : r, i = t.damping, a = i === void 0 ? 8 : i, o = t.dt, u = o === void 0 ? 16.67 : o, l = 1, c = [0], s = 0, f = 0, d = 1e4, h = 0; h < d; ) {
    var v = -(s - l) * n, p = f * a;
    if (f += (v - p) * u / 1e3, s += f * u / 1e3, c.push(s), Math.abs(s - l) < Qi && Math.abs(f) < Qi)
      break;
    h++;
  }
  c[c.length - 1] = l;
  var m = c.length - 1;
  return (y) => {
    var b, x, O;
    if (y <= 0) return 0;
    if (y >= 1) return l;
    var A = y * m, g = Math.floor(A), E = A - g;
    return ((b = c[g]) !== null && b !== void 0 ? b : 0) + (((x = c[g + 1]) !== null && x !== void 0 ? x : 0) - ((O = c[g]) !== null && O !== void 0 ? O : 0)) * E;
  };
}, u1 = (e) => {
  if (typeof e == "string")
    switch (e) {
      case "ease":
      case "ease-in-out":
      case "ease-out":
      case "ease-in":
      case "linear":
        return ms(e);
      case "spring":
        return o1();
      default:
        if (e.split("(")[0] === "cubic-bezier")
          return ms(e);
    }
  return typeof e == "function" ? e : null;
}, l1 = (e, t, r) => {
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
}, Hv = /* @__PURE__ */ Je(l1);
Hv.Provider;
function c1(e) {
  var t = kt(Hv);
  return Kt(() => e ?? t, [e, t]);
}
function s1(e, t, r) {
  return (t = f1(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function f1(e) {
  var t = d1(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function d1(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var ys = "init", gs = "pending", bs = "active", h1 = "completed";
function Oo(e) {
  return Math.max(0, e);
}
class v1 {
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
    s1(this, "state", ys), this.animationId = t.animationId, this.onAnimationEnd = t.onAnimationEnd, this.animationDuration = Oo(t.animationDuration), this.animationBegin = Oo(t.animationBegin), this.progress = 0, this.from = t.from, this.to = t.to, this.easing = t.easing, (r = t.onAnimationStart) === null || r === void 0 || r.call(t);
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
    if (this.getState() === ys)
      return this.state = gs, this.beginStartedTime = t, this.animationBegin;
    if (this.getState() === gs) {
      if (this.beginStartedTime == null)
        throw new Error();
      var r = t - this.beginStartedTime;
      return r >= this.animationBegin ? (this.state = bs, this.animationStartedTime = t, this.nextAnimationUpdate(0)) : Oo(this.animationBegin - r);
    }
    if (this.getState() === bs) {
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
    this.state = h1;
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
class p1 extends v1 {
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
    return this.easing(Nt(this.getFrom(), this.getTo(), this.getProgress()));
  }
}
class m1 {
  setTimeout(t) {
    var r = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : 0, n = performance.now(), i = null, a = (o) => {
      o - n >= r ? t(o) : i = requestAnimationFrame(a);
    };
    return i = requestAnimationFrame(a), () => {
      i != null && cancelAnimationFrame(i);
    };
  }
}
function y1(e, t) {
  return x1(e) || w1(e, t) || b1(e, t) || g1();
}
function g1() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function b1(e, t) {
  if (e) {
    if (typeof e == "string") return ws(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? ws(e, t) : void 0;
  }
}
function ws(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function w1(e, t) {
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
function x1(e) {
  if (Array.isArray(e)) return e;
}
var O1 = {
  begin: 0,
  duration: 1e3,
  easing: "ease",
  isActive: !0,
  canBegin: !0,
  onAnimationEnd: () => {
  },
  onAnimationStart: () => {
  }
}, xs = 0, Ao = 1;
function Gv(e) {
  var t = et(e, O1), r = t.animationId, n = t.isActive, i = t.canBegin, a = t.duration, o = t.easing, u = t.begin, l = t.onAnimationEnd, c = t.onAnimationStart, s = t.children, f = Fv(), d = n === "auto" ? !Qn.isSsr && !f : n, h = c1(t.animationController), v = fe(d ? xs : Ao), p = y1(v, 2), m = p[0], y = p[1];
  return he(() => {
    d || y(Ao);
  }, [d]), he(() => {
    var b = u1(o);
    if (!d || !i || b == null)
      return nn;
    var x = new m1(), O = new p1({
      animationId: r,
      easing: b,
      animationDuration: a,
      animationBegin: u,
      onAnimationStart: c,
      onAnimationEnd: l,
      from: xs,
      to: Ao
    });
    return h(x, O, y);
  }, [h, r, d, i, a, o, u, c, l]), s(Number(m));
}
function Yv(e) {
  var t = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : "animation-", r = V(Dn(t)), n = V(e);
  return n.current !== e && (r.current = Dn(t), n.current = e), r.current;
}
var A1 = (e) => e.replace(/([A-Z])/g, (t) => "-".concat(t.toLowerCase())), S1 = (e, t, r) => e.map((n) => "".concat(A1(n), " ").concat(t, "ms ").concat(r)).join(","), E1 = ["radius"], P1 = ["radius"], Os, As, Ss, Es, Ps, _s, Is, ks, Cs, Ts;
function Ds(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function js(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Ds(Object(r), !0).forEach(function(n) {
      _1(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Ds(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function _1(e, t, r) {
  return (t = I1(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function I1(e) {
  var t = k1(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function k1(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function Ji() {
  return Ji = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Ji.apply(null, arguments);
}
function Ns(e, t) {
  if (e == null) return {};
  var r, n, i = C1(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function C1(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function T1(e, t) {
  return M1(e) || N1(e, t) || j1(e, t) || D1();
}
function D1() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function j1(e, t) {
  if (e) {
    if (typeof e == "string") return Ms(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Ms(e, t) : void 0;
  }
}
function Ms(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function N1(e, t) {
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
function M1(e) {
  if (Array.isArray(e)) return e;
}
function pt(e, t) {
  return t || (t = e.slice(0)), Object.freeze(Object.defineProperties(e, { raw: { value: Object.freeze(t) } }));
}
var $s = (e, t, r, n, i) => {
  var a = jt(r), o = jt(n), u = Math.min(Math.abs(a) / 2, Math.abs(o) / 2), l = o >= 0 ? 1 : -1, c = a >= 0 ? 1 : -1, s = o >= 0 && a >= 0 || o < 0 && a < 0 ? 1 : 0, f;
  if (u > 0 && Array.isArray(i)) {
    for (var d = [0, 0, 0, 0], h = 0, v = 4; h < v; h++) {
      var p, m = (p = i[h]) !== null && p !== void 0 ? p : 0;
      d[h] = m > u ? u : m;
    }
    f = _e(Os || (Os = pt(["M", ",", ""])), e, t + l * d[0]), d[0] > 0 && (f += _e(As || (As = pt(["A ", ",", ",0,0,", ",", ",", ""])), d[0], d[0], s, e + c * d[0], t)), f += _e(Ss || (Ss = pt(["L ", ",", ""])), e + r - c * d[1], t), d[1] > 0 && (f += _e(Es || (Es = pt(["A ", ",", ",0,0,", `,
        `, ",", ""])), d[1], d[1], s, e + r, t + l * d[1])), f += _e(Ps || (Ps = pt(["L ", ",", ""])), e + r, t + n - l * d[2]), d[2] > 0 && (f += _e(_s || (_s = pt(["A ", ",", ",0,0,", `,
        `, ",", ""])), d[2], d[2], s, e + r - c * d[2], t + n)), f += _e(Is || (Is = pt(["L ", ",", ""])), e + c * d[3], t + n), d[3] > 0 && (f += _e(ks || (ks = pt(["A ", ",", ",0,0,", `,
        `, ",", ""])), d[3], d[3], s, e, t + n - l * d[3])), f += "Z";
  } else if (u > 0 && i === +i && i > 0) {
    var y = Math.min(u, i);
    f = _e(Cs || (Cs = pt(["M ", ",", `
            A `, ",", ",0,0,", ",", ",", `
            L `, ",", `
            A `, ",", ",0,0,", ",", ",", `
            L `, ",", `
            A `, ",", ",0,0,", ",", ",", `
            L `, ",", `
            A `, ",", ",0,0,", ",", ",", " Z"])), e, t + l * y, y, y, s, e + c * y, t, e + r - c * y, t, y, y, s, e + r, t + l * y, e + r, t + n - l * y, y, y, s, e + r - c * y, t + n, e + c * y, t + n, y, y, s, e, t + n - l * y);
  } else
    f = _e(Ts || (Ts = pt(["M ", ",", " h ", " v ", " h ", " Z"])), e, t, r, n, -r);
  return f;
}, Ls = {
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
}, $1 = (e) => {
  var t = et(e, Ls), r = V(null), n = fe(-1), i = T1(n, 2), a = i[0], o = i[1];
  he(() => {
    if (r.current && r.current.getTotalLength)
      try {
        var B = r.current.getTotalLength();
        B && o(B);
      } catch {
      }
  }, []);
  var u = t.x, l = t.y, c = t.width, s = t.height, f = t.radius, d = t.className, h = t.animationEasing, v = t.animationDuration, p = t.animationBegin, m = t.isAnimationActive, y = t.isUpdateAnimationActive, b = V(c), x = V(s), O = V(u), A = V(l), g = Kt(() => ({
    x: u,
    y: l,
    width: c,
    height: s,
    radius: f
  }), [u, l, c, s, f]), E = Yv(g, "rectangle-");
  if (u !== +u || l !== +l || c !== +c || s !== +s || c === 0 || s === 0)
    return null;
  var _ = ie("recharts-rectangle", d);
  if (!y) {
    var I = at(t);
    I.radius;
    var C = Ns(I, E1);
    return /* @__PURE__ */ w.createElement("path", Ji({}, C, {
      x: jt(u),
      y: jt(l),
      width: jt(c),
      height: jt(s),
      radius: typeof f == "number" ? f : void 0,
      className: _,
      d: $s(u, l, c, s, f)
    }));
  }
  var T = b.current, P = x.current, z = O.current, $ = A.current, Q = "0px ".concat(a === -1 ? 1 : a, "px"), K = "".concat(a, "px ").concat(a, "px"), H = S1(["strokeDasharray"], v, typeof h == "string" ? h : Ls.animationEasing);
  return /* @__PURE__ */ w.createElement(Gv, {
    animationId: E,
    key: E,
    canBegin: a > 0,
    duration: v,
    easing: h,
    isActive: y,
    begin: p
  }, (B) => {
    var X = Nt(T, c, B), W = Nt(P, s, B), Te = Nt(z, u, B), Ee = Nt($, l, B);
    r.current && (b.current = X, x.current = W, O.current = Te, A.current = Ee);
    var se;
    m ? B > 0 ? se = {
      transition: H,
      strokeDasharray: K
    } : se = {
      strokeDasharray: Q
    } : se = {
      strokeDasharray: K
    };
    var Ke = at(t);
    Ke.radius;
    var He = Ns(Ke, P1);
    return /* @__PURE__ */ w.createElement("path", Ji({}, He, {
      radius: typeof f == "number" ? f : void 0,
      className: _,
      d: $s(Te, Ee, X, W, f),
      ref: r,
      style: js(js({}, se), t.style)
    }));
  });
};
function Rs(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function zs(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Rs(Object(r), !0).forEach(function(n) {
      L1(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Rs(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function L1(e, t, r) {
  return (t = R1(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function R1(e) {
  var t = z1(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function z1(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var ea = Math.PI / 180, B1 = (e) => e * 180 / Math.PI, ke = (e, t, r, n) => ({
  x: e + Math.cos(-ea * n) * r,
  y: t + Math.sin(-ea * n) * r
}), F1 = function(t, r) {
  var n = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : {
    top: 0,
    right: 0,
    bottom: 0,
    left: 0
  };
  return Math.min(Math.abs(t - (n.left || 0) - (n.right || 0)), Math.abs(r - (n.top || 0) - (n.bottom || 0))) / 2;
}, W1 = (e, t) => {
  var r = e.x, n = e.y, i = t.x, a = t.y;
  return Math.sqrt((r - i) ** 2 + (n - a) ** 2);
}, U1 = (e, t) => {
  var r = e.x, n = e.y, i = t.cx, a = t.cy, o = W1({
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
    angle: B1(l),
    angleInRadian: l
  };
}, V1 = (e) => {
  var t = e.startAngle, r = e.endAngle, n = Math.floor(t / 360), i = Math.floor(r / 360), a = Math.min(n, i);
  return {
    startAngle: t - a * 360,
    endAngle: r - a * 360
  };
}, K1 = (e, t) => {
  var r = t.startAngle, n = t.endAngle, i = Math.floor(r / 360), a = Math.floor(n / 360), o = Math.min(i, a);
  return e + o * 360;
}, H1 = (e, t) => {
  var r = e.relativeX, n = e.relativeY, i = U1({
    x: r,
    y: n
  }, t), a = i.radius, o = i.angle, u = t.innerRadius, l = t.outerRadius;
  if (a < u || a > l || a === 0)
    return null;
  var c = V1(t), s = c.startAngle, f = c.endAngle, d = o, h;
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
  return h ? zs(zs({}, t), {}, {
    radius: a,
    angle: K1(d, t)
  }) : null;
};
function qv(e) {
  var t = e.cx, r = e.cy, n = e.radius, i = e.startAngle, a = e.endAngle, o = ke(t, r, n, i), u = ke(t, r, n, a);
  return {
    points: [o, u],
    cx: t,
    cy: r,
    radius: n,
    startAngle: i,
    endAngle: a
  };
}
var Bs, Fs, Ws, Us, Vs, Ks, Hs;
function du() {
  return du = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, du.apply(null, arguments);
}
function br(e, t) {
  return t || (t = e.slice(0)), Object.freeze(Object.defineProperties(e, { raw: { value: Object.freeze(t) } }));
}
var G1 = (e, t) => {
  var r = rt(t - e), n = Math.min(Math.abs(t - e), 359.999);
  return r * n;
}, xi = (e) => {
  var t = e.cx, r = e.cy, n = e.radius, i = e.angle, a = e.sign, o = e.isExternal, u = e.cornerRadius, l = e.cornerIsExternal, c = u * (o ? 1 : -1) + n, s = Math.asin(u / c) / ea, f = l ? i : i + a * s, d = ke(t, r, c, f), h = ke(t, r, n, f), v = l ? i - a * s : i, p = ke(t, r, c * Math.cos(s * ea), v);
  return {
    center: d,
    circleTangency: h,
    lineTangency: p,
    theta: s
  };
}, Xv = (e) => {
  var t = e.cx, r = e.cy, n = e.innerRadius, i = e.outerRadius, a = e.startAngle, o = e.endAngle, u = G1(a, o), l = a + u, c = ke(t, r, i, a), s = ke(t, r, i, l), f = _e(Bs || (Bs = br(["M ", ",", `
    A `, ",", `,0,
    `, ",", `,
    `, ",", `
  `])), c.x, c.y, i, i, +(Math.abs(u) > 180), +(a > l), s.x, s.y);
  if (n > 0) {
    var d = ke(t, r, n, a), h = ke(t, r, n, l);
    f += _e(Fs || (Fs = br(["L ", ",", `
            A `, ",", `,0,
            `, ",", `,
            `, ",", " Z"])), h.x, h.y, n, n, +(Math.abs(u) > 180), +(a <= l), d.x, d.y);
  } else
    f += _e(Ws || (Ws = br(["L ", ",", " Z"])), t, r);
  return f;
}, Y1 = (e) => {
  var t = e.cx, r = e.cy, n = e.innerRadius, i = e.outerRadius, a = e.cornerRadius, o = e.forceCornerRadius, u = e.cornerIsExternal, l = e.startAngle, c = e.endAngle, s = rt(c - l), f = xi({
    cx: t,
    cy: r,
    radius: i,
    angle: l,
    sign: s,
    cornerRadius: a,
    cornerIsExternal: u
  }), d = f.circleTangency, h = f.lineTangency, v = f.theta, p = xi({
    cx: t,
    cy: r,
    radius: i,
    angle: c,
    sign: -s,
    cornerRadius: a,
    cornerIsExternal: u
  }), m = p.circleTangency, y = p.lineTangency, b = p.theta, x = u ? Math.abs(l - c) : Math.abs(l - c) - v - b;
  if (x < 0)
    return o ? _e(Us || (Us = br(["M ", ",", `
        a`, ",", ",0,0,1,", `,0
        a`, ",", ",0,0,1,", `,0
      `])), h.x, h.y, a, a, a * 2, a, a, -a * 2) : Xv({
      cx: t,
      cy: r,
      innerRadius: n,
      outerRadius: i,
      startAngle: l,
      endAngle: c
    });
  var O = _e(Vs || (Vs = br(["M ", ",", `
    A`, ",", ",0,0,", ",", ",", `
    A`, ",", ",0,", ",", ",", ",", `
    A`, ",", ",0,0,", ",", ",", `
  `])), h.x, h.y, a, a, +(s < 0), d.x, d.y, i, i, +(x > 180), +(s < 0), m.x, m.y, a, a, +(s < 0), y.x, y.y);
  if (n > 0) {
    var A = xi({
      cx: t,
      cy: r,
      radius: n,
      angle: l,
      sign: s,
      isExternal: !0,
      cornerRadius: a,
      cornerIsExternal: u
    }), g = A.circleTangency, E = A.lineTangency, _ = A.theta, I = xi({
      cx: t,
      cy: r,
      radius: n,
      angle: c,
      sign: -s,
      isExternal: !0,
      cornerRadius: a,
      cornerIsExternal: u
    }), C = I.circleTangency, T = I.lineTangency, P = I.theta, z = u ? Math.abs(l - c) : Math.abs(l - c) - _ - P;
    if (z < 0 && a === 0)
      return "".concat(O, "L").concat(t, ",").concat(r, "Z");
    O += _e(Ks || (Ks = br(["L", ",", `
      A`, ",", ",0,0,", ",", ",", `
      A`, ",", ",0,", ",", ",", ",", `
      A`, ",", ",0,0,", ",", ",", "Z"])), T.x, T.y, a, a, +(s < 0), C.x, C.y, n, n, +(z > 180), +(s > 0), g.x, g.y, a, a, +(s < 0), E.x, E.y);
  } else
    O += _e(Hs || (Hs = br(["L", ",", "Z"])), t, r);
  return O;
}, q1 = {
  cx: 0,
  cy: 0,
  innerRadius: 0,
  outerRadius: 0,
  startAngle: 0,
  endAngle: 0,
  cornerRadius: 0,
  forceCornerRadius: !1,
  cornerIsExternal: !1
}, X1 = (e) => {
  var t = et(e, q1), r = t.cx, n = t.cy, i = t.innerRadius, a = t.outerRadius, o = t.cornerRadius, u = t.forceCornerRadius, l = t.cornerIsExternal, c = t.startAngle, s = t.endAngle, f = t.className;
  if (a < i || c === s)
    return null;
  var d = ie("recharts-sector", f), h = a - i, v = ur(o, h, 0, !0), p;
  return v > 0 && Math.abs(c - s) < 360 ? p = Y1({
    cx: r,
    cy: n,
    innerRadius: i,
    outerRadius: a,
    cornerRadius: Math.min(v, h / 2),
    forceCornerRadius: u,
    cornerIsExternal: l,
    startAngle: c,
    endAngle: s
  }) : p = Xv({
    cx: r,
    cy: n,
    innerRadius: i,
    outerRadius: a,
    startAngle: c,
    endAngle: s
  }), /* @__PURE__ */ w.createElement("path", du({}, at(t), {
    className: d,
    d: p
  }));
};
function Z1(e, t, r) {
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
  if (Ch(t)) {
    if (e === "centric") {
      var n = t.cx, i = t.cy, a = t.innerRadius, o = t.outerRadius, u = t.angle, l = ke(n, i, a, u), c = ke(n, i, o, u);
      return [{
        x: l.x,
        y: l.y
      }, {
        x: c.x,
        y: c.y
      }];
    }
    return qv(t);
  }
}
function Q1(e) {
  return Kh(e) ? NaN : Number(e);
}
function So(e) {
  return e ? (e = Q1(e), e === 1 / 0 || e === -1 / 0 ? (e < 0 ? -1 : 1) * Number.MAX_VALUE : e === e ? e : 0) : e === 0 ? e : 0;
}
function Zv(e, t, r) {
  r && typeof r != "number" && Zo(e, t, r) && (t = r = void 0), e = So(e), t === void 0 ? (t = e, e = 0) : t = So(t), r = r === void 0 ? e < t ? 1 : -1 : So(r);
  const n = Math.max(Math.ceil((t - e) / (r || 1)), 0), i = new Array(n);
  for (let a = 0; a < n; a++)
    i[a] = e, e += r;
  return i;
}
var Ct = (e) => e.chartData, Qv = S([Ct], (e) => {
  var t = e.chartData != null ? e.chartData.length - 1 : 0;
  return {
    chartData: e.chartData,
    computedData: e.computedData,
    dataEndIndex: t,
    dataStartIndex: 0
  };
}), Jn = (e, t, r, n) => n ? Qv(e) : Ct(e), J1 = S([Jn], (e) => {
  var t = e.chartData, r = e.dataStartIndex, n = e.dataEndIndex;
  return t != null ? t.slice(r, n + 1) : [];
});
S([Qv], (e) => {
  var t = e.chartData, r = e.dataStartIndex, n = e.dataEndIndex;
  return t != null ? t.slice(r, n + 1) : [];
});
var eA = S([Ct], (e) => {
  var t = e.chartData, r = e.dataStartIndex, n = e.dataEndIndex;
  return t != null ? t.slice(r, n + 1) : [];
});
function Xu(e, t) {
  return iA(e) || nA(e, t) || rA(e, t) || tA();
}
function tA() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function rA(e, t) {
  if (e) {
    if (typeof e == "string") return Gs(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Gs(e, t) : void 0;
  }
}
function Gs(e, t) {
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
function At(e) {
  if (Array.isArray(e) && e.length === 2) {
    var t = Xu(e, 2), r = t[0], n = t[1];
    if (Y(r) && Y(n))
      return !0;
  }
  return !1;
}
function Ys(e, t, r) {
  return r ? e : [Math.min(e[0], t[0]), Math.max(e[1], t[1])];
}
function Jv(e, t) {
  if (t && typeof e != "function" && Array.isArray(e) && e.length === 2) {
    var r = Xu(e, 2), n = r[0], i = r[1], a, o;
    if (Y(n))
      a = n;
    else if (typeof n == "function")
      return;
    if (Y(i))
      o = i;
    else if (typeof i == "function")
      return;
    var u = [a, o];
    if (At(u))
      return u;
  }
}
function aA(e, t, r) {
  if (!(!r && t == null)) {
    if (typeof e == "function" && t != null)
      try {
        var n = e(t, r);
        if (At(n))
          return Ys(n, t, r);
      } catch {
      }
    if (Array.isArray(e) && e.length === 2) {
      var i = Xu(e, 2), a = i[0], o = i[1], u, l;
      if (a === "auto")
        t != null && (u = Math.min(...t));
      else if (M(a))
        u = a;
      else if (typeof a == "function")
        try {
          t != null && (u = a(t == null ? void 0 : t[0]));
        } catch {
        }
      else if (typeof a == "string" && Hc.test(a)) {
        var c = Hc.exec(a);
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
      else if (M(o))
        l = o;
      else if (typeof o == "function")
        try {
          t != null && (l = o(t == null ? void 0 : t[1]));
        } catch {
        }
      else if (typeof o == "string" && Gc.test(o)) {
        var f = Gc.exec(o);
        if (f == null || f[1] == null || t == null)
          l = void 0;
        else {
          var d = +f[1];
          l = t[1] + d;
        }
      } else
        l = t == null ? void 0 : t[1];
      var h = [u, l];
      if (At(h))
        return t == null ? h : Ys(h, t, r);
    }
  }
}
var on = 1e9, oA = {
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
}, Qu, ae = !0, ot = "[DecimalError] ", Er = ot + "Invalid argument: ", Zu = ot + "Exponent out of range: ", un = Math.floor, mr = Math.pow, uA = /^(\d+(\.\d*)?|\.\d+)(e[+-]?\d+)?$/i, qe, be = 1e7, ne = 7, ep = 9007199254740991, ta = un(ep / ne), j = {};
j.absoluteValue = j.abs = function() {
  var e = new this.constructor(this);
  return e.s && (e.s = 1), e;
};
j.comparedTo = j.cmp = function(e) {
  var t, r, n, i, a = this;
  if (e = new a.constructor(e), a.s !== e.s) return a.s || -e.s;
  if (a.e !== e.e) return a.e > e.e ^ a.s < 0 ? 1 : -1;
  for (n = a.d.length, i = e.d.length, t = 0, r = n < i ? n : i; t < r; ++t)
    if (a.d[t] !== e.d[t]) return a.d[t] > e.d[t] ^ a.s < 0 ? 1 : -1;
  return n === i ? 0 : n > i ^ a.s < 0 ? 1 : -1;
};
j.decimalPlaces = j.dp = function() {
  var e = this, t = e.d.length - 1, r = (t - e.e) * ne;
  if (t = e.d[t], t) for (; t % 10 == 0; t /= 10) r--;
  return r < 0 ? 0 : r;
};
j.dividedBy = j.div = function(e) {
  return Lt(this, new this.constructor(e));
};
j.dividedToIntegerBy = j.idiv = function(e) {
  var t = this, r = t.constructor;
  return te(Lt(t, new r(e), 0, 1), r.precision);
};
j.equals = j.eq = function(e) {
  return !this.cmp(e);
};
j.exponent = function() {
  return de(this);
};
j.greaterThan = j.gt = function(e) {
  return this.cmp(e) > 0;
};
j.greaterThanOrEqualTo = j.gte = function(e) {
  return this.cmp(e) >= 0;
};
j.isInteger = j.isint = function() {
  return this.e > this.d.length - 2;
};
j.isNegative = j.isneg = function() {
  return this.s < 0;
};
j.isPositive = j.ispos = function() {
  return this.s > 0;
};
j.isZero = function() {
  return this.s === 0;
};
j.lessThan = j.lt = function(e) {
  return this.cmp(e) < 0;
};
j.lessThanOrEqualTo = j.lte = function(e) {
  return this.cmp(e) < 1;
};
j.logarithm = j.log = function(e) {
  var t, r = this, n = r.constructor, i = n.precision, a = i + 5;
  if (e === void 0)
    e = new n(10);
  else if (e = new n(e), e.s < 1 || e.eq(qe)) throw Error(ot + "NaN");
  if (r.s < 1) throw Error(ot + (r.s ? "NaN" : "-Infinity"));
  return r.eq(qe) ? new n(0) : (ae = !1, t = Lt($n(r, a), $n(e, a), a), ae = !0, te(t, i));
};
j.minus = j.sub = function(e) {
  var t = this;
  return e = new t.constructor(e), t.s == e.s ? np(t, e) : tp(t, (e.s = -e.s, e));
};
j.modulo = j.mod = function(e) {
  var t, r = this, n = r.constructor, i = n.precision;
  if (e = new n(e), !e.s) throw Error(ot + "NaN");
  return r.s ? (ae = !1, t = Lt(r, e, 0, 1).times(e), ae = !0, r.minus(t)) : te(new n(r), i);
};
j.naturalExponential = j.exp = function() {
  return rp(this);
};
j.naturalLogarithm = j.ln = function() {
  return $n(this);
};
j.negated = j.neg = function() {
  var e = new this.constructor(this);
  return e.s = -e.s || 0, e;
};
j.plus = j.add = function(e) {
  var t = this;
  return e = new t.constructor(e), t.s == e.s ? tp(t, e) : np(t, (e.s = -e.s, e));
};
j.precision = j.sd = function(e) {
  var t, r, n, i = this;
  if (e !== void 0 && e !== !!e && e !== 1 && e !== 0) throw Error(Er + e);
  if (t = de(i) + 1, n = i.d.length - 1, r = n * ne + 1, n = i.d[n], n) {
    for (; n % 10 == 0; n /= 10) r--;
    for (n = i.d[0]; n >= 10; n /= 10) r++;
  }
  return e && t > r ? t : r;
};
j.squareRoot = j.sqrt = function() {
  var e, t, r, n, i, a, o, u = this, l = u.constructor;
  if (u.s < 1) {
    if (!u.s) return new l(0);
    throw Error(ot + "NaN");
  }
  for (e = de(u), ae = !1, i = Math.sqrt(+u), i == 0 || i == 1 / 0 ? (t = Ot(u.d), (t.length + e) % 2 == 0 && (t += "0"), i = Math.sqrt(t), e = un((e + 1) / 2) - (e < 0 || e % 2), i == 1 / 0 ? t = "5e" + e : (t = i.toExponential(), t = t.slice(0, t.indexOf("e") + 1) + e), n = new l(t)) : n = new l(i.toString()), r = l.precision, i = o = r + 3; ; )
    if (a = n, n = a.plus(Lt(u, a, o + 2)).times(0.5), Ot(a.d).slice(0, o) === (t = Ot(n.d)).slice(0, o)) {
      if (t = t.slice(o - 3, o + 1), i == o && t == "4999") {
        if (te(a, r + 1, 0), a.times(a).eq(u)) {
          n = a;
          break;
        }
      } else if (t != "9999")
        break;
      o += 4;
    }
  return ae = !0, te(n, r);
};
j.times = j.mul = function(e) {
  var t, r, n, i, a, o, u, l, c, s = this, f = s.constructor, d = s.d, h = (e = new f(e)).d;
  if (!s.s || !e.s) return new f(0);
  for (e.s *= s.s, r = s.e + e.e, l = d.length, c = h.length, l < c && (a = d, d = h, h = a, o = l, l = c, c = o), a = [], o = l + c, n = o; n--; ) a.push(0);
  for (n = c; --n >= 0; ) {
    for (t = 0, i = l + n; i > n; )
      u = a[i] + h[n] * d[i - n - 1] + t, a[i--] = u % be | 0, t = u / be | 0;
    a[i] = (a[i] + t) % be | 0;
  }
  for (; !a[--o]; ) a.pop();
  return t ? ++r : a.shift(), e.d = a, e.e = r, ae ? te(e, f.precision) : e;
};
j.toDecimalPlaces = j.todp = function(e, t) {
  var r = this, n = r.constructor;
  return r = new n(r), e === void 0 ? r : (It(e, 0, on), t === void 0 ? t = n.rounding : It(t, 0, 8), te(r, e + de(r) + 1, t));
};
j.toExponential = function(e, t) {
  var r, n = this, i = n.constructor;
  return e === void 0 ? r = Cr(n, !0) : (It(e, 0, on), t === void 0 ? t = i.rounding : It(t, 0, 8), n = te(new i(n), e + 1, t), r = Cr(n, !0, e + 1)), r;
};
j.toFixed = function(e, t) {
  var r, n, i = this, a = i.constructor;
  return e === void 0 ? Cr(i) : (It(e, 0, on), t === void 0 ? t = a.rounding : It(t, 0, 8), n = te(new a(i), e + de(i) + 1, t), r = Cr(n.abs(), !1, e + de(n) + 1), i.isneg() && !i.isZero() ? "-" + r : r);
};
j.toInteger = j.toint = function() {
  var e = this, t = e.constructor;
  return te(new t(e), de(e) + 1, t.rounding);
};
j.toNumber = function() {
  return +this;
};
j.toPower = j.pow = function(e) {
  var t, r, n, i, a, o, u = this, l = u.constructor, c = 12, s = +(e = new l(e));
  if (!e.s) return new l(qe);
  if (u = new l(u), !u.s) {
    if (e.s < 1) throw Error(ot + "Infinity");
    return u;
  }
  if (u.eq(qe)) return u;
  if (n = l.precision, e.eq(qe)) return te(u, n);
  if (t = e.e, r = e.d.length - 1, o = t >= r, a = u.s, o) {
    if ((r = s < 0 ? -s : s) <= ep) {
      for (i = new l(qe), t = Math.ceil(n / ne + 4), ae = !1; r % 2 && (i = i.times(u), Xs(i.d, t)), r = un(r / 2), r !== 0; )
        u = u.times(u), Xs(u.d, t);
      return ae = !0, e.s < 0 ? new l(qe).div(i) : te(i, n);
    }
  } else if (a < 0) throw Error(ot + "NaN");
  return a = a < 0 && e.d[Math.max(t, r)] & 1 ? -1 : 1, u.s = 1, ae = !1, i = e.times($n(u, n + c)), ae = !0, i = rp(i), i.s = a, i;
};
j.toPrecision = function(e, t) {
  var r, n, i = this, a = i.constructor;
  return e === void 0 ? (r = de(i), n = Cr(i, r <= a.toExpNeg || r >= a.toExpPos)) : (It(e, 1, on), t === void 0 ? t = a.rounding : It(t, 0, 8), i = te(new a(i), e, t), r = de(i), n = Cr(i, e <= r || r <= a.toExpNeg, e)), n;
};
j.toSignificantDigits = j.tosd = function(e, t) {
  var r = this, n = r.constructor;
  return e === void 0 ? (e = n.precision, t = n.rounding) : (It(e, 1, on), t === void 0 ? t = n.rounding : It(t, 0, 8)), te(new n(r), e, t);
};
j.toString = j.valueOf = j.val = j.toJSON = j[Symbol.for("nodejs.util.inspect.custom")] = function() {
  var e = this, t = de(e), r = e.constructor;
  return Cr(e, t <= r.toExpNeg || t >= r.toExpPos);
};
function tp(e, t) {
  var r, n, i, a, o, u, l, c, s = e.constructor, f = s.precision;
  if (!e.s || !t.s)
    return t.s || (t = new s(e)), ae ? te(t, f) : t;
  if (l = e.d, c = t.d, o = e.e, i = t.e, l = l.slice(), a = o - i, a) {
    for (a < 0 ? (n = l, a = -a, u = c.length) : (n = c, i = o, u = l.length), o = Math.ceil(f / ne), u = o > u ? o + 1 : u + 1, a > u && (a = u, n.length = 1), n.reverse(); a--; ) n.push(0);
    n.reverse();
  }
  for (u = l.length, a = c.length, u - a < 0 && (a = u, n = c, c = l, l = n), r = 0; a; )
    r = (l[--a] = l[a] + c[a] + r) / be | 0, l[a] %= be;
  for (r && (l.unshift(r), ++i), u = l.length; l[--u] == 0; ) l.pop();
  return t.d = l, t.e = i, ae ? te(t, f) : t;
}
function It(e, t, r) {
  if (e !== ~~e || e < t || e > r)
    throw Error(Er + e);
}
function Ot(e) {
  var t, r, n, i = e.length - 1, a = "", o = e[0];
  if (i > 0) {
    for (a += o, t = 1; t < i; t++)
      n = e[t] + "", r = ne - n.length, r && (a += rr(r)), a += n;
    o = e[t], n = o + "", r = ne - n.length, r && (a += rr(r));
  } else if (o === 0)
    return "0";
  for (; o % 10 === 0; ) o /= 10;
  return a + o;
}
var Lt = /* @__PURE__ */ function() {
  function e(n, i) {
    var a, o = 0, u = n.length;
    for (n = n.slice(); u--; )
      a = n[u] * i + o, n[u] = a % be | 0, o = a / be | 0;
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
      n[a] -= o, o = n[a] < i[a] ? 1 : 0, n[a] = o * be + n[a] - i[a];
    for (; !n[0] && n.length > 1; ) n.shift();
  }
  return function(n, i, a, o) {
    var u, l, c, s, f, d, h, v, p, m, y, b, x, O, A, g, E, _, I = n.constructor, C = n.s == i.s ? 1 : -1, T = n.d, P = i.d;
    if (!n.s) return new I(n);
    if (!i.s) throw Error(ot + "Division by zero");
    for (l = n.e - i.e, E = P.length, A = T.length, h = new I(C), v = h.d = [], c = 0; P[c] == (T[c] || 0); ) ++c;
    if (P[c] > (T[c] || 0) && --l, a == null ? b = a = I.precision : o ? b = a + (de(n) - de(i)) + 1 : b = a, b < 0) return new I(0);
    if (b = b / ne + 2 | 0, c = 0, E == 1)
      for (s = 0, P = P[0], b++; (c < A || s) && b--; c++)
        x = s * be + (T[c] || 0), v[c] = x / P | 0, s = x % P | 0;
    else {
      for (s = be / (P[0] + 1) | 0, s > 1 && (P = e(P, s), T = e(T, s), E = P.length, A = T.length), O = E, p = T.slice(0, E), m = p.length; m < E; ) p[m++] = 0;
      _ = P.slice(), _.unshift(0), g = P[0], P[1] >= be / 2 && ++g;
      do
        s = 0, u = t(P, p, E, m), u < 0 ? (y = p[0], E != m && (y = y * be + (p[1] || 0)), s = y / g | 0, s > 1 ? (s >= be && (s = be - 1), f = e(P, s), d = f.length, m = p.length, u = t(f, p, d, m), u == 1 && (s--, r(f, E < d ? _ : P, d))) : (s == 0 && (u = s = 1), f = P.slice()), d = f.length, d < m && f.unshift(0), r(p, f, m), u == -1 && (m = p.length, u = t(P, p, E, m), u < 1 && (s++, r(p, E < m ? _ : P, m))), m = p.length) : u === 0 && (s++, p = [0]), v[c++] = s, u && p[0] ? p[m++] = T[O] || 0 : (p = [T[O]], m = 1);
      while ((O++ < A || p[0] !== void 0) && b--);
    }
    return v[0] || v.shift(), h.e = l, te(h, o ? a + de(h) + 1 : a);
  };
}();
function rp(e, t) {
  var r, n, i, a, o, u, l = 0, c = 0, s = e.constructor, f = s.precision;
  if (de(e) > 16) throw Error(Zu + de(e));
  if (!e.s) return new s(qe);
  for (ae = !1, u = f, o = new s(0.03125); e.abs().gte(0.1); )
    e = e.times(o), c += 5;
  for (n = Math.log(mr(2, c)) / Math.LN10 * 2 + 5 | 0, u += n, r = i = a = new s(qe), s.precision = u; ; ) {
    if (i = te(i.times(e), u), r = r.times(++l), o = a.plus(Lt(i, r, u)), Ot(o.d).slice(0, u) === Ot(a.d).slice(0, u)) {
      for (; c--; ) a = te(a.times(a), u);
      return s.precision = f, t == null ? (ae = !0, te(a, f)) : a;
    }
    a = o;
  }
}
function de(e) {
  for (var t = e.e * ne, r = e.d[0]; r >= 10; r /= 10) t++;
  return t;
}
function Eo(e, t, r) {
  if (t > e.LN10.sd())
    throw ae = !0, r && (e.precision = r), Error(ot + "LN10 precision limit exceeded");
  return te(new e(e.LN10), t);
}
function rr(e) {
  for (var t = ""; e--; ) t += "0";
  return t;
}
function $n(e, t) {
  var r, n, i, a, o, u, l, c, s, f = 1, d = 10, h = e, v = h.d, p = h.constructor, m = p.precision;
  if (h.s < 1) throw Error(ot + (h.s ? "NaN" : "-Infinity"));
  if (h.eq(qe)) return new p(0);
  if (t == null ? (ae = !1, c = m) : c = t, h.eq(10))
    return t == null && (ae = !0), Eo(p, c);
  if (c += d, p.precision = c, r = Ot(v), n = r.charAt(0), a = de(h), Math.abs(a) < 15e14) {
    for (; n < 7 && n != 1 || n == 1 && r.charAt(1) > 3; )
      h = h.times(e), r = Ot(h.d), n = r.charAt(0), f++;
    a = de(h), n > 1 ? (h = new p("0." + r), a++) : h = new p(n + "." + r.slice(1));
  } else
    return l = Eo(p, c + 2, m).times(a + ""), h = $n(new p(n + "." + r.slice(1)), c - d).plus(l), p.precision = m, t == null ? (ae = !0, te(h, m)) : h;
  for (u = o = h = Lt(h.minus(qe), h.plus(qe), c), s = te(h.times(h), c), i = 3; ; ) {
    if (o = te(o.times(s), c), l = u.plus(Lt(o, new p(i), c)), Ot(l.d).slice(0, c) === Ot(u.d).slice(0, c))
      return u = u.times(2), a !== 0 && (u = u.plus(Eo(p, c + 2, m).times(a + ""))), u = Lt(u, new p(f), c), p.precision = m, t == null ? (ae = !0, te(u, m)) : u;
    u = l, i += 2;
  }
}
function qs(e, t) {
  var r, n, i;
  for ((r = t.indexOf(".")) > -1 && (t = t.replace(".", "")), (n = t.search(/e/i)) > 0 ? (r < 0 && (r = n), r += +t.slice(n + 1), t = t.substring(0, n)) : r < 0 && (r = t.length), n = 0; t.charCodeAt(n) === 48; ) ++n;
  for (i = t.length; t.charCodeAt(i - 1) === 48; ) --i;
  if (t = t.slice(n, i), t) {
    if (i -= n, r = r - n - 1, e.e = un(r / ne), e.d = [], n = (r + 1) % ne, r < 0 && (n += ne), n < i) {
      for (n && e.d.push(+t.slice(0, n)), i -= ne; n < i; ) e.d.push(+t.slice(n, n += ne));
      t = t.slice(n), n = ne - t.length;
    } else
      n -= i;
    for (; n--; ) t += "0";
    if (e.d.push(+t), ae && (e.e > ta || e.e < -ta)) throw Error(Zu + r);
  } else
    e.s = 0, e.e = 0, e.d = [0];
  return e;
}
function te(e, t, r) {
  var n, i, a, o, u, l, c, s, f = e.d;
  for (o = 1, a = f[0]; a >= 10; a /= 10) o++;
  if (n = t - o, n < 0)
    n += ne, i = t, c = f[s = 0];
  else {
    if (s = Math.ceil((n + 1) / ne), a = f.length, s >= a) return e;
    for (c = a = f[s], o = 1; a >= 10; a /= 10) o++;
    n %= ne, i = n - ne + o;
  }
  if (r !== void 0 && (a = mr(10, o - i - 1), u = c / a % 10 | 0, l = t < 0 || f[s + 1] !== void 0 || c % a, l = r < 4 ? (u || l) && (r == 0 || r == (e.s < 0 ? 3 : 2)) : u > 5 || u == 5 && (r == 4 || l || r == 6 && // Check whether the digit to the left of the rounding digit is odd.
  (n > 0 ? i > 0 ? c / mr(10, o - i) : 0 : f[s - 1]) % 10 & 1 || r == (e.s < 0 ? 8 : 7))), t < 1 || !f[0])
    return l ? (a = de(e), f.length = 1, t = t - a - 1, f[0] = mr(10, (ne - t % ne) % ne), e.e = un(-t / ne) || 0) : (f.length = 1, f[0] = e.e = e.s = 0), e;
  if (n == 0 ? (f.length = s, a = 1, s--) : (f.length = s + 1, a = mr(10, ne - n), f[s] = i > 0 ? (c / mr(10, o - i) % mr(10, i) | 0) * a : 0), l)
    for (; ; )
      if (s == 0) {
        (f[0] += a) == be && (f[0] = 1, ++e.e);
        break;
      } else {
        if (f[s] += a, f[s] != be) break;
        f[s--] = 0, a = 1;
      }
  for (n = f.length; f[--n] === 0; ) f.pop();
  if (ae && (e.e > ta || e.e < -ta))
    throw Error(Zu + de(e));
  return e;
}
function np(e, t) {
  var r, n, i, a, o, u, l, c, s, f, d = e.constructor, h = d.precision;
  if (!e.s || !t.s)
    return t.s ? t.s = -t.s : t = new d(e), ae ? te(t, h) : t;
  if (l = e.d, f = t.d, n = t.e, c = e.e, l = l.slice(), o = c - n, o) {
    for (s = o < 0, s ? (r = l, o = -o, u = f.length) : (r = f, n = c, u = l.length), i = Math.max(Math.ceil(h / ne), u) + 2, o > i && (o = i, r.length = 1), r.reverse(), i = o; i--; ) r.push(0);
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
      for (a = i; a && l[--a] === 0; ) l[a] = be - 1;
      --l[a], l[i] += be;
    }
    l[i] -= f[i];
  }
  for (; l[--u] === 0; ) l.pop();
  for (; l[0] === 0; l.shift()) --n;
  return l[0] ? (t.d = l, t.e = n, ae ? te(t, h) : t) : new d(0);
}
function Cr(e, t, r) {
  var n, i = de(e), a = Ot(e.d), o = a.length;
  return t ? (r && (n = r - o) > 0 ? a = a.charAt(0) + "." + a.slice(1) + rr(n) : o > 1 && (a = a.charAt(0) + "." + a.slice(1)), a = a + (i < 0 ? "e" : "e+") + i) : i < 0 ? (a = "0." + rr(-i - 1) + a, r && (n = r - o) > 0 && (a += rr(n))) : i >= o ? (a += rr(i + 1 - o), r && (n = r - i - 1) > 0 && (a = a + "." + rr(n))) : ((n = i + 1) < o && (a = a.slice(0, n) + "." + a.slice(n)), r && (n = r - o) > 0 && (i + 1 === o && (a += "."), a += rr(n))), e.s < 0 ? "-" + a : a;
}
function Xs(e, t) {
  if (e.length > t)
    return e.length = t, !0;
}
function ip(e) {
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
        throw Error(Er + a);
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
      return qs(o, a.toString());
    } else if (typeof a != "string")
      throw Error(Er + a);
    if (a.charCodeAt(0) === 45 ? (a = a.slice(1), o.s = -1) : o.s = 1, uA.test(a)) qs(o, a);
    else throw Error(Er + a);
  }
  if (i.prototype = j, i.ROUND_UP = 0, i.ROUND_DOWN = 1, i.ROUND_CEIL = 2, i.ROUND_FLOOR = 3, i.ROUND_HALF_UP = 4, i.ROUND_HALF_DOWN = 5, i.ROUND_HALF_EVEN = 6, i.ROUND_HALF_CEIL = 7, i.ROUND_HALF_FLOOR = 8, i.clone = ip, i.config = i.set = lA, e === void 0 && (e = {}), e)
    for (n = ["precision", "rounding", "toExpNeg", "toExpPos", "LN10"], t = 0; t < n.length; ) e.hasOwnProperty(r = n[t++]) || (e[r] = this[r]);
  return i.config(e), i;
}
function lA(e) {
  if (!e || typeof e != "object")
    throw Error(ot + "Object expected");
  var t, r, n, i = [
    "precision",
    1,
    on,
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
      if (un(n) === n && n >= i[t + 1] && n <= i[t + 2]) this[r] = n;
      else throw Error(Er + r + ": " + n);
  if ((n = e[r = "LN10"]) !== void 0)
    if (n == Math.LN10) this[r] = new this(n);
    else throw Error(Er + r + ": " + n);
  return this;
}
var Qu = ip(oA);
qe = new Qu(1);
const U = Qu;
function ap(e) {
  var t;
  return e === 0 ? t = 1 : t = Math.floor(new U(e).abs().log(10).toNumber()) + 1, t;
}
function op(e, t, r) {
  for (var n = new U(e), i = 0, a = []; n.lt(t) && i < 1e5; )
    a.push(n.toNumber()), n = n.add(r), i++;
  return a;
}
function Ln(e, t) {
  return dA(e) || fA(e, t) || sA(e, t) || cA();
}
function cA() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function sA(e, t) {
  if (e) {
    if (typeof e == "string") return Zs(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Zs(e, t) : void 0;
  }
}
function Zs(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function fA(e, t) {
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
function dA(e) {
  if (Array.isArray(e)) return e;
}
var up = (e) => {
  var t = Ln(e, 2), r = t[0], n = t[1], i = r, a = n;
  return r > n && (i = n, a = r), [i, a];
}, Ju = (e, t, r) => {
  if (e.lte(0))
    return new U(0);
  var n = ap(e.toNumber()), i = new U(10).pow(n), a = e.div(i), o = n !== 1 ? 0.05 : 0.1, u = new U(Math.ceil(a.div(o).toNumber())).add(r).mul(o), l = u.mul(i);
  return t ? new U(l.toNumber()) : new U(Math.ceil(l.toNumber()));
}, lp = (e, t, r) => {
  var n;
  if (e.lte(0))
    return new U(0);
  var i = [1, 2, 2.5, 5], a = e.toNumber(), o = Math.floor(new U(a).abs().log(10).toNumber()), u = new U(10).pow(o), l = e.div(u).toNumber(), c = i.findIndex((h) => h >= l - 1e-10);
  if (c === -1 && (u = u.mul(10), c = 0), c += r, c >= i.length) {
    var s = Math.floor(c / i.length);
    c %= i.length, u = u.mul(new U(10).pow(s));
  }
  var f = (n = i[c]) !== null && n !== void 0 ? n : 1, d = new U(f).mul(u);
  return t ? d : new U(Math.ceil(d.toNumber()));
}, hA = (e, t, r) => {
  var n = new U(1), i = new U(e);
  if (!i.isint() && r) {
    var a = Math.abs(e);
    a < 1 ? (n = new U(10).pow(ap(e) - 1), i = new U(Math.floor(i.div(n).toNumber())).mul(n)) : a > 1 && (i = new U(Math.floor(e)));
  } else e === 0 ? i = new U(Math.floor((t - 1) / 2)) : r || (i = new U(Math.floor(e)));
  for (var o = Math.floor((t - 1) / 2), u = [], l = 0; l < t; l++)
    u.push(i.add(new U(l - o).mul(n)).toNumber());
  return u;
}, cp = function(t, r, n, i) {
  var a = arguments.length > 4 && arguments[4] !== void 0 ? arguments[4] : 0, o = arguments.length > 5 && arguments[5] !== void 0 ? arguments[5] : Ju;
  if (!Number.isFinite((r - t) / (n - 1)))
    return {
      step: new U(0),
      tickMin: new U(0),
      tickMax: new U(0)
    };
  var u = o(new U(r).sub(t).div(n - 1), i, a), l;
  t <= 0 && r >= 0 ? l = new U(0) : (l = new U(t).add(r).div(2), l = l.sub(new U(l).mod(u)));
  var c = Math.ceil(l.sub(t).div(u).toNumber()), s = Math.ceil(new U(r).sub(l).div(u).toNumber()), f = c + s + 1;
  return f > n ? cp(t, r, n, i, a + 1, o) : (f < n && (s = r > 0 ? s + (n - f) : s, c = r > 0 ? c : c + (n - f)), {
    step: u,
    tickMin: l.sub(new U(c).mul(u)),
    tickMax: l.add(new U(s).mul(u))
  });
}, Qs = function(t) {
  var r = Ln(t, 2), n = r[0], i = r[1], a = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : 6, o = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : !0, u = arguments.length > 3 && arguments[3] !== void 0 ? arguments[3] : "auto", l = Math.max(a, 2), c = up([n, i]), s = Ln(c, 2), f = s[0], d = s[1];
  if (f === -1 / 0 || d === 1 / 0) {
    var h = d === 1 / 0 ? [f, ...Array(a - 1).fill(1 / 0)] : [...Array(a - 1).fill(-1 / 0), d];
    return n > i ? h.reverse() : h;
  }
  if (f === d)
    return hA(f, a, o);
  var v = u === "snap125" ? lp : Ju, p = cp(f, d, l, o, 0, v), m = p.step, y = p.tickMin, b = p.tickMax, x = op(y, b.add(new U(0.1).mul(m)), m);
  return n > i ? x.reverse() : x;
}, Js = function(t, r) {
  var n = Ln(t, 2), i = n[0], a = n[1], o = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : !0, u = arguments.length > 3 && arguments[3] !== void 0 ? arguments[3] : "auto", l = up([i, a]), c = Ln(l, 2), s = c[0], f = c[1];
  if (s === -1 / 0 || f === 1 / 0)
    return [i, a];
  if (s === f)
    return [s];
  var d = u === "snap125" ? lp : Ju, h = Math.max(r, 2), v = d(new U(f).sub(s).div(h - 1), o, 0), p = [...op(new U(s), new U(f), v), f];
  if (o === !1) {
    p = p.map((y) => Math.round(y));
    var m = p.length - 1;
    m > 0 && p[m] === p[m - 1] && (p = p.slice(0, m));
  }
  return i > a ? p.reverse() : p;
}, vA = (e) => e.rootProps.barCategoryGap, Va = (e) => e.rootProps.stackOffset, sp = (e) => e.rootProps.reverseStackOrder, el = (e) => e.options.chartName, tl = (e) => e.rootProps.syncId, fp = (e) => e.rootProps.syncMethod, rl = (e) => e.options.eventEmitter, Re = {
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
}, fr = {
  allowDecimals: !1,
  // if I set this to false then Tooltip synchronisation stops working in Radar, wtf
  allowDataOverflow: !1,
  angleAxisId: 0,
  reversed: !1,
  scale: "auto",
  tick: !0,
  type: "auto"
}, mt = {
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
}, Ka = (e, t) => {
  if (!(!e || !t))
    return e != null && e.reversed ? [t[1], t[0]] : t;
};
function Ha(e, t, r) {
  if (r !== "auto")
    return r;
  if (e != null)
    return Ht(e, t) ? "category" : "number";
}
function ef(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function ra(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? ef(Object(r), !0).forEach(function(n) {
      pA(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : ef(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function pA(e, t, r) {
  return (t = mA(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function mA(e) {
  var t = yA(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function yA(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var tf = {
  allowDataOverflow: fr.allowDataOverflow,
  allowDecimals: fr.allowDecimals,
  allowDuplicatedCategory: !1,
  // defaultPolarAngleAxisProps.allowDuplicatedCategory has it set to true but the actual axis rendering ignores the prop because reasons,
  dataKey: void 0,
  domain: void 0,
  id: fr.angleAxisId,
  includeHidden: !1,
  name: void 0,
  reversed: fr.reversed,
  scale: fr.scale,
  tick: fr.tick,
  tickCount: void 0,
  ticks: void 0,
  type: fr.type,
  unit: void 0,
  niceTicks: "auto"
}, rf = {
  allowDataOverflow: mt.allowDataOverflow,
  allowDecimals: mt.allowDecimals,
  allowDuplicatedCategory: mt.allowDuplicatedCategory,
  dataKey: void 0,
  domain: void 0,
  id: mt.radiusAxisId,
  includeHidden: mt.includeHidden,
  name: void 0,
  reversed: mt.reversed,
  scale: mt.scale,
  tick: mt.tick,
  tickCount: mt.tickCount,
  ticks: void 0,
  type: mt.type,
  unit: void 0,
  niceTicks: "auto"
}, gA = (e, t) => {
  if (t != null)
    return e.polarAxis.angleAxis[t];
}, nl = S([gA, zv], (e, t) => {
  var r;
  if (e != null)
    return e;
  var n = (r = Ha(t, "angleAxis", tf.type)) !== null && r !== void 0 ? r : "category";
  return ra(ra({}, tf), {}, {
    type: n
  });
}), bA = (e, t) => e.polarAxis.radiusAxis[t], il = S([bA, zv], (e, t) => {
  var r;
  if (e != null)
    return e;
  var n = (r = Ha(t, "radiusAxis", rf.type)) !== null && r !== void 0 ? r : "category";
  return ra(ra({}, rf), {}, {
    type: n
  });
}), Ga = (e) => e.polarOptions, al = S([Gt, Yt, Ce], F1), dp = S([Ga, al], (e, t) => {
  if (e != null)
    return ur(e.innerRadius, t, 0);
}), hp = S([Ga, al], (e, t) => {
  if (e != null)
    return ur(e.outerRadius, t, t * 0.8);
}), wA = (e) => {
  if (e == null)
    return [0, 0];
  var t = e.startAngle, r = e.endAngle;
  return [t, r];
}, vp = S([Ga], wA);
S([nl, vp], Ka);
var pp = S([al, dp, hp], (e, t, r) => {
  if (!(e == null || t == null || r == null))
    return [t, r];
});
S([il, pp], Ka);
var mp = S([ue, Ga, dp, hp, Gt, Yt], (e, t, r, n, i, a) => {
  if (!(e !== "centric" && e !== "radial" || t == null || r == null || n == null)) {
    var o = t.cx, u = t.cy, l = t.startAngle, c = t.endAngle;
    return {
      cx: ur(o, i, i / 2),
      cy: ur(u, a, a / 2),
      innerRadius: r,
      outerRadius: n,
      startAngle: l,
      endAngle: c,
      clockWise: !1
      // this property look useful, why not use it?
    };
  }
}), Oe = (e, t) => t, Ya = (e, t, r) => r;
function yp(e) {
  return e == null ? void 0 : e.id;
}
function gp(e, t, r) {
  var n = t.chartData, i = n === void 0 ? [] : n, a = r.allowDuplicatedCategory, o = r.dataKey, u = /* @__PURE__ */ new Map();
  return e.forEach((l) => {
    var c, s = (c = l.data) !== null && c !== void 0 ? c : i;
    if (!(s == null || s.length === 0)) {
      var f = yp(l);
      s.forEach((d, h) => {
        var v = o == null || a ? h : String(we(d, o, null)), p = we(d, l.dataKey, 0), m;
        u.has(v) ? m = u.get(v) : m = {}, Object.assign(m, {
          [f]: p
        }), u.set(v, m);
      });
    }
  }), Array.from(u.values());
}
function ol(e) {
  return "stackId" in e && e.stackId != null && e.dataKey != null;
}
var ei = (e, t) => e === t ? !0 : e == null || t == null ? !1 : e[0] === t[0] && e[1] === t[1];
function qa(e, t) {
  return Array.isArray(e) && Array.isArray(t) && e.length === 0 && t.length === 0 ? !0 : e === t;
}
function xA(e, t) {
  if (e.length === t.length) {
    for (var r = 0; r < e.length; r++)
      if (e[r] !== t[r])
        return !1;
    return !0;
  }
  return !1;
}
var Ae = (e) => {
  var t = ue(e);
  return t === "horizontal" ? "xAxis" : t === "vertical" ? "yAxis" : t === "centric" ? "angleAxis" : "radiusAxis";
}, ln = (e) => e.tooltip.settings.axisId;
function ul(e) {
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
var OA = (e, t) => {
  if (t != null)
    switch (e) {
      case "linear": {
        if (!At(t)) {
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
function ar(e, t) {
  return e == null || t == null ? NaN : e < t ? -1 : e > t ? 1 : e >= t ? 0 : NaN;
}
function AA(e, t) {
  return e == null || t == null ? NaN : t < e ? -1 : t > e ? 1 : t >= e ? 0 : NaN;
}
function ll(e) {
  let t, r, n;
  e.length !== 2 ? (t = ar, r = (u, l) => ar(e(u), l), n = (u, l) => e(u) - l) : (t = e === ar || e === AA ? e : SA, r = e, n = e);
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
function SA() {
  return 0;
}
function bp(e) {
  return e === null ? NaN : +e;
}
function* EA(e, t) {
  for (let r of e)
    r != null && (r = +r) >= r && (yield r);
}
const PA = ll(ar), ti = PA.right;
ll(bp).center;
class nf extends Map {
  constructor(t, r = kA) {
    if (super(), Object.defineProperties(this, { _intern: { value: /* @__PURE__ */ new Map() }, _key: { value: r } }), t != null) for (const [n, i] of t) this.set(n, i);
  }
  get(t) {
    return super.get(af(this, t));
  }
  has(t) {
    return super.has(af(this, t));
  }
  set(t, r) {
    return super.set(_A(this, t), r);
  }
  delete(t) {
    return super.delete(IA(this, t));
  }
}
function af({ _intern: e, _key: t }, r) {
  const n = t(r);
  return e.has(n) ? e.get(n) : r;
}
function _A({ _intern: e, _key: t }, r) {
  const n = t(r);
  return e.has(n) ? e.get(n) : (e.set(n, r), r);
}
function IA({ _intern: e, _key: t }, r) {
  const n = t(r);
  return e.has(n) && (r = e.get(n), e.delete(n)), r;
}
function kA(e) {
  return e !== null && typeof e == "object" ? e.valueOf() : e;
}
function CA(e = ar) {
  if (e === ar) return wp;
  if (typeof e != "function") throw new TypeError("compare is not a function");
  return (t, r) => {
    const n = e(t, r);
    return n || n === 0 ? n : (e(r, r) === 0) - (e(t, t) === 0);
  };
}
function wp(e, t) {
  return (e == null || !(e >= e)) - (t == null || !(t >= t)) || (e < t ? -1 : e > t ? 1 : 0);
}
const TA = Math.sqrt(50), DA = Math.sqrt(10), jA = Math.sqrt(2);
function na(e, t, r) {
  const n = (t - e) / Math.max(0, r), i = Math.floor(Math.log10(n)), a = n / Math.pow(10, i), o = a >= TA ? 10 : a >= DA ? 5 : a >= jA ? 2 : 1;
  let u, l, c;
  return i < 0 ? (c = Math.pow(10, -i) / o, u = Math.round(e * c), l = Math.round(t * c), u / c < e && ++u, l / c > t && --l, c = -c) : (c = Math.pow(10, i) * o, u = Math.round(e / c), l = Math.round(t / c), u * c < e && ++u, l * c > t && --l), l < u && 0.5 <= r && r < 2 ? na(e, t, r * 2) : [u, l, c];
}
function hu(e, t, r) {
  if (t = +t, e = +e, r = +r, !(r > 0)) return [];
  if (e === t) return [e];
  const n = t < e, [i, a, o] = n ? na(t, e, r) : na(e, t, r);
  if (!(a >= i)) return [];
  const u = a - i + 1, l = new Array(u);
  if (n)
    if (o < 0) for (let c = 0; c < u; ++c) l[c] = (a - c) / -o;
    else for (let c = 0; c < u; ++c) l[c] = (a - c) * o;
  else if (o < 0) for (let c = 0; c < u; ++c) l[c] = (i + c) / -o;
  else for (let c = 0; c < u; ++c) l[c] = (i + c) * o;
  return l;
}
function vu(e, t, r) {
  return t = +t, e = +e, r = +r, na(e, t, r)[2];
}
function pu(e, t, r) {
  t = +t, e = +e, r = +r;
  const n = t < e, i = n ? vu(t, e, r) : vu(e, t, r);
  return (n ? -1 : 1) * (i < 0 ? 1 / -i : i);
}
function of(e, t) {
  let r;
  for (const n of e)
    n != null && (r < n || r === void 0 && n >= n) && (r = n);
  return r;
}
function uf(e, t) {
  let r;
  for (const n of e)
    n != null && (r > n || r === void 0 && n >= n) && (r = n);
  return r;
}
function xp(e, t, r = 0, n = 1 / 0, i) {
  if (t = Math.floor(t), r = Math.floor(Math.max(0, r)), n = Math.floor(Math.min(e.length - 1, n)), !(r <= t && t <= n)) return e;
  for (i = i === void 0 ? wp : CA(i); n > r; ) {
    if (n - r > 600) {
      const l = n - r + 1, c = t - r + 1, s = Math.log(l), f = 0.5 * Math.exp(2 * s / 3), d = 0.5 * Math.sqrt(s * f * (l - f) / l) * (c - l / 2 < 0 ? -1 : 1), h = Math.max(r, Math.floor(t - c * f / l + d)), v = Math.min(n, Math.floor(t + (l - c) * f / l + d));
      xp(e, t, h, v, i);
    }
    const a = e[t];
    let o = r, u = n;
    for (xn(e, r, t), i(e[n], a) > 0 && xn(e, r, n); o < u; ) {
      for (xn(e, o, u), ++o, --u; i(e[o], a) < 0; ) ++o;
      for (; i(e[u], a) > 0; ) --u;
    }
    i(e[r], a) === 0 ? xn(e, r, u) : (++u, xn(e, u, n)), u <= t && (r = u + 1), t <= u && (n = u - 1);
  }
  return e;
}
function xn(e, t, r) {
  const n = e[t];
  e[t] = e[r], e[r] = n;
}
function NA(e, t, r) {
  if (e = Float64Array.from(EA(e)), !(!(n = e.length) || isNaN(t = +t))) {
    if (t <= 0 || n < 2) return uf(e);
    if (t >= 1) return of(e);
    var n, i = (n - 1) * t, a = Math.floor(i), o = of(xp(e, a).subarray(0, a + 1)), u = uf(e.subarray(a + 1));
    return o + (u - o) * (i - a);
  }
}
function MA(e, t, r = bp) {
  if (!(!(n = e.length) || isNaN(t = +t))) {
    if (t <= 0 || n < 2) return +r(e[0], 0, e);
    if (t >= 1) return +r(e[n - 1], n - 1, e);
    var n, i = (n - 1) * t, a = Math.floor(i), o = +r(e[a], a, e), u = +r(e[a + 1], a + 1, e);
    return o + (u - o) * (i - a);
  }
}
function $A(e, t, r) {
  e = +e, t = +t, r = (i = arguments.length) < 2 ? (t = e, e = 0, 1) : i < 3 ? 1 : +r;
  for (var n = -1, i = Math.max(0, Math.ceil((t - e) / r)) | 0, a = new Array(i); ++n < i; )
    a[n] = e + n * r;
  return a;
}
function ut(e, t) {
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
function qt(e, t) {
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
const mu = Symbol("implicit");
function cl() {
  var e = new nf(), t = [], r = [], n = mu;
  function i(a) {
    let o = e.get(a);
    if (o === void 0) {
      if (n !== mu) return n;
      e.set(a, o = t.push(a) - 1);
    }
    return r[o % r.length];
  }
  return i.domain = function(a) {
    if (!arguments.length) return t.slice();
    t = [], e = new nf();
    for (const o of a)
      e.has(o) || e.set(o, t.push(o) - 1);
    return i;
  }, i.range = function(a) {
    return arguments.length ? (r = Array.from(a), i) : r.slice();
  }, i.unknown = function(a) {
    return arguments.length ? (n = a, i) : n;
  }, i.copy = function() {
    return cl(t, r).unknown(n);
  }, ut.apply(i, arguments), i;
}
function sl() {
  var e = cl().unknown(void 0), t = e.domain, r = e.range, n = 0, i = 1, a, o, u = !1, l = 0, c = 0, s = 0.5;
  delete e.unknown;
  function f() {
    var d = t().length, h = i < n, v = h ? i : n, p = h ? n : i;
    a = (p - v) / Math.max(1, d - l + c * 2), u && (a = Math.floor(a)), v += (p - v - a * (d - l)) * s, o = a * (1 - l), u && (v = Math.round(v), o = Math.round(o));
    var m = $A(d).map(function(y) {
      return v + a * y;
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
    return sl(t(), [n, i]).round(u).paddingInner(l).paddingOuter(c).align(s);
  }, ut.apply(f(), arguments);
}
function Op(e) {
  var t = e.copy;
  return e.padding = e.paddingOuter, delete e.paddingInner, delete e.paddingOuter, e.copy = function() {
    return Op(t());
  }, e;
}
function LA() {
  return Op(sl.apply(null, arguments).paddingInner(1));
}
function fl(e, t, r) {
  e.prototype = t.prototype = r, r.constructor = e;
}
function Ap(e, t) {
  var r = Object.create(e.prototype);
  for (var n in t) r[n] = t[n];
  return r;
}
function ri() {
}
var Rn = 0.7, ia = 1 / Rn, Gr = "\\s*([+-]?\\d+)\\s*", zn = "\\s*([+-]?(?:\\d*\\.)?\\d+(?:[eE][+-]?\\d+)?)\\s*", St = "\\s*([+-]?(?:\\d*\\.)?\\d+(?:[eE][+-]?\\d+)?)%\\s*", RA = /^#([0-9a-f]{3,8})$/, zA = new RegExp(`^rgb\\(${Gr},${Gr},${Gr}\\)$`), BA = new RegExp(`^rgb\\(${St},${St},${St}\\)$`), FA = new RegExp(`^rgba\\(${Gr},${Gr},${Gr},${zn}\\)$`), WA = new RegExp(`^rgba\\(${St},${St},${St},${zn}\\)$`), UA = new RegExp(`^hsl\\(${zn},${St},${St}\\)$`), VA = new RegExp(`^hsla\\(${zn},${St},${St},${zn}\\)$`), lf = {
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
fl(ri, Bn, {
  copy(e) {
    return Object.assign(new this.constructor(), this, e);
  },
  displayable() {
    return this.rgb().displayable();
  },
  hex: cf,
  // Deprecated! Use color.formatHex.
  formatHex: cf,
  formatHex8: KA,
  formatHsl: HA,
  formatRgb: sf,
  toString: sf
});
function cf() {
  return this.rgb().formatHex();
}
function KA() {
  return this.rgb().formatHex8();
}
function HA() {
  return Sp(this).formatHsl();
}
function sf() {
  return this.rgb().formatRgb();
}
function Bn(e) {
  var t, r;
  return e = (e + "").trim().toLowerCase(), (t = RA.exec(e)) ? (r = t[1].length, t = parseInt(t[1], 16), r === 6 ? ff(t) : r === 3 ? new We(t >> 8 & 15 | t >> 4 & 240, t >> 4 & 15 | t & 240, (t & 15) << 4 | t & 15, 1) : r === 8 ? Oi(t >> 24 & 255, t >> 16 & 255, t >> 8 & 255, (t & 255) / 255) : r === 4 ? Oi(t >> 12 & 15 | t >> 8 & 240, t >> 8 & 15 | t >> 4 & 240, t >> 4 & 15 | t & 240, ((t & 15) << 4 | t & 15) / 255) : null) : (t = zA.exec(e)) ? new We(t[1], t[2], t[3], 1) : (t = BA.exec(e)) ? new We(t[1] * 255 / 100, t[2] * 255 / 100, t[3] * 255 / 100, 1) : (t = FA.exec(e)) ? Oi(t[1], t[2], t[3], t[4]) : (t = WA.exec(e)) ? Oi(t[1] * 255 / 100, t[2] * 255 / 100, t[3] * 255 / 100, t[4]) : (t = UA.exec(e)) ? vf(t[1], t[2] / 100, t[3] / 100, 1) : (t = VA.exec(e)) ? vf(t[1], t[2] / 100, t[3] / 100, t[4]) : lf.hasOwnProperty(e) ? ff(lf[e]) : e === "transparent" ? new We(NaN, NaN, NaN, 0) : null;
}
function ff(e) {
  return new We(e >> 16 & 255, e >> 8 & 255, e & 255, 1);
}
function Oi(e, t, r, n) {
  return n <= 0 && (e = t = r = NaN), new We(e, t, r, n);
}
function GA(e) {
  return e instanceof ri || (e = Bn(e)), e ? (e = e.rgb(), new We(e.r, e.g, e.b, e.opacity)) : new We();
}
function yu(e, t, r, n) {
  return arguments.length === 1 ? GA(e) : new We(e, t, r, n ?? 1);
}
function We(e, t, r, n) {
  this.r = +e, this.g = +t, this.b = +r, this.opacity = +n;
}
fl(We, yu, Ap(ri, {
  brighter(e) {
    return e = e == null ? ia : Math.pow(ia, e), new We(this.r * e, this.g * e, this.b * e, this.opacity);
  },
  darker(e) {
    return e = e == null ? Rn : Math.pow(Rn, e), new We(this.r * e, this.g * e, this.b * e, this.opacity);
  },
  rgb() {
    return this;
  },
  clamp() {
    return new We(Pr(this.r), Pr(this.g), Pr(this.b), aa(this.opacity));
  },
  displayable() {
    return -0.5 <= this.r && this.r < 255.5 && -0.5 <= this.g && this.g < 255.5 && -0.5 <= this.b && this.b < 255.5 && 0 <= this.opacity && this.opacity <= 1;
  },
  hex: df,
  // Deprecated! Use color.formatHex.
  formatHex: df,
  formatHex8: YA,
  formatRgb: hf,
  toString: hf
}));
function df() {
  return `#${wr(this.r)}${wr(this.g)}${wr(this.b)}`;
}
function YA() {
  return `#${wr(this.r)}${wr(this.g)}${wr(this.b)}${wr((isNaN(this.opacity) ? 1 : this.opacity) * 255)}`;
}
function hf() {
  const e = aa(this.opacity);
  return `${e === 1 ? "rgb(" : "rgba("}${Pr(this.r)}, ${Pr(this.g)}, ${Pr(this.b)}${e === 1 ? ")" : `, ${e})`}`;
}
function aa(e) {
  return isNaN(e) ? 1 : Math.max(0, Math.min(1, e));
}
function Pr(e) {
  return Math.max(0, Math.min(255, Math.round(e) || 0));
}
function wr(e) {
  return e = Pr(e), (e < 16 ? "0" : "") + e.toString(16);
}
function vf(e, t, r, n) {
  return n <= 0 ? e = t = r = NaN : r <= 0 || r >= 1 ? e = t = NaN : t <= 0 && (e = NaN), new dt(e, t, r, n);
}
function Sp(e) {
  if (e instanceof dt) return new dt(e.h, e.s, e.l, e.opacity);
  if (e instanceof ri || (e = Bn(e)), !e) return new dt();
  if (e instanceof dt) return e;
  e = e.rgb();
  var t = e.r / 255, r = e.g / 255, n = e.b / 255, i = Math.min(t, r, n), a = Math.max(t, r, n), o = NaN, u = a - i, l = (a + i) / 2;
  return u ? (t === a ? o = (r - n) / u + (r < n) * 6 : r === a ? o = (n - t) / u + 2 : o = (t - r) / u + 4, u /= l < 0.5 ? a + i : 2 - a - i, o *= 60) : u = l > 0 && l < 1 ? 0 : o, new dt(o, u, l, e.opacity);
}
function qA(e, t, r, n) {
  return arguments.length === 1 ? Sp(e) : new dt(e, t, r, n ?? 1);
}
function dt(e, t, r, n) {
  this.h = +e, this.s = +t, this.l = +r, this.opacity = +n;
}
fl(dt, qA, Ap(ri, {
  brighter(e) {
    return e = e == null ? ia : Math.pow(ia, e), new dt(this.h, this.s, this.l * e, this.opacity);
  },
  darker(e) {
    return e = e == null ? Rn : Math.pow(Rn, e), new dt(this.h, this.s, this.l * e, this.opacity);
  },
  rgb() {
    var e = this.h % 360 + (this.h < 0) * 360, t = isNaN(e) || isNaN(this.s) ? 0 : this.s, r = this.l, n = r + (r < 0.5 ? r : 1 - r) * t, i = 2 * r - n;
    return new We(
      Po(e >= 240 ? e - 240 : e + 120, i, n),
      Po(e, i, n),
      Po(e < 120 ? e + 240 : e - 120, i, n),
      this.opacity
    );
  },
  clamp() {
    return new dt(pf(this.h), Ai(this.s), Ai(this.l), aa(this.opacity));
  },
  displayable() {
    return (0 <= this.s && this.s <= 1 || isNaN(this.s)) && 0 <= this.l && this.l <= 1 && 0 <= this.opacity && this.opacity <= 1;
  },
  formatHsl() {
    const e = aa(this.opacity);
    return `${e === 1 ? "hsl(" : "hsla("}${pf(this.h)}, ${Ai(this.s) * 100}%, ${Ai(this.l) * 100}%${e === 1 ? ")" : `, ${e})`}`;
  }
}));
function pf(e) {
  return e = (e || 0) % 360, e < 0 ? e + 360 : e;
}
function Ai(e) {
  return Math.max(0, Math.min(1, e || 0));
}
function Po(e, t, r) {
  return (e < 60 ? t + (r - t) * e / 60 : e < 180 ? r : e < 240 ? t + (r - t) * (240 - e) / 60 : t) * 255;
}
const dl = (e) => () => e;
function XA(e, t) {
  return function(r) {
    return e + r * t;
  };
}
function ZA(e, t, r) {
  return e = Math.pow(e, r), t = Math.pow(t, r) - e, r = 1 / r, function(n) {
    return Math.pow(e + n * t, r);
  };
}
function QA(e) {
  return (e = +e) == 1 ? Ep : function(t, r) {
    return r - t ? ZA(t, r, e) : dl(isNaN(t) ? r : t);
  };
}
function Ep(e, t) {
  var r = t - e;
  return r ? XA(e, r) : dl(isNaN(e) ? t : e);
}
const mf = function e(t) {
  var r = QA(t);
  function n(i, a) {
    var o = r((i = yu(i)).r, (a = yu(a)).r), u = r(i.g, a.g), l = r(i.b, a.b), c = Ep(i.opacity, a.opacity);
    return function(s) {
      return i.r = o(s), i.g = u(s), i.b = l(s), i.opacity = c(s), i + "";
    };
  }
  return n.gamma = e, n;
}(1);
function JA(e, t) {
  t || (t = []);
  var r = e ? Math.min(t.length, e.length) : 0, n = t.slice(), i;
  return function(a) {
    for (i = 0; i < r; ++i) n[i] = e[i] * (1 - a) + t[i] * a;
    return n;
  };
}
function eS(e) {
  return ArrayBuffer.isView(e) && !(e instanceof DataView);
}
function tS(e, t) {
  var r = t ? t.length : 0, n = e ? Math.min(r, e.length) : 0, i = new Array(n), a = new Array(r), o;
  for (o = 0; o < n; ++o) i[o] = cn(e[o], t[o]);
  for (; o < r; ++o) a[o] = t[o];
  return function(u) {
    for (o = 0; o < n; ++o) a[o] = i[o](u);
    return a;
  };
}
function rS(e, t) {
  var r = /* @__PURE__ */ new Date();
  return e = +e, t = +t, function(n) {
    return r.setTime(e * (1 - n) + t * n), r;
  };
}
function oa(e, t) {
  return e = +e, t = +t, function(r) {
    return e * (1 - r) + t * r;
  };
}
function nS(e, t) {
  var r = {}, n = {}, i;
  (e === null || typeof e != "object") && (e = {}), (t === null || typeof t != "object") && (t = {});
  for (i in t)
    i in e ? r[i] = cn(e[i], t[i]) : n[i] = t[i];
  return function(a) {
    for (i in r) n[i] = r[i](a);
    return n;
  };
}
var gu = /[-+]?(?:\d+\.?\d*|\.?\d+)(?:[eE][-+]?\d+)?/g, _o = new RegExp(gu.source, "g");
function iS(e) {
  return function() {
    return e;
  };
}
function aS(e) {
  return function(t) {
    return e(t) + "";
  };
}
function oS(e, t) {
  var r = gu.lastIndex = _o.lastIndex = 0, n, i, a, o = -1, u = [], l = [];
  for (e = e + "", t = t + ""; (n = gu.exec(e)) && (i = _o.exec(t)); )
    (a = i.index) > r && (a = t.slice(r, a), u[o] ? u[o] += a : u[++o] = a), (n = n[0]) === (i = i[0]) ? u[o] ? u[o] += i : u[++o] = i : (u[++o] = null, l.push({ i: o, x: oa(n, i) })), r = _o.lastIndex;
  return r < t.length && (a = t.slice(r), u[o] ? u[o] += a : u[++o] = a), u.length < 2 ? l[0] ? aS(l[0].x) : iS(t) : (t = l.length, function(c) {
    for (var s = 0, f; s < t; ++s) u[(f = l[s]).i] = f.x(c);
    return u.join("");
  });
}
function cn(e, t) {
  var r = typeof t, n;
  return t == null || r === "boolean" ? dl(t) : (r === "number" ? oa : r === "string" ? (n = Bn(t)) ? (t = n, mf) : oS : t instanceof Bn ? mf : t instanceof Date ? rS : eS(t) ? JA : Array.isArray(t) ? tS : typeof t.valueOf != "function" && typeof t.toString != "function" || isNaN(t) ? nS : oa)(e, t);
}
function hl(e, t) {
  return e = +e, t = +t, function(r) {
    return Math.round(e * (1 - r) + t * r);
  };
}
function uS(e, t) {
  t === void 0 && (t = e, e = cn);
  for (var r = 0, n = t.length - 1, i = t[0], a = new Array(n < 0 ? 0 : n); r < n; ) a[r] = e(i, i = t[++r]);
  return function(o) {
    var u = Math.max(0, Math.min(n - 1, Math.floor(o *= n)));
    return a[u](o - u);
  };
}
function lS(e) {
  return function() {
    return e;
  };
}
function ua(e) {
  return +e;
}
var yf = [0, 1];
function ze(e) {
  return e;
}
function bu(e, t) {
  return (t -= e = +e) ? function(r) {
    return (r - e) / t;
  } : lS(isNaN(t) ? NaN : 0.5);
}
function cS(e, t) {
  var r;
  return e > t && (r = e, e = t, t = r), function(n) {
    return Math.max(e, Math.min(t, n));
  };
}
function sS(e, t, r) {
  var n = e[0], i = e[1], a = t[0], o = t[1];
  return i < n ? (n = bu(i, n), a = r(o, a)) : (n = bu(n, i), a = r(a, o)), function(u) {
    return a(n(u));
  };
}
function fS(e, t, r) {
  var n = Math.min(e.length, t.length) - 1, i = new Array(n), a = new Array(n), o = -1;
  for (e[n] < e[0] && (e = e.slice().reverse(), t = t.slice().reverse()); ++o < n; )
    i[o] = bu(e[o], e[o + 1]), a[o] = r(t[o], t[o + 1]);
  return function(u) {
    var l = ti(e, u, 1, n) - 1;
    return a[l](i[l](u));
  };
}
function ni(e, t) {
  return t.domain(e.domain()).range(e.range()).interpolate(e.interpolate()).clamp(e.clamp()).unknown(e.unknown());
}
function Xa() {
  var e = yf, t = yf, r = cn, n, i, a, o = ze, u, l, c;
  function s() {
    var d = Math.min(e.length, t.length);
    return o !== ze && (o = cS(e[0], e[d - 1])), u = d > 2 ? fS : sS, l = c = null, f;
  }
  function f(d) {
    return d == null || isNaN(d = +d) ? a : (l || (l = u(e.map(n), t, r)))(n(o(d)));
  }
  return f.invert = function(d) {
    return o(i((c || (c = u(t, e.map(n), oa)))(d)));
  }, f.domain = function(d) {
    return arguments.length ? (e = Array.from(d, ua), s()) : e.slice();
  }, f.range = function(d) {
    return arguments.length ? (t = Array.from(d), s()) : t.slice();
  }, f.rangeRound = function(d) {
    return t = Array.from(d), r = hl, s();
  }, f.clamp = function(d) {
    return arguments.length ? (o = d ? !0 : ze, s()) : o !== ze;
  }, f.interpolate = function(d) {
    return arguments.length ? (r = d, s()) : r;
  }, f.unknown = function(d) {
    return arguments.length ? (a = d, f) : a;
  }, function(d, h) {
    return n = d, i = h, s();
  };
}
function vl() {
  return Xa()(ze, ze);
}
function dS(e) {
  return Math.abs(e = Math.round(e)) >= 1e21 ? e.toLocaleString("en").replace(/,/g, "") : e.toString(10);
}
function la(e, t) {
  if (!isFinite(e) || e === 0) return null;
  var r = (e = t ? e.toExponential(t - 1) : e.toExponential()).indexOf("e"), n = e.slice(0, r);
  return [
    n.length > 1 ? n[0] + n.slice(2) : n,
    +e.slice(r + 1)
  ];
}
function Xr(e) {
  return e = la(Math.abs(e)), e ? e[1] : NaN;
}
function hS(e, t) {
  return function(r, n) {
    for (var i = r.length, a = [], o = 0, u = e[0], l = 0; i > 0 && u > 0 && (l + u + 1 > n && (u = Math.max(1, n - l)), a.push(r.substring(i -= u, i + u)), !((l += u + 1) > n)); )
      u = e[o = (o + 1) % e.length];
    return a.reverse().join(t);
  };
}
function vS(e) {
  return function(t) {
    return t.replace(/[0-9]/g, function(r) {
      return e[+r];
    });
  };
}
var pS = /^(?:(.)?([<>=^]))?([+\-( ])?([$#])?(0)?(\d+)?(,)?(\.\d+)?(~)?([a-z%])?$/i;
function Fn(e) {
  if (!(t = pS.exec(e))) throw new Error("invalid format: " + e);
  var t;
  return new pl({
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
Fn.prototype = pl.prototype;
function pl(e) {
  this.fill = e.fill === void 0 ? " " : e.fill + "", this.align = e.align === void 0 ? ">" : e.align + "", this.sign = e.sign === void 0 ? "-" : e.sign + "", this.symbol = e.symbol === void 0 ? "" : e.symbol + "", this.zero = !!e.zero, this.width = e.width === void 0 ? void 0 : +e.width, this.comma = !!e.comma, this.precision = e.precision === void 0 ? void 0 : +e.precision, this.trim = !!e.trim, this.type = e.type === void 0 ? "" : e.type + "";
}
pl.prototype.toString = function() {
  return this.fill + this.align + this.sign + this.symbol + (this.zero ? "0" : "") + (this.width === void 0 ? "" : Math.max(1, this.width | 0)) + (this.comma ? "," : "") + (this.precision === void 0 ? "" : "." + Math.max(0, this.precision | 0)) + (this.trim ? "~" : "") + this.type;
};
function mS(e) {
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
var ca;
function yS(e, t) {
  var r = la(e, t);
  if (!r) return ca = void 0, e.toPrecision(t);
  var n = r[0], i = r[1], a = i - (ca = Math.max(-8, Math.min(8, Math.floor(i / 3))) * 3) + 1, o = n.length;
  return a === o ? n : a > o ? n + new Array(a - o + 1).join("0") : a > 0 ? n.slice(0, a) + "." + n.slice(a) : "0." + new Array(1 - a).join("0") + la(e, Math.max(0, t + a - 1))[0];
}
function gf(e, t) {
  var r = la(e, t);
  if (!r) return e + "";
  var n = r[0], i = r[1];
  return i < 0 ? "0." + new Array(-i).join("0") + n : n.length > i + 1 ? n.slice(0, i + 1) + "." + n.slice(i + 1) : n + new Array(i - n.length + 2).join("0");
}
const bf = {
  "%": (e, t) => (e * 100).toFixed(t),
  b: (e) => Math.round(e).toString(2),
  c: (e) => e + "",
  d: dS,
  e: (e, t) => e.toExponential(t),
  f: (e, t) => e.toFixed(t),
  g: (e, t) => e.toPrecision(t),
  o: (e) => Math.round(e).toString(8),
  p: (e, t) => gf(e * 100, t),
  r: gf,
  s: yS,
  X: (e) => Math.round(e).toString(16).toUpperCase(),
  x: (e) => Math.round(e).toString(16)
};
function wf(e) {
  return e;
}
var xf = Array.prototype.map, Of = ["y", "z", "a", "f", "p", "n", "µ", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y"];
function gS(e) {
  var t = e.grouping === void 0 || e.thousands === void 0 ? wf : hS(xf.call(e.grouping, Number), e.thousands + ""), r = e.currency === void 0 ? "" : e.currency[0] + "", n = e.currency === void 0 ? "" : e.currency[1] + "", i = e.decimal === void 0 ? "." : e.decimal + "", a = e.numerals === void 0 ? wf : vS(xf.call(e.numerals, String)), o = e.percent === void 0 ? "%" : e.percent + "", u = e.minus === void 0 ? "−" : e.minus + "", l = e.nan === void 0 ? "NaN" : e.nan + "";
  function c(f, d) {
    f = Fn(f);
    var h = f.fill, v = f.align, p = f.sign, m = f.symbol, y = f.zero, b = f.width, x = f.comma, O = f.precision, A = f.trim, g = f.type;
    g === "n" ? (x = !0, g = "g") : bf[g] || (O === void 0 && (O = 12), A = !0, g = "g"), (y || h === "0" && v === "=") && (y = !0, h = "0", v = "=");
    var E = (d && d.prefix !== void 0 ? d.prefix : "") + (m === "$" ? r : m === "#" && /[boxX]/.test(g) ? "0" + g.toLowerCase() : ""), _ = (m === "$" ? n : /[%p]/.test(g) ? o : "") + (d && d.suffix !== void 0 ? d.suffix : ""), I = bf[g], C = /[defgprs%]/.test(g);
    O = O === void 0 ? 6 : /[gprs]/.test(g) ? Math.max(1, Math.min(21, O)) : Math.max(0, Math.min(20, O));
    function T(P) {
      var z = E, $ = _, Q, K, H;
      if (g === "c")
        $ = I(P) + $, P = "";
      else {
        P = +P;
        var B = P < 0 || 1 / P < 0;
        if (P = isNaN(P) ? l : I(Math.abs(P), O), A && (P = mS(P)), B && +P == 0 && p !== "+" && (B = !1), z = (B ? p === "(" ? p : u : p === "-" || p === "(" ? "" : p) + z, $ = (g === "s" && !isNaN(P) && ca !== void 0 ? Of[8 + ca / 3] : "") + $ + (B && p === "(" ? ")" : ""), C) {
          for (Q = -1, K = P.length; ++Q < K; )
            if (H = P.charCodeAt(Q), 48 > H || H > 57) {
              $ = (H === 46 ? i + P.slice(Q + 1) : P.slice(Q)) + $, P = P.slice(0, Q);
              break;
            }
        }
      }
      x && !y && (P = t(P, 1 / 0));
      var X = z.length + P.length + $.length, W = X < b ? new Array(b - X + 1).join(h) : "";
      switch (x && y && (P = t(W + P, W.length ? b - $.length : 1 / 0), W = ""), v) {
        case "<":
          P = z + P + $ + W;
          break;
        case "=":
          P = z + W + P + $;
          break;
        case "^":
          P = W.slice(0, X = W.length >> 1) + z + P + $ + W.slice(X);
          break;
        default:
          P = W + z + P + $;
          break;
      }
      return a(P);
    }
    return T.toString = function() {
      return f + "";
    }, T;
  }
  function s(f, d) {
    var h = Math.max(-8, Math.min(8, Math.floor(Xr(d) / 3))) * 3, v = Math.pow(10, -h), p = c((f = Fn(f), f.type = "f", f), { suffix: Of[8 + h / 3] });
    return function(m) {
      return p(v * m);
    };
  }
  return {
    format: c,
    formatPrefix: s
  };
}
var Si, ml, Pp;
bS({
  thousands: ",",
  grouping: [3],
  currency: ["$", ""]
});
function bS(e) {
  return Si = gS(e), ml = Si.format, Pp = Si.formatPrefix, Si;
}
function wS(e) {
  return Math.max(0, -Xr(Math.abs(e)));
}
function xS(e, t) {
  return Math.max(0, Math.max(-8, Math.min(8, Math.floor(Xr(t) / 3))) * 3 - Xr(Math.abs(e)));
}
function OS(e, t) {
  return e = Math.abs(e), t = Math.abs(t) - e, Math.max(0, Xr(t) - Xr(e)) + 1;
}
function _p(e, t, r, n) {
  var i = pu(e, t, r), a;
  switch (n = Fn(n ?? ",f"), n.type) {
    case "s": {
      var o = Math.max(Math.abs(e), Math.abs(t));
      return n.precision == null && !isNaN(a = xS(i, o)) && (n.precision = a), Pp(n, o);
    }
    case "":
    case "e":
    case "g":
    case "p":
    case "r": {
      n.precision == null && !isNaN(a = OS(i, Math.max(Math.abs(e), Math.abs(t)))) && (n.precision = a - (n.type === "e"));
      break;
    }
    case "f":
    case "%": {
      n.precision == null && !isNaN(a = wS(i)) && (n.precision = a - (n.type === "%") * 2);
      break;
    }
  }
  return ml(n);
}
function lr(e) {
  var t = e.domain;
  return e.ticks = function(r) {
    var n = t();
    return hu(n[0], n[n.length - 1], r ?? 10);
  }, e.tickFormat = function(r, n) {
    var i = t();
    return _p(i[0], i[i.length - 1], r ?? 10, n);
  }, e.nice = function(r) {
    r == null && (r = 10);
    var n = t(), i = 0, a = n.length - 1, o = n[i], u = n[a], l, c, s = 10;
    for (u < o && (c = o, o = u, u = c, c = i, i = a, a = c); s-- > 0; ) {
      if (c = vu(o, u, r), c === l)
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
function Ip() {
  var e = vl();
  return e.copy = function() {
    return ni(e, Ip());
  }, ut.apply(e, arguments), lr(e);
}
function kp(e) {
  var t;
  function r(n) {
    return n == null || isNaN(n = +n) ? t : n;
  }
  return r.invert = r, r.domain = r.range = function(n) {
    return arguments.length ? (e = Array.from(n, ua), r) : e.slice();
  }, r.unknown = function(n) {
    return arguments.length ? (t = n, r) : t;
  }, r.copy = function() {
    return kp(e).unknown(t);
  }, e = arguments.length ? Array.from(e, ua) : [0, 1], lr(r);
}
function Cp(e, t) {
  e = e.slice();
  var r = 0, n = e.length - 1, i = e[r], a = e[n], o;
  return a < i && (o = r, r = n, n = o, o = i, i = a, a = o), e[r] = t.floor(i), e[n] = t.ceil(a), e;
}
function Af(e) {
  return Math.log(e);
}
function Sf(e) {
  return Math.exp(e);
}
function AS(e) {
  return -Math.log(-e);
}
function SS(e) {
  return -Math.exp(-e);
}
function ES(e) {
  return isFinite(e) ? +("1e" + e) : e < 0 ? 0 : e;
}
function PS(e) {
  return e === 10 ? ES : e === Math.E ? Math.exp : (t) => Math.pow(e, t);
}
function _S(e) {
  return e === Math.E ? Math.log : e === 10 && Math.log10 || e === 2 && Math.log2 || (e = Math.log(e), (t) => Math.log(t) / e);
}
function Ef(e) {
  return (t, r) => -e(-t, r);
}
function yl(e) {
  const t = e(Af, Sf), r = t.domain;
  let n = 10, i, a;
  function o() {
    return i = _S(n), a = PS(n), r()[0] < 0 ? (i = Ef(i), a = Ef(a), e(AS, SS)) : e(Af, Sf), t;
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
    let d = i(c), h = i(s), v, p;
    const m = u == null ? 10 : +u;
    let y = [];
    if (!(n % 1) && h - d < m) {
      if (d = Math.floor(d), h = Math.ceil(h), c > 0) {
        for (; d <= h; ++d)
          for (v = 1; v < n; ++v)
            if (p = d < 0 ? v / a(-d) : v * a(d), !(p < c)) {
              if (p > s) break;
              y.push(p);
            }
      } else for (; d <= h; ++d)
        for (v = n - 1; v >= 1; --v)
          if (p = d > 0 ? v / a(-d) : v * a(d), !(p < c)) {
            if (p > s) break;
            y.push(p);
          }
      y.length * 2 < m && (y = hu(c, s, m));
    } else
      y = hu(d, h, Math.min(h - d, m)).map(a);
    return f ? y.reverse() : y;
  }, t.tickFormat = (u, l) => {
    if (u == null && (u = 10), l == null && (l = n === 10 ? "s" : ","), typeof l != "function" && (!(n % 1) && (l = Fn(l)).precision == null && (l.trim = !0), l = ml(l)), u === 1 / 0) return l;
    const c = Math.max(1, n * u / t.ticks().length);
    return (s) => {
      let f = s / a(Math.round(i(s)));
      return f * n < n - 0.5 && (f *= n), f <= c ? l(s) : "";
    };
  }, t.nice = () => r(Cp(r(), {
    floor: (u) => a(Math.floor(i(u))),
    ceil: (u) => a(Math.ceil(i(u)))
  })), t;
}
function Tp() {
  const e = yl(Xa()).domain([1, 10]);
  return e.copy = () => ni(e, Tp()).base(e.base()), ut.apply(e, arguments), e;
}
function Pf(e) {
  return function(t) {
    return Math.sign(t) * Math.log1p(Math.abs(t / e));
  };
}
function _f(e) {
  return function(t) {
    return Math.sign(t) * Math.expm1(Math.abs(t)) * e;
  };
}
function gl(e) {
  var t = 1, r = e(Pf(t), _f(t));
  return r.constant = function(n) {
    return arguments.length ? e(Pf(t = +n), _f(t)) : t;
  }, lr(r);
}
function Dp() {
  var e = gl(Xa());
  return e.copy = function() {
    return ni(e, Dp()).constant(e.constant());
  }, ut.apply(e, arguments);
}
function If(e) {
  return function(t) {
    return t < 0 ? -Math.pow(-t, e) : Math.pow(t, e);
  };
}
function IS(e) {
  return e < 0 ? -Math.sqrt(-e) : Math.sqrt(e);
}
function kS(e) {
  return e < 0 ? -e * e : e * e;
}
function bl(e) {
  var t = e(ze, ze), r = 1;
  function n() {
    return r === 1 ? e(ze, ze) : r === 0.5 ? e(IS, kS) : e(If(r), If(1 / r));
  }
  return t.exponent = function(i) {
    return arguments.length ? (r = +i, n()) : r;
  }, lr(t);
}
function wl() {
  var e = bl(Xa());
  return e.copy = function() {
    return ni(e, wl()).exponent(e.exponent());
  }, ut.apply(e, arguments), e;
}
function CS() {
  return wl.apply(null, arguments).exponent(0.5);
}
function kf(e) {
  return Math.sign(e) * e * e;
}
function TS(e) {
  return Math.sign(e) * Math.sqrt(Math.abs(e));
}
function jp() {
  var e = vl(), t = [0, 1], r = !1, n;
  function i(a) {
    var o = TS(e(a));
    return isNaN(o) ? n : r ? Math.round(o) : o;
  }
  return i.invert = function(a) {
    return e.invert(kf(a));
  }, i.domain = function(a) {
    return arguments.length ? (e.domain(a), i) : e.domain();
  }, i.range = function(a) {
    return arguments.length ? (e.range((t = Array.from(a, ua)).map(kf)), i) : t.slice();
  }, i.rangeRound = function(a) {
    return i.range(a).round(!0);
  }, i.round = function(a) {
    return arguments.length ? (r = !!a, i) : r;
  }, i.clamp = function(a) {
    return arguments.length ? (e.clamp(a), i) : e.clamp();
  }, i.unknown = function(a) {
    return arguments.length ? (n = a, i) : n;
  }, i.copy = function() {
    return jp(e.domain(), t).round(r).clamp(e.clamp()).unknown(n);
  }, ut.apply(i, arguments), lr(i);
}
function Np() {
  var e = [], t = [], r = [], n;
  function i() {
    var o = 0, u = Math.max(1, t.length);
    for (r = new Array(u - 1); ++o < u; ) r[o - 1] = MA(e, o / u);
    return a;
  }
  function a(o) {
    return o == null || isNaN(o = +o) ? n : t[ti(r, o)];
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
    return e.sort(ar), i();
  }, a.range = function(o) {
    return arguments.length ? (t = Array.from(o), i()) : t.slice();
  }, a.unknown = function(o) {
    return arguments.length ? (n = o, a) : n;
  }, a.quantiles = function() {
    return r.slice();
  }, a.copy = function() {
    return Np().domain(e).range(t).unknown(n);
  }, ut.apply(a, arguments);
}
function Mp() {
  var e = 0, t = 1, r = 1, n = [0.5], i = [0, 1], a;
  function o(l) {
    return l != null && l <= l ? i[ti(n, l, 0, r)] : a;
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
    return Mp().domain([e, t]).range(i).unknown(a);
  }, ut.apply(lr(o), arguments);
}
function $p() {
  var e = [0.5], t = [0, 1], r, n = 1;
  function i(a) {
    return a != null && a <= a ? t[ti(e, a, 0, n)] : r;
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
    return $p().domain(e).range(t).unknown(r);
  }, ut.apply(i, arguments);
}
const Io = /* @__PURE__ */ new Date(), ko = /* @__PURE__ */ new Date();
function ye(e, t, r, n) {
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
  }, i.filter = (a) => ye((o) => {
    if (o >= o) for (; e(o), !a(o); ) o.setTime(o - 1);
  }, (o, u) => {
    if (o >= o)
      if (u < 0) for (; ++u <= 0; )
        for (; t(o, -1), !a(o); )
          ;
      else for (; --u >= 0; )
        for (; t(o, 1), !a(o); )
          ;
  }), r && (i.count = (a, o) => (Io.setTime(+a), ko.setTime(+o), e(Io), e(ko), Math.floor(r(Io, ko))), i.every = (a) => (a = Math.floor(a), !isFinite(a) || !(a > 0) ? null : a > 1 ? i.filter(n ? (o) => n(o) % a === 0 : (o) => i.count(0, o) % a === 0) : i)), i;
}
const sa = ye(() => {
}, (e, t) => {
  e.setTime(+e + t);
}, (e, t) => t - e);
sa.every = (e) => (e = Math.floor(e), !isFinite(e) || !(e > 0) ? null : e > 1 ? ye((t) => {
  t.setTime(Math.floor(t / e) * e);
}, (t, r) => {
  t.setTime(+t + r * e);
}, (t, r) => (r - t) / e) : sa);
sa.range;
const Mt = 1e3, it = Mt * 60, $t = it * 60, Wt = $t * 24, xl = Wt * 7, Cf = Wt * 30, Co = Wt * 365, xr = ye((e) => {
  e.setTime(e - e.getMilliseconds());
}, (e, t) => {
  e.setTime(+e + t * Mt);
}, (e, t) => (t - e) / Mt, (e) => e.getUTCSeconds());
xr.range;
const Ol = ye((e) => {
  e.setTime(e - e.getMilliseconds() - e.getSeconds() * Mt);
}, (e, t) => {
  e.setTime(+e + t * it);
}, (e, t) => (t - e) / it, (e) => e.getMinutes());
Ol.range;
const Al = ye((e) => {
  e.setUTCSeconds(0, 0);
}, (e, t) => {
  e.setTime(+e + t * it);
}, (e, t) => (t - e) / it, (e) => e.getUTCMinutes());
Al.range;
const Sl = ye((e) => {
  e.setTime(e - e.getMilliseconds() - e.getSeconds() * Mt - e.getMinutes() * it);
}, (e, t) => {
  e.setTime(+e + t * $t);
}, (e, t) => (t - e) / $t, (e) => e.getHours());
Sl.range;
const El = ye((e) => {
  e.setUTCMinutes(0, 0, 0);
}, (e, t) => {
  e.setTime(+e + t * $t);
}, (e, t) => (t - e) / $t, (e) => e.getUTCHours());
El.range;
const ii = ye(
  (e) => e.setHours(0, 0, 0, 0),
  (e, t) => e.setDate(e.getDate() + t),
  (e, t) => (t - e - (t.getTimezoneOffset() - e.getTimezoneOffset()) * it) / Wt,
  (e) => e.getDate() - 1
);
ii.range;
const Za = ye((e) => {
  e.setUTCHours(0, 0, 0, 0);
}, (e, t) => {
  e.setUTCDate(e.getUTCDate() + t);
}, (e, t) => (t - e) / Wt, (e) => e.getUTCDate() - 1);
Za.range;
const Lp = ye((e) => {
  e.setUTCHours(0, 0, 0, 0);
}, (e, t) => {
  e.setUTCDate(e.getUTCDate() + t);
}, (e, t) => (t - e) / Wt, (e) => Math.floor(e / Wt));
Lp.range;
function Nr(e) {
  return ye((t) => {
    t.setDate(t.getDate() - (t.getDay() + 7 - e) % 7), t.setHours(0, 0, 0, 0);
  }, (t, r) => {
    t.setDate(t.getDate() + r * 7);
  }, (t, r) => (r - t - (r.getTimezoneOffset() - t.getTimezoneOffset()) * it) / xl);
}
const Qa = Nr(0), fa = Nr(1), DS = Nr(2), jS = Nr(3), Zr = Nr(4), NS = Nr(5), MS = Nr(6);
Qa.range;
fa.range;
DS.range;
jS.range;
Zr.range;
NS.range;
MS.range;
function Mr(e) {
  return ye((t) => {
    t.setUTCDate(t.getUTCDate() - (t.getUTCDay() + 7 - e) % 7), t.setUTCHours(0, 0, 0, 0);
  }, (t, r) => {
    t.setUTCDate(t.getUTCDate() + r * 7);
  }, (t, r) => (r - t) / xl);
}
const Ja = Mr(0), da = Mr(1), $S = Mr(2), LS = Mr(3), Qr = Mr(4), RS = Mr(5), zS = Mr(6);
Ja.range;
da.range;
$S.range;
LS.range;
Qr.range;
RS.range;
zS.range;
const Pl = ye((e) => {
  e.setDate(1), e.setHours(0, 0, 0, 0);
}, (e, t) => {
  e.setMonth(e.getMonth() + t);
}, (e, t) => t.getMonth() - e.getMonth() + (t.getFullYear() - e.getFullYear()) * 12, (e) => e.getMonth());
Pl.range;
const _l = ye((e) => {
  e.setUTCDate(1), e.setUTCHours(0, 0, 0, 0);
}, (e, t) => {
  e.setUTCMonth(e.getUTCMonth() + t);
}, (e, t) => t.getUTCMonth() - e.getUTCMonth() + (t.getUTCFullYear() - e.getUTCFullYear()) * 12, (e) => e.getUTCMonth());
_l.range;
const Ut = ye((e) => {
  e.setMonth(0, 1), e.setHours(0, 0, 0, 0);
}, (e, t) => {
  e.setFullYear(e.getFullYear() + t);
}, (e, t) => t.getFullYear() - e.getFullYear(), (e) => e.getFullYear());
Ut.every = (e) => !isFinite(e = Math.floor(e)) || !(e > 0) ? null : ye((t) => {
  t.setFullYear(Math.floor(t.getFullYear() / e) * e), t.setMonth(0, 1), t.setHours(0, 0, 0, 0);
}, (t, r) => {
  t.setFullYear(t.getFullYear() + r * e);
});
Ut.range;
const Vt = ye((e) => {
  e.setUTCMonth(0, 1), e.setUTCHours(0, 0, 0, 0);
}, (e, t) => {
  e.setUTCFullYear(e.getUTCFullYear() + t);
}, (e, t) => t.getUTCFullYear() - e.getUTCFullYear(), (e) => e.getUTCFullYear());
Vt.every = (e) => !isFinite(e = Math.floor(e)) || !(e > 0) ? null : ye((t) => {
  t.setUTCFullYear(Math.floor(t.getUTCFullYear() / e) * e), t.setUTCMonth(0, 1), t.setUTCHours(0, 0, 0, 0);
}, (t, r) => {
  t.setUTCFullYear(t.getUTCFullYear() + r * e);
});
Vt.range;
function Rp(e, t, r, n, i, a) {
  const o = [
    [xr, 1, Mt],
    [xr, 5, 5 * Mt],
    [xr, 15, 15 * Mt],
    [xr, 30, 30 * Mt],
    [a, 1, it],
    [a, 5, 5 * it],
    [a, 15, 15 * it],
    [a, 30, 30 * it],
    [i, 1, $t],
    [i, 3, 3 * $t],
    [i, 6, 6 * $t],
    [i, 12, 12 * $t],
    [n, 1, Wt],
    [n, 2, 2 * Wt],
    [r, 1, xl],
    [t, 1, Cf],
    [t, 3, 3 * Cf],
    [e, 1, Co]
  ];
  function u(c, s, f) {
    const d = s < c;
    d && ([c, s] = [s, c]);
    const h = f && typeof f.range == "function" ? f : l(c, s, f), v = h ? h.range(c, +s + 1) : [];
    return d ? v.reverse() : v;
  }
  function l(c, s, f) {
    const d = Math.abs(s - c) / f, h = ll(([, , m]) => m).right(o, d);
    if (h === o.length) return e.every(pu(c / Co, s / Co, f));
    if (h === 0) return sa.every(Math.max(pu(c, s, f), 1));
    const [v, p] = o[d / o[h - 1][2] < o[h][2] / d ? h - 1 : h];
    return v.every(p);
  }
  return [u, l];
}
const [BS, FS] = Rp(Vt, _l, Ja, Lp, El, Al), [WS, US] = Rp(Ut, Pl, Qa, ii, Sl, Ol);
function To(e) {
  if (0 <= e.y && e.y < 100) {
    var t = new Date(-1, e.m, e.d, e.H, e.M, e.S, e.L);
    return t.setFullYear(e.y), t;
  }
  return new Date(e.y, e.m, e.d, e.H, e.M, e.S, e.L);
}
function Do(e) {
  if (0 <= e.y && e.y < 100) {
    var t = new Date(Date.UTC(-1, e.m, e.d, e.H, e.M, e.S, e.L));
    return t.setUTCFullYear(e.y), t;
  }
  return new Date(Date.UTC(e.y, e.m, e.d, e.H, e.M, e.S, e.L));
}
function On(e, t, r) {
  return { y: e, m: t, d: r, H: 0, M: 0, S: 0, L: 0 };
}
function VS(e) {
  var t = e.dateTime, r = e.date, n = e.time, i = e.periods, a = e.days, o = e.shortDays, u = e.months, l = e.shortMonths, c = An(i), s = Sn(i), f = An(a), d = Sn(a), h = An(o), v = Sn(o), p = An(u), m = Sn(u), y = An(l), b = Sn(l), x = {
    a: H,
    A: B,
    b: X,
    B: W,
    c: null,
    d: $f,
    e: $f,
    f: hE,
    g: AE,
    G: EE,
    H: sE,
    I: fE,
    j: dE,
    L: zp,
    m: vE,
    M: pE,
    p: Te,
    q: Ee,
    Q: zf,
    s: Bf,
    S: mE,
    u: yE,
    U: gE,
    V: bE,
    w: wE,
    W: xE,
    x: null,
    X: null,
    y: OE,
    Y: SE,
    Z: PE,
    "%": Rf
  }, O = {
    a: se,
    A: Ke,
    b: He,
    B: ct,
    c: null,
    d: Lf,
    e: Lf,
    f: CE,
    g: BE,
    G: WE,
    H: _E,
    I: IE,
    j: kE,
    L: Fp,
    m: TE,
    M: DE,
    p: st,
    q: mn,
    Q: zf,
    s: Bf,
    S: jE,
    u: NE,
    U: ME,
    V: $E,
    w: LE,
    W: RE,
    x: null,
    X: null,
    y: zE,
    Y: FE,
    Z: UE,
    "%": Rf
  }, A = {
    a: C,
    A: T,
    b: P,
    B: z,
    c: $,
    d: Nf,
    e: Nf,
    f: oE,
    g: jf,
    G: Df,
    H: Mf,
    I: Mf,
    j: rE,
    L: aE,
    m: tE,
    M: nE,
    p: I,
    q: eE,
    Q: lE,
    s: cE,
    S: iE,
    u: qS,
    U: XS,
    V: ZS,
    w: YS,
    W: QS,
    x: Q,
    X: K,
    y: jf,
    Y: Df,
    Z: JS,
    "%": uE
  };
  x.x = g(r, x), x.X = g(n, x), x.c = g(t, x), O.x = g(r, O), O.X = g(n, O), O.c = g(t, O);
  function g(D, F) {
    return function(R) {
      var k = [], De = -1, J = 0, L = D.length, Ge, sr, lc;
      for (R instanceof Date || (R = /* @__PURE__ */ new Date(+R)); ++De < L; )
        D.charCodeAt(De) === 37 && (k.push(D.slice(J, De)), (sr = Tf[Ge = D.charAt(++De)]) != null ? Ge = D.charAt(++De) : sr = Ge === "e" ? " " : "0", (lc = F[Ge]) && (Ge = lc(R, sr)), k.push(Ge), J = De + 1);
      return k.push(D.slice(J, De)), k.join("");
    };
  }
  function E(D, F) {
    return function(R) {
      var k = On(1900, void 0, 1), De = _(k, D, R += "", 0), J, L;
      if (De != R.length) return null;
      if ("Q" in k) return new Date(k.Q);
      if ("s" in k) return new Date(k.s * 1e3 + ("L" in k ? k.L : 0));
      if (F && !("Z" in k) && (k.Z = 0), "p" in k && (k.H = k.H % 12 + k.p * 12), k.m === void 0 && (k.m = "q" in k ? k.q : 0), "V" in k) {
        if (k.V < 1 || k.V > 53) return null;
        "w" in k || (k.w = 1), "Z" in k ? (J = Do(On(k.y, 0, 1)), L = J.getUTCDay(), J = L > 4 || L === 0 ? da.ceil(J) : da(J), J = Za.offset(J, (k.V - 1) * 7), k.y = J.getUTCFullYear(), k.m = J.getUTCMonth(), k.d = J.getUTCDate() + (k.w + 6) % 7) : (J = To(On(k.y, 0, 1)), L = J.getDay(), J = L > 4 || L === 0 ? fa.ceil(J) : fa(J), J = ii.offset(J, (k.V - 1) * 7), k.y = J.getFullYear(), k.m = J.getMonth(), k.d = J.getDate() + (k.w + 6) % 7);
      } else ("W" in k || "U" in k) && ("w" in k || (k.w = "u" in k ? k.u % 7 : "W" in k ? 1 : 0), L = "Z" in k ? Do(On(k.y, 0, 1)).getUTCDay() : To(On(k.y, 0, 1)).getDay(), k.m = 0, k.d = "W" in k ? (k.w + 6) % 7 + k.W * 7 - (L + 5) % 7 : k.w + k.U * 7 - (L + 6) % 7);
      return "Z" in k ? (k.H += k.Z / 100 | 0, k.M += k.Z % 100, Do(k)) : To(k);
    };
  }
  function _(D, F, R, k) {
    for (var De = 0, J = F.length, L = R.length, Ge, sr; De < J; ) {
      if (k >= L) return -1;
      if (Ge = F.charCodeAt(De++), Ge === 37) {
        if (Ge = F.charAt(De++), sr = A[Ge in Tf ? F.charAt(De++) : Ge], !sr || (k = sr(D, R, k)) < 0) return -1;
      } else if (Ge != R.charCodeAt(k++))
        return -1;
    }
    return k;
  }
  function I(D, F, R) {
    var k = c.exec(F.slice(R));
    return k ? (D.p = s.get(k[0].toLowerCase()), R + k[0].length) : -1;
  }
  function C(D, F, R) {
    var k = h.exec(F.slice(R));
    return k ? (D.w = v.get(k[0].toLowerCase()), R + k[0].length) : -1;
  }
  function T(D, F, R) {
    var k = f.exec(F.slice(R));
    return k ? (D.w = d.get(k[0].toLowerCase()), R + k[0].length) : -1;
  }
  function P(D, F, R) {
    var k = y.exec(F.slice(R));
    return k ? (D.m = b.get(k[0].toLowerCase()), R + k[0].length) : -1;
  }
  function z(D, F, R) {
    var k = p.exec(F.slice(R));
    return k ? (D.m = m.get(k[0].toLowerCase()), R + k[0].length) : -1;
  }
  function $(D, F, R) {
    return _(D, t, F, R);
  }
  function Q(D, F, R) {
    return _(D, r, F, R);
  }
  function K(D, F, R) {
    return _(D, n, F, R);
  }
  function H(D) {
    return o[D.getDay()];
  }
  function B(D) {
    return a[D.getDay()];
  }
  function X(D) {
    return l[D.getMonth()];
  }
  function W(D) {
    return u[D.getMonth()];
  }
  function Te(D) {
    return i[+(D.getHours() >= 12)];
  }
  function Ee(D) {
    return 1 + ~~(D.getMonth() / 3);
  }
  function se(D) {
    return o[D.getUTCDay()];
  }
  function Ke(D) {
    return a[D.getUTCDay()];
  }
  function He(D) {
    return l[D.getUTCMonth()];
  }
  function ct(D) {
    return u[D.getUTCMonth()];
  }
  function st(D) {
    return i[+(D.getUTCHours() >= 12)];
  }
  function mn(D) {
    return 1 + ~~(D.getUTCMonth() / 3);
  }
  return {
    format: function(D) {
      var F = g(D += "", x);
      return F.toString = function() {
        return D;
      }, F;
    },
    parse: function(D) {
      var F = E(D += "", !1);
      return F.toString = function() {
        return D;
      }, F;
    },
    utcFormat: function(D) {
      var F = g(D += "", O);
      return F.toString = function() {
        return D;
      }, F;
    },
    utcParse: function(D) {
      var F = E(D += "", !0);
      return F.toString = function() {
        return D;
      }, F;
    }
  };
}
var Tf = { "-": "", _: " ", 0: "0" }, Se = /^\s*\d+/, KS = /^%/, HS = /[\\^$*+?|[\]().{}]/g;
function q(e, t, r) {
  var n = e < 0 ? "-" : "", i = (n ? -e : e) + "", a = i.length;
  return n + (a < r ? new Array(r - a + 1).join(t) + i : i);
}
function GS(e) {
  return e.replace(HS, "\\$&");
}
function An(e) {
  return new RegExp("^(?:" + e.map(GS).join("|") + ")", "i");
}
function Sn(e) {
  return new Map(e.map((t, r) => [t.toLowerCase(), r]));
}
function YS(e, t, r) {
  var n = Se.exec(t.slice(r, r + 1));
  return n ? (e.w = +n[0], r + n[0].length) : -1;
}
function qS(e, t, r) {
  var n = Se.exec(t.slice(r, r + 1));
  return n ? (e.u = +n[0], r + n[0].length) : -1;
}
function XS(e, t, r) {
  var n = Se.exec(t.slice(r, r + 2));
  return n ? (e.U = +n[0], r + n[0].length) : -1;
}
function ZS(e, t, r) {
  var n = Se.exec(t.slice(r, r + 2));
  return n ? (e.V = +n[0], r + n[0].length) : -1;
}
function QS(e, t, r) {
  var n = Se.exec(t.slice(r, r + 2));
  return n ? (e.W = +n[0], r + n[0].length) : -1;
}
function Df(e, t, r) {
  var n = Se.exec(t.slice(r, r + 4));
  return n ? (e.y = +n[0], r + n[0].length) : -1;
}
function jf(e, t, r) {
  var n = Se.exec(t.slice(r, r + 2));
  return n ? (e.y = +n[0] + (+n[0] > 68 ? 1900 : 2e3), r + n[0].length) : -1;
}
function JS(e, t, r) {
  var n = /^(Z)|([+-]\d\d)(?::?(\d\d))?/.exec(t.slice(r, r + 6));
  return n ? (e.Z = n[1] ? 0 : -(n[2] + (n[3] || "00")), r + n[0].length) : -1;
}
function eE(e, t, r) {
  var n = Se.exec(t.slice(r, r + 1));
  return n ? (e.q = n[0] * 3 - 3, r + n[0].length) : -1;
}
function tE(e, t, r) {
  var n = Se.exec(t.slice(r, r + 2));
  return n ? (e.m = n[0] - 1, r + n[0].length) : -1;
}
function Nf(e, t, r) {
  var n = Se.exec(t.slice(r, r + 2));
  return n ? (e.d = +n[0], r + n[0].length) : -1;
}
function rE(e, t, r) {
  var n = Se.exec(t.slice(r, r + 3));
  return n ? (e.m = 0, e.d = +n[0], r + n[0].length) : -1;
}
function Mf(e, t, r) {
  var n = Se.exec(t.slice(r, r + 2));
  return n ? (e.H = +n[0], r + n[0].length) : -1;
}
function nE(e, t, r) {
  var n = Se.exec(t.slice(r, r + 2));
  return n ? (e.M = +n[0], r + n[0].length) : -1;
}
function iE(e, t, r) {
  var n = Se.exec(t.slice(r, r + 2));
  return n ? (e.S = +n[0], r + n[0].length) : -1;
}
function aE(e, t, r) {
  var n = Se.exec(t.slice(r, r + 3));
  return n ? (e.L = +n[0], r + n[0].length) : -1;
}
function oE(e, t, r) {
  var n = Se.exec(t.slice(r, r + 6));
  return n ? (e.L = Math.floor(n[0] / 1e3), r + n[0].length) : -1;
}
function uE(e, t, r) {
  var n = KS.exec(t.slice(r, r + 1));
  return n ? r + n[0].length : -1;
}
function lE(e, t, r) {
  var n = Se.exec(t.slice(r));
  return n ? (e.Q = +n[0], r + n[0].length) : -1;
}
function cE(e, t, r) {
  var n = Se.exec(t.slice(r));
  return n ? (e.s = +n[0], r + n[0].length) : -1;
}
function $f(e, t) {
  return q(e.getDate(), t, 2);
}
function sE(e, t) {
  return q(e.getHours(), t, 2);
}
function fE(e, t) {
  return q(e.getHours() % 12 || 12, t, 2);
}
function dE(e, t) {
  return q(1 + ii.count(Ut(e), e), t, 3);
}
function zp(e, t) {
  return q(e.getMilliseconds(), t, 3);
}
function hE(e, t) {
  return zp(e, t) + "000";
}
function vE(e, t) {
  return q(e.getMonth() + 1, t, 2);
}
function pE(e, t) {
  return q(e.getMinutes(), t, 2);
}
function mE(e, t) {
  return q(e.getSeconds(), t, 2);
}
function yE(e) {
  var t = e.getDay();
  return t === 0 ? 7 : t;
}
function gE(e, t) {
  return q(Qa.count(Ut(e) - 1, e), t, 2);
}
function Bp(e) {
  var t = e.getDay();
  return t >= 4 || t === 0 ? Zr(e) : Zr.ceil(e);
}
function bE(e, t) {
  return e = Bp(e), q(Zr.count(Ut(e), e) + (Ut(e).getDay() === 4), t, 2);
}
function wE(e) {
  return e.getDay();
}
function xE(e, t) {
  return q(fa.count(Ut(e) - 1, e), t, 2);
}
function OE(e, t) {
  return q(e.getFullYear() % 100, t, 2);
}
function AE(e, t) {
  return e = Bp(e), q(e.getFullYear() % 100, t, 2);
}
function SE(e, t) {
  return q(e.getFullYear() % 1e4, t, 4);
}
function EE(e, t) {
  var r = e.getDay();
  return e = r >= 4 || r === 0 ? Zr(e) : Zr.ceil(e), q(e.getFullYear() % 1e4, t, 4);
}
function PE(e) {
  var t = e.getTimezoneOffset();
  return (t > 0 ? "-" : (t *= -1, "+")) + q(t / 60 | 0, "0", 2) + q(t % 60, "0", 2);
}
function Lf(e, t) {
  return q(e.getUTCDate(), t, 2);
}
function _E(e, t) {
  return q(e.getUTCHours(), t, 2);
}
function IE(e, t) {
  return q(e.getUTCHours() % 12 || 12, t, 2);
}
function kE(e, t) {
  return q(1 + Za.count(Vt(e), e), t, 3);
}
function Fp(e, t) {
  return q(e.getUTCMilliseconds(), t, 3);
}
function CE(e, t) {
  return Fp(e, t) + "000";
}
function TE(e, t) {
  return q(e.getUTCMonth() + 1, t, 2);
}
function DE(e, t) {
  return q(e.getUTCMinutes(), t, 2);
}
function jE(e, t) {
  return q(e.getUTCSeconds(), t, 2);
}
function NE(e) {
  var t = e.getUTCDay();
  return t === 0 ? 7 : t;
}
function ME(e, t) {
  return q(Ja.count(Vt(e) - 1, e), t, 2);
}
function Wp(e) {
  var t = e.getUTCDay();
  return t >= 4 || t === 0 ? Qr(e) : Qr.ceil(e);
}
function $E(e, t) {
  return e = Wp(e), q(Qr.count(Vt(e), e) + (Vt(e).getUTCDay() === 4), t, 2);
}
function LE(e) {
  return e.getUTCDay();
}
function RE(e, t) {
  return q(da.count(Vt(e) - 1, e), t, 2);
}
function zE(e, t) {
  return q(e.getUTCFullYear() % 100, t, 2);
}
function BE(e, t) {
  return e = Wp(e), q(e.getUTCFullYear() % 100, t, 2);
}
function FE(e, t) {
  return q(e.getUTCFullYear() % 1e4, t, 4);
}
function WE(e, t) {
  var r = e.getUTCDay();
  return e = r >= 4 || r === 0 ? Qr(e) : Qr.ceil(e), q(e.getUTCFullYear() % 1e4, t, 4);
}
function UE() {
  return "+0000";
}
function Rf() {
  return "%";
}
function zf(e) {
  return +e;
}
function Bf(e) {
  return Math.floor(+e / 1e3);
}
var zr, Up, Vp;
VE({
  dateTime: "%x, %X",
  date: "%-m/%-d/%Y",
  time: "%-I:%M:%S %p",
  periods: ["AM", "PM"],
  days: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
  shortDays: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
  months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
  shortMonths: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
});
function VE(e) {
  return zr = VS(e), Up = zr.format, zr.parse, Vp = zr.utcFormat, zr.utcParse, zr;
}
function KE(e) {
  return new Date(e);
}
function HE(e) {
  return e instanceof Date ? +e : +/* @__PURE__ */ new Date(+e);
}
function Il(e, t, r, n, i, a, o, u, l, c) {
  var s = vl(), f = s.invert, d = s.domain, h = c(".%L"), v = c(":%S"), p = c("%I:%M"), m = c("%I %p"), y = c("%a %d"), b = c("%b %d"), x = c("%B"), O = c("%Y");
  function A(g) {
    return (l(g) < g ? h : u(g) < g ? v : o(g) < g ? p : a(g) < g ? m : n(g) < g ? i(g) < g ? y : b : r(g) < g ? x : O)(g);
  }
  return s.invert = function(g) {
    return new Date(f(g));
  }, s.domain = function(g) {
    return arguments.length ? d(Array.from(g, HE)) : d().map(KE);
  }, s.ticks = function(g) {
    var E = d();
    return e(E[0], E[E.length - 1], g ?? 10);
  }, s.tickFormat = function(g, E) {
    return E == null ? A : c(E);
  }, s.nice = function(g) {
    var E = d();
    return (!g || typeof g.range != "function") && (g = t(E[0], E[E.length - 1], g ?? 10)), g ? d(Cp(E, g)) : s;
  }, s.copy = function() {
    return ni(s, Il(e, t, r, n, i, a, o, u, l, c));
  }, s;
}
function GE() {
  return ut.apply(Il(WS, US, Ut, Pl, Qa, ii, Sl, Ol, xr, Up).domain([new Date(2e3, 0, 1), new Date(2e3, 0, 2)]), arguments);
}
function YE() {
  return ut.apply(Il(BS, FS, Vt, _l, Ja, Za, El, Al, xr, Vp).domain([Date.UTC(2e3, 0, 1), Date.UTC(2e3, 0, 2)]), arguments);
}
function eo() {
  var e = 0, t = 1, r, n, i, a, o = ze, u = !1, l;
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
      var h, v;
      return arguments.length ? ([h, v] = d, o = f(h, v), c) : [o(0), o(1)];
    };
  }
  return c.range = s(cn), c.rangeRound = s(hl), c.unknown = function(f) {
    return arguments.length ? (l = f, c) : l;
  }, function(f) {
    return a = f, r = f(e), n = f(t), i = r === n ? 0 : 1 / (n - r), c;
  };
}
function cr(e, t) {
  return t.domain(e.domain()).interpolator(e.interpolator()).clamp(e.clamp()).unknown(e.unknown());
}
function Kp() {
  var e = lr(eo()(ze));
  return e.copy = function() {
    return cr(e, Kp());
  }, qt.apply(e, arguments);
}
function Hp() {
  var e = yl(eo()).domain([1, 10]);
  return e.copy = function() {
    return cr(e, Hp()).base(e.base());
  }, qt.apply(e, arguments);
}
function Gp() {
  var e = gl(eo());
  return e.copy = function() {
    return cr(e, Gp()).constant(e.constant());
  }, qt.apply(e, arguments);
}
function kl() {
  var e = bl(eo());
  return e.copy = function() {
    return cr(e, kl()).exponent(e.exponent());
  }, qt.apply(e, arguments);
}
function qE() {
  return kl.apply(null, arguments).exponent(0.5);
}
function Yp() {
  var e = [], t = ze;
  function r(n) {
    if (n != null && !isNaN(n = +n)) return t((ti(e, n, 1) - 1) / (e.length - 1));
  }
  return r.domain = function(n) {
    if (!arguments.length) return e.slice();
    e = [];
    for (let i of n) i != null && !isNaN(i = +i) && e.push(i);
    return e.sort(ar), r;
  }, r.interpolator = function(n) {
    return arguments.length ? (t = n, r) : t;
  }, r.range = function() {
    return e.map((n, i) => t(i / (e.length - 1)));
  }, r.quantiles = function(n) {
    return Array.from({ length: n + 1 }, (i, a) => NA(e, a / n));
  }, r.copy = function() {
    return Yp(t).domain(e);
  }, qt.apply(r, arguments);
}
function to() {
  var e = 0, t = 0.5, r = 1, n = 1, i, a, o, u, l, c = ze, s, f = !1, d;
  function h(p) {
    return isNaN(p = +p) ? d : (p = 0.5 + ((p = +s(p)) - a) * (n * p < n * a ? u : l), c(f ? Math.max(0, Math.min(1, p)) : p));
  }
  h.domain = function(p) {
    return arguments.length ? ([e, t, r] = p, i = s(e = +e), a = s(t = +t), o = s(r = +r), u = i === a ? 0 : 0.5 / (a - i), l = a === o ? 0 : 0.5 / (o - a), n = a < i ? -1 : 1, h) : [e, t, r];
  }, h.clamp = function(p) {
    return arguments.length ? (f = !!p, h) : f;
  }, h.interpolator = function(p) {
    return arguments.length ? (c = p, h) : c;
  };
  function v(p) {
    return function(m) {
      var y, b, x;
      return arguments.length ? ([y, b, x] = m, c = uS(p, [y, b, x]), h) : [c(0), c(0.5), c(1)];
    };
  }
  return h.range = v(cn), h.rangeRound = v(hl), h.unknown = function(p) {
    return arguments.length ? (d = p, h) : d;
  }, function(p) {
    return s = p, i = p(e), a = p(t), o = p(r), u = i === a ? 0 : 0.5 / (a - i), l = a === o ? 0 : 0.5 / (o - a), n = a < i ? -1 : 1, h;
  };
}
function qp() {
  var e = lr(to()(ze));
  return e.copy = function() {
    return cr(e, qp());
  }, qt.apply(e, arguments);
}
function Xp() {
  var e = yl(to()).domain([0.1, 1, 10]);
  return e.copy = function() {
    return cr(e, Xp()).base(e.base());
  }, qt.apply(e, arguments);
}
function Zp() {
  var e = gl(to());
  return e.copy = function() {
    return cr(e, Zp()).constant(e.constant());
  }, qt.apply(e, arguments);
}
function Cl() {
  var e = bl(to());
  return e.copy = function() {
    return cr(e, Cl()).exponent(e.exponent());
  }, qt.apply(e, arguments);
}
function XE() {
  return Cl.apply(null, arguments).exponent(0.5);
}
const Qp = /* @__PURE__ */ Object.freeze(/* @__PURE__ */ Object.defineProperty({
  __proto__: null,
  scaleBand: sl,
  scaleDiverging: qp,
  scaleDivergingLog: Xp,
  scaleDivergingPow: Cl,
  scaleDivergingSqrt: XE,
  scaleDivergingSymlog: Zp,
  scaleIdentity: kp,
  scaleImplicit: mu,
  scaleLinear: Ip,
  scaleLog: Tp,
  scaleOrdinal: cl,
  scalePoint: LA,
  scalePow: wl,
  scaleQuantile: Np,
  scaleQuantize: Mp,
  scaleRadial: jp,
  scaleSequential: Kp,
  scaleSequentialLog: Hp,
  scaleSequentialPow: kl,
  scaleSequentialQuantile: Yp,
  scaleSequentialSqrt: qE,
  scaleSequentialSymlog: Gp,
  scaleSqrt: CS,
  scaleSymlog: Dp,
  scaleThreshold: $p,
  scaleTime: GE,
  scaleUtc: YE,
  tickFormat: _p
}, Symbol.toStringTag, { value: "Module" }));
function ZE(e) {
  var t = Qp;
  if (e in t && typeof t[e] == "function")
    return t[e]();
  var r = "scale".concat(Ru(e));
  if (r in t && typeof t[r] == "function")
    return t[r]();
}
function Ff(e, t, r) {
  if (typeof e == "function")
    return e.copy().domain(t).range(r);
  if (e != null) {
    var n = ZE(e);
    if (n != null)
      return n.domain(t).range(r), n;
  }
}
function Tl(e, t, r, n) {
  if (!(r == null || n == null))
    return typeof e.scale == "function" ? Ff(e.scale, r, n) : Ff(t, r, n);
}
function QE(e) {
  return "scale".concat(Ru(e));
}
function JE(e) {
  return QE(e) in Qp;
}
var Jp = (e, t, r) => {
  if (e != null) {
    var n = e.scale, i = e.type;
    if (n === "auto")
      return i === "category" && r && (r.indexOf("LineChart") >= 0 || r.indexOf("AreaChart") >= 0 || r.indexOf("ComposedChart") >= 0 && !t) ? "point" : i === "category" ? "band" : "linear";
    if (typeof n == "string")
      return JE(n) ? n : "point";
  }
};
function eP(e, t) {
  for (var r = 0, n = e.length, i = e[0] < e[e.length - 1]; r < n; ) {
    var a = Math.floor((r + n) / 2);
    (i ? e[a] < t : e[a] > t) ? r = a + 1 : n = a;
  }
  return r;
}
function em(e, t) {
  if (e) {
    var r = t ?? e.domain(), n = r.map((a) => {
      var o;
      return (o = e(a)) !== null && o !== void 0 ? o : 0;
    }), i = e.range();
    if (!(r.length === 0 || i.length < 2))
      return (a) => {
        var o, u, l = eP(n, a);
        if (l <= 0)
          return r[0];
        if (l >= r.length)
          return r[r.length - 1];
        var c = (o = n[l - 1]) !== null && o !== void 0 ? o : 0, s = (u = n[l]) !== null && u !== void 0 ? u : 0;
        return Math.abs(a - c) <= Math.abs(a - s) ? r[l - 1] : r[l];
      };
  }
}
function tP(e) {
  if (e != null)
    return "invert" in e && typeof e.invert == "function" ? e.invert.bind(e) : em(e, void 0);
}
function Wf(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function ha(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Wf(Object(r), !0).forEach(function(n) {
      rP(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Wf(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function rP(e, t, r) {
  return (t = nP(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function nP(e) {
  var t = iP(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function iP(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function tm(e, t) {
  return lP(e) || uP(e, t) || oP(e, t) || aP();
}
function aP() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function oP(e, t) {
  if (e) {
    if (typeof e == "string") return Uf(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Uf(e, t) : void 0;
  }
}
function Uf(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function uP(e, t) {
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
function lP(e) {
  if (Array.isArray(e)) return e;
}
var wu = [0, "auto"], pe = {
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
}, rm = (e, t) => e.cartesianAxis.xAxis[t], Xt = (e, t) => {
  var r = rm(e, t);
  return r ?? pe;
}, me = {
  allowDataOverflow: !1,
  allowDecimals: !0,
  allowDuplicatedCategory: !0,
  angle: 0,
  dataKey: void 0,
  domain: wu,
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
  width: qn
}, nm = (e, t) => e.cartesianAxis.yAxis[t], Zt = (e, t) => {
  var r = nm(e, t);
  return r ?? me;
}, cP = {
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
}, Dl = (e, t) => {
  var r = e.cartesianAxis.zAxis[t];
  return r ?? cP;
}, Le = (e, t, r) => {
  switch (t) {
    case "xAxis":
      return Xt(e, r);
    case "yAxis":
      return Zt(e, r);
    case "zAxis":
      return Dl(e, r);
    case "angleAxis":
      return nl(e, r);
    case "radiusAxis":
      return il(e, r);
    default:
      throw new Error("Unexpected axis type: ".concat(t));
  }
}, sP = (e, t, r) => {
  switch (t) {
    case "xAxis":
      return Xt(e, r);
    case "yAxis":
      return Zt(e, r);
    default:
      throw new Error("Unexpected axis type: ".concat(t));
  }
}, ai = (e, t, r) => {
  switch (t) {
    case "xAxis":
      return Xt(e, r);
    case "yAxis":
      return Zt(e, r);
    case "angleAxis":
      return nl(e, r);
    case "radiusAxis":
      return il(e, r);
    default:
      throw new Error("Unexpected axis type: ".concat(t));
  }
}, im = (e) => e.graphicalItems.cartesianItems.some((t) => t.type === "bar") || e.graphicalItems.polarItems.some((t) => t.type === "radialBar");
function am(e, t) {
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
var om = (e) => e.graphicalItems.cartesianItems, fP = S([Oe, Ya], am), um = (e, t, r) => e.filter(r).filter((n) => (t == null ? void 0 : t.includeHidden) === !0 ? !0 : !n.hide), sn = S([om, Le, fP], um, {
  memoizeOptions: {
    resultEqualityCheck: qa
  }
}), lm = S([sn], (e) => e.filter((t) => t.type === "area" || t.type === "bar").filter(ol)), cm = (e) => e.filter((t) => !("stackId" in t) || t.stackId === void 0), dP = S([sn], cm), sm = (e) => e.map((t) => t.data).filter(Boolean).flat(1), hP = S([sn], (e) => e.some((t) => !t.data)), fm = S([sn], sm, {
  memoizeOptions: {
    resultEqualityCheck: qa
  }
}), dm = (e, t) => {
  var r = t.chartData, n = r === void 0 ? [] : r, i = t.dataStartIndex, a = t.dataEndIndex;
  return e.length > 0 ? e : n.slice(i, a + 1);
}, jl = S([fm, Jn], dm), vP = (e, t, r) => (t == null ? void 0 : t.dataKey) != null ? e.map((n) => ({
  value: we(n, t.dataKey)
})) : r.length > 0 ? r.map((n) => n.dataKey).flatMap((n) => e.map((i) => ({
  value: we(i, n)
}))) : e.map((n) => ({
  value: n
})), hm = (e, t, r, n, i, a) => {
  var o = n.chartData, u = o === void 0 ? [] : o, l = n.dataStartIndex, c = n.dataEndIndex, s = vP(e, t, r);
  if (i && (t == null ? void 0 : t.dataKey) != null && a.length > 0) {
    var f = u.slice(l, c + 1), d = f.map((h) => ({
      value: we(h, t.dataKey)
    })).filter((h) => h.value != null);
    return [...d, ...s];
  }
  return s;
}, oi = S([jl, Le, sn, Jn, hP, fm], hm);
function Yr(e) {
  if (Pt(e) || e instanceof Date) {
    var t = Number(e);
    if (Y(t))
      return t;
  }
}
function Vf(e) {
  if (Array.isArray(e)) {
    var t = [Yr(e[0]), Yr(e[1])];
    return At(t) ? t : void 0;
  }
  var r = Yr(e);
  if (r != null)
    return [r, r];
}
function vt(e) {
  return e.map(Yr).filter(Fe);
}
function pP(e, t) {
  var r = Yr(e), n = Yr(t);
  return r == null && n == null ? 0 : r == null ? -1 : n == null ? 1 : r - n;
}
var mP = S([oi], (e) => e == null ? void 0 : e.map((t) => t.value).sort(pP));
function vm(e, t) {
  switch (e) {
    case "xAxis":
      return t.direction === "x";
    case "yAxis":
      return t.direction === "y";
    default:
      return !1;
  }
}
function yP(e, t, r) {
  if (!r)
    return [];
  if (!r.length)
    return [];
  var n;
  if (typeof t == "number" && !Bt(t))
    n = t;
  else if (Array.isArray(t)) {
    var i = vt(t);
    i.length > 0 && (n = Math.max(...i));
  }
  return n == null ? [] : vt(r.flatMap((a) => {
    var o = we(e, a.dataKey), u, l;
    if (Array.isArray(o)) {
      var c = tm(o, 2);
      u = c[0], l = c[1];
    } else
      u = l = o;
    if (!(!Y(u) || !Y(l)))
      return [n - u, n + l];
  }));
}
var ge = (e) => {
  var t = Ae(e), r = ln(e);
  return ai(e, t, r);
}, Jr = S([ge], (e) => e == null ? void 0 : e.dataKey), gP = S([lm, Jn, ge], gp), pm = (e, t, r, n) => {
  var i = {}, a = t.reduce((o, u) => {
    if (u.stackId == null)
      return o;
    var l = o[u.stackId];
    return l == null && (l = []), l.push(u), o[u.stackId] = l, o;
  }, i);
  return Object.fromEntries(Object.entries(a).map((o) => {
    var u = tm(o, 2), l = u[0], c = u[1], s = n ? [...c].reverse() : c, f = s.map(yp);
    return [l, {
      // @ts-expect-error getStackedData requires that the input is array of objects, Recharts does not test for that
      stackedData: Xw(e, f, r),
      graphicalItems: s
    }];
  }));
}, bP = S([gP, lm, Va, sp], pm), mm = (e, t, r, n) => {
  var i = t.dataStartIndex, a = t.dataEndIndex;
  if (n == null && r !== "zAxis")
    return Jw(e, i, a);
}, wP = S([Le], (e) => e.allowDataOverflow), Nl = (e) => {
  var t;
  if (e == null || !("domain" in e))
    return wu;
  if (e.domain != null)
    return e.domain;
  if ("ticks" in e && e.ticks != null) {
    if (e.type === "number") {
      var r = vt(e.ticks);
      return [Math.min(...r), Math.max(...r)];
    }
    if (e.type === "category")
      return e.ticks.map(String);
  }
  return (t = e == null ? void 0 : e.domain) !== null && t !== void 0 ? t : wu;
}, ym = S([Le], Nl), gm = S([ym, wP], Jv), xP = S([bP, Ct, Oe, gm], mm, {
  memoizeOptions: {
    resultEqualityCheck: ei
  }
}), Ml = (e) => e.errorBars, OP = (e, t, r) => e.flatMap((n) => t[n.id]).filter(Boolean).filter((n) => vm(r, n)), va = function() {
  for (var t = arguments.length, r = new Array(t), n = 0; n < t; n++)
    r[n] = arguments[n];
  var i = r.filter(Boolean);
  if (i.length !== 0) {
    var a = i.flat(), o = Math.min(...a), u = Math.max(...a);
    return [o, u];
  }
}, bm = function(t, r, n, i, a) {
  var o = arguments.length > 5 && arguments[5] !== void 0 ? arguments[5] : [], u, l;
  if (n.length > 0 && n.forEach((c) => {
    var s, f = c.data != null ? [...c.data] : o, d = (s = i[c.id]) === null || s === void 0 ? void 0 : s.filter((h) => vm(a, h));
    f.forEach((h) => {
      var v, p = we(h, (v = r.dataKey) !== null && v !== void 0 ? v : c.dataKey), m = yP(h, p, d);
      if (m.length >= 2) {
        var y = Math.min(...m), b = Math.max(...m);
        (u == null || y < u) && (u = y), (l == null || b > l) && (l = b);
      }
      var x = Vf(p);
      x != null && (u = u == null ? x[0] : Math.min(u, x[0]), l = l == null ? x[1] : Math.max(l, x[1]));
    });
  }), (r == null ? void 0 : r.dataKey) != null && n.length === 0 && t.forEach((c) => {
    var s = Vf(we(c, r.dataKey));
    s != null && (u = u == null ? s[0] : Math.min(u, s[0]), l = l == null ? s[1] : Math.max(l, s[1]));
  }), Y(u) && Y(l))
    return [u, l];
}, AP = S([jl, Le, dP, Ml, Oe, J1], bm, {
  memoizeOptions: {
    resultEqualityCheck: ei
  }
});
function SP(e) {
  var t = e.value;
  if (Pt(t) || t instanceof Date)
    return t;
}
var EP = (e, t, r) => {
  var n = e.map(SP).filter((i) => i != null);
  return r && (t.dataKey == null || t.allowDuplicatedCategory && Ih(n)) ? Zv(0, e.length) : t.allowDuplicatedCategory ? n : Array.from(new Set(n));
}, wm = (e) => e.referenceElements.dots, fn = (e, t, r) => e.filter((n) => n.ifOverflow === "extendDomain").filter((n) => t === "xAxis" ? n.xAxisId === r : n.yAxisId === r), PP = S([wm, Oe, Ya], fn), xm = (e) => e.referenceElements.areas, _P = S([xm, Oe, Ya], fn), Om = (e) => e.referenceElements.lines, IP = S([Om, Oe, Ya], fn), Am = (e, t) => {
  if (e != null) {
    var r = vt(e.map((n) => t === "xAxis" ? n.x : n.y));
    if (r.length !== 0)
      return [Math.min(...r), Math.max(...r)];
  }
}, kP = S(PP, Oe, Am), Sm = (e, t) => {
  if (e != null) {
    var r = vt(e.flatMap((n) => [t === "xAxis" ? n.x1 : n.y1, t === "xAxis" ? n.x2 : n.y2]));
    if (r.length !== 0)
      return [Math.min(...r), Math.max(...r)];
  }
}, CP = S([_P, Oe], Sm);
function TP(e) {
  var t;
  if (e.x != null)
    return vt([e.x]);
  var r = (t = e.segment) === null || t === void 0 ? void 0 : t.map((n) => n.x);
  return r == null || r.length === 0 ? [] : vt(r);
}
function DP(e) {
  var t;
  if (e.y != null)
    return vt([e.y]);
  var r = (t = e.segment) === null || t === void 0 ? void 0 : t.map((n) => n.y);
  return r == null || r.length === 0 ? [] : vt(r);
}
var Em = (e, t) => {
  if (e != null) {
    var r = e.flatMap((n) => t === "xAxis" ? TP(n) : DP(n));
    if (r.length !== 0)
      return [Math.min(...r), Math.max(...r)];
  }
}, jP = S([IP, Oe], Em), NP = S(kP, jP, CP, (e, t, r) => va(e, r, t)), Pm = (e, t, r, n, i, a, o, u, l) => {
  if (r != null)
    return r;
  var c = o === "vertical" && u === "xAxis" || o === "horizontal" && u === "yAxis", s = c ? va(n, a, i) : va(a, i), f = aA(t, s, e.allowDataOverflow);
  return f ?? (e.allowDataOverflow && s == null && l != null ? l : f);
}, MP = (e) => {
  if (!(e == null || e.type !== "number" || !("ticks" in e) || e.ticks == null)) {
    var t = vt(e.ticks);
    if (t.length !== 0)
      return [Math.min(...t), Math.max(...t)];
  }
}, $P = S([Le], MP, {
  memoizeOptions: {
    resultEqualityCheck: ei
  }
}), LP = S([Le, ym, gm, xP, AP, NP, ue, Oe, $P], Pm, {
  memoizeOptions: {
    resultEqualityCheck: ei
  }
}), RP = [0, 1], _m = (e, t, r, n, i, a, o) => {
  if (!((e == null || r == null || r.length === 0) && o === void 0)) {
    var u = e.dataKey, l = e.type, c = Ht(t, a);
    if (c && u == null) {
      var s;
      return Zv(0, (s = r == null ? void 0 : r.length) !== null && s !== void 0 ? s : 0);
    }
    return l === "category" ? EP(n, e, c) : i === "expand" && !c ? RP : o;
  }
}, $l = S([Le, ue, jl, oi, Va, Oe, LP], _m), dn = S([Le, im, el], Jp), Im = (e, t, r) => {
  var n = t.niceTicks;
  if (n !== "none") {
    var i = Nl(t), a = Array.isArray(i) && (i[0] === "auto" || i[1] === "auto");
    if ((n === "snap125" || n === "adaptive") && t != null && t.tickCount && At(e)) {
      if (a)
        return Qs(e, t.tickCount, t.allowDecimals, n);
      if (t.type === "number")
        return Js(e, t.tickCount, t.allowDecimals, n);
    }
    if (n === "auto" && r === "linear" && t != null && t.tickCount) {
      if (a && At(e))
        return Qs(e, t.tickCount, t.allowDecimals, "adaptive");
      if (t.type === "number" && At(e))
        return Js(e, t.tickCount, t.allowDecimals, "adaptive");
    }
  }
}, Ll = S([$l, ai, dn], Im), km = (e, t, r, n) => {
  if (
    /*
     * Angle axis for some reason uses nice ticks when rendering axis tick labels,
     * but doesn't use nice ticks for extending domain like all the other axes do.
     * Not really sure why? Is there a good reason,
     * or is it just because someone added support for nice ticks to the other axes and forgot this one?
     */
    n !== "angleAxis" && (e == null ? void 0 : e.type) === "number" && At(t) && Array.isArray(r) && r.length > 0
  ) {
    var i, a, o = t[0], u = (i = r[0]) !== null && i !== void 0 ? i : 0, l = t[1], c = (a = r[r.length - 1]) !== null && a !== void 0 ? a : 0;
    return [Math.min(o, u), Math.max(l, c)];
  }
  return t;
}, zP = S([Le, $l, Ll, Oe], km), BP = S(oi, Le, (e, t) => {
  if (!(!t || t.type !== "number")) {
    var r = 1 / 0, n = Array.from(vt(e.map((f) => f.value))).sort((f, d) => f - d), i = n[0], a = n[n.length - 1];
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
}), Cm = S(BP, ue, vA, Ce, (e, t, r, n, i) => i, (e, t, r, n, i) => {
  if (!Y(e))
    return 0;
  var a = t === "vertical" ? n.height : n.width;
  if (i === "gap")
    return e * a / 2;
  if (i === "no-gap") {
    var o = ur(r, e * a), u = e * a / 2;
    return u - o - (u - o) / a * o;
  }
  return 0;
}), FP = (e, t, r) => {
  var n = Xt(e, t);
  return n == null || typeof n.padding != "string" ? 0 : Cm(e, "xAxis", t, r, n.padding);
}, WP = (e, t, r) => {
  var n = Zt(e, t);
  return n == null || typeof n.padding != "string" ? 0 : Cm(e, "yAxis", t, r, n.padding);
}, UP = S(Xt, FP, (e, t) => {
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
}), VP = S(Zt, WP, (e, t) => {
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
}), Tm = S([Ce, UP, Fa, Ba, (e, t, r) => r], (e, t, r, n, i) => {
  var a = n.padding;
  return i ? [a.left, r.width - a.right] : [e.left + t.left, e.left + e.width - t.right];
}), Dm = S([Ce, ue, VP, Fa, Ba, (e, t, r) => r], (e, t, r, n, i, a) => {
  var o = i.padding;
  return a ? [n.height - o.bottom, o.top] : t === "horizontal" ? [e.top + e.height - r.bottom, e.top + r.top] : [e.top + r.top, e.top + e.height - r.bottom];
}), ui = (e, t, r, n) => {
  var i;
  switch (t) {
    case "xAxis":
      return Tm(e, r, n);
    case "yAxis":
      return Dm(e, r, n);
    case "zAxis":
      return (i = Dl(e, r)) === null || i === void 0 ? void 0 : i.range;
    case "angleAxis":
      return vp(e);
    case "radiusAxis":
      return pp(e, r);
    default:
      return;
  }
}, jm = S([Le, ui], Ka), KP = S([dn, zP], OA), Rl = S([Le, dn, KP, jm], Tl), Nm = (e, t, r, n) => {
  if (!(r == null || r.dataKey == null)) {
    var i = r.type, a = r.scale, o = Ht(e, n);
    if (o && (i === "number" || a !== "auto"))
      return t.map((u) => u.value);
  }
}, zl = S([ue, oi, ai, Oe], Nm), ro = S([Rl], ul);
S([Rl], tP);
S([Rl, mP], em);
S([sn, Ml, Oe], OP);
function Mm(e, t) {
  return e.id < t.id ? -1 : e.id > t.id ? 1 : 0;
}
var no = (e, t) => t, io = (e, t, r) => r, HP = S(Ra, no, io, (e, t, r) => e.filter((n) => n.orientation === t).filter((n) => n.mirror === r).sort(Mm)), GP = S(za, no, io, (e, t, r) => e.filter((n) => n.orientation === t).filter((n) => n.mirror === r).sort(Mm)), $m = (e, t) => ({
  width: e.width,
  height: t.height
}), YP = (e, t) => {
  var r = typeof t.width == "number" ? t.width : qn;
  return {
    width: r,
    height: e.height
  };
}, qP = S(Ce, Xt, $m), XP = (e, t, r) => {
  switch (t) {
    case "top":
      return e.top;
    case "bottom":
      return r - e.bottom;
    default:
      return 0;
  }
}, ZP = (e, t, r) => {
  switch (t) {
    case "left":
      return e.left;
    case "right":
      return r - e.right;
    default:
      return 0;
  }
}, QP = S(Yt, Ce, HP, no, io, (e, t, r, n, i) => {
  var a = {}, o;
  return r.forEach((u) => {
    var l = $m(t, u);
    o == null && (o = XP(t, n, e));
    var c = n === "top" && !i || n === "bottom" && i;
    a[u.id] = o - Number(c) * l.height, o += (c ? -1 : 1) * l.height;
  }), a;
}), JP = S(Gt, Ce, GP, no, io, (e, t, r, n, i) => {
  var a = {}, o;
  return r.forEach((u) => {
    var l = YP(t, u);
    o == null && (o = ZP(t, n, e));
    var c = n === "left" && !i || n === "right" && i;
    a[u.id] = o - Number(c) * l.width, o += (c ? -1 : 1) * l.width;
  }), a;
}), e_ = (e, t) => {
  var r = Xt(e, t);
  if (r != null)
    return QP(e, r.orientation, r.mirror);
}, t_ = S([Ce, Xt, e_, (e, t) => t], (e, t, r, n) => {
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
}), r_ = (e, t) => {
  var r = Zt(e, t);
  if (r != null)
    return JP(e, r.orientation, r.mirror);
}, n_ = S([Ce, Zt, r_, (e, t) => t], (e, t, r, n) => {
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
}), i_ = S(Ce, Zt, (e, t) => {
  var r = typeof t.width == "number" ? t.width : qn;
  return {
    width: r,
    height: e.height
  };
}), Lm = (e, t, r, n) => {
  if (r != null) {
    var i = r.allowDuplicatedCategory, a = r.type, o = r.dataKey, u = Ht(e, n), l = t.map((s) => s.value), c = l.filter((s) => s != null);
    if (o && u && a === "category" && i && Ih(c))
      return l;
  }
}, Bl = S([ue, oi, Le, Oe], Lm), Kf = S([ue, sP, dn, ro, Bl, zl, ui, Ll, Oe], (e, t, r, n, i, a, o, u, l) => {
  if (t != null) {
    var c = Ht(e, l);
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
}), a_ = (e, t, r, n, i, a, o, u, l) => {
  if (!(t == null || n == null)) {
    var c = Ht(e, l), s = t.type, f = t.ticks, d = t.tickCount, h = (
      // @ts-expect-error This is testing for `scaleBand` but for band axis the type is reported as `band` so this looks like a dead code with a workaround elsewhere?
      r === "scaleBand" && typeof n.bandwidth == "function" ? n.bandwidth() / 2 : 2
    ), v = s === "category" && n.bandwidth ? n.bandwidth() / h : 0;
    v = l === "angleAxis" && a != null && a.length >= 2 ? rt(a[0] - a[1]) * 2 * v : v;
    var p = f || i;
    return p ? p.map((m, y) => {
      var b = o ? o.indexOf(m) : m, x = n.map(b);
      return Y(x) ? {
        index: y,
        coordinate: x + v,
        value: m,
        offset: v
      } : null;
    }).filter(Fe) : c && u ? u.map((m, y) => {
      var b = n.map(m);
      return Y(b) ? {
        coordinate: b + v,
        value: m,
        index: y,
        offset: v
      } : null;
    }).filter(Fe) : n.ticks ? n.ticks(d).map((m, y) => {
      var b = n.map(m);
      return Y(b) ? {
        coordinate: b + v,
        value: m,
        index: y,
        offset: v
      } : null;
    }).filter(Fe) : n.domain().map((m, y) => {
      var b = n.map(m);
      return Y(b) ? {
        coordinate: b + v,
        // @ts-expect-error can't use Date as index
        value: o ? o[m] : m,
        index: y,
        offset: v
      } : null;
    }).filter(Fe);
  }
}, Rm = S([ue, ai, dn, ro, Ll, ui, Bl, zl, Oe], a_), o_ = (e, t, r, n, i, a, o) => {
  if (!(t == null || r == null || n == null || n[0] === n[1])) {
    var u = Ht(e, o), l = t.tickCount, c = 0;
    return c = o === "angleAxis" && (n == null ? void 0 : n.length) >= 2 ? rt(n[0] - n[1]) * 2 * c : c, u && a ? a.map((s, f) => {
      var d = r.map(s);
      return Y(d) ? {
        coordinate: d + c,
        value: s,
        index: f,
        offset: c
      } : null;
    }).filter(Fe) : r.ticks ? r.ticks(l).map((s, f) => {
      var d = r.map(s);
      return Y(d) ? {
        coordinate: d + c,
        value: s,
        index: f,
        offset: c
      } : null;
    }).filter(Fe) : r.domain().map((s, f) => {
      var d = r.map(s);
      return Y(d) ? {
        coordinate: d + c,
        // @ts-expect-error can't use unknown as index
        value: i ? i[s] : s,
        index: f,
        offset: c
      } : null;
    }).filter(Fe);
  }
}, zm = S([ue, ai, ro, ui, Bl, zl, Oe], o_), Bm = S(Le, ro, (e, t) => {
  if (!(e == null || t == null))
    return ha(ha({}, e), {}, {
      scale: t
    });
}), u_ = S([Le, dn, $l, jm], Tl), l_ = S([u_], ul);
S((e, t, r) => Dl(e, r), l_, (e, t) => {
  if (!(e == null || t == null))
    return ha(ha({}, e), {}, {
      scale: t
    });
});
var c_ = S([ue, Ra, za], (e, t, r) => {
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
}), s_ = (e, t, r) => {
  var n;
  return (n = e.renderedTicks[t]) === null || n === void 0 ? void 0 : n[r];
};
S([s_], (e) => {
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
var Fm = (e) => e.options.defaultTooltipEventType, Wm = (e) => e.options.validateTooltipEventTypes;
function Um(e, t, r) {
  if (e == null)
    return t;
  var n = e ? "axis" : "item";
  return r == null ? t : r.includes(n) ? n : t;
}
function li(e, t) {
  var r = Fm(e), n = Wm(e);
  return Um(t, r, n);
}
function f_(e) {
  return N((t) => li(t, e));
}
var Vm = (e, t) => {
  var r, n = Number(t);
  if (!(Bt(n) || t == null))
    return n >= 0 ? e == null || (r = e[n]) === null || r === void 0 ? void 0 : r.value : void 0;
}, d_ = (e) => e.tooltip.settings, ir = {
  active: !1,
  index: null,
  dataKey: void 0,
  graphicalItemId: void 0,
  coordinate: void 0
}, h_ = {
  itemInteraction: {
    click: ir,
    hover: ir
  },
  axisInteraction: {
    click: ir,
    hover: ir
  },
  keyboardInteraction: ir,
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
}, Km = $e({
  name: "tooltip",
  initialState: h_,
  reducers: {
    addTooltipEntrySettings: {
      reducer(e, t) {
        e.tooltipItemPayloads.push(G(t.payload));
      },
      prepare: re()
    },
    replaceTooltipEntrySettings: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next, a = nt(e).tooltipItemPayloads.indexOf(G(n));
        a > -1 && (e.tooltipItemPayloads[a] = G(i));
      },
      prepare: re()
    },
    removeTooltipEntrySettings: {
      reducer(e, t) {
        var r = nt(e).tooltipItemPayloads.indexOf(G(t.payload));
        r > -1 && e.tooltipItemPayloads.splice(r, 1);
      },
      prepare: re()
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
}), lt = Km.actions, v_ = lt.addTooltipEntrySettings, p_ = lt.replaceTooltipEntrySettings, m_ = lt.removeTooltipEntrySettings, y_ = lt.setTooltipSettingsState, g_ = lt.setActiveMouseOverItemIndex;
lt.mouseLeaveItem;
var Hm = lt.mouseLeaveChart;
lt.setActiveClickItemIndex;
var Gm = lt.setMouseOverAxisIndex, b_ = lt.setMouseClickAxisIndex, In = lt.setSyncInteraction, pa = lt.setKeyboardInteraction, w_ = Km.reducer;
function Hf(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Ei(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Hf(Object(r), !0).forEach(function(n) {
      x_(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Hf(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function x_(e, t, r) {
  return (t = O_(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function O_(e) {
  var t = A_(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function A_(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function S_(e, t, r) {
  return t === "axis" ? r === "click" ? e.axisInteraction.click : e.axisInteraction.hover : r === "click" ? e.itemInteraction.click : e.itemInteraction.hover;
}
function E_(e) {
  return e.index != null;
}
var Ym = (e, t, r, n) => {
  if (t == null)
    return ir;
  var i = S_(e, t, r);
  if (i == null)
    return ir;
  if (i.active)
    return i;
  if (e.keyboardInteraction.active)
    return e.keyboardInteraction;
  if (e.syncInteraction.active && e.syncInteraction.index != null)
    return e.syncInteraction;
  var a = e.settings.active === !0;
  if (E_(i)) {
    if (a)
      return Ei(Ei({}, i), {}, {
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
  return Ei(Ei({}, ir), {}, {
    coordinate: i.coordinate
  });
};
function P_(e) {
  if (typeof e == "number")
    return Number.isFinite(e) ? e : void 0;
  if (e instanceof Date) {
    var t = e.valueOf();
    return Number.isFinite(t) ? t : void 0;
  }
  var r = Number(e);
  return Number.isFinite(r) ? r : void 0;
}
function __(e, t) {
  var r = P_(e), n = t[0], i = t[1];
  if (r === void 0)
    return !1;
  var a = Math.min(n, i), o = Math.max(n, i);
  return r >= a && r <= o;
}
function I_(e, t, r) {
  if (r == null || t == null)
    return !0;
  var n = we(e, t);
  return n == null || !At(r) ? !0 : __(n, r);
}
var Cn = (e, t, r, n) => {
  var i = e == null ? void 0 : e.index;
  if (i == null)
    return null;
  var a = Number(i);
  if (!Y(a))
    return i;
  var o = 0, u = 1 / 0;
  t.length > 0 && (u = t.length - 1);
  var l = Math.max(o, Math.min(a, u)), c = t[l];
  return c == null || I_(c, r, n) ? String(l) : null;
}, qm = (e, t, r, n, i, a, o) => {
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
}, Xm = (e, t, r, n) => {
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
}, Zm = (e) => e.options.tooltipPayloadSearcher, hn = (e) => e.tooltip;
function Gf(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Yf(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Gf(Object(r), !0).forEach(function(n) {
      k_(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Gf(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function k_(e, t, r) {
  return (t = C_(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function C_(e) {
  var t = T_(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function T_(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function D_(e) {
  if (typeof e == "string" || typeof e == "number")
    return e;
}
function j_(e) {
  if (typeof e == "string" || typeof e == "number" || typeof e == "boolean")
    return e;
}
function N_(e) {
  if (typeof e == "string" || typeof e == "number")
    return e;
  if (typeof e == "function")
    return (t) => e(t);
}
function qf(e) {
  if (typeof e == "string")
    return e;
}
function M_(e) {
  if (!(e == null || typeof e != "object")) {
    var t = "name" in e ? D_(e.name) : void 0, r = "unit" in e ? j_(e.unit) : void 0, n = "dataKey" in e ? N_(e.dataKey) : void 0, i = "payload" in e ? e.payload : void 0, a = "color" in e ? qf(e.color) : void 0, o = "fill" in e ? qf(e.fill) : void 0;
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
function $_(e, t) {
  return e ?? t;
}
var Qm = (e, t, r, n, i, a, o) => {
  if (!(t == null || a == null)) {
    var u = r.chartData, l = r.computedData, c = r.dataStartIndex, s = r.dataEndIndex, f = [];
    return e.reduce((d, h) => {
      var v, p = h.dataDefinedOnItem, m = h.settings, y = $_(p, u), b = Array.isArray(y) ? Pv(y, c, s) : y, x = (v = m == null ? void 0 : m.dataKey) !== null && v !== void 0 ? v : n, O = m == null ? void 0 : m.nameKey, A;
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
      o === "axis" ? A = kh(b, n, i) : A = a(b, t, l, O), Array.isArray(A))
        A.forEach((E) => {
          var _, I, C = M_(E), T = C == null ? void 0 : C.name, P = C == null ? void 0 : C.dataKey, z = C == null ? void 0 : C.payload, $ = Yf(Yf({}, m), {}, {
            name: T,
            unit: C == null ? void 0 : C.unit,
            // Preserve item-level color/fill from graphical items.
            color: (_ = C == null ? void 0 : C.color) !== null && _ !== void 0 ? _ : m == null ? void 0 : m.color,
            fill: (I = C == null ? void 0 : C.fill) !== null && I !== void 0 ? I : m == null ? void 0 : m.fill
          });
          d.push(Yc({
            tooltipEntrySettings: $,
            dataKey: P,
            payload: z,
            value: we(z, P),
            name: T == null ? void 0 : String(T)
          }));
        });
      else {
        var g;
        d.push(Yc({
          tooltipEntrySettings: m,
          dataKey: x,
          payload: A,
          // getValueByDataKey does not validate the output type
          value: we(A, x),
          // getValueByDataKey does not validate the output type
          name: (g = we(A, O)) !== null && g !== void 0 ? g : m == null ? void 0 : m.name
        }));
      }
      return d;
    }, f);
  }
}, Fl = S([ge, im, el], Jp), L_ = S([(e) => e.graphicalItems.cartesianItems, (e) => e.graphicalItems.polarItems], (e, t) => [...e, ...t]), R_ = S([Ae, ln], am), $r = S([L_, ge, R_], um, {
  memoizeOptions: {
    resultEqualityCheck: qa
  }
}), z_ = S([$r], (e) => e.filter(ol)), Jm = S([$r], sm, {
  memoizeOptions: {
    resultEqualityCheck: qa
  }
}), B_ = S([$r], (e) => e.some((t) => !t.data)), Tr = S([Jm, Ct], dm), F_ = S([z_, Ct, ge], gp), Wl = S([Tr, ge, $r, Ct, B_, Jm], hm), ey = S([ge], Nl), W_ = S([ge], (e) => e.allowDataOverflow), ty = S([ey, W_], Jv), U_ = S([$r], (e) => e.filter(ol)), V_ = S([F_, U_, Va, sp], pm), K_ = S([V_, Ct, Ae, ty], mm), H_ = S([$r], cm), G_ = S([Tr, ge, H_, Ml, Ae, eA], bm, {
  memoizeOptions: {
    resultEqualityCheck: ei
  }
}), Y_ = S([wm, Ae, ln], fn), q_ = S([Y_, Ae], Am), X_ = S([xm, Ae, ln], fn), Z_ = S([X_, Ae], Sm), Q_ = S([Om, Ae, ln], fn), J_ = S([Q_, Ae], Em), eI = S([q_, J_, Z_], va), tI = S([ge, ey, ty, K_, G_, eI, ue, Ae], Pm), en = S([ge, ue, Tr, Wl, Va, Ae, tI], _m), rI = S([en, ge, Fl], Im), nI = S([ge, en, rI, Ae], km), ry = (e) => {
  var t = Ae(e), r = ln(e), n = !1;
  return ui(e, t, r, n);
}, ny = S([ge, ry], Ka), iI = S([ge, Fl, nI, ny], Tl), iy = S([iI], ul), aI = S([ue, Wl, ge, Ae], Lm), oI = S([ue, Wl, ge, Ae], Nm), uI = (e, t, r, n, i, a, o, u) => {
  if (t) {
    var l = t.type, c = Ht(e, u);
    if (n) {
      var s = r === "scaleBand" && n.bandwidth ? n.bandwidth() / 2 : 2, f = l === "category" && n.bandwidth ? n.bandwidth() / s : 0;
      return f = u === "angleAxis" && i != null && (i == null ? void 0 : i.length) >= 2 ? rt(i[0] - i[1]) * 2 * f : f, c && o ? o.map((d, h) => {
        var v = n.map(d);
        return Y(v) ? {
          coordinate: v + f,
          value: d,
          index: h,
          offset: f
        } : null;
      }).filter(Fe) : n.domain().map((d, h) => {
        var v = n.map(d);
        return Y(v) ? {
          coordinate: v + f,
          // @ts-expect-error can't use Date as an index
          value: a ? a[d] : d,
          index: h,
          offset: f
        } : null;
      }).filter(Fe);
    }
  }
}, Qt = S([ue, ge, Fl, iy, ry, aI, oI, Ae], uI), Ul = S([Fm, Wm, d_], (e, t, r) => Um(r.shared, e, t)), ay = (e) => e.tooltip.settings.trigger, Vl = (e) => e.tooltip.settings.defaultIndex, ci = S([hn, Ul, ay, Vl], Ym), Wn = S([ci, Tr, Jr, en], Cn), oy = S([Qt, Wn], Vm), lI = S([ci], (e) => {
  if (e)
    return e.dataKey;
}), cI = S([ci], (e) => {
  if (e)
    return e.graphicalItemId;
}), uy = S([hn, Ul, ay, Vl], Xm), sI = S([Gt, Yt, ue, Ce, Qt, Vl, uy], qm), fI = S([ci, sI], (e, t) => e != null && e.coordinate ? e.coordinate : t), dI = S([ci], (e) => {
  var t;
  return (t = e == null ? void 0 : e.active) !== null && t !== void 0 ? t : !1;
}), hI = S([uy, Wn, Ct, Jr, oy, Zm, Ul], Qm), vI = S([hI], (e) => {
  if (e != null) {
    var t = e.map((r) => r.payload).filter((r) => r != null);
    return Array.from(new Set(t));
  }
});
function Xf(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Zf(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Xf(Object(r), !0).forEach(function(n) {
      pI(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Xf(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function pI(e, t, r) {
  return (t = mI(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function mI(e) {
  var t = yI(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function yI(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var gI = () => N(ge), bI = () => {
  var e = gI(), t = N(Qt), r = N(iy);
  return Yi(!e || !r ? void 0 : Zf(Zf({}, e), {}, {
    scale: r
  }), t);
};
function Qf(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Br(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Qf(Object(r), !0).forEach(function(n) {
      wI(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Qf(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function wI(e, t, r) {
  return (t = xI(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function xI(e) {
  var t = OI(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function OI(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var AI = (e, t, r, n) => {
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
}, SI = (e, t, r, n) => {
  var i = t.find((c) => c && c.index === r);
  if (i) {
    if (e === "centric") {
      var a = i.coordinate, o = n.radius;
      return Br(Br(Br({}, n), ke(n.cx, n.cy, o, a)), {}, {
        angle: a,
        radius: o
      });
    }
    var u = i.coordinate, l = n.angle;
    return Br(Br(Br({}, n), ke(n.cx, n.cy, u, l)), {}, {
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
function EI(e, t) {
  var r = e.relativeX, n = e.relativeY;
  return r >= t.left && r <= t.left + t.width && n >= t.top && n <= t.top + t.height;
}
var ly = (e, t, r, n, i) => {
  var a, o = (a = t == null ? void 0 : t.length) !== null && a !== void 0 ? a : 0;
  if (o <= 1 || e == null)
    return 0;
  if (n === "angleAxis" && i != null && Math.abs(Math.abs(i[1] - i[0]) - 360) <= 1e-6)
    for (var u = 0; u < o; u++) {
      var l, c, s, f, d, h = u > 0 ? (l = r[u - 1]) === null || l === void 0 ? void 0 : l.coordinate : (c = r[o - 1]) === null || c === void 0 ? void 0 : c.coordinate, v = (s = r[u]) === null || s === void 0 ? void 0 : s.coordinate, p = u >= o - 1 ? (f = r[0]) === null || f === void 0 ? void 0 : f.coordinate : (d = r[u + 1]) === null || d === void 0 ? void 0 : d.coordinate, m = void 0;
      if (!(h == null || v == null || p == null))
        if (rt(v - h) !== rt(p - v)) {
          var y = [];
          if (rt(p - v) === rt(i[1] - i[0])) {
            m = p;
            var b = v + i[1] - i[0];
            y[0] = Math.min(b, (b + h) / 2), y[1] = Math.max(b, (b + h) / 2);
          } else {
            m = h;
            var x = p + i[1] - i[0];
            y[0] = Math.min(v, (x + v) / 2), y[1] = Math.max(v, (x + v) / 2);
          }
          var O = [Math.min(v, (m + v) / 2), Math.max(v, (m + v) / 2)];
          if (e > O[0] && e <= O[1] || e >= y[0] && e <= y[1]) {
            var A;
            return (A = r[u]) === null || A === void 0 ? void 0 : A.index;
          }
        } else {
          var g = Math.min(h, p), E = Math.max(h, p);
          if (e > (g + v) / 2 && e <= (E + v) / 2) {
            var _;
            return (_ = r[u]) === null || _ === void 0 ? void 0 : _.index;
          }
        }
    }
  else if (t)
    for (var I = 0; I < o; I++) {
      var C = t[I];
      if (C != null) {
        var T = t[I + 1], P = t[I - 1];
        if (I === 0 && T != null && e <= (C.coordinate + T.coordinate) / 2 || I === o - 1 && P != null && e > (C.coordinate + P.coordinate) / 2 || I > 0 && I < o - 1 && P != null && T != null && e > (C.coordinate + P.coordinate) / 2 && e <= (C.coordinate + T.coordinate) / 2)
          return C.index;
      }
    }
  return -1;
}, PI = () => N(el), Kl = (e, t) => t, cy = (e, t, r) => r, Hl = (e, t, r, n) => n, _I = S(Qt, (e) => Ia(e, (t) => t.coordinate)), Gl = S([hn, Kl, cy, Hl], Ym), Yl = S([Gl, Tr, Jr, en], Cn), II = (e, t, r) => {
  if (t != null) {
    var n = hn(e);
    return t === "axis" ? r === "hover" ? n.axisInteraction.hover.dataKey : n.axisInteraction.click.dataKey : r === "hover" ? n.itemInteraction.hover.dataKey : n.itemInteraction.click.dataKey;
  }
}, sy = S([hn, Kl, cy, Hl], Xm), ma = S([Gt, Yt, ue, Ce, Qt, Hl, sy], qm), kI = S([Gl, ma], (e, t) => {
  var r;
  return (r = e.coordinate) !== null && r !== void 0 ? r : t;
}), fy = S([Qt, Yl], Vm), CI = S([sy, Yl, Ct, Jr, fy, Zm, Kl], Qm), TI = S([Gl, Yl], (e, t) => ({
  isActive: e.active && t != null,
  activeIndex: t
})), DI = (e, t, r, n, i, a, o) => {
  if (!(!e || !r || !n || !i) && EI(e, o)) {
    var u = ex(e, t), l = ly(u, a, i, r, n), c = AI(t, i, l, e);
    return {
      activeIndex: String(l),
      activeCoordinate: c
    };
  }
}, jI = (e, t, r, n, i, a, o) => {
  if (!(!e || !n || !i || !a || !r)) {
    var u = H1(e, r);
    if (u) {
      var l = tx(u, t), c = ly(l, o, a, n, i), s = SI(t, a, c, u);
      return {
        activeIndex: String(c),
        activeCoordinate: s
      };
    }
  }
}, NI = (e, t, r, n, i, a, o, u) => {
  if (!(!e || !t || !n || !i || !a))
    return t === "horizontal" || t === "vertical" ? DI(e, t, n, i, a, o, u) : jI(e, t, r, n, i, a, o);
}, MI = S((e) => e.zIndex.zIndexMap, (e, t) => t, (e, t, r) => r, (e, t, r) => {
  if (t != null) {
    var n = e[t];
    if (n != null)
      return r ? n.panoramaElement : n.element;
  }
}), $I = S((e) => e.zIndex.zIndexMap, (e) => {
  var t = Object.keys(e).map((n) => parseInt(n, 10)).concat(Object.values(Re)), r = Array.from(new Set(t));
  return r.sort((n, i) => n - i);
}, {
  memoizeOptions: {
    resultEqualityCheck: xA
  }
});
function Jf(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function ed(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Jf(Object(r), !0).forEach(function(n) {
      LI(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Jf(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function LI(e, t, r) {
  return (t = RI(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function RI(e) {
  var t = zI(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function zI(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var BI = {}, FI = {
  zIndexMap: Object.values(Re).reduce((e, t) => ed(ed({}, e), {}, {
    [t]: {
      element: void 0,
      panoramaElement: void 0,
      consumers: 0
    }
  }), BI)
}, WI = new Set(Object.values(Re));
function UI(e) {
  return WI.has(e);
}
var dy = $e({
  name: "zIndex",
  initialState: FI,
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
      prepare: re()
    },
    unregisterZIndexPortal: {
      reducer: (e, t) => {
        var r = t.payload.zIndex;
        e.zIndexMap[r] && (e.zIndexMap[r].consumers -= 1, e.zIndexMap[r].consumers <= 0 && !UI(r) && delete e.zIndexMap[r]);
      },
      prepare: re()
    },
    registerZIndexPortalElement: {
      reducer: (e, t) => {
        var r = t.payload, n = r.zIndex, i = r.element, a = r.isPanorama;
        e.zIndexMap[n] ? a ? e.zIndexMap[n].panoramaElement = G(i) : e.zIndexMap[n].element = G(i) : e.zIndexMap[n] = {
          consumers: 0,
          element: a ? void 0 : G(i),
          panoramaElement: a ? G(i) : void 0
        };
      },
      prepare: re()
    },
    unregisterZIndexPortalElement: {
      reducer: (e, t) => {
        var r = t.payload.zIndex;
        e.zIndexMap[r] && (t.payload.isPanorama ? e.zIndexMap[r].panoramaElement = void 0 : e.zIndexMap[r].element = void 0);
      },
      prepare: re()
    }
  }
}), ao = dy.actions, VI = ao.registerZIndexPortal, jo = ao.unregisterZIndexPortal, KI = ao.registerZIndexPortalElement, HI = ao.unregisterZIndexPortalElement, GI = dy.reducer;
function Jt(e) {
  var t = e.zIndex, r = e.children, n = zx(), i = n && t !== void 0 && t !== 0, a = Ve(), o = V(void 0), u = V(/* @__PURE__ */ new Set()), l = ve(), c = N((f) => MI(f, t, a));
  if (Ue(() => {
    if (!i) {
      var f = u.current;
      f.forEach((h) => {
        l(jo({
          zIndex: h
        }));
      }), f.clear(), o.current = void 0;
      return;
    }
    if (u.current.has(t) || (l(VI({
      zIndex: t
    })), u.current.add(t)), c) {
      o.current = c;
      var d = u.current;
      d.forEach((h) => {
        h !== t && (l(jo({
          zIndex: h
        })), d.delete(h));
      });
    }
  }, [l, t, i, c]), Ue(() => {
    var f = u.current;
    return () => {
      f.forEach((d) => {
        l(jo({
          zIndex: d
        }));
      }), f.clear();
    };
  }, [l]), !i)
    return r;
  var s = c ?? o.current;
  return s ? /* @__PURE__ */ uh(r, s) : null;
}
function xu() {
  return xu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, xu.apply(null, arguments);
}
function td(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Pi(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? td(Object(r), !0).forEach(function(n) {
      YI(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : td(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function YI(e, t, r) {
  return (t = qI(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function qI(e) {
  var t = XI(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function XI(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function ZI(e) {
  var t = e.cursor, r = e.cursorComp, n = e.cursorProps;
  return /* @__PURE__ */ Ze(t) ? /* @__PURE__ */ rn(t, n) : /* @__PURE__ */ oh(r, n);
}
function QI(e) {
  var t, r = e.coordinate, n = e.payload, i = e.index, a = e.offset, o = e.tooltipAxisBandSize, u = e.layout, l = e.cursor, c = e.tooltipEventType, s = e.chartName, f = r, d = n, h = i;
  if (!l || !f || s !== "ScatterChart" && c !== "axis")
    return null;
  var v, p, m;
  if (s === "ScatterChart")
    v = f, p = e1, m = Re.cursorLine;
  else if (s === "BarChart")
    v = t1(u, f, a, o), p = $1, m = Re.cursorRectangle;
  else if (u === "radial" && Ch(f)) {
    var y = qv(f), b = y.cx, x = y.cy, O = y.radius, A = y.startAngle, g = y.endAngle;
    v = {
      cx: b,
      cy: x,
      startAngle: A,
      endAngle: g,
      innerRadius: O,
      outerRadius: O
    }, p = X1, m = Re.cursorLine;
  } else
    v = {
      points: Z1(u, f, a)
    }, p = Uv, m = Re.cursorLine;
  var E = typeof l == "object" && "className" in l ? l.className : void 0, _ = Pi(Pi(Pi(Pi({
    stroke: "#ccc",
    pointerEvents: "none"
  }, a), v), Ea(l)), {}, {
    payload: d,
    payloadIndex: h,
    className: ie("recharts-tooltip-cursor", E)
  });
  return /* @__PURE__ */ w.createElement(Jt, {
    zIndex: (t = e.zIndex) !== null && t !== void 0 ? t : m
  }, /* @__PURE__ */ w.createElement(ZI, {
    cursor: l,
    cursorComp: p,
    cursorProps: _
  }));
}
function JI(e) {
  var t = bI(), r = Mv(), n = an(), i = PI();
  return t == null || r == null || n == null || i == null ? null : /* @__PURE__ */ w.createElement(QI, xu({}, e, {
    offset: r,
    layout: n,
    tooltipAxisBandSize: t,
    chartName: i
  }));
}
var hy = /* @__PURE__ */ Je(null), ek = () => kt(hy), vy = { exports: {} };
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
    var h = new i(s, f || l, d), v = r ? r + c : c;
    return l._events[v] ? l._events[v].fn ? l._events[v] = [l._events[v], h] : l._events[v].push(h) : (l._events[v] = h, l._eventsCount++), l;
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
    for (var d = 0, h = f.length, v = new Array(h); d < h; d++)
      v[d] = f[d].fn;
    return v;
  }, u.prototype.listenerCount = function(c) {
    var s = r ? r + c : c, f = this._events[s];
    return f ? f.fn ? 1 : f.length : 0;
  }, u.prototype.emit = function(c, s, f, d, h, v) {
    var p = r ? r + c : c;
    if (!this._events[p]) return !1;
    var m = this._events[p], y = arguments.length, b, x;
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
          return m.fn.call(m.context, s, f, d, h, v), !0;
      }
      for (x = 1, b = new Array(y - 1); x < y; x++)
        b[x - 1] = arguments[x];
      m.fn.apply(m.context, b);
    } else {
      var O = m.length, A;
      for (x = 0; x < O; x++)
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
            if (!b) for (A = 1, b = new Array(y - 1); A < y; A++)
              b[A - 1] = arguments[A];
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
    var v = this._events[h];
    if (v.fn)
      v.fn === s && (!d || v.once) && (!f || v.context === f) && o(this, h);
    else {
      for (var p = 0, m = [], y = v.length; p < y; p++)
        (v[p].fn !== s || d && !v[p].once || f && v[p].context !== f) && m.push(v[p]);
      m.length ? this._events[h] = m.length === 1 ? m[0] : m : o(this, h);
    }
    return this;
  }, u.prototype.removeAllListeners = function(c) {
    var s;
    return c ? (s = r ? r + c : c, this._events[s] && o(this, s)) : (this._events = new n(), this._eventsCount = 0), this;
  }, u.prototype.off = u.prototype.removeListener, u.prototype.addListener = u.prototype.on, u.prefixed = r, u.EventEmitter = u, e.exports = u;
})(vy);
var tk = vy.exports;
const rk = /* @__PURE__ */ bg(tk);
var Un = new rk(), Ou = "recharts.syncEvent.tooltip", rd = "recharts.syncEvent.brush", nk = (e, t) => {
  if (t && Array.isArray(e)) {
    var r = Number.parseInt(t, 10);
    if (!Bt(r))
      return e[r];
  }
}, ik = {
  chartName: "",
  tooltipPayloadSearcher: () => {
  },
  eventEmitter: void 0,
  defaultTooltipEventType: "axis"
}, py = $e({
  name: "options",
  initialState: ik,
  reducers: {
    createEventEmitter: (e) => {
      e.eventEmitter == null && (e.eventEmitter = Symbol("rechartsEventEmitter"));
    }
  }
}), ak = py.reducer, ok = py.actions.createEventEmitter;
function uk(e) {
  return e.tooltip.syncInteraction;
}
var lk = {
  chartData: void 0,
  computedData: void 0,
  dataStartIndex: 0,
  dataEndIndex: 0
}, my = $e({
  name: "chartData",
  initialState: lk,
  reducers: {
    setChartData(e, t) {
      if (e.chartData = G(t.payload), t.payload == null) {
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
}), ql = my.actions, nd = ql.setChartData, ck = ql.setDataStartEndIndexes;
ql.setComputedData;
var sk = my.reducer, fk = ["x", "y"];
function id(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Fr(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? id(Object(r), !0).forEach(function(n) {
      dk(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : id(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function dk(e, t, r) {
  return (t = hk(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function hk(e) {
  var t = vk(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function vk(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function pk(e, t) {
  if (e == null) return {};
  var r, n, i = mk(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function mk(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function yk() {
  var e = N(tl), t = N(rl), r = ve(), n = N(fp), i = N(Qt), a = an(), o = Wa(), u = N((l) => l.rootProps.className);
  he(() => {
    if (e == null)
      return nn;
    var l = (c, s, f) => {
      if (t !== f && e === c) {
        if (s.payload.active === !1) {
          r(In({
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
            var h = s.payload.coordinate, v = h.x, p = h.y, m = pk(h, fk), y = s.payload.sourceViewBox, b = y.x, x = y.y, O = y.width, A = y.height, g = Fr(Fr({}, m), {}, {
              x: o.x + (O ? (v - b) / O : 0) * o.width,
              y: o.y + (A ? (p - x) / A : 0) * o.height
            });
            r(Fr(Fr({}, s), {}, {
              payload: Fr(Fr({}, s.payload), {}, {
                coordinate: g
              })
            }));
          } else
            r(s);
          return;
        }
        if (i != null) {
          var E;
          if (typeof n == "function") {
            var _ = {
              activeTooltipIndex: s.payload.index == null ? void 0 : Number(s.payload.index),
              isTooltipActive: s.payload.active,
              activeIndex: s.payload.index == null ? void 0 : Number(s.payload.index),
              activeLabel: s.payload.label,
              activeDataKey: s.payload.dataKey,
              activeCoordinate: s.payload.coordinate
            }, I = n(i, _);
            E = i[I];
          } else n === "value" && (E = i.find((H) => String(H.value) === s.payload.label));
          var C = s.payload.coordinate;
          if (C == null || o == null) {
            r(In({
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
          if (E == null) {
            r(In({
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
          var T = C.x, P = C.y, z = Math.min(T, o.x + o.width), $ = Math.min(P, o.y + o.height), Q = {
            x: a === "horizontal" ? E.coordinate : z,
            y: a === "horizontal" ? $ : E.coordinate
          }, K = In({
            active: s.payload.active,
            coordinate: Q,
            dataKey: s.payload.dataKey,
            index: String(E.index),
            label: s.payload.label,
            sourceViewBox: s.payload.sourceViewBox,
            graphicalItemId: s.payload.graphicalItemId
          });
          r(K);
        }
      }
    };
    return Un.on(Ou, l), () => {
      Un.off(Ou, l);
    };
  }, [u, r, t, e, n, i, a, o]);
}
function gk() {
  var e = N(tl), t = N(rl), r = ve();
  he(() => {
    if (e == null)
      return nn;
    var n = (i, a, o) => {
      t !== o && e === i && r(ck(a));
    };
    return Un.on(rd, n), () => {
      Un.off(rd, n);
    };
  }, [r, t, e]);
}
function bk() {
  var e = ve();
  he(() => {
    e(ok());
  }, [e]), yk(), gk();
}
function wk(e, t, r, n, i, a) {
  var o = N((v) => II(v, e, t)), u = N(cI), l = N(rl), c = N(tl), s = N(fp), f = N(uk), d = (f == null ? void 0 : f.sourceViewBox) != null, h = Wa();
  he(() => {
    if (!d && c != null && l != null) {
      var v = In({
        active: a,
        coordinate: r,
        dataKey: o,
        index: i,
        label: typeof n == "number" ? String(n) : n,
        sourceViewBox: h,
        graphicalItemId: u
      });
      Un.emit(Ou, c, v, l);
    }
  }, [d, r, o, u, i, n, l, c, s, a, h]);
}
function ad(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function od(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? ad(Object(r), !0).forEach(function(n) {
      xk(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : ad(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function xk(e, t, r) {
  return (t = Ok(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function Ok(e) {
  var t = Ak(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Ak(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function Sk(e, t) {
  return Ik(e) || _k(e, t) || Pk(e, t) || Ek();
}
function Ek() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function Pk(e, t) {
  if (e) {
    if (typeof e == "string") return ud(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? ud(e, t) : void 0;
  }
}
function ud(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function _k(e, t) {
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
function Ik(e) {
  if (Array.isArray(e)) return e;
}
function kk(e) {
  return e.dataKey;
}
function Ck(e, t) {
  return /* @__PURE__ */ w.isValidElement(e) ? /* @__PURE__ */ w.cloneElement(e, t) : typeof e == "function" ? /* @__PURE__ */ w.createElement(e, t) : /* @__PURE__ */ w.createElement(bO, t);
}
var ld = [], Tk = {
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
function Dk(e) {
  var t, r, n = et(e, Tk), i = n.active, a = n.allowEscapeViewBox, o = n.animationDuration, u = n.animationEasing, l = n.content, c = n.filterNull, s = n.isAnimationActive, f = n.offset, d = n.payloadUniqBy, h = n.position, v = n.reverseDirection, p = n.useTranslate3d, m = n.wrapperStyle, y = n.cursor, b = n.shared, x = n.trigger, O = n.defaultIndex, A = n.portal, g = n.axisId, E = ve(), _ = typeof O == "number" ? String(O) : O;
  he(() => {
    E(y_({
      shared: b,
      trigger: x,
      axisId: g,
      active: i,
      defaultIndex: _
    }));
  }, [E, b, x, g, i, _]);
  var I = Wa(), C = Wv(), T = f_(b), P = (t = N((R) => TI(R, T, x, _))) !== null && t !== void 0 ? t : {}, z = P.activeIndex, $ = P.isActive, Q = N((R) => CI(R, T, x, _)), K = N((R) => fy(R, T, x, _)), H = N((R) => kI(R, T, x, _)), B = Q, X = ek(), W = (r = i ?? $) !== null && r !== void 0 ? r : !1, Te = Ib([B, W]), Ee = Sk(Te, 2), se = Ee[0], Ke = Ee[1], He = T === "axis" ? K : void 0;
  wk(T, x, H, He, z, W);
  var ct = A ?? X;
  if (ct == null || I == null || T == null)
    return null;
  var st = B ?? ld;
  W || (st = ld), c && st.length && (st = Y0(st.filter((R) => R.value != null && (R.hide !== !0 || n.includeHidden)), d, kk));
  var mn = st.length > 0, D = od(od({}, n), {}, {
    payload: st,
    label: He,
    active: W,
    activeIndex: z,
    coordinate: H,
    accessibilityLayer: C
  }), F = /* @__PURE__ */ w.createElement(BO, {
    allowEscapeViewBox: a,
    animationDuration: o,
    animationEasing: u,
    isAnimationActive: s,
    active: W,
    coordinate: H,
    hasPayload: mn,
    offset: f,
    position: h,
    reverseDirection: v,
    useTranslate3d: p,
    viewBox: I,
    wrapperStyle: m,
    lastBoundingBox: se,
    innerRef: Ke,
    hasPortalFromProps: !!A
  }, Ck(l, D));
  return /* @__PURE__ */ w.createElement(w.Fragment, null, /* @__PURE__ */ uh(F, ct), W && /* @__PURE__ */ w.createElement(JI, {
    cursor: y,
    tooltipEventType: T,
    coordinate: H,
    payload: st,
    index: z
  }));
}
function jk(e, t, r) {
  return (t = Nk(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function Nk(e) {
  var t = Mk(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Mk(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
class $k {
  constructor(t) {
    jk(this, "cache", /* @__PURE__ */ new Map()), this.maxSize = t;
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
function Lk(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? cd(Object(r), !0).forEach(function(n) {
      Rk(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : cd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function Rk(e, t, r) {
  return (t = zk(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function zk(e) {
  var t = Bk(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Bk(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var Fk = {
  cacheSize: 2e3,
  enableCache: !0
}, yy = Lk({}, Fk), sd = new $k(yy.cacheSize), Wk = {
  position: "absolute",
  top: "-20000px",
  left: 0,
  padding: 0,
  margin: 0,
  border: "none",
  whiteSpace: "pre"
}, fd = "recharts_measurement_span";
function Uk(e, t) {
  var r = t.fontSize || "", n = t.fontFamily || "", i = t.fontWeight || "", a = t.fontStyle || "", o = t.letterSpacing || "", u = t.textTransform || "";
  return "".concat(e, "|").concat(r, "|").concat(n, "|").concat(i, "|").concat(a, "|").concat(o, "|").concat(u);
}
var dd = (e, t) => {
  try {
    var r = document.getElementById(fd);
    r || (r = document.createElement("span"), r.setAttribute("id", fd), r.setAttribute("aria-hidden", "true"), document.body.appendChild(r)), Object.assign(r.style, Wk, t), r.textContent = "".concat(e);
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
}, Tn = function(t) {
  var r = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : {};
  if (t == null || Qn.isSsr)
    return {
      width: 0,
      height: 0
    };
  if (!yy.enableCache)
    return dd(t, r);
  var n = Uk(t, r), i = sd.get(n);
  if (i)
    return i;
  var a = dd(t, r);
  return sd.set(n, a), a;
}, gy;
function ya(e, t) {
  return Gk(e) || Hk(e, t) || Kk(e, t) || Vk();
}
function Vk() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function Kk(e, t) {
  if (e) {
    if (typeof e == "string") return hd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? hd(e, t) : void 0;
  }
}
function hd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function Hk(e, t) {
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
function Gk(e) {
  if (Array.isArray(e)) return e;
}
function Yk(e, t, r) {
  return (t = qk(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function qk(e) {
  var t = Xk(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function Xk(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var vd = /(-?\d+(?:\.\d+)?[a-zA-Z%]*)([*/])(-?\d+(?:\.\d+)?[a-zA-Z%]*)/, pd = /(-?\d+(?:\.\d+)?[a-zA-Z%]*)([+-])(-?\d+(?:\.\d+)?[a-zA-Z%]*)/, Zk = /^(px|cm|vh|vw|em|rem|%|mm|in|pt|pc|ex|ch|vmin|vmax|Q)$/, Qk = /(-?\d+(?:\.\d+)?)([a-zA-Z%]+)?/, Jk = {
  cm: 96 / 2.54,
  mm: 96 / 25.4,
  pt: 96 / 72,
  pc: 96 / 6,
  in: 96,
  Q: 96 / (2.54 * 40),
  px: 1
}, eC = ["cm", "mm", "pt", "pc", "in", "Q", "px"];
function tC(e) {
  return eC.includes(e);
}
var Kr = "NaN";
function rC(e, t) {
  return e * Jk[t];
}
class Ie {
  static parse(t) {
    var r, n = (r = Qk.exec(t)) !== null && r !== void 0 ? r : [], i = ya(n, 3), a = i[1], o = i[2];
    return a == null ? Ie.NaN : new Ie(parseFloat(a), o ?? "");
  }
  constructor(t, r) {
    this.num = t, this.unit = r, this.num = t, this.unit = r, Bt(t) && (this.unit = ""), r !== "" && !Zk.test(r) && (this.num = NaN, this.unit = ""), tC(r) && (this.num = rC(t, r), this.unit = "px");
  }
  add(t) {
    return this.unit !== t.unit ? new Ie(NaN, "") : new Ie(this.num + t.num, this.unit);
  }
  subtract(t) {
    return this.unit !== t.unit ? new Ie(NaN, "") : new Ie(this.num - t.num, this.unit);
  }
  multiply(t) {
    return this.unit !== "" && t.unit !== "" && this.unit !== t.unit ? new Ie(NaN, "") : new Ie(this.num * t.num, this.unit || t.unit);
  }
  divide(t) {
    return this.unit !== "" && t.unit !== "" && this.unit !== t.unit ? new Ie(NaN, "") : new Ie(this.num / t.num, this.unit || t.unit);
  }
  toString() {
    return "".concat(this.num).concat(this.unit);
  }
  isNaN() {
    return Bt(this.num);
  }
}
gy = Ie;
Yk(Ie, "NaN", new gy(NaN, ""));
function by(e) {
  if (e == null || e.includes(Kr))
    return Kr;
  for (var t = e; t.includes("*") || t.includes("/"); ) {
    var r, n = (r = vd.exec(t)) !== null && r !== void 0 ? r : [], i = ya(n, 4), a = i[1], o = i[2], u = i[3], l = Ie.parse(a ?? ""), c = Ie.parse(u ?? ""), s = o === "*" ? l.multiply(c) : l.divide(c);
    if (s.isNaN())
      return Kr;
    t = t.replace(vd, s.toString());
  }
  for (; t.includes("+") || /.-\d+(?:\.\d+)?/.test(t); ) {
    var f, d = (f = pd.exec(t)) !== null && f !== void 0 ? f : [], h = ya(d, 4), v = h[1], p = h[2], m = h[3], y = Ie.parse(v ?? ""), b = Ie.parse(m ?? ""), x = p === "+" ? y.add(b) : y.subtract(b);
    if (x.isNaN())
      return Kr;
    t = t.replace(pd, x.toString());
  }
  return t;
}
var md = /\(([^()]*)\)/;
function nC(e) {
  for (var t = e, r; (r = md.exec(t)) != null; ) {
    var n = r, i = ya(n, 2), a = i[1];
    t = t.replace(md, by(a));
  }
  return t;
}
function iC(e) {
  var t = e.replace(/\s+/g, "");
  return t = nC(t), t = by(t), t;
}
function aC(e) {
  try {
    return iC(e);
  } catch {
    return Kr;
  }
}
function No(e) {
  var t = aC(e.slice(5, -1));
  return t === Kr ? "" : t;
}
var oC = ["x", "y", "lineHeight", "capHeight", "fill", "scaleToFit", "textAnchor", "verticalAnchor"], uC = ["dx", "dy", "angle", "className", "breakAll"];
function Au() {
  return Au = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Au.apply(null, arguments);
}
function yd(e, t) {
  if (e == null) return {};
  var r, n, i = lC(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function lC(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function gd(e, t) {
  return dC(e) || fC(e, t) || sC(e, t) || cC();
}
function cC() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function sC(e, t) {
  if (e) {
    if (typeof e == "string") return bd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? bd(e, t) : void 0;
  }
}
function bd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function fC(e, t) {
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
function dC(e) {
  if (Array.isArray(e)) return e;
}
var wy = /[ \f\n\r\t\v\u2028\u2029]+/, xy = (e) => {
  var t = e.children, r = e.breakAll, n = e.style;
  try {
    var i = [];
    xe(t) || (r ? i = t.toString().split("") : i = t.toString().split(wy));
    var a = i.map((u) => ({
      word: u,
      width: Tn(u, n).width
    })), o = r ? 0 : Tn(" ", n).width;
    return {
      wordsWithComputedWidth: a,
      spaceWidth: o
    };
  } catch {
    return null;
  }
};
function Oy(e) {
  return e === "start" || e === "middle" || e === "end" || e === "inherit";
}
function hC(e) {
  return xe(e) || typeof e == "string" || typeof e == "number" || typeof e == "boolean";
}
var Ay = (e, t, r, n) => e.reduce((i, a) => {
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
}, []), Sy = (e) => e.reduce((t, r) => t.width > r.width ? t : r), vC = "…", wd = (e, t, r, n, i, a, o, u) => {
  var l = e.slice(0, t), c = xy({
    breakAll: r,
    style: n,
    children: l + vC
  });
  if (!c)
    return [!1, []];
  var s = Ay(c.wordsWithComputedWidth, a, o, u), f = s.length > i || Sy(s).width > Number(a);
  return [f, s];
}, pC = (e, t, r, n, i) => {
  var a = e.maxLines, o = e.children, u = e.style, l = e.breakAll, c = M(a), s = String(o), f = Ay(t, n, r, i);
  if (!c || i)
    return f;
  var d = f.length > a || Sy(f).width > Number(n);
  if (!d)
    return f;
  for (var h = 0, v = s.length - 1, p = 0, m; h <= v && p <= s.length - 1; ) {
    var y = Math.floor((h + v) / 2), b = y - 1, x = wd(s, b, l, u, a, n, r, i), O = gd(x, 2), A = O[0], g = O[1], E = wd(s, y, l, u, a, n, r, i), _ = gd(E, 1), I = _[0];
    if (!A && !I && (h = y + 1), A && I && (v = y - 1), !A && I) {
      m = g;
      break;
    }
    p++;
  }
  return m || f;
}, xd = (e) => {
  var t = xe(e) ? [] : e.toString().split(wy);
  return [{
    words: t,
    width: void 0
  }];
}, mC = (e) => {
  var t = e.width, r = e.scaleToFit, n = e.children, i = e.style, a = e.breakAll, o = e.maxLines;
  if ((t || r) && !Qn.isSsr) {
    var u, l, c = xy({
      breakAll: a,
      children: n,
      style: i
    });
    if (c) {
      var s = c.wordsWithComputedWidth, f = c.spaceWidth;
      u = s, l = f;
    } else
      return xd(n);
    return pC({
      breakAll: a,
      children: n,
      maxLines: o,
      style: i
    }, u, l, t, !!r);
  }
  return xd(n);
}, Ey = "#808080", yC = {
  angle: 0,
  breakAll: !1,
  // Magic number from d3
  capHeight: "0.71em",
  fill: Ey,
  lineHeight: "1em",
  scaleToFit: !1,
  textAnchor: "start",
  // Maintain compat with existing charts / default SVG behavior
  verticalAnchor: "end",
  x: 0,
  y: 0
}, Xl = /* @__PURE__ */ Me((e, t) => {
  var r = et(e, yC), n = r.x, i = r.y, a = r.lineHeight, o = r.capHeight, u = r.fill, l = r.scaleToFit, c = r.textAnchor, s = r.verticalAnchor, f = yd(r, oC), d = Kt(() => mC({
    breakAll: f.breakAll,
    children: f.children,
    maxLines: f.maxLines,
    scaleToFit: l,
    style: f.style,
    width: f.width
  }), [f.breakAll, f.children, f.maxLines, l, f.style, f.width]), h = f.dx, v = f.dy, p = f.angle, m = f.className, y = f.breakAll, b = yd(f, uC);
  if (!Pt(n) || !Pt(i) || d.length === 0)
    return null;
  var x = Number(n) + (M(h) ? h : 0), O = Number(i) + (M(v) ? v : 0);
  if (!Y(x) || !Y(O))
    return null;
  var A;
  switch (s) {
    case "start":
      A = No("calc(".concat(o, ")"));
      break;
    case "middle":
      A = No("calc(".concat((d.length - 1) / 2, " * -").concat(a, " + (").concat(o, " / 2))"));
      break;
    default:
      A = No("calc(".concat(d.length - 1, " * -").concat(a, ")"));
      break;
  }
  var g = [], E = d[0];
  if (l && E != null) {
    var _ = E.width, I = f.width;
    g.push("scale(".concat(M(I) && M(_) ? I / _ : 1, ")"));
  }
  return p && g.push("rotate(".concat(p, ", ").concat(x, ", ").concat(O, ")")), g.length && (b.transform = g.join(" ")), /* @__PURE__ */ w.createElement("text", Au({}, at(b), {
    ref: t,
    x,
    y: O,
    className: ie("recharts-text", m),
    textAnchor: c,
    fill: u.includes("url") ? Ey : u
  }), d.map((C, T) => {
    var P = C.words.join(y ? "" : " ");
    return (
      // duplicate words will cause duplicate keys which is why we add the array index here
      /* @__PURE__ */ w.createElement("tspan", {
        x,
        dy: T === 0 ? A : a,
        key: "".concat(P, "-").concat(T)
      }, P)
    );
  }));
});
Xl.displayName = "Text";
function Od(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function yt(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Od(Object(r), !0).forEach(function(n) {
      gC(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Od(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function gC(e, t, r) {
  return (t = bC(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function bC(e) {
  var t = wC(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function wC(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var xC = (e) => {
  var t = e.viewBox, r = e.position, n = e.offset, i = n === void 0 ? 0 : n, a = e.parentViewBox, o = qu(t), u = o.x, l = o.y, c = o.height, s = o.upperWidth, f = o.lowerWidth, d = u, h = u + (s - f) / 2, v = (d + h) / 2, p = (s + f) / 2, m = d + s / 2, y = c >= 0 ? 1 : -1, b = y * i, x = y > 0 ? "end" : "start", O = y > 0 ? "start" : "end", A = s >= 0 ? 1 : -1, g = A * i, E = A > 0 ? "end" : "start", _ = A > 0 ? "start" : "end", I = a;
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
      verticalAnchor: O
    };
    return I && (T.height = Math.max(I.y + I.height - (l + c), 0), T.width = f), T;
  }
  if (r === "left") {
    var P = {
      x: v - g,
      y: l + c / 2,
      horizontalAnchor: E,
      verticalAnchor: "middle"
    };
    return I && (P.width = Math.max(P.x - I.x, 0), P.height = c), P;
  }
  if (r === "right") {
    var z = {
      x: v + p + g,
      y: l + c / 2,
      horizontalAnchor: _,
      verticalAnchor: "middle"
    };
    return I && (z.width = Math.max(I.x + I.width - z.x, 0), z.height = c), z;
  }
  var $ = I ? {
    width: p,
    height: c
  } : {};
  return r === "insideLeft" ? yt({
    x: v + g,
    y: l + c / 2,
    horizontalAnchor: _,
    verticalAnchor: "middle"
  }, $) : r === "insideRight" ? yt({
    x: v + p - g,
    y: l + c / 2,
    horizontalAnchor: E,
    verticalAnchor: "middle"
  }, $) : r === "insideTop" ? yt({
    x: d + s / 2,
    y: l + b,
    horizontalAnchor: "middle",
    verticalAnchor: O
  }, $) : r === "insideBottom" ? yt({
    x: h + f / 2,
    y: l + c - b,
    horizontalAnchor: "middle",
    verticalAnchor: x
  }, $) : r === "insideTopLeft" ? yt({
    x: d + g,
    y: l + b,
    horizontalAnchor: _,
    verticalAnchor: O
  }, $) : r === "insideTopRight" ? yt({
    x: d + s - g,
    y: l + b,
    horizontalAnchor: E,
    verticalAnchor: O
  }, $) : r === "insideBottomLeft" ? yt({
    x: h + g,
    y: l + c - b,
    horizontalAnchor: _,
    verticalAnchor: x
  }, $) : r === "insideBottomRight" ? yt({
    x: h + f - g,
    y: l + c - b,
    horizontalAnchor: E,
    verticalAnchor: x
  }, $) : r && typeof r == "object" && (M(r.x) || Ir(r.x)) && (M(r.y) || Ir(r.y)) ? yt({
    x: u + ur(r.x, p),
    y: l + ur(r.y, c),
    horizontalAnchor: "end",
    verticalAnchor: "end"
  }, $) : yt({
    x: m,
    y: l + c / 2,
    horizontalAnchor: "middle",
    verticalAnchor: "middle"
  }, $);
}, OC = ["labelRef"], AC = ["content"];
function Ad(e, t) {
  if (e == null) return {};
  var r, n, i = SC(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function SC(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Sd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function kn(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Sd(Object(r), !0).forEach(function(n) {
      EC(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Sd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function EC(e, t, r) {
  return (t = PC(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function PC(e) {
  var t = _C(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function _C(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function Dt() {
  return Dt = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Dt.apply(null, arguments);
}
var Py = /* @__PURE__ */ Je(null), IC = (e) => {
  var t = e.x, r = e.y, n = e.upperWidth, i = e.lowerWidth, a = e.width, o = e.height, u = e.children, l = Kt(() => ({
    x: t,
    y: r,
    upperWidth: n,
    lowerWidth: i,
    width: a,
    height: o
  }), [t, r, n, i, a, o]);
  return /* @__PURE__ */ w.createElement(Py.Provider, {
    value: l
  }, u);
}, _y = () => {
  var e = kt(Py), t = Wa();
  return e || (t ? qu(t) : void 0);
}, kC = /* @__PURE__ */ Je(null), CC = () => {
  var e = kt(kC), t = N(mp);
  return e || t;
}, TC = (e) => {
  var t = e.value, r = e.formatter, n = xe(e.children) ? t : e.children;
  return typeof r == "function" ? r(n) : n;
}, Zl = (e) => e != null && typeof e == "function", DC = (e, t) => {
  var r = rt(t - e), n = Math.min(Math.abs(t - e), 360);
  return r * n;
}, jC = (e, t, r, n, i) => {
  var a = e.offset, o = e.className, u = i.cx, l = i.cy, c = i.innerRadius, s = i.outerRadius, f = i.startAngle, d = i.endAngle, h = i.clockWise, v = (c + s) / 2, p = DC(f, d), m = p >= 0 ? 1 : -1, y, b;
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
  b = p <= 0 ? b : !b;
  var x = ke(u, l, v, y), O = ke(u, l, v, y + (b ? 1 : -1) * 359), A = "M".concat(x.x, ",").concat(x.y, `
    A`).concat(v, ",").concat(v, ",0,1,").concat(b ? 0 : 1, `,
    `).concat(O.x, ",").concat(O.y), g = xe(e.id) ? Dn("recharts-radial-line-") : e.id;
  return /* @__PURE__ */ w.createElement("text", Dt({}, n, {
    dominantBaseline: "central",
    className: ie("recharts-radial-bar-label", o)
  }), /* @__PURE__ */ w.createElement("defs", null, /* @__PURE__ */ w.createElement("path", {
    id: g,
    d: A
  })), /* @__PURE__ */ w.createElement("textPath", {
    xlinkHref: "#".concat(g)
  }, r));
}, NC = (e, t, r) => {
  var n = e.cx, i = e.cy, a = e.innerRadius, o = e.outerRadius, u = e.startAngle, l = e.endAngle, c = (u + l) / 2;
  if (r === "outside") {
    var s = ke(n, i, o + t, c), f = s.x, d = s.y;
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
  var h = (a + o) / 2, v = ke(n, i, h, c), p = v.x, m = v.y;
  return {
    x: p,
    y: m,
    textAnchor: "middle",
    verticalAnchor: "middle"
  };
}, Ni = (e) => e != null && "cx" in e && M(e.cx), MC = {
  angle: 0,
  offset: 5,
  zIndex: Re.label,
  position: "middle",
  textBreakAll: !1
};
function $C(e) {
  if (!Ni(e))
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
function nr(e) {
  var t = et(e, MC), r = t.viewBox, n = t.parentViewBox, i = t.position, a = t.value, o = t.children, u = t.content, l = t.className, c = l === void 0 ? "" : l, s = t.textBreakAll, f = t.labelRef, d = CC(), h = _y(), v = i === "center" ? h : d ?? h, p, m, y;
  r == null ? p = v : Ni(r) ? p = r : p = qu(r);
  var b = $C(p);
  if (!p || xe(a) && xe(o) && !/* @__PURE__ */ Ze(u) && typeof u != "function")
    return null;
  var x = kn(kn({}, t), {}, {
    viewBox: p
  });
  if (/* @__PURE__ */ Ze(u)) {
    x.labelRef;
    var O = Ad(x, OC);
    return /* @__PURE__ */ rn(u, O);
  }
  if (typeof u == "function") {
    x.content;
    var A = Ad(x, AC);
    if (m = /* @__PURE__ */ oh(u, A), /* @__PURE__ */ Ze(m))
      return m;
  } else
    m = TC(t);
  var g = at(t);
  if (Ni(p)) {
    if (i === "insideStart" || i === "insideEnd" || i === "end")
      return jC(t, i, m, g, p);
    y = NC(p, t.offset, t.position);
  } else {
    if (!b)
      return null;
    var E = xC({
      viewBox: b,
      position: i,
      offset: t.offset,
      parentViewBox: Ni(n) ? void 0 : n
    });
    y = kn(kn({
      x: E.x,
      y: E.y,
      textAnchor: E.horizontalAnchor,
      verticalAnchor: E.verticalAnchor
    }, E.width !== void 0 ? {
      width: E.width
    } : {}), E.height !== void 0 ? {
      height: E.height
    } : {});
  }
  return /* @__PURE__ */ w.createElement(Jt, {
    zIndex: t.zIndex
  }, /* @__PURE__ */ w.createElement(Xl, Dt({
    ref: f,
    className: ie("recharts-label", c)
  }, g, y, {
    /*
     * textAnchor is decided by default based on the `position`
     * but we allow overriding via props for precise control.
     */
    textAnchor: Oy(g.textAnchor) ? g.textAnchor : y.textAnchor,
    breakAll: s
  }), m));
}
nr.displayName = "Label";
var LC = (e, t, r) => {
  if (!e)
    return null;
  var n = {
    viewBox: t,
    labelRef: r
  };
  return e === !0 ? /* @__PURE__ */ w.createElement(nr, Dt({
    key: "label-implicit"
  }, n)) : Pt(e) ? /* @__PURE__ */ w.createElement(nr, Dt({
    key: "label-implicit",
    value: e
  }, n)) : /* @__PURE__ */ Ze(e) ? e.type === nr ? /* @__PURE__ */ rn(e, kn({
    key: "label-implicit"
  }, n)) : /* @__PURE__ */ w.createElement(nr, Dt({
    key: "label-implicit",
    content: e
  }, n)) : Zl(e) ? /* @__PURE__ */ w.createElement(nr, Dt({
    key: "label-implicit",
    content: e
  }, n)) : e && typeof e == "object" ? /* @__PURE__ */ w.createElement(nr, Dt({}, e, {
    key: "label-implicit"
  }, n)) : null;
};
function RC(e) {
  var t = e.label, r = e.labelRef, n = _y();
  return LC(t, n, r) || null;
}
var zC = ["valueAccessor"], BC = ["dataKey", "clockWise", "id", "textBreakAll", "zIndex"];
function ga() {
  return ga = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, ga.apply(null, arguments);
}
function Ed(e, t) {
  if (e == null) return {};
  var r, n, i = FC(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function FC(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var WC = (e) => {
  var t = Array.isArray(e.value) ? e.value[e.value.length - 1] : e.value;
  if (hC(t))
    return t;
}, Iy = /* @__PURE__ */ Je(void 0), UC = Iy.Provider, ky = /* @__PURE__ */ Je(void 0);
ky.Provider;
function VC() {
  return kt(Iy);
}
function KC() {
  return kt(ky);
}
function Mi(e) {
  var t = e.valueAccessor, r = t === void 0 ? WC : t, n = Ed(e, zC), i = n.dataKey;
  n.clockWise;
  var a = n.id, o = n.textBreakAll, u = n.zIndex, l = Ed(n, BC), c = VC(), s = KC(), f = c || s;
  return !f || !f.length ? null : /* @__PURE__ */ w.createElement(Jt, {
    zIndex: u ?? Re.label
  }, /* @__PURE__ */ w.createElement(zt, {
    className: "recharts-label-list"
  }, f.map((d, h) => {
    var v, p = xe(i) ? r(d, h) : we(d.payload, i), m = xe(a) ? {} : {
      id: "".concat(a, "-").concat(h)
    };
    return /* @__PURE__ */ w.createElement(nr, ga({
      key: "label-".concat(h)
    }, at(d), l, m, {
      /*
       * Prefer to use the explicit fill from LabelList props.
       * Only in an absence of that, fall back to the fill of the entry.
       * The entry fill can be quite difficult to see especially in Bar, Pie, RadialBar in inside positions.
       * On the other hand it's quite convenient in Scatter, Line, or when the position is outside the Bar, Pie filled shapes.
       */
      fill: (v = n.fill) !== null && v !== void 0 ? v : d.fill,
      parentViewBox: d.parentViewBox,
      value: p,
      textBreakAll: o,
      viewBox: d.viewBox,
      index: h,
      zIndex: 0
    }));
  })));
}
Mi.displayName = "LabelList";
function HC(e) {
  var t = e.label;
  return t ? t === !0 ? /* @__PURE__ */ w.createElement(Mi, {
    key: "labelList-implicit"
  }) : /* @__PURE__ */ w.isValidElement(t) || Zl(t) ? /* @__PURE__ */ w.createElement(Mi, {
    key: "labelList-implicit",
    content: t
  }) : typeof t == "object" ? /* @__PURE__ */ w.createElement(Mi, ga({
    key: "labelList-implicit"
  }, t, {
    type: String(t.type)
  })) : null : null;
}
function Su() {
  return Su = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Su.apply(null, arguments);
}
var Cy = (e) => {
  var t = e.cx, r = e.cy, n = e.r, i = e.className, a = ie("recharts-dot", i);
  return M(t) && M(r) && M(n) ? /* @__PURE__ */ w.createElement("circle", Su({}, Et(e), zu(e), {
    className: a,
    cx: t,
    cy: r,
    r: n
  })) : null;
}, GC = {
  radiusAxis: {},
  angleAxis: {}
}, Ty = $e({
  name: "polarAxis",
  initialState: GC,
  reducers: {
    addRadiusAxis(e, t) {
      e.radiusAxis[t.payload.id] = G(t.payload);
    },
    removeRadiusAxis(e, t) {
      delete e.radiusAxis[t.payload.id];
    },
    addAngleAxis(e, t) {
      e.angleAxis[t.payload.id] = G(t.payload);
    },
    removeAngleAxis(e, t) {
      delete e.angleAxis[t.payload.id];
    }
  }
}), oo = Ty.actions;
oo.addRadiusAxis;
oo.removeRadiusAxis;
oo.addAngleAxis;
oo.removeAngleAxis;
var YC = Ty.reducer;
function qC(e) {
  return e && typeof e == "object" && "className" in e && typeof e.className == "string" ? e.className : "";
}
var Dy = (e) => e && typeof e == "object" && "clipDot" in e ? !!e.clipDot : !0;
function Pd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function _d(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Pd(Object(r), !0).forEach(function(n) {
      XC(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Pd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function XC(e, t, r) {
  return (t = ZC(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function ZC(e) {
  var t = QC(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function QC(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function jy(e, t) {
  return _d(_d({}, t), e);
}
function JC(e) {
  return /* @__PURE__ */ Ze(e) ? e.props : e;
}
function eT(e, t) {
  return /* @__PURE__ */ rn(e, jy(JC(e), t));
}
function tT(e) {
  if ("index" in e) {
    var t = e.index;
    return typeof t == "number" || typeof t == "string" ? t : void 0;
  }
}
function rT(e) {
  return "isActive" in e && e.isActive === !0;
}
function nT(e) {
  var t = e.option, r = e.DefaultShape, n = e.shapeProps, i = e.activeClassName, a = i === void 0 ? "recharts-active-shape" : i, o = e.inActiveClassName, u = o === void 0 ? "recharts-shape" : o, l = tT(n), c;
  return /* @__PURE__ */ Ze(t) ? c = eT(t, n) : t === r ? c = /* @__PURE__ */ w.createElement(r, n) : typeof t == "function" ? c = t(n, l) : typeof t == "object" ? c = /* @__PURE__ */ w.createElement(r, jy(t, n)) : c = /* @__PURE__ */ w.createElement(r, n), rT(n) ? /* @__PURE__ */ w.createElement(zt, {
    className: a
  }, c) : /* @__PURE__ */ w.createElement(zt, {
    className: u
  }, c);
}
function iT(e) {
  var t = e.tooltipEntrySettings, r = ve(), n = Ve(), i = V(null);
  return Ue(() => {
    n || (i.current === null ? r(v_(t)) : i.current !== t && r(p_({
      prev: i.current,
      next: t
    })), i.current = t);
  }, [t, r, n]), Ue(() => () => {
    i.current && (r(m_(i.current)), i.current = null);
  }, [r]), null;
}
function aT(e) {
  var t = e.legendPayload, r = ve(), n = Ve(), i = V(null);
  return Ue(() => {
    n || (i.current === null ? r(Fx(t)) : i.current !== t && r(Wx({
      prev: i.current,
      next: t
    })), i.current = t);
  }, [r, n, t]), Ue(() => () => {
    i.current && (r(Ux(i.current)), i.current = null);
  }, [r]), null;
}
function oT(e, t) {
  return sT(e) || cT(e, t) || lT(e, t) || uT();
}
function uT() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function lT(e, t) {
  if (e) {
    if (typeof e == "string") return Id(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Id(e, t) : void 0;
  }
}
function Id(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function cT(e, t) {
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
function sT(e) {
  if (Array.isArray(e)) return e;
}
var Ql = "index", fT = "append";
function Jl(e, t) {
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
function dT(e, t) {
  var r = e.length / t.length, n = t.map((i, a) => e[Math.floor(a * r)]);
  return Jl(n, t);
}
function hT(e, t) {
  var r = t.map((n, i) => e[i]);
  return Jl(r, t);
}
function vT(e, t) {
  for (var r = /* @__PURE__ */ new Map(), n = 0; n < e.length; n++) {
    var i = e[n];
    if (i != null) {
      var a = t(i, n);
      a != null && !r.has(a) && r.set(a, i);
    }
  }
  return r;
}
function pT(e, t, r) {
  var n = vT(e, r), i = /* @__PURE__ */ new Set(), a = t.map((f, d) => {
    var h = r(f, d);
    if (h != null) {
      var v = n.get(h);
      if (v !== void 0)
        return i.add(h), v;
    }
  }), o = [];
  for (var u of n) {
    var l = oT(u, 2), c = l[0], s = l[1];
    i.has(c) || o.push(s);
  }
  return Jl(a, t, o);
}
function mT(e, t, r) {
  return t == null ? null : e == null ? t.map((n) => ({
    status: "added",
    next: n
  })) : r === Ql ? dT(e, t) : r === fT ? hT(e, t) : pT(e, t, r);
}
function yT(e, t) {
  var r = V(e), n = V(t.current), i = V(!0);
  r.current !== e && (r.current = e, n.current = t.current, i.current = !1);
  var a = ee(function(o, u) {
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
function gT(e, t) {
  return OT(e) || xT(e, t) || wT(e, t) || bT();
}
function bT() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function wT(e, t) {
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
function xT(e, t) {
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
function OT(e) {
  if (Array.isArray(e)) return e;
}
function AT(e, t) {
  var r = fe(!1), n = gT(r, 2), i = n[0], a = n[1], o = ee(() => {
    typeof e == "function" && e(), a(!0);
  }, [e]), u = ee(() => {
    typeof t == "function" && t(), a(!1);
  }, [t]);
  return {
    isAnimating: i,
    handleAnimationStart: o,
    handleAnimationEnd: u
  };
}
function ST(e) {
  var t, r = e.animationInput, n = e.animationIdPrefix, i = e.items, a = e.previousItemsRef, o = e.isAnimationActive, u = e.animationBegin, l = e.animationDuration, c = e.animationEasing, s = e.onAnimationStart, f = e.onAnimationEnd, d = e.animationInterpolateFn, h = e.animationMatchBy, v = e.shouldUpdatePreviousRef, p = e.children, m = e.layout, y = Yv(r, n), b = yT(y, a), x = (t = b.startValue) !== null && t !== void 0 ? t : null, O = mT(x, i, h ?? Ql);
  return /* @__PURE__ */ w.createElement(Gv, {
    animationId: y,
    begin: u,
    duration: l,
    isActive: o,
    easing: c,
    onAnimationEnd: f,
    onAnimationStart: s,
    key: y
  }, (A) => {
    var g = x == null, E = i == null ? i : d(O, A, m), _ = v ? v(A) : A > 0;
    return b.syncStepValue(E, A, _), E == null ? null : p(E, A, g);
  });
}
var Mo;
function ET(e, t) {
  return kT(e) || IT(e, t) || _T(e, t) || PT();
}
function PT() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function _T(e, t) {
  if (e) {
    if (typeof e == "string") return Cd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? Cd(e, t) : void 0;
  }
}
function Cd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function IT(e, t) {
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
function kT(e) {
  if (Array.isArray(e)) return e;
}
var CT = () => {
  var e = w.useState(() => Dn("uid-")), t = ET(e, 1), r = t[0];
  return r;
}, TT = (Mo = w.useId) !== null && Mo !== void 0 ? Mo : CT;
function DT(e, t) {
  var r = TT();
  return t || (e ? "".concat(e, "-").concat(r) : r);
}
var jT = /* @__PURE__ */ Je(void 0), NT = (e) => {
  var t = e.id, r = e.type, n = e.children, i = DT("recharts-".concat(r), t);
  return /* @__PURE__ */ w.createElement(jT.Provider, {
    value: i
  }, n(i));
}, MT = {
  cartesianItems: [],
  polarItems: []
}, Ny = $e({
  name: "graphicalItems",
  initialState: MT,
  reducers: {
    addCartesianGraphicalItem: {
      reducer(e, t) {
        e.cartesianItems.push(G(t.payload));
      },
      prepare: re()
    },
    replaceCartesianGraphicalItem: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next, a = nt(e).cartesianItems.indexOf(G(n));
        a > -1 && (e.cartesianItems[a] = G(i));
      },
      prepare: re()
    },
    removeCartesianGraphicalItem: {
      reducer(e, t) {
        var r = nt(e).cartesianItems.indexOf(G(t.payload));
        r > -1 && e.cartesianItems.splice(r, 1);
      },
      prepare: re()
    },
    addPolarGraphicalItem: {
      reducer(e, t) {
        e.polarItems.push(G(t.payload));
      },
      prepare: re()
    },
    removePolarGraphicalItem: {
      reducer(e, t) {
        var r = nt(e).polarItems.indexOf(G(t.payload));
        r > -1 && e.polarItems.splice(r, 1);
      },
      prepare: re()
    },
    replacePolarGraphicalItem: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next, a = nt(e).polarItems.indexOf(G(n));
        a > -1 && (e.polarItems[a] = G(i));
      },
      prepare: re()
    }
  }
}), vn = Ny.actions, $T = vn.addCartesianGraphicalItem, LT = vn.replaceCartesianGraphicalItem, RT = vn.removeCartesianGraphicalItem;
vn.addPolarGraphicalItem;
vn.removePolarGraphicalItem;
vn.replacePolarGraphicalItem;
var zT = Ny.reducer, BT = (e) => {
  var t = ve(), r = V(null);
  return Ue(() => {
    r.current === null ? t($T(e)) : r.current !== e && t(LT({
      prev: r.current,
      next: e
    })), r.current = e;
  }, [t, e]), Ue(() => () => {
    r.current && (t(RT(r.current)), r.current = null);
  }, [t]), null;
}, FT = /* @__PURE__ */ ju(BT), WT = ["points"];
function Td(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function $o(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Td(Object(r), !0).forEach(function(n) {
      UT(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Td(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function UT(e, t, r) {
  return (t = VT(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function VT(e) {
  var t = KT(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function KT(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function ba() {
  return ba = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, ba.apply(null, arguments);
}
function HT(e, t) {
  if (e == null) return {};
  var r, n, i = GT(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function GT(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function YT(e) {
  var t = e.option, r = e.dotProps, n = e.className;
  if (/* @__PURE__ */ Ze(t))
    return /* @__PURE__ */ rn(t, r);
  if (typeof t == "function")
    return t(r);
  var i = ie(n, typeof t != "boolean" ? t.className : ""), a = r ?? {};
  a.points;
  var o = HT(a, WT);
  return /* @__PURE__ */ w.createElement(Cy, ba({}, o, {
    className: i
  }));
}
function qT(e, t) {
  return e == null ? !1 : t ? !0 : e.length === 1;
}
function XT(e) {
  var t = e.points, r = e.dot, n = e.className, i = e.dotClassName, a = e.dataKey, o = e.baseProps, u = e.needClip, l = e.clipPathId, c = e.zIndex, s = c === void 0 ? Re.scatter : c;
  if (!qT(t, r))
    return null;
  var f = Dy(r), d = Ag(r), h = t.map((p, m) => {
    var y, b, x = $o($o($o({
      r: 3
    }, o), d), {}, {
      index: m,
      cx: (y = p.x) !== null && y !== void 0 ? y : void 0,
      cy: (b = p.y) !== null && b !== void 0 ? b : void 0,
      dataKey: a,
      value: p.value,
      payload: p.payload,
      points: t
    });
    return /* @__PURE__ */ w.createElement(YT, {
      key: "dot-".concat(m),
      option: r,
      dotProps: x,
      className: i
    });
  }), v = {};
  return u && l != null && (v.clipPath = "url(#clipPath-".concat(f ? "" : "dots-").concat(l, ")")), /* @__PURE__ */ w.createElement(Jt, {
    zIndex: s
  }, /* @__PURE__ */ w.createElement(zt, ba({
    className: n
  }, v), h));
}
function Dd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function jd(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Dd(Object(r), !0).forEach(function(n) {
      ZT(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Dd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function ZT(e, t, r) {
  return (t = QT(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function QT(e) {
  var t = JT(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function JT(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var eD = {
  xAxis: {},
  yAxis: {},
  zAxis: {}
}, My = $e({
  name: "cartesianAxis",
  initialState: eD,
  reducers: {
    addXAxis: {
      reducer(e, t) {
        e.xAxis[t.payload.id] = G(t.payload);
      },
      prepare: re()
    },
    replaceXAxis: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next;
        e.xAxis[n.id] !== void 0 && (n.id !== i.id && delete e.xAxis[n.id], e.xAxis[i.id] = G(i));
      },
      prepare: re()
    },
    removeXAxis: {
      reducer(e, t) {
        delete e.xAxis[t.payload.id];
      },
      prepare: re()
    },
    addYAxis: {
      reducer(e, t) {
        e.yAxis[t.payload.id] = G(t.payload);
      },
      prepare: re()
    },
    replaceYAxis: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next;
        e.yAxis[n.id] !== void 0 && (n.id !== i.id && delete e.yAxis[n.id], e.yAxis[i.id] = G(i));
      },
      prepare: re()
    },
    removeYAxis: {
      reducer(e, t) {
        delete e.yAxis[t.payload.id];
      },
      prepare: re()
    },
    addZAxis: {
      reducer(e, t) {
        e.zAxis[t.payload.id] = G(t.payload);
      },
      prepare: re()
    },
    replaceZAxis: {
      reducer(e, t) {
        var r = t.payload, n = r.prev, i = r.next;
        e.zAxis[n.id] !== void 0 && (n.id !== i.id && delete e.zAxis[n.id], e.zAxis[i.id] = G(i));
      },
      prepare: re()
    },
    removeZAxis: {
      reducer(e, t) {
        delete e.zAxis[t.payload.id];
      },
      prepare: re()
    },
    updateYAxisWidth(e, t) {
      var r = t.payload, n = r.id, i = r.width, a = e.yAxis[n];
      if (a) {
        var o, u = a.widthHistory || [];
        if (u.length === 3 && u[0] === u[2] && i === u[1] && i !== a.width && Math.abs(i - ((o = u[0]) !== null && o !== void 0 ? o : 0)) <= 1)
          return;
        var l = [...u, i].slice(-3);
        e.yAxis[n] = jd(jd({}, a), {}, {
          width: i,
          widthHistory: l
        });
      }
    }
  }
}), Tt = My.actions, tD = Tt.addXAxis, rD = Tt.replaceXAxis, nD = Tt.removeXAxis, iD = Tt.addYAxis, aD = Tt.replaceYAxis, oD = Tt.removeYAxis;
Tt.addZAxis;
Tt.replaceZAxis;
Tt.removeZAxis;
var uD = Tt.updateYAxisWidth, lD = My.reducer, cD = S([Ce], (e) => ({
  top: e.top,
  bottom: e.bottom,
  left: e.left,
  right: e.right
})), sD = S([cD, Gt, Yt], (e, t, r) => {
  if (!(!e || t == null || r == null))
    return {
      x: e.left,
      y: e.top,
      width: Math.max(0, t - e.left - e.right),
      height: Math.max(0, r - e.top - e.bottom)
    };
}), ec = () => N(sD), fD = () => N(vI);
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
function Lo(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Nd(Object(r), !0).forEach(function(n) {
      dD(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Nd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function dD(e, t, r) {
  return (t = hD(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function hD(e) {
  var t = vD(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function vD(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var pD = (e) => {
  var t = e.point, r = e.childIndex, n = e.mainColor, i = e.activeDot, a = e.dataKey, o = e.clipPath;
  if (i === !1 || t.x == null || t.y == null)
    return null;
  var u = {
    index: r,
    dataKey: a,
    cx: t.x,
    cy: t.y,
    r: 4,
    fill: n ?? "none",
    strokeWidth: 2,
    stroke: "#fff",
    payload: t.payload,
    value: t.value
  }, l = Lo(Lo(Lo({}, u), Ea(i)), zu(i)), c;
  return /* @__PURE__ */ Ze(i) ? c = /* @__PURE__ */ rn(i, l) : typeof i == "function" ? c = i(l) : c = /* @__PURE__ */ w.createElement(Cy, l), /* @__PURE__ */ w.createElement(zt, {
    className: "recharts-active-dot",
    clipPath: o
  }, c);
};
function mD(e) {
  var t = e.points, r = e.mainColor, n = e.activeDot, i = e.itemDataKey, a = e.clipPath, o = e.zIndex, u = o === void 0 ? Re.activeDot : o, l = N(Wn), c = fD();
  if (t == null || c == null)
    return null;
  var s = t.find((f) => c.includes(f.payload));
  return xe(s) ? null : /* @__PURE__ */ w.createElement(Jt, {
    zIndex: u
  }, /* @__PURE__ */ w.createElement(pD, {
    point: s,
    childIndex: Number(l),
    mainColor: r,
    dataKey: i,
    activeDot: n,
    clipPath: a
  }));
}
var yD = (e) => {
  var t = e.chartData, r = ve(), n = Ve();
  return he(() => n ? () => {
  } : (r(nd(t)), () => {
    r(nd(void 0));
  }), [t, r, n]), null;
}, Md = {
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
}, $y = $e({
  name: "brush",
  initialState: Md,
  reducers: {
    setBrushSettings(e, t) {
      return t.payload == null ? Md : t.payload;
    }
  }
});
$y.actions.setBrushSettings;
var gD = $y.reducer;
function bD(e) {
  return (e % 180 + 180) % 180;
}
var wD = function(t) {
  var r = t.width, n = t.height, i = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : 0, a = bD(i), o = a * Math.PI / 180, u = Math.atan(n / r), l = o > u && o < Math.PI - u ? n / Math.sin(o) : r / Math.cos(o);
  return Math.abs(l);
}, xD = {
  dots: [],
  areas: [],
  lines: []
}, Ly = $e({
  name: "referenceElements",
  initialState: xD,
  reducers: {
    addDot: (e, t) => {
      e.dots.push(t.payload);
    },
    removeDot: (e, t) => {
      var r = nt(e).dots.findIndex((n) => n === t.payload);
      r !== -1 && e.dots.splice(r, 1);
    },
    addArea: (e, t) => {
      e.areas.push(t.payload);
    },
    removeArea: (e, t) => {
      var r = nt(e).areas.findIndex((n) => n === t.payload);
      r !== -1 && e.areas.splice(r, 1);
    },
    addLine: (e, t) => {
      e.lines.push(G(t.payload));
    },
    removeLine: (e, t) => {
      var r = nt(e).lines.findIndex((n) => n === t.payload);
      r !== -1 && e.lines.splice(r, 1);
    }
  }
}), pn = Ly.actions;
pn.addDot;
pn.removeDot;
pn.addArea;
pn.removeArea;
pn.addLine;
pn.removeLine;
var OD = Ly.reducer;
function AD(e, t) {
  return _D(e) || PD(e, t) || ED(e, t) || SD();
}
function SD() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function ED(e, t) {
  if (e) {
    if (typeof e == "string") return $d(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? $d(e, t) : void 0;
  }
}
function $d(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function PD(e, t) {
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
function _D(e) {
  if (Array.isArray(e)) return e;
}
var ID = /* @__PURE__ */ Je(void 0), kD = (e) => {
  var t = e.children, r = fe("".concat(Dn("recharts"), "-clip")), n = AD(r, 1), i = n[0], a = ec();
  if (a == null)
    return null;
  var o = a.x, u = a.y, l = a.width, c = a.height;
  return /* @__PURE__ */ w.createElement(ID.Provider, {
    value: i
  }, /* @__PURE__ */ w.createElement("defs", null, /* @__PURE__ */ w.createElement("clipPath", {
    id: i
  }, /* @__PURE__ */ w.createElement("rect", {
    x: o,
    y: u,
    height: c,
    width: l
  }))), t);
};
function Ry(e, t) {
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
function CD(e, t, r) {
  var n = {
    width: e.width + t.width,
    height: e.height + t.height
  };
  return wD(n, r);
}
function TD(e, t, r) {
  var n = r === "width", i = e.x, a = e.y, o = e.width, u = e.height;
  return t === 1 ? {
    start: n ? i : a,
    end: n ? i + o : a + u
  } : {
    start: n ? i + o : a + u,
    end: n ? i : a
  };
}
function Vn(e, t, r, n, i) {
  if (e * t < e * n || e * t > e * i)
    return !1;
  var a = r();
  return e * (t - e * a / 2 - n) >= 0 && e * (t + e * a / 2 - i) <= 0;
}
function DD(e, t) {
  return Ry(e, t + 1);
}
function jD(e, t, r, n, i) {
  for (var a = (n || []).slice(), o = t.start, u = t.end, l = 0, c = 1, s = o, f = function() {
    var v = n == null ? void 0 : n[l];
    if (v === void 0)
      return {
        v: Ry(n, c)
      };
    var p = l, m, y = () => (m === void 0 && (m = r(v, p)), m), b = v.coordinate, x = l === 0 || Vn(e, b, y, s, u);
    x || (l = 0, s = o, c += 1), x && (s = b + e * (y() / 2 + i), l += c);
  }, d; c <= a.length; )
    if (d = f(), d) return d.v;
  return [];
}
function ND(e, t, r, n, i) {
  var a = (n || []).slice(), o = a.length;
  if (o === 0)
    return [];
  for (var u = t.start, l = t.end, c = 1; c <= o; c++) {
    for (var s = (o - 1) % c, f = u, d = !0, h = function() {
      var O = n[p];
      if (O == null)
        return 0;
      var A = p, g, E = () => (g === void 0 && (g = r(O, A)), g), _ = O.coordinate, I = p === s || Vn(e, _, E, f, l);
      if (!I)
        return d = !1, 1;
      I && (f = _ + e * (E() / 2 + i));
    }, v, p = s; p < o && (v = h(), !(v !== 0 && v === 1)); p += c)
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
function Ld(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function je(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Ld(Object(r), !0).forEach(function(n) {
      MD(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Ld(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function MD(e, t, r) {
  return (t = $D(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function $D(e) {
  var t = LD(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function LD(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function RD(e, t, r, n, i) {
  for (var a = (n || []).slice(), o = a.length, u = t.start, l = t.end, c = function(d) {
    var h = a[d];
    if (h == null)
      return 1;
    var v = h, p, m = () => (p === void 0 && (p = r(h, d)), p);
    if (d === o - 1) {
      var y = e * (v.coordinate + e * m() / 2 - l);
      a[d] = v = je(je({}, v), {}, {
        tickCoord: y > 0 ? v.coordinate - y * e : v.coordinate
      });
    } else
      a[d] = v = je(je({}, v), {}, {
        tickCoord: v.coordinate
      });
    if (v.tickCoord != null) {
      var b = Vn(e, v.tickCoord, m, u, l);
      b && (l = v.tickCoord - e * (m() / 2 + i), a[d] = je(je({}, v), {}, {
        isShow: !0
      }));
    }
  }, s = o - 1; s >= 0; s--)
    c(s);
  return a;
}
function zD(e, t, r, n, i, a) {
  var o = (n || []).slice(), u = o.length, l = t.start, c = t.end;
  if (a) {
    var s = n[u - 1];
    if (s != null) {
      var f = r(s, u - 1), d = e * (s.coordinate + e * f / 2 - c);
      if (o[u - 1] = s = je(je({}, s), {}, {
        tickCoord: d > 0 ? s.coordinate - d * e : s.coordinate
      }), s.tickCoord != null) {
        var h = Vn(e, s.tickCoord, () => f, l, c);
        h && (c = s.tickCoord - e * (f / 2 + i), o[u - 1] = je(je({}, s), {}, {
          isShow: !0
        }));
      }
    }
  }
  for (var v = a ? u - 1 : u, p = function(b) {
    var x = o[b];
    if (x == null)
      return 1;
    var O = x, A, g = () => (A === void 0 && (A = r(x, b)), A);
    if (b === 0) {
      var E = e * (O.coordinate - e * g() / 2 - l);
      o[b] = O = je(je({}, O), {}, {
        tickCoord: E < 0 ? O.coordinate - E * e : O.coordinate
      });
    } else
      o[b] = O = je(je({}, O), {}, {
        tickCoord: O.coordinate
      });
    if (O.tickCoord != null) {
      var _ = Vn(e, O.tickCoord, g, l, c);
      _ && (l = O.tickCoord + e * (g() / 2 + i), o[b] = je(je({}, O), {}, {
        isShow: !0
      }));
    }
  }, m = 0; m < v; m++)
    p(m);
  return o;
}
function tc(e, t, r) {
  var n = e.tick, i = e.ticks, a = e.viewBox, o = e.minTickGap, u = e.orientation, l = e.interval, c = e.tickFormatter, s = e.unit, f = e.angle;
  if (!i || !i.length || !n)
    return [];
  if (M(l) || Qn.isSsr) {
    var d;
    return (d = DD(i, M(l) ? l : 0)) !== null && d !== void 0 ? d : [];
  }
  var h = [], v = u === "top" || u === "bottom" ? "width" : "height", p = s && v === "width" ? Tn(s, {
    fontSize: t,
    letterSpacing: r
  }) : {
    width: 0,
    height: 0
  }, m = (A, g) => {
    var E = typeof c == "function" ? c(A.value, g) : A.value;
    return v === "width" ? CD(Tn(E, {
      fontSize: t,
      letterSpacing: r
    }), p, f) : Tn(E, {
      fontSize: t,
      letterSpacing: r
    })[v];
  }, y = i[0], b = i[1], x = i.length >= 2 && y != null && b != null ? rt(b.coordinate - y.coordinate) : 1, O = TD(a, x, v);
  return l === "equidistantPreserveStart" ? jD(x, O, m, i, o) : l === "equidistantPreserveEnd" ? ND(x, O, m, i, o) : (l === "preserveStart" || l === "preserveStartEnd" ? h = zD(x, O, m, i, o, l === "preserveStartEnd") : h = RD(x, O, m, i, o), h.filter((A) => A.isShow));
}
var BD = (e) => {
  var t = e.ticks, r = e.label, n = e.labelGapWithTick, i = n, a = e.tickSize, o = a === void 0 ? 0 : a, u = e.tickMargin, l = u === void 0 ? 0 : u, c = 0;
  if (t) {
    Array.from(t).forEach((h) => {
      if (h) {
        var v = h.getBoundingClientRect();
        v.width > c && (c = v.width);
      }
    });
    var s = r ? r.getBoundingClientRect().width : 0, f = o + l, d = c + f + s + (r ? i : 0);
    return Math.round(d);
  }
  return 0;
}, FD = {
  xAxis: {},
  yAxis: {}
}, zy = $e({
  name: "renderedTicks",
  initialState: FD,
  reducers: {
    setRenderedTicks: (e, t) => {
      var r = t.payload, n = r.axisType, i = r.axisId, a = r.ticks;
      e[n][i] = G(a);
    },
    removeRenderedTicks: (e, t) => {
      var r = t.payload, n = r.axisType, i = r.axisId;
      delete e[n][i];
    }
  }
}), By = zy.actions, WD = By.setRenderedTicks, UD = By.removeRenderedTicks, VD = zy.reducer, KD = ["axisLine", "width", "height", "className", "hide", "ticks", "axisType", "axisId"];
function Rd(e, t) {
  return qD(e) || YD(e, t) || GD(e, t) || HD();
}
function HD() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function GD(e, t) {
  if (e) {
    if (typeof e == "string") return zd(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? zd(e, t) : void 0;
  }
}
function zd(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function YD(e, t) {
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
function qD(e) {
  if (Array.isArray(e)) return e;
}
function XD(e, t) {
  if (e == null) return {};
  var r, n, i = ZD(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function ZD(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Dr() {
  return Dr = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Dr.apply(null, arguments);
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
function ce(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Bd(Object(r), !0).forEach(function(n) {
      QD(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Bd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function QD(e, t, r) {
  return (t = JD(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function JD(e) {
  var t = ej(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function ej(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var Rt = {
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
  zIndex: Re.axis
};
function tj(e) {
  var t = e.x, r = e.y, n = e.width, i = e.height, a = e.orientation, o = e.mirror, u = e.axisLine, l = e.otherSvgProps;
  if (!u)
    return null;
  var c = ce(ce(ce({}, l), Et(u)), {}, {
    fill: "none"
  });
  if (a === "top" || a === "bottom") {
    var s = +(a === "top" && !o || a === "bottom" && o);
    c = ce(ce({}, c), {}, {
      x1: t,
      y1: r + s * i,
      x2: t + n,
      y2: r + s * i
    });
  } else {
    var f = +(a === "left" && !o || a === "right" && o);
    c = ce(ce({}, c), {}, {
      x1: t + f * n,
      y1: r,
      x2: t + f * n,
      y2: r + i
    });
  }
  return /* @__PURE__ */ w.createElement("line", Dr({}, c, {
    className: ie("recharts-cartesian-axis-line", jr(u, "className"))
  }));
}
function rj(e, t, r, n, i, a, o, u, l) {
  var c, s, f, d, h, v, p = u ? -1 : 1, m = e.tickSize || o, y = M(e.tickCoord) ? e.tickCoord : e.coordinate;
  switch (a) {
    case "top":
      c = s = e.coordinate, d = r + +!u * i, f = d - p * m, v = f - p * l, h = y;
      break;
    case "left":
      f = d = e.coordinate, s = t + +!u * n, c = s - p * m, h = c - p * l, v = y;
      break;
    case "right":
      f = d = e.coordinate, s = t + +u * n, c = s + p * m, h = c + p * l, v = y;
      break;
    default:
      c = s = e.coordinate, d = r + +u * i, f = d + p * m, v = f + p * l, h = y;
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
      y: v
    }
  };
}
function nj(e, t) {
  switch (e) {
    case "left":
      return t ? "start" : "end";
    case "right":
      return t ? "end" : "start";
    default:
      return "middle";
  }
}
function ij(e, t) {
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
function aj(e) {
  var t = e.option, r = e.tickProps, n = e.value, i, a = ie(r.className, "recharts-cartesian-axis-tick-value");
  if (/* @__PURE__ */ w.isValidElement(t))
    i = /* @__PURE__ */ w.cloneElement(t, ce(ce({}, r), {}, {
      className: a
    }));
  else if (typeof t == "function")
    i = t(ce(ce({}, r), {}, {
      className: a
    }));
  else {
    var o = "recharts-cartesian-axis-tick-value";
    typeof t != "boolean" && (o = ie(o, qC(t))), i = /* @__PURE__ */ w.createElement(Xl, Dr({}, r, {
      className: o
    }), n);
  }
  return i;
}
function oj(e) {
  var t = e.ticks, r = e.axisType, n = e.axisId, i = ve();
  return he(() => {
    if (n == null || r == null)
      return nn;
    var a = t.map((o) => ({
      value: o.value,
      coordinate: o.coordinate,
      offset: o.offset,
      index: o.index
    }));
    return i(WD({
      ticks: a,
      axisId: n,
      axisType: r
    })), () => {
      i(UD({
        axisId: n,
        axisType: r
      }));
    };
  }, [i, t, n, r]), null;
}
var uj = /* @__PURE__ */ Me((e, t) => {
  var r = e.ticks, n = r === void 0 ? [] : r, i = e.tick, a = e.tickLine, o = e.stroke, u = e.tickFormatter, l = e.unit, c = e.padding, s = e.tickTextProps, f = e.orientation, d = e.mirror, h = e.x, v = e.y, p = e.width, m = e.height, y = e.tickSize, b = e.tickMargin, x = e.fontSize, O = e.letterSpacing, A = e.getTicksConfig, g = e.events, E = e.axisType, _ = e.axisId, I = tc(ce(ce({}, A), {}, {
    ticks: n
  }), x, O), C = Et(A), T = Ea(i), P = Oy(C.textAnchor) ? C.textAnchor : nj(f, d), z = ij(f, d), $ = {};
  typeof a == "object" && ($ = a);
  var Q = ce(ce({}, C), {}, {
    fill: "none"
  }, $), K = I.map((X) => ce({
    entry: X
  }, rj(X, h, v, p, m, f, y, d, b))), H = K.map((X) => {
    var W = X.entry, Te = X.line;
    return /* @__PURE__ */ w.createElement(zt, {
      className: "recharts-cartesian-axis-tick",
      key: "tick-".concat(W.value, "-").concat(W.coordinate, "-").concat(W.tickCoord)
    }, a && /* @__PURE__ */ w.createElement("line", Dr({}, Q, Te, {
      className: ie("recharts-cartesian-axis-tick-line", jr(a, "className"))
    })));
  }), B = K.map((X, W) => {
    var Te, Ee, se = X.entry, Ke = X.tick, He = ce(ce(ce(ce({
      verticalAnchor: z
    }, C), {}, {
      textAnchor: P,
      stroke: "none",
      fill: o
    }, Ke), {}, {
      index: W,
      payload: se,
      visibleTicksCount: I.length,
      tickFormatter: u,
      padding: c
    }, s), {}, {
      angle: (Te = (Ee = s == null ? void 0 : s.angle) !== null && Ee !== void 0 ? Ee : C.angle) !== null && Te !== void 0 ? Te : 0
    }), ct = ce(ce({}, He), T);
    return /* @__PURE__ */ w.createElement(zt, Dr({
      className: "recharts-cartesian-axis-tick-label",
      key: "tick-label-".concat(se.value, "-").concat(se.coordinate, "-").concat(se.tickCoord)
    }, r0(g, se, W)), i && /* @__PURE__ */ w.createElement(aj, {
      option: i,
      tickProps: ct,
      value: "".concat(typeof u == "function" ? u(se.value, W) : se.value).concat(l || "")
    }));
  });
  return /* @__PURE__ */ w.createElement("g", {
    className: "recharts-cartesian-axis-ticks recharts-".concat(E, "-ticks")
  }, /* @__PURE__ */ w.createElement(oj, {
    ticks: I,
    axisId: _,
    axisType: E
  }), B.length > 0 && /* @__PURE__ */ w.createElement(Jt, {
    zIndex: Re.label
  }, /* @__PURE__ */ w.createElement("g", {
    className: "recharts-cartesian-axis-tick-labels recharts-".concat(E, "-tick-labels"),
    ref: t
  }, B)), H.length > 0 && /* @__PURE__ */ w.createElement("g", {
    className: "recharts-cartesian-axis-tick-lines recharts-".concat(E, "-tick-lines")
  }, H));
}), lj = /* @__PURE__ */ Me((e, t) => {
  var r = e.axisLine, n = e.width, i = e.height, a = e.className, o = e.hide, u = e.ticks, l = e.axisType, c = e.axisId, s = XD(e, KD), f = fe(""), d = Rd(f, 2), h = d[0], v = d[1], p = fe(""), m = Rd(p, 2), y = m[0], b = m[1], x = V(null);
  ah(t, () => ({
    getCalculatedWidth: () => {
      var A;
      return BD({
        ticks: x.current,
        label: (A = e.labelRef) === null || A === void 0 ? void 0 : A.current,
        labelGapWithTick: 5,
        tickSize: e.tickSize,
        tickMargin: e.tickMargin
      });
    }
  }));
  var O = ee((A) => {
    if (A) {
      var g = A.getElementsByClassName("recharts-cartesian-axis-tick-value");
      x.current = g;
      var E = g[0];
      if (E) {
        var _ = window.getComputedStyle(E), I = _.fontSize, C = _.letterSpacing;
        (I !== h || C !== y) && (v(I), b(C));
      }
    }
  }, [h, y]);
  return o || n != null && n <= 0 || i != null && i <= 0 ? null : /* @__PURE__ */ w.createElement(Jt, {
    zIndex: e.zIndex
  }, /* @__PURE__ */ w.createElement(zt, {
    className: ie("recharts-cartesian-axis", a)
  }, /* @__PURE__ */ w.createElement(tj, {
    x: e.x,
    y: e.y,
    width: n,
    height: i,
    orientation: e.orientation,
    mirror: e.mirror,
    axisLine: r,
    otherSvgProps: Et(e)
  }), /* @__PURE__ */ w.createElement(uj, {
    ref: O,
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
  }), /* @__PURE__ */ w.createElement(IC, {
    x: e.x,
    y: e.y,
    width: e.width,
    height: e.height,
    lowerWidth: e.width,
    upperWidth: e.width
  }, /* @__PURE__ */ w.createElement(RC, {
    label: e.label,
    labelRef: e.labelRef
  }), e.children)));
}), rc = /* @__PURE__ */ w.forwardRef((e, t) => {
  var r = et(e, Rt);
  return /* @__PURE__ */ w.createElement(lj, Dr({}, r, {
    ref: t
  }));
});
rc.displayName = "CartesianAxis";
var cj = ["x1", "y1", "x2", "y2", "key"], sj = ["offset"], fj = ["xAxisId", "yAxisId"], dj = ["xAxisId", "yAxisId"];
function Fd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Ne(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Fd(Object(r), !0).forEach(function(n) {
      hj(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Fd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function hj(e, t, r) {
  return (t = vj(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function vj(e) {
  var t = pj(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function pj(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function Or() {
  return Or = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Or.apply(null, arguments);
}
function wa(e, t) {
  if (e == null) return {};
  var r, n, i = mj(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function mj(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var yj = (e) => {
  var t = e.fill;
  if (!t || t === "none")
    return null;
  var r = e.fillOpacity, n = e.x, i = e.y, a = e.width, o = e.height, u = e.ry;
  return /* @__PURE__ */ w.createElement("rect", {
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
function Fy(e) {
  var t = e.option, r = e.lineItemProps, n;
  if (/* @__PURE__ */ w.isValidElement(t))
    n = /* @__PURE__ */ w.cloneElement(t, r);
  else if (typeof t == "function")
    n = t(r);
  else {
    var i, a = r.x1, o = r.y1, u = r.x2, l = r.y2, c = r.key, s = wa(r, cj), f = (i = Et(s)) !== null && i !== void 0 ? i : {};
    f.offset;
    var d = wa(f, sj);
    n = /* @__PURE__ */ w.createElement("line", Or({}, d, {
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
function gj(e) {
  var t = e.x, r = e.width, n = e.horizontal, i = n === void 0 ? !0 : n, a = e.horizontalPoints;
  if (!i || !a || !a.length)
    return null;
  e.xAxisId, e.yAxisId;
  var o = wa(e, fj), u = a.map((l, c) => {
    var s = Ne(Ne({}, o), {}, {
      x1: t,
      y1: l,
      x2: t + r,
      y2: l,
      key: "line-".concat(c),
      index: c
    });
    return /* @__PURE__ */ w.createElement(Fy, {
      key: "line-".concat(c),
      option: i,
      lineItemProps: s
    });
  });
  return /* @__PURE__ */ w.createElement("g", {
    className: "recharts-cartesian-grid-horizontal"
  }, u);
}
function bj(e) {
  var t = e.y, r = e.height, n = e.vertical, i = n === void 0 ? !0 : n, a = e.verticalPoints;
  if (!i || !a || !a.length)
    return null;
  e.xAxisId, e.yAxisId;
  var o = wa(e, dj), u = a.map((l, c) => {
    var s = Ne(Ne({}, o), {}, {
      x1: l,
      y1: t,
      x2: l,
      y2: t + r,
      key: "line-".concat(c),
      index: c
    });
    return /* @__PURE__ */ w.createElement(Fy, {
      option: i,
      lineItemProps: s,
      key: "line-".concat(c)
    });
  });
  return /* @__PURE__ */ w.createElement("g", {
    className: "recharts-cartesian-grid-vertical"
  }, u);
}
function wj(e) {
  var t = e.horizontalFill, r = e.fillOpacity, n = e.x, i = e.y, a = e.width, o = e.height, u = e.horizontalPoints, l = e.horizontal, c = l === void 0 ? !0 : l;
  if (!c || !t || !t.length || u == null)
    return null;
  var s = u.map((d) => Math.round(d + i - i)).sort((d, h) => d - h);
  i !== s[0] && s.unshift(0);
  var f = s.map((d, h) => {
    var v = s[h + 1], p = v == null, m = p ? i + o - d : v - d;
    if (m <= 0)
      return null;
    var y = h % t.length;
    return /* @__PURE__ */ w.createElement("rect", {
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
  return /* @__PURE__ */ w.createElement("g", {
    className: "recharts-cartesian-gridstripes-horizontal"
  }, f);
}
function xj(e) {
  var t = e.vertical, r = t === void 0 ? !0 : t, n = e.verticalFill, i = e.fillOpacity, a = e.x, o = e.y, u = e.width, l = e.height, c = e.verticalPoints;
  if (!r || !n || !n.length)
    return null;
  var s = c.map((d) => Math.round(d + a - a)).sort((d, h) => d - h);
  a !== s[0] && s.unshift(0);
  var f = s.map((d, h) => {
    var v = s[h + 1], p = v == null, m = p ? a + u - d : v - d;
    if (m <= 0)
      return null;
    var y = h % n.length;
    return /* @__PURE__ */ w.createElement("rect", {
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
  return /* @__PURE__ */ w.createElement("g", {
    className: "recharts-cartesian-gridstripes-vertical"
  }, f);
}
var Oj = (e, t) => {
  var r = e.xAxis, n = e.width, i = e.height, a = e.offset;
  return _v(tc(Ne(Ne(Ne({}, Rt), r), {}, {
    ticks: Iv(r),
    viewBox: {
      x: 0,
      y: 0,
      width: n,
      height: i
    }
  })), a.left, a.left + a.width, t);
}, Aj = (e, t) => {
  var r = e.yAxis, n = e.width, i = e.height, a = e.offset;
  return _v(tc(Ne(Ne(Ne({}, Rt), r), {}, {
    ticks: Iv(r),
    viewBox: {
      x: 0,
      y: 0,
      width: n,
      height: i
    }
  })), a.top, a.top + a.height, t);
}, Sj = {
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
  zIndex: Re.grid
};
function Wy(e) {
  var t = $v(), r = Lv(), n = Mv(), i = Ne(Ne({}, et(e, Sj)), {}, {
    x: M(e.x) ? e.x : n.left,
    y: M(e.y) ? e.y : n.top,
    width: M(e.width) ? e.width : n.width,
    height: M(e.height) ? e.height : n.height
  }), a = i.xAxisId, o = i.yAxisId, u = i.x, l = i.y, c = i.width, s = i.height, f = i.syncWithTicks, d = i.horizontalValues, h = i.verticalValues, v = Ve(), p = N((I) => Kf(I, "xAxis", a, v)), m = N((I) => Kf(I, "yAxis", o, v));
  if (!_t(c) || !_t(s) || !M(u) || !M(l))
    return null;
  var y = i.verticalCoordinatesGenerator || Oj, b = i.horizontalCoordinatesGenerator || Aj, x = i.horizontalPoints, O = i.verticalPoints;
  if ((!x || !x.length) && typeof b == "function") {
    var A = d && d.length, g = b({
      yAxis: m ? Ne(Ne({}, m), {}, {
        ticks: A ? d : m.ticks
      }) : void 0,
      width: t ?? c,
      height: r ?? s,
      offset: n
    }, A ? !0 : f);
    qi(Array.isArray(g), "horizontalCoordinatesGenerator should return Array but instead it returned [".concat(typeof g, "]")), Array.isArray(g) && (x = g);
  }
  if ((!O || !O.length) && typeof y == "function") {
    var E = h && h.length, _ = y({
      xAxis: p ? Ne(Ne({}, p), {}, {
        ticks: E ? h : p.ticks
      }) : void 0,
      width: t ?? c,
      height: r ?? s,
      offset: n
    }, E ? !0 : f);
    qi(Array.isArray(_), "verticalCoordinatesGenerator should return Array but instead it returned [".concat(typeof _, "]")), Array.isArray(_) && (O = _);
  }
  return /* @__PURE__ */ w.createElement(Jt, {
    zIndex: i.zIndex
  }, /* @__PURE__ */ w.createElement("g", {
    className: "recharts-cartesian-grid"
  }, /* @__PURE__ */ w.createElement(yj, {
    fill: i.fill,
    fillOpacity: i.fillOpacity,
    x: i.x,
    y: i.y,
    width: i.width,
    height: i.height,
    ry: i.ry
  }), /* @__PURE__ */ w.createElement(wj, Or({}, i, {
    horizontalPoints: x
  })), /* @__PURE__ */ w.createElement(xj, Or({}, i, {
    verticalPoints: O
  })), /* @__PURE__ */ w.createElement(gj, Or({}, i, {
    offset: n,
    horizontalPoints: x,
    xAxis: p,
    yAxis: m
  })), /* @__PURE__ */ w.createElement(bj, Or({}, i, {
    offset: n,
    verticalPoints: O,
    xAxis: p,
    yAxis: m
  }))));
}
Wy.displayName = "CartesianGrid";
var Ej = ["animationElapsedTime", "isAnimating", "isEntrance", "visibleLength", "strokeDasharray", "connectNulls"];
function Eu() {
  return Eu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Eu.apply(null, arguments);
}
function Pj(e, t) {
  if (e == null) return {};
  var r, n, i = _j(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function _j(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Ij(e) {
  try {
    return e && e.getTotalLength && e.getTotalLength() || 0;
  } catch {
    return 0;
  }
}
function Uy(e, t) {
  return "".concat(t, "px ").concat(e, "px");
}
function kj(e) {
  return e.length % 2 !== 0 ? [...e, ...e] : e;
}
function Cj(e, t) {
  for (var r = [], n = 0; n < t; ++n)
    r.push(...e);
  return r;
}
function Tj(e, t, r) {
  var n = kj(r), i = n.reduce((h, v) => h + v, 0);
  if (!i)
    return Uy(t, e);
  for (var a = Math.floor(e / i), o = e % i, u = [], l = 0, c = 0; l < n.length; c += (s = n[l]) !== null && s !== void 0 ? s : 0, ++l) {
    var s, f = n[l];
    if (f != null && c + f > o) {
      u = [...n.slice(0, l), o - c];
      break;
    }
  }
  var d = u.length % 2 === 0 ? [0, t] : [t];
  return [...Cj(n, a), ...u, ...d].map((h) => "".concat(h, "px")).join(", ");
}
function Dj(e, t, r) {
  if (e) {
    var n = "".concat(e).split(/[,\s]+/gim).map((i) => parseFloat(i));
    return Tj(r, t, n);
  }
  return Uy(t, r);
}
function jj(e) {
  e.animationElapsedTime, e.isAnimating, e.isEntrance;
  var t = e.visibleLength, r = e.strokeDasharray, n = e.connectNulls, i = Pj(e, Ej), a = n ?? !1, o;
  if (t != null) {
    var u, l = i.pathRef, c = Ij((u = l == null ? void 0 : l.current) !== null && u !== void 0 ? u : null);
    o = Dj(r, c, t);
  } else r != null && (o = String(r));
  return /* @__PURE__ */ w.createElement(Uv, Eu({}, i, {
    connectNulls: a,
    strokeDasharray: o
  }));
}
function Nj(e) {
  var t = V(0), r = V(0), n = V(!1), i = V(e);
  return i.current !== e && (t.current = r.current, i.current = e), ee((a, o) => {
    if (n.current)
      return null;
    var u = Math.min(jt(t.current + a * o), o);
    return a > 0 && o > 0 && (r.current = Math.max(r.current, u), u >= o) ? (n.current = !0, null) : u;
  }, []);
}
var Mj = {}, Vy = $e({
  name: "errorBars",
  initialState: Mj,
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
}), nc = Vy.actions;
nc.addErrorBar;
nc.replaceErrorBar;
nc.removeErrorBar;
var $j = Vy.reducer, Lj = ["children"];
function Rj(e, t) {
  if (e == null) return {};
  var r, n, i = zj(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function zj(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var Bj = {
  data: [],
  xAxisId: "xAxis-0",
  yAxisId: "yAxis-0",
  dataPointFormatter: () => ({
    x: 0,
    y: 0,
    value: 0
  }),
  errorBarOffset: 0
}, Fj = /* @__PURE__ */ Je(Bj);
function Wj(e) {
  var t = e.children, r = Rj(e, Lj);
  return /* @__PURE__ */ w.createElement(Fj.Provider, {
    value: r
  }, t);
}
function Ky(e, t) {
  var r, n, i = N((c) => Xt(c, e)), a = N((c) => Zt(c, t)), o = (r = i == null ? void 0 : i.allowDataOverflow) !== null && r !== void 0 ? r : pe.allowDataOverflow, u = (n = a == null ? void 0 : a.allowDataOverflow) !== null && n !== void 0 ? n : me.allowDataOverflow, l = o || u;
  return {
    needClip: l,
    needClipX: o,
    needClipY: u
  };
}
function Uj(e) {
  var t = e.xAxisId, r = e.yAxisId, n = e.clipPathId, i = ec(), a = Ky(t, r), o = a.needClipX, u = a.needClipY, l = a.needClip, c = N((x) => Tm(x, t, !1)), s = N((x) => Dm(x, r, !1));
  if (!l || !i)
    return null;
  var f = i.x, d = i.y, h = i.width, v = i.height, p = o && c ? Math.min(c[0], c[1]) : f - h / 2, m = u && s ? Math.min(s[0], s[1]) : d - v / 2, y = o && c ? Math.abs(c[1] - c[0]) : h * 2, b = u && s ? Math.abs(s[1] - s[0]) : v * 2;
  return /* @__PURE__ */ w.createElement("clipPath", {
    id: "clipPath-".concat(n)
  }, /* @__PURE__ */ w.createElement("rect", {
    x: p,
    y: m,
    width: y,
    height: b
  }));
}
var Hy = (e, t, r, n) => Bm(e, "xAxis", t, n), Gy = (e, t, r, n) => zm(e, "xAxis", t, n), Yy = (e, t, r, n) => Bm(e, "yAxis", r, n), qy = (e, t, r, n) => zm(e, "yAxis", r, n), Vj = S([ue, Hy, Yy, Gy, qy], (e, t, r, n, i) => Ht(e, "xAxis") ? Yi(t, n, !1) : Yi(r, i, !1)), Kj = (e, t, r, n, i) => i;
function Hj(e) {
  return e.type === "line";
}
var Gj = S([om, Kj], (e, t) => e.filter(Hj).find((r) => r.id === t)), Yj = S([ue, Hy, Yy, Gy, qy, Gj, Vj, Jn], (e, t, r, n, i, a, o, u) => {
  var l = u.chartData, c = u.dataStartIndex, s = u.dataEndIndex;
  if (!(a == null || t == null || r == null || n == null || i == null || n.length === 0 || i.length === 0 || o == null || e !== "horizontal" && e !== "vertical")) {
    var f = a.dataKey, d = a.data, h;
    if (d != null && d.length > 0 ? h = d : h = l == null ? void 0 : l.slice(c, s + 1), h != null)
      return mN({
        layout: e,
        xAxis: t,
        yAxis: r,
        xAxisTicks: n,
        yAxisTicks: i,
        dataKey: f,
        bandSize: o,
        displayedData: h
      });
  }
});
function qj(e) {
  var t = Ea(e), r = 3, n = 2;
  if (t != null) {
    var i = t.r, a = t.strokeWidth, o = Number(i), u = Number(a);
    return (Number.isNaN(o) || o < 0) && (o = r), (Number.isNaN(u) || u < 0) && (u = n), {
      r: o,
      strokeWidth: u
    };
  }
  return {
    r,
    strokeWidth: n
  };
}
var Xj = ["id"], Zj = ["type", "layout", "connectNulls", "needClip", "shape", "strokeDasharray"], Qj = ["activeDot", "animateNewValues", "animationBegin", "animationDuration", "animationEasing", "connectNulls", "dot", "hide", "isAnimationActive", "label", "legendType", "xAxisId", "yAxisId", "id"];
function xa() {
  return xa = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, xa.apply(null, arguments);
}
function ic(e, t) {
  if (e == null) return {};
  var r, n, i = Jj(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function Jj(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Wd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function wt(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Wd(Object(r), !0).forEach(function(n) {
      eN(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Wd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function eN(e, t, r) {
  return (t = tN(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function tN(e) {
  var t = rN(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function rN(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function nN(e) {
  try {
    return e && e.getTotalLength && e.getTotalLength() || 0;
  } catch {
    return 0;
  }
}
function iN(e) {
  var t = 0, r = 0;
  for (var n of e)
    n.status === "matched" && n.prev.x != null && n.next.x != null && (t += n.next.x - n.prev.x, r++);
  return r > 0 ? t / r : 0;
}
var aN = (e, t) => {
  if (e == null)
    return [];
  if (t === 1) return e.flatMap((u) => u.status === "removed" ? [] : [u.next]);
  var r = iN(e), n = [];
  for (var i of e)
    if (i.status === "matched")
      n.push(wt(wt({}, i.next), {}, {
        x: Nt(i.prev.x, i.next.x, t),
        y: Nt(i.prev.y, i.next.y, t)
      }));
    else if (i.status === "added")
      if (i.next.x != null) {
        var a = i.next.x - r;
        n.push(wt(wt({}, i.next), {}, {
          x: Nt(a, i.next.x, t),
          y: i.next.y
        }));
      } else
        n.push(i.next);
    else if (i.status === "removed" && i.prev.x != null) {
      var o = i.prev.x + r;
      n.push(wt(wt({}, i.prev), {}, {
        x: Nt(i.prev.x, o, t),
        y: i.prev.y
      }));
    }
  return n;
}, ac = {
  activeDot: !0,
  animateNewValues: !0,
  animationBegin: 0,
  animationDuration: 1500,
  animationEasing: "ease",
  animationInterpolateFn: aN,
  animationMatchBy: Ql,
  connectNulls: !1,
  dot: !0,
  fill: "#fff",
  hide: !1,
  isAnimationActive: "auto",
  label: !1,
  legendType: "line",
  shape: jj,
  stroke: "#3182bd",
  strokeWidth: 1,
  xAxisId: 0,
  yAxisId: 0,
  zIndex: Re.line,
  type: "linear"
}, oN = (e) => {
  var t = e.dataKey, r = e.name, n = e.stroke, i = e.legendType, a = e.hide;
  return [{
    inactive: a,
    dataKey: t,
    type: i,
    color: n,
    value: kv(r, t),
    payload: e
  }];
}, uN = /* @__PURE__ */ w.memo((e) => {
  var t = e.dataKey, r = e.data, n = e.stroke, i = e.strokeWidth, a = e.fill, o = e.name, u = e.hide, l = e.unit, c = e.formatter, s = e.tooltipType, f = e.id, d = {
    dataDefinedOnItem: r,
    getPosition: nn,
    settings: {
      stroke: n,
      strokeWidth: i,
      fill: a,
      dataKey: t,
      nameKey: void 0,
      name: kv(o, t),
      hide: u,
      type: s,
      color: n,
      unit: l,
      formatter: c,
      graphicalItemId: f
    }
  };
  return /* @__PURE__ */ w.createElement(iT, {
    tooltipEntrySettings: d
  });
});
function lN(e) {
  var t = e.clipPathId, r = e.points, n = e.props, i = n.dot, a = n.dataKey, o = n.needClip;
  n.id;
  var u = ic(n, Xj), l = Et(u);
  return /* @__PURE__ */ w.createElement(XT, {
    points: r,
    dot: i,
    className: "recharts-line-dots",
    dotClassName: "recharts-line-dot",
    dataKey: a,
    baseProps: l,
    needClip: o,
    clipPathId: t
  });
}
function cN(e) {
  var t = e.showLabels, r = e.children, n = e.points, i = Kt(() => n == null ? void 0 : n.map((a) => {
    var o, u, l = {
      x: (o = a.x) !== null && o !== void 0 ? o : 0,
      y: (u = a.y) !== null && u !== void 0 ? u : 0,
      width: 0,
      lowerWidth: 0,
      upperWidth: 0,
      height: 0
    };
    return wt(wt({}, l), {}, {
      value: a.value,
      payload: a.payload,
      viewBox: l,
      /*
       * Line is not passing parentViewBox to the LabelList so the labels can escape - looks like a bug, should we pass parentViewBox?
       * Or should this just be the root chart viewBox?
       */
      parentViewBox: void 0,
      fill: void 0
    });
  }), [n]);
  return /* @__PURE__ */ w.createElement(UC, {
    value: t ? i : void 0
  }, r);
}
function sN(e) {
  var t = e.clipPathId, r = e.pathRef, n = e.points, i = e.props, a = e.animationElapsedTime, o = e.isAnimating, u = e.isEntrance, l = e.visibleLength, c = i.type, s = i.layout, f = i.connectNulls, d = i.needClip, h = i.shape, v = i.strokeDasharray, p = ic(i, Zj), m = wt(wt({}, at(p)), {}, {
    fill: "none",
    className: "recharts-line-curve",
    clipPath: d ? "url(#clipPath-".concat(t, ")") : void 0,
    points: n,
    type: c,
    layout: s,
    connectNulls: f,
    strokeDasharray: v ?? i.strokeDasharray,
    pathRef: r,
    animationElapsedTime: a,
    isAnimating: o,
    isEntrance: i.animateNewValues ? u : !1,
    visibleLength: l
  });
  return /* @__PURE__ */ w.createElement(w.Fragment, null, (n == null ? void 0 : n.length) > 1 && /* @__PURE__ */ w.createElement(nT, {
    option: h,
    DefaultShape: ac.shape,
    shapeProps: m
  }), /* @__PURE__ */ w.createElement(lN, {
    points: n,
    clipPathId: t,
    props: i
  }));
}
function fN(e) {
  var t = e.clipPathId, r = e.props, n = e.pathRef, i = e.previousPointsRef, a = r.points, o = r.isAnimationActive, u = r.animationBegin, l = r.animationDuration, c = r.animationEasing, s = r.animationMatchBy, f = r.animationInterpolateFn, d = r.layout, h = nN(n.current), v = AT(r.onAnimationStart, r.onAnimationEnd), p = v.isAnimating, m = v.handleAnimationStart, y = v.handleAnimationEnd, b = !p, x = Nj(a), O = ee((A) => A > 0 && h > 0, [h]);
  return /* @__PURE__ */ w.createElement(cN, {
    points: a,
    showLabels: b
  }, r.children, /* @__PURE__ */ w.createElement(ST, {
    animationInput: a,
    animationIdPrefix: "recharts-line-",
    items: a,
    previousItemsRef: i,
    isAnimationActive: o,
    animationBegin: u,
    animationDuration: l,
    animationEasing: c,
    onAnimationStart: m,
    onAnimationEnd: y,
    animationInterpolateFn: f,
    animationMatchBy: s,
    shouldUpdatePreviousRef: O,
    layout: d
  }, (A, g, E) => {
    var _ = p || g < 1, I = _ ? x(g, h) : null;
    return /* @__PURE__ */ w.createElement(sN, {
      props: r,
      points: A,
      clipPathId: t,
      pathRef: n,
      animationElapsedTime: g,
      isAnimating: _,
      isEntrance: E,
      visibleLength: I
    });
  }), /* @__PURE__ */ w.createElement(HC, {
    label: r.label
  }));
}
function dN(e) {
  var t = e.clipPathId, r = e.props, n = V(null), i = V(null);
  return /* @__PURE__ */ w.createElement(fN, {
    props: r,
    clipPathId: t,
    previousPointsRef: n,
    pathRef: i
  });
}
var hN = (e, t) => {
  var r, n;
  return {
    x: (r = e.x) !== null && r !== void 0 ? r : void 0,
    y: (n = e.y) !== null && n !== void 0 ? n : void 0,
    value: e.value,
    // getValueByDataKey does not validate the output type
    errorVal: we(e.payload, t)
  };
};
class vN extends mg {
  render() {
    var t = this.props, r = t.hide, n = t.dot, i = t.points, a = t.className, o = t.xAxisId, u = t.yAxisId, l = t.top, c = t.left, s = t.width, f = t.height, d = t.id, h = t.needClip, v = t.zIndex;
    if (r)
      return null;
    var p = ie("recharts-line", a), m = d, y = qj(n), b = y.r, x = y.strokeWidth, O = Dy(n), A = b * 2 + x, g = h ? "url(#clipPath-".concat(O ? "" : "dots-").concat(m, ")") : void 0;
    return /* @__PURE__ */ w.createElement(Jt, {
      zIndex: v
    }, /* @__PURE__ */ w.createElement(zt, {
      className: p
    }, h && /* @__PURE__ */ w.createElement("defs", null, /* @__PURE__ */ w.createElement(Uj, {
      clipPathId: m,
      xAxisId: o,
      yAxisId: u
    }), !O && /* @__PURE__ */ w.createElement("clipPath", {
      id: "clipPath-dots-".concat(m)
    }, /* @__PURE__ */ w.createElement("rect", {
      x: c - A / 2,
      y: l - A / 2,
      width: s + A,
      height: f + A
    }))), /* @__PURE__ */ w.createElement(Wj, {
      xAxisId: o,
      yAxisId: u,
      data: i,
      dataPointFormatter: hN,
      errorBarOffset: 0
    }, /* @__PURE__ */ w.createElement(dN, {
      props: this.props,
      clipPathId: m
    }))), /* @__PURE__ */ w.createElement(mD, {
      activeDot: this.props.activeDot,
      points: i,
      mainColor: this.props.stroke,
      itemDataKey: this.props.dataKey,
      clipPath: g
    }));
  }
}
function pN(e) {
  var t = et(e, ac), r = t.activeDot, n = t.animateNewValues, i = t.animationBegin, a = t.animationDuration, o = t.animationEasing, u = t.connectNulls, l = t.dot, c = t.hide, s = t.isAnimationActive, f = t.label, d = t.legendType, h = t.xAxisId, v = t.yAxisId, p = t.id, m = ic(t, Qj), y = Ky(h, v), b = y.needClip, x = ec(), O = an(), A = Ve(), g = N((T) => Yj(T, h, v, A, p));
  if (O !== "horizontal" && O !== "vertical" || g == null || x == null)
    return null;
  var E = x.height, _ = x.width, I = x.x, C = x.y;
  return /* @__PURE__ */ w.createElement(vN, xa({}, m, {
    id: p,
    connectNulls: u,
    dot: l,
    activeDot: r,
    animateNewValues: n,
    animationBegin: i,
    animationDuration: a,
    animationEasing: o,
    isAnimationActive: s,
    hide: c,
    label: f,
    legendType: d,
    xAxisId: h,
    yAxisId: v,
    points: g,
    layout: O,
    height: E,
    width: _,
    left: I,
    top: C,
    needClip: b
  }));
}
function mN(e) {
  var t = e.layout, r = e.xAxis, n = e.yAxis, i = e.xAxisTicks, a = e.yAxisTicks, o = e.dataKey, u = e.bandSize, l = e.displayedData;
  return l.map((c, s) => {
    var f = we(c, o);
    if (t === "horizontal") {
      var d = Kc({
        axis: r,
        ticks: i,
        bandSize: u,
        entry: c,
        index: s
      }), h = xe(f) ? null : n.scale.map(f);
      return {
        x: d,
        y: h ?? null,
        value: f,
        payload: c
      };
    }
    var v = xe(f) ? null : r.scale.map(f), p = Kc({
      axis: n,
      ticks: a,
      bandSize: u,
      entry: c,
      index: s
    });
    return v == null || p == null ? null : {
      x: v,
      y: p,
      value: f,
      payload: c
    };
  }).filter(Boolean);
}
function yN(e) {
  var t = et(e, ac), r = Ve();
  return /* @__PURE__ */ w.createElement(NT, {
    id: t.id,
    type: "line"
  }, (n) => /* @__PURE__ */ w.createElement(w.Fragment, null, /* @__PURE__ */ w.createElement(aT, {
    legendPayload: oN(t)
  }), /* @__PURE__ */ w.createElement(uN, {
    dataKey: t.dataKey,
    data: t.data,
    stroke: t.stroke,
    strokeWidth: t.strokeWidth,
    fill: t.fill,
    name: t.name,
    hide: t.hide,
    unit: t.unit,
    formatter: t.formatter,
    tooltipType: t.tooltipType,
    id: n
  }), /* @__PURE__ */ w.createElement(FT, {
    type: "line",
    id: n,
    data: t.data,
    xAxisId: t.xAxisId,
    yAxisId: t.yAxisId,
    zAxisId: 0,
    dataKey: t.dataKey,
    hide: t.hide,
    isPanorama: r
  }), /* @__PURE__ */ w.createElement(pN, xa({}, t, {
    id: n
  }))));
}
var Xy = /* @__PURE__ */ w.memo(yN, Ua);
Xy.displayName = "Line";
var gN = ["domain", "range"], bN = ["domain", "range"];
function Ud(e, t) {
  if (e == null) return {};
  var r, n, i = wN(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function wN(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Vd(e, t) {
  return e === t ? !0 : Array.isArray(e) && e.length === 2 && Array.isArray(t) && t.length === 2 ? e[0] === t[0] && e[1] === t[1] : !1;
}
function Zy(e, t) {
  if (e === t)
    return !0;
  var r = e.domain, n = e.range, i = Ud(e, gN), a = t.domain, o = t.range, u = Ud(t, bN);
  return !Vd(r, a) || !Vd(n, o) ? !1 : Ua(i, u);
}
var xN = ["type"], ON = ["dangerouslySetInnerHTML", "ticks", "scale"], AN = ["id", "scale"];
function Pu() {
  return Pu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Pu.apply(null, arguments);
}
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
      SN(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Kd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function SN(e, t, r) {
  return (t = EN(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function EN(e) {
  var t = PN(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function PN(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function _u(e, t) {
  if (e == null) return {};
  var r, n, i = _N(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function _N(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function IN(e) {
  var t = ve(), r = V(null), n = Rv(), i = e.type, a = _u(e, xN), o = Ha(n, "xAxis", i), u = Kt(() => {
    if (o != null)
      return Hd(Hd({}, a), {}, {
        type: o
      });
  }, [a, o]);
  return Ue(() => {
    u != null && (r.current === null ? t(tD(u)) : r.current !== u && t(rD({
      prev: r.current,
      next: u
    })), r.current = u);
  }, [u, t]), Ue(() => () => {
    r.current && (t(nD(r.current)), r.current = null);
  }, [t]), null;
}
var kN = (e) => {
  var t = e.xAxisId, r = e.className, n = N(Tv), i = Ve(), a = "xAxis", o = N((d) => Rm(d, a, t, i)), u = N((d) => qP(d, t)), l = N((d) => t_(d, t)), c = N((d) => rm(d, t));
  if (u == null || l == null || c == null)
    return null;
  e.dangerouslySetInnerHTML, e.ticks, e.scale;
  var s = _u(e, ON);
  c.id, c.scale;
  var f = _u(c, AN);
  return /* @__PURE__ */ w.createElement(rc, Pu({}, s, f, {
    x: l.x,
    y: l.y,
    width: u.width,
    height: u.height,
    className: ie("recharts-".concat(a, " ").concat(a), r),
    viewBox: n,
    ticks: o,
    axisType: a,
    axisId: t
  }));
}, CN = {
  allowDataOverflow: pe.allowDataOverflow,
  allowDecimals: pe.allowDecimals,
  allowDuplicatedCategory: pe.allowDuplicatedCategory,
  angle: pe.angle,
  axisLine: Rt.axisLine,
  height: pe.height,
  hide: !1,
  includeHidden: pe.includeHidden,
  interval: pe.interval,
  label: !1,
  minTickGap: pe.minTickGap,
  mirror: pe.mirror,
  orientation: pe.orientation,
  padding: pe.padding,
  reversed: pe.reversed,
  scale: pe.scale,
  tick: pe.tick,
  tickCount: pe.tickCount,
  tickLine: Rt.tickLine,
  tickSize: Rt.tickSize,
  type: pe.type,
  niceTicks: pe.niceTicks,
  xAxisId: 0
}, TN = (e) => {
  var t = et(e, CN);
  return /* @__PURE__ */ w.createElement(w.Fragment, null, /* @__PURE__ */ w.createElement(IN, {
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
  }), /* @__PURE__ */ w.createElement(kN, t));
}, Qy = /* @__PURE__ */ w.memo(TN, Zy);
Qy.displayName = "XAxis";
var DN = ["type"], jN = ["dangerouslySetInnerHTML", "ticks", "scale"], NN = ["id", "scale"];
function Iu() {
  return Iu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Iu.apply(null, arguments);
}
function Gd(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function Yd(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? Gd(Object(r), !0).forEach(function(n) {
      MN(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : Gd(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function MN(e, t, r) {
  return (t = $N(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function $N(e) {
  var t = LN(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function LN(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function ku(e, t) {
  if (e == null) return {};
  var r, n, i = RN(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function RN(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function zN(e) {
  var t = ve(), r = V(null), n = Rv(), i = e.type, a = ku(e, DN), o = Ha(n, "yAxis", i), u = Kt(() => {
    if (o != null)
      return Yd(Yd({}, a), {}, {
        type: o
      });
  }, [o, a]);
  return Ue(() => {
    u != null && (r.current === null ? t(iD(u)) : r.current !== u && t(aD({
      prev: r.current,
      next: u
    })), r.current = u);
  }, [u, t]), Ue(() => () => {
    r.current && (t(oD(r.current)), r.current = null);
  }, [t]), null;
}
function BN(e) {
  var t = e.yAxisId, r = e.className, n = e.width, i = e.label, a = V(null), o = V(null), u = N(Tv), l = Ve(), c = ve(), s = "yAxis", f = N((y) => i_(y, t)), d = N((y) => n_(y, t)), h = N((y) => Rm(y, s, t, l)), v = N((y) => nm(y, t));
  if (Ue(() => {
    if (!(n !== "auto" || !f || Zl(i) || /* @__PURE__ */ Ze(i) || v == null)) {
      var y = a.current;
      if (y) {
        var b = y.getCalculatedWidth();
        Math.round(f.width) !== Math.round(b) && c(uD({
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
    v
  ]), f == null || d == null || v == null)
    return null;
  e.dangerouslySetInnerHTML, e.ticks, e.scale;
  var p = ku(e, jN);
  v.id, v.scale;
  var m = ku(v, NN);
  return /* @__PURE__ */ w.createElement(rc, Iu({}, p, m, {
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
    className: ie("recharts-".concat(s, " ").concat(s), r),
    viewBox: u,
    ticks: h,
    axisType: s,
    axisId: t
  }));
}
var FN = {
  allowDataOverflow: me.allowDataOverflow,
  allowDecimals: me.allowDecimals,
  allowDuplicatedCategory: me.allowDuplicatedCategory,
  angle: me.angle,
  axisLine: Rt.axisLine,
  hide: !1,
  includeHidden: me.includeHidden,
  interval: me.interval,
  label: !1,
  minTickGap: me.minTickGap,
  mirror: me.mirror,
  orientation: me.orientation,
  padding: me.padding,
  reversed: me.reversed,
  scale: me.scale,
  tick: me.tick,
  tickCount: me.tickCount,
  tickLine: Rt.tickLine,
  tickSize: Rt.tickSize,
  type: me.type,
  niceTicks: me.niceTicks,
  width: me.width,
  yAxisId: 0
}, WN = (e) => {
  var t = et(e, FN);
  return /* @__PURE__ */ w.createElement(w.Fragment, null, /* @__PURE__ */ w.createElement(zN, {
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
  }), /* @__PURE__ */ w.createElement(BN, t));
}, Jy = /* @__PURE__ */ w.memo(WN, Zy);
Jy.displayName = "YAxis";
var UN = (e, t) => t, oc = S([UN, ue, mp, Ae, ny, Qt, _I, Ce], NI);
function VN(e) {
  return "getBBox" in e.currentTarget && typeof e.currentTarget.getBBox == "function";
}
function uc(e) {
  var t = e.currentTarget.getBoundingClientRect(), r, n;
  if (VN(e)) {
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
var eg = Qe("mouseClick"), tg = Yn();
tg.startListening({
  actionCreator: eg,
  effect: (e, t) => {
    var r = e.payload, n = oc(t.getState(), uc(r));
    (n == null ? void 0 : n.activeIndex) != null && t.dispatch(b_({
      activeIndex: n.activeIndex,
      activeDataKey: void 0,
      activeCoordinate: n.activeCoordinate
    }));
  }
});
var Cu = Qe("mouseMove"), rg = Yn(), Wr = null, dr = null, Ro = null;
rg.startListening({
  actionCreator: Cu,
  effect: (e, t) => {
    var r = e.payload, n = t.getState(), i = n.eventSettings, a = i.throttleDelay, o = i.throttledEvents, u = o === "all" || (o == null ? void 0 : o.includes("mousemove"));
    Wr !== null && (cancelAnimationFrame(Wr), Wr = null), dr !== null && (typeof a != "number" || !u) && (clearTimeout(dr), dr = null), Ro = uc(r);
    var l = () => {
      var c = t.getState(), s = li(c, c.tooltip.settings.shared);
      if (!Ro) {
        Wr = null, dr = null;
        return;
      }
      if (s === "axis") {
        var f = oc(c, Ro);
        (f == null ? void 0 : f.activeIndex) != null ? t.dispatch(Gm({
          activeIndex: f.activeIndex,
          activeDataKey: void 0,
          activeCoordinate: f.activeCoordinate
        })) : t.dispatch(Hm());
      }
      Wr = null, dr = null;
    };
    if (!u) {
      l();
      return;
    }
    a === "raf" ? Wr = requestAnimationFrame(l) : typeof a == "number" && dr === null && (dr = setTimeout(l, a));
  }
});
function KN(e, t) {
  return t instanceof HTMLElement ? "HTMLElement <".concat(t.tagName, ' class="').concat(t.className, '">') : t === window ? "global.window" : e === "children" && typeof t == "object" && t !== null ? "<<CHILDREN>>" : t;
}
var qd = {
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
}, ng = $e({
  name: "rootProps",
  initialState: qd,
  reducers: {
    updateOptions: (e, t) => {
      var r;
      e.accessibilityLayer = t.payload.accessibilityLayer, e.barCategoryGap = t.payload.barCategoryGap, e.barGap = (r = t.payload.barGap) !== null && r !== void 0 ? r : qd.barGap, e.barSize = t.payload.barSize, e.maxBarSize = t.payload.maxBarSize, e.stackOffset = t.payload.stackOffset, e.syncId = t.payload.syncId, e.syncMethod = t.payload.syncMethod, e.className = t.payload.className, e.baseValue = t.payload.baseValue, e.reverseStackOrder = t.payload.reverseStackOrder;
    }
  }
}), HN = ng.reducer, GN = ng.actions.updateOptions, YN = null, qN = {
  updatePolarOptions: (e, t) => e === null ? t.payload : (e.startAngle = t.payload.startAngle, e.endAngle = t.payload.endAngle, e.cx = t.payload.cx, e.cy = t.payload.cy, e.innerRadius = t.payload.innerRadius, e.outerRadius = t.payload.outerRadius, e)
}, ig = $e({
  name: "polarOptions",
  initialState: YN,
  reducers: qN
});
ig.actions.updatePolarOptions;
var XN = ig.reducer, ag = Qe("keyDown"), og = Qe("focus"), ug = Qe("blur"), uo = Yn(), Ur = null, hr = null, _i = null;
uo.startListening({
  actionCreator: ag,
  effect: (e, t) => {
    _i = e.payload, Ur !== null && (cancelAnimationFrame(Ur), Ur = null);
    var r = t.getState(), n = r.eventSettings, i = n.throttleDelay, a = n.throttledEvents, o = a === "all" || a.includes("keydown");
    hr !== null && (typeof i != "number" || !o) && (clearTimeout(hr), hr = null);
    var u = () => {
      try {
        var l = t.getState(), c = l.rootProps.accessibilityLayer !== !1;
        if (!c)
          return;
        var s = l.tooltip.keyboardInteraction, f = _i;
        if (f !== "ArrowRight" && f !== "ArrowLeft" && f !== "Enter")
          return;
        var d = Cn(s, Tr(l), Jr(l), en(l)), h = d == null ? -1 : Number(d), v = !Number.isFinite(h) || h < 0, p = Qt(l), m = Tr(l), y = li(l, l.tooltip.settings.shared);
        if (f === "Enter") {
          if (v)
            return;
          var b = ma(l, y, "hover", String(s.index));
          t.dispatch(pa({
            active: !s.active,
            activeIndex: s.index,
            activeCoordinate: b
          }));
          return;
        }
        var x = c_(l), O = x === "left-to-right" ? 1 : -1, A = f === "ArrowRight" ? 1 : -1, g;
        if (v) {
          var E = Jr(l), _ = en(l), I = A * O, C = (Q) => ({
            active: !1,
            index: String(Q),
            dataKey: void 0,
            graphicalItemId: void 0,
            coordinate: void 0
          });
          if (g = -1, I > 0) {
            for (var T = 0; T < m.length; T++)
              if (Cn(C(T), m, E, _) != null) {
                g = T;
                break;
              }
          } else
            for (var P = m.length - 1; P >= 0; P--)
              if (Cn(C(P), m, E, _) != null) {
                g = P;
                break;
              }
          if (g < 0)
            return;
        } else {
          g = h + A * O;
          var z = (p == null ? void 0 : p.length) || m.length;
          if (z === 0 || g >= z || g < 0)
            return;
        }
        var $ = ma(l, y, "hover", String(g));
        t.dispatch(pa({
          active: !0,
          activeIndex: g.toString(),
          activeCoordinate: $
        }));
      } finally {
        Ur = null, hr = null;
      }
    };
    if (!o) {
      u();
      return;
    }
    i === "raf" ? Ur = requestAnimationFrame(u) : typeof i == "number" && hr === null && (u(), _i = null, hr = setTimeout(() => {
      _i ? u() : (hr = null, Ur = null);
    }, i));
  }
});
uo.startListening({
  actionCreator: og,
  effect: (e, t) => {
    var r = t.getState(), n = r.rootProps.accessibilityLayer !== !1;
    if (n) {
      var i = r.tooltip.keyboardInteraction;
      if (!i.active && i.index == null) {
        var a = "0", o = li(r, r.tooltip.settings.shared), u = ma(r, o, "hover", String(a));
        t.dispatch(pa({
          active: !0,
          activeIndex: a,
          activeCoordinate: u
        }));
      }
    }
  }
});
uo.startListening({
  actionCreator: ug,
  effect: (e, t) => {
    var r = t.getState(), n = r.rootProps.accessibilityLayer !== !1;
    if (n) {
      var i = r.tooltip.keyboardInteraction;
      i.active && t.dispatch(pa({
        active: !1,
        activeIndex: i.index,
        activeCoordinate: i.coordinate
      }));
    }
  }
});
function lg(e) {
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
var tt = Qe("externalEvent"), cg = Yn(), Ii = /* @__PURE__ */ new Map(), En = /* @__PURE__ */ new Map(), zo = /* @__PURE__ */ new Map();
cg.startListening({
  actionCreator: tt,
  effect: (e, t) => {
    var r = e.payload, n = r.handler, i = r.reactEvent;
    if (n != null) {
      var a = i.type, o = lg(i);
      zo.set(a, {
        handler: n,
        reactEvent: o
      });
      var u = Ii.get(a);
      u !== void 0 && (cancelAnimationFrame(u), Ii.delete(a));
      var l = t.getState(), c = l.eventSettings, s = c.throttleDelay, f = c.throttledEvents, d = f, h = d === "all" || (d == null ? void 0 : d.includes(a)), v = En.get(a);
      v !== void 0 && (typeof s != "number" || !h) && (clearTimeout(v), En.delete(a));
      var p = () => {
        var b = zo.get(a);
        try {
          if (!b)
            return;
          var x = b.handler, O = b.reactEvent, A = t.getState(), g = {
            activeCoordinate: fI(A),
            activeDataKey: lI(A),
            activeIndex: Wn(A),
            activeLabel: oy(A),
            activeTooltipIndex: Wn(A),
            isTooltipActive: dI(A)
          };
          x && x(g, O);
        } finally {
          Ii.delete(a), En.delete(a), zo.delete(a);
        }
      };
      if (!h) {
        p();
        return;
      }
      if (s === "raf") {
        var m = requestAnimationFrame(p);
        Ii.set(a, m);
      } else if (typeof s == "number") {
        if (!En.has(a)) {
          p();
          var y = setTimeout(p, s);
          En.set(a, y);
        }
      } else
        p();
    }
  }
});
var ZN = S([hn], (e) => e.tooltipItemPayloads), QN = S([ZN, (e, t) => t, (e, t, r) => r], (e, t, r) => {
  if (t != null) {
    var n = e.find((a) => a.settings.graphicalItemId === r);
    if (n != null) {
      var i = n.getPosition;
      if (i != null)
        return i(t);
    }
  }
}), sg = Qe("touchMove"), fg = Yn(), vr = null, er = null, Xd = null, Pn = null;
fg.startListening({
  actionCreator: sg,
  effect: (e, t) => {
    var r = e.payload;
    if (!(r.touches == null || r.touches.length === 0)) {
      Pn = lg(r);
      var n = t.getState(), i = n.eventSettings, a = i.throttleDelay, o = i.throttledEvents, u = o === "all" || o.includes("touchmove");
      vr !== null && (cancelAnimationFrame(vr), vr = null), er !== null && (typeof a != "number" || !u) && (clearTimeout(er), er = null), Xd = Array.from(r.touches).map((c) => uc({
        clientX: c.clientX,
        clientY: c.clientY,
        currentTarget: r.currentTarget
      }));
      var l = () => {
        if (Pn != null) {
          var c = t.getState(), s = li(c, c.tooltip.settings.shared);
          if (s === "axis") {
            var f, d = (f = Xd) === null || f === void 0 ? void 0 : f[0];
            if (d == null) {
              vr = null, er = null;
              return;
            }
            var h = oc(c, d);
            (h == null ? void 0 : h.activeIndex) != null && t.dispatch(Gm({
              activeIndex: h.activeIndex,
              activeDataKey: void 0,
              activeCoordinate: h.activeCoordinate
            }));
          } else if (s === "item") {
            var v, p = Pn.touches[0];
            if (document.elementFromPoint == null || p == null)
              return;
            var m = document.elementFromPoint(p.clientX, p.clientY);
            if (!m || !m.getAttribute)
              return;
            var y = m.getAttribute(nx), b = (v = m.getAttribute(ix)) !== null && v !== void 0 ? v : void 0, x = $r(c).find((g) => g.id === b);
            if (y == null || x == null || b == null)
              return;
            var O = x.dataKey, A = QN(c, y, b);
            t.dispatch(g_({
              activeDataKey: O,
              activeIndex: y,
              activeCoordinate: A,
              activeGraphicalItemId: b
            }));
          }
          vr = null, er = null;
        }
      };
      if (!u) {
        l();
        return;
      }
      a === "raf" ? vr = requestAnimationFrame(l) : typeof a == "number" && er === null && (l(), Pn = null, er = setTimeout(() => {
        Pn ? l() : (er = null, vr = null);
      }, a));
    }
  }
});
var dg = {
  throttleDelay: "raf",
  throttledEvents: ["mousemove", "touchmove", "pointermove", "scroll", "wheel"]
}, hg = $e({
  name: "eventSettings",
  initialState: dg,
  reducers: {
    setEventSettings: (e, t) => {
      t.payload.throttleDelay != null && (e.throttleDelay = t.payload.throttleDelay), t.payload.throttledEvents != null && (e.throttledEvents = G(t.payload.throttledEvents));
    }
  }
}), JN = hg.actions.setEventSettings, eM = hg.reducer, tM = Yh({
  brush: gD,
  cartesianAxis: lD,
  chartData: sk,
  errorBars: $j,
  eventSettings: eM,
  graphicalItems: zT,
  layout: Ww,
  legend: Vx,
  options: ak,
  polarAxis: YC,
  polarOptions: XN,
  referenceElements: OD,
  renderedTicks: VD,
  rootProps: HN,
  tooltip: w_,
  zIndex: GI
}), rM = function(t) {
  var r = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : "Chart";
  return hw({
    reducer: tM,
    // redux-toolkit v1 types are unhappy with the preloadedState type. Remove the `as any` when bumping to v2
    preloadedState: t,
    // @ts-expect-error redux-toolkit v1 types are unhappy with the middleware array. Remove this comment when bumping to v2
    middleware: (n) => {
      var i;
      return n({
        serializableCheck: !1,
        immutableCheck: !["commonjs", "es6", "production"].includes((i = "es6") !== null && i !== void 0 ? i : "")
      }).concat([tg.middleware, rg.middleware, uo.middleware, cg.middleware, fg.middleware]);
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
      return typeof n == "function" && (i = n()), i.concat(hv({
        type: "raf"
      }));
    },
    devTools: {
      serialize: {
        replacer: KN
      },
      name: "recharts-".concat(r)
    }
  });
};
function nM(e) {
  var t = e.preloadedState, r = e.children, n = e.reduxStoreName, i = Ve(), a = V(null);
  if (i)
    return r;
  a.current == null && (a.current = rM(t, n));
  var o = Fu;
  return /* @__PURE__ */ w.createElement(oO, {
    context: o,
    store: a.current
  }, r);
}
function iM(e) {
  var t = e.layout, r = e.margin, n = ve(), i = Ve();
  return he(() => {
    i || (n(zw(t)), n(Rw(r)));
  }, [n, i, t, r]), null;
}
var aM = /* @__PURE__ */ ju(iM, Ua);
function oM(e) {
  var t = ve();
  return he(() => {
    t(GN(e));
  }, [t, e]), null;
}
var uM = (e) => {
  var t = ve();
  return he(() => {
    t(JN(e));
  }, [t, e]), null;
}, lM = /* @__PURE__ */ ju(uM, Ua);
function Zd(e) {
  var t = e.zIndex, r = e.isPanorama, n = V(null), i = ve();
  return Ue(() => (n.current && i(KI({
    zIndex: t,
    element: n.current,
    isPanorama: r
  })), () => {
    i(HI({
      zIndex: t,
      isPanorama: r
    }));
  }), [i, t, r]), /* @__PURE__ */ w.createElement("g", {
    tabIndex: -1,
    ref: n,
    className: "recharts-zIndex-layer_".concat(t)
  });
}
function Qd(e) {
  var t = e.children, r = e.isPanorama, n = N($I);
  if (!n || n.length === 0)
    return t;
  var i = n.filter((o) => o < 0), a = n.filter((o) => o > 0);
  return /* @__PURE__ */ w.createElement(w.Fragment, null, i.map((o) => /* @__PURE__ */ w.createElement(Zd, {
    key: o,
    zIndex: o,
    isPanorama: r
  })), t, a.map((o) => /* @__PURE__ */ w.createElement(Zd, {
    key: o,
    zIndex: o,
    isPanorama: r
  })));
}
var cM = ["children"];
function sM(e, t) {
  if (e == null) return {};
  var r, n, i = fM(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function fM(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
function Oa() {
  return Oa = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Oa.apply(null, arguments);
}
var dM = {
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
}, hM = /* @__PURE__ */ Me((e, t) => {
  var r = $v(), n = Lv(), i = Wv();
  if (!_t(r) || !_t(n))
    return null;
  var a = e.children, o = e.otherAttributes, u = e.title, l = e.desc, c, s;
  return o != null && (typeof o.tabIndex == "number" ? c = o.tabIndex : c = i ? 0 : void 0, typeof o.role == "string" ? s = o.role : s = i ? "application" : void 0), /* @__PURE__ */ w.createElement(fh, Oa({}, o, {
    title: u,
    desc: l,
    role: s,
    tabIndex: c,
    width: r,
    height: n,
    style: dM,
    ref: t
  }), a);
}), vM = (e) => {
  var t = e.children, r = N(Fa);
  if (!r)
    return null;
  var n = r.width, i = r.height, a = r.y, o = r.x;
  return /* @__PURE__ */ w.createElement(fh, {
    width: n,
    height: i,
    x: o,
    y: a
  }, t);
}, Jd = /* @__PURE__ */ Me((e, t) => {
  var r = e.children, n = sM(e, cM), i = Ve();
  return i ? /* @__PURE__ */ w.createElement(vM, null, /* @__PURE__ */ w.createElement(Qd, {
    isPanorama: !0
  }, r)) : /* @__PURE__ */ w.createElement(hM, Oa({
    ref: t
  }, n), /* @__PURE__ */ w.createElement(Qd, {
    isPanorama: !1
  }, r));
});
function pM(e, t) {
  return bM(e) || gM(e, t) || yM(e, t) || mM();
}
function mM() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function yM(e, t) {
  if (e) {
    if (typeof e == "string") return eh(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? eh(e, t) : void 0;
  }
}
function eh(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function gM(e, t) {
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
function bM(e) {
  if (Array.isArray(e)) return e;
}
function wM() {
  var e = ve(), t = fe(null), r = pM(t, 2), n = r[0], i = r[1], a = N(rx);
  return he(() => {
    if (n != null) {
      var o = n.getBoundingClientRect(), u = o.width / n.offsetWidth;
      Y(u) && u !== a && e(Fw(u));
    }
  }, [n, e, a]), i;
}
function th(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function xM(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? th(Object(r), !0).forEach(function(n) {
      OM(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : th(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function OM(e, t, r) {
  return (t = AM(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function AM(e) {
  var t = SM(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function SM(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
function or() {
  return or = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, or.apply(null, arguments);
}
function Aa(e, t) {
  return IM(e) || _M(e, t) || PM(e, t) || EM();
}
function EM() {
  throw new TypeError(`Invalid attempt to destructure non-iterable instance.
In order to be iterable, non-array objects must have a [Symbol.iterator]() method.`);
}
function PM(e, t) {
  if (e) {
    if (typeof e == "string") return rh(e, t);
    var r = {}.toString.call(e).slice(8, -1);
    return r === "Object" && e.constructor && (r = e.constructor.name), r === "Map" || r === "Set" ? Array.from(e) : r === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(r) ? rh(e, t) : void 0;
  }
}
function rh(e, t) {
  (t == null || t > e.length) && (t = e.length);
  for (var r = 0, n = Array(t); r < t; r++) n[r] = e[r];
  return n;
}
function _M(e, t) {
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
function IM(e) {
  if (Array.isArray(e)) return e;
}
var kM = () => (bk(), null);
function Sa(e) {
  if (typeof e == "number")
    return e;
  if (typeof e == "string") {
    var t = parseFloat(e);
    if (!Number.isNaN(t))
      return t;
  }
  return 0;
}
var CM = /* @__PURE__ */ Me((e, t) => {
  var r, n, i = V(null), a = fe({
    containerWidth: Sa((r = e.style) === null || r === void 0 ? void 0 : r.width),
    containerHeight: Sa((n = e.style) === null || n === void 0 ? void 0 : n.height)
  }), o = Aa(a, 2), u = o[0], l = o[1], c = ee((f, d) => {
    l((h) => {
      var v = Math.round(f), p = Math.round(d);
      return h.containerWidth === v && h.containerHeight === p ? h : {
        containerWidth: v,
        containerHeight: p
      };
    });
  }, []), s = ee((f) => {
    if (typeof t == "function" && t(f), i.current != null && (i.current.disconnect(), i.current = null), f != null && typeof ResizeObserver < "u") {
      var d = f.getBoundingClientRect(), h = d.width, v = d.height;
      c(h, v);
      var p = (y) => {
        var b = y[0];
        if (b != null) {
          var x = b.contentRect, O = x.width, A = x.height;
          c(O, A);
        }
      }, m = new ResizeObserver(p);
      m.observe(f), i.current = m;
    }
  }, [t, c]);
  return he(() => () => {
    var f = i.current;
    f != null && f.disconnect();
  }, [c]), /* @__PURE__ */ w.createElement(w.Fragment, null, /* @__PURE__ */ w.createElement(Xn, {
    width: u.containerWidth,
    height: u.containerHeight
  }), /* @__PURE__ */ w.createElement("div", or({
    ref: s
  }, e)));
}), TM = /* @__PURE__ */ Me((e, t) => {
  var r = e.width, n = e.height, i = fe({
    containerWidth: Sa(r),
    containerHeight: Sa(n)
  }), a = Aa(i, 2), o = a[0], u = a[1], l = ee((s, f) => {
    u((d) => {
      var h = Math.round(s), v = Math.round(f);
      return d.containerWidth === h && d.containerHeight === v ? d : {
        containerWidth: h,
        containerHeight: v
      };
    });
  }, []), c = ee((s) => {
    if (typeof t == "function" && t(s), s != null) {
      var f = s.getBoundingClientRect(), d = f.width, h = f.height;
      l(d, h);
    }
  }, [t, l]);
  return /* @__PURE__ */ w.createElement(w.Fragment, null, /* @__PURE__ */ w.createElement(Xn, {
    width: o.containerWidth,
    height: o.containerHeight
  }), /* @__PURE__ */ w.createElement("div", or({
    ref: c
  }, e)));
}), DM = /* @__PURE__ */ Me((e, t) => {
  var r = e.width, n = e.height;
  return /* @__PURE__ */ w.createElement(w.Fragment, null, /* @__PURE__ */ w.createElement(Xn, {
    width: r,
    height: n
  }), /* @__PURE__ */ w.createElement("div", or({
    ref: t
  }, e)));
}), jM = /* @__PURE__ */ Me((e, t) => {
  var r = e.width, n = e.height;
  return typeof r == "string" || typeof n == "string" ? /* @__PURE__ */ w.createElement(TM, or({}, e, {
    ref: t
  })) : typeof r == "number" && typeof n == "number" ? /* @__PURE__ */ w.createElement(DM, or({}, e, {
    width: r,
    height: n,
    ref: t
  })) : /* @__PURE__ */ w.createElement(w.Fragment, null, /* @__PURE__ */ w.createElement(Xn, {
    width: r,
    height: n
  }), /* @__PURE__ */ w.createElement("div", or({
    ref: t
  }, e)));
});
function NM(e) {
  return e ? CM : jM;
}
var MM = /* @__PURE__ */ Me((e, t) => {
  var r = e.children, n = e.className, i = e.height, a = e.onClick, o = e.onContextMenu, u = e.onDoubleClick, l = e.onMouseDown, c = e.onMouseEnter, s = e.onMouseLeave, f = e.onMouseMove, d = e.onMouseUp, h = e.onTouchEnd, v = e.onTouchMove, p = e.onTouchStart, m = e.style, y = e.width, b = e.responsive, x = e.dispatchTouchEvents, O = x === void 0 ? !0 : x, A = V(null), g = ve(), E = fe(null), _ = Aa(E, 2), I = _[0], C = _[1], T = fe(null), P = Aa(T, 2), z = P[0], $ = P[1], Q = wM(), K = Yu(), H = (K == null ? void 0 : K.width) > 0 ? K.width : y, B = (K == null ? void 0 : K.height) > 0 ? K.height : i, X = ee((L) => {
    Q(L), typeof t == "function" && t(L), C(L), $(L), L != null && (A.current = L);
  }, [Q, t, C, $]), W = ee((L) => {
    g(eg(L)), g(tt({
      handler: a,
      reactEvent: L
    }));
  }, [g, a]), Te = ee((L) => {
    g(Cu(L)), g(tt({
      handler: c,
      reactEvent: L
    }));
  }, [g, c]), Ee = ee((L) => {
    g(Hm()), g(tt({
      handler: s,
      reactEvent: L
    }));
  }, [g, s]), se = ee((L) => {
    g(Cu(L)), g(tt({
      handler: f,
      reactEvent: L
    }));
  }, [g, f]), Ke = ee(() => {
    g(og());
  }, [g]), He = ee(() => {
    g(ug());
  }, [g]), ct = ee((L) => {
    g(ag(L.key));
  }, [g]), st = ee((L) => {
    g(tt({
      handler: o,
      reactEvent: L
    }));
  }, [g, o]), mn = ee((L) => {
    g(tt({
      handler: u,
      reactEvent: L
    }));
  }, [g, u]), D = ee((L) => {
    g(tt({
      handler: l,
      reactEvent: L
    }));
  }, [g, l]), F = ee((L) => {
    g(tt({
      handler: d,
      reactEvent: L
    }));
  }, [g, d]), R = ee((L) => {
    g(tt({
      handler: p,
      reactEvent: L
    }));
  }, [g, p]), k = ee((L) => {
    O && g(sg(L)), g(tt({
      handler: v,
      reactEvent: L
    }));
  }, [g, O, v]), De = ee((L) => {
    g(tt({
      handler: h,
      reactEvent: L
    }));
  }, [g, h]), J = NM(b);
  return /* @__PURE__ */ w.createElement(hy.Provider, {
    value: I
  }, /* @__PURE__ */ w.createElement(Cg.Provider, {
    value: z
  }, /* @__PURE__ */ w.createElement(J, {
    width: H ?? (m == null ? void 0 : m.width),
    height: B ?? (m == null ? void 0 : m.height),
    className: ie("recharts-wrapper", n),
    style: xM({
      position: "relative",
      cursor: "default",
      width: H,
      height: B
    }, m),
    onClick: W,
    onContextMenu: st,
    onDoubleClick: mn,
    onFocus: Ke,
    onBlur: He,
    onKeyDown: ct,
    onMouseDown: D,
    onMouseEnter: Te,
    onMouseLeave: Ee,
    onMouseMove: se,
    onMouseUp: F,
    onTouchEnd: De,
    onTouchMove: k,
    onTouchStart: R,
    ref: X
  }, /* @__PURE__ */ w.createElement(kM, null), r)));
}), $M = ["width", "height", "responsive", "children", "className", "style", "compact", "title", "desc"];
function LM(e, t) {
  if (e == null) return {};
  var r, n, i = RM(e, t);
  if (Object.getOwnPropertySymbols) {
    var a = Object.getOwnPropertySymbols(e);
    for (n = 0; n < a.length; n++) r = a[n], t.indexOf(r) === -1 && {}.propertyIsEnumerable.call(e, r) && (i[r] = e[r]);
  }
  return i;
}
function RM(e, t) {
  if (e == null) return {};
  var r = {};
  for (var n in e) if ({}.hasOwnProperty.call(e, n)) {
    if (t.indexOf(n) !== -1) continue;
    r[n] = e[n];
  }
  return r;
}
var zM = /* @__PURE__ */ Me((e, t) => {
  var r = e.width, n = e.height, i = e.responsive, a = e.children, o = e.className, u = e.style, l = e.compact, c = e.title, s = e.desc, f = LM(e, $M), d = Et(f);
  return l ? /* @__PURE__ */ w.createElement(w.Fragment, null, /* @__PURE__ */ w.createElement(Xn, {
    width: r,
    height: n
  }), /* @__PURE__ */ w.createElement(Jd, {
    otherAttributes: d,
    title: c,
    desc: s
  }, a)) : /* @__PURE__ */ w.createElement(MM, {
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
  }, /* @__PURE__ */ w.createElement(Jd, {
    otherAttributes: d,
    title: c,
    desc: s,
    ref: t
  }, /* @__PURE__ */ w.createElement(kD, null, a)));
});
function Tu() {
  return Tu = Object.assign ? Object.assign.bind() : function(e) {
    for (var t = 1; t < arguments.length; t++) {
      var r = arguments[t];
      for (var n in r) ({}).hasOwnProperty.call(r, n) && (e[n] = r[n]);
    }
    return e;
  }, Tu.apply(null, arguments);
}
function nh(e, t) {
  var r = Object.keys(e);
  if (Object.getOwnPropertySymbols) {
    var n = Object.getOwnPropertySymbols(e);
    t && (n = n.filter(function(i) {
      return Object.getOwnPropertyDescriptor(e, i).enumerable;
    })), r.push.apply(r, n);
  }
  return r;
}
function BM(e) {
  for (var t = 1; t < arguments.length; t++) {
    var r = arguments[t] != null ? arguments[t] : {};
    t % 2 ? nh(Object(r), !0).forEach(function(n) {
      FM(e, n, r[n]);
    }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(r)) : nh(Object(r)).forEach(function(n) {
      Object.defineProperty(e, n, Object.getOwnPropertyDescriptor(r, n));
    });
  }
  return e;
}
function FM(e, t, r) {
  return (t = WM(t)) in e ? Object.defineProperty(e, t, { value: r, enumerable: !0, configurable: !0, writable: !0 }) : e[t] = r, e;
}
function WM(e) {
  var t = UM(e, "string");
  return typeof t == "symbol" ? t : t + "";
}
function UM(e, t) {
  if (typeof e != "object" || !e) return e;
  var r = e[Symbol.toPrimitive];
  if (r !== void 0) {
    var n = r.call(e, t);
    if (typeof n != "object") return n;
    throw new TypeError("@@toPrimitive must return a primitive value.");
  }
  return (t === "string" ? String : Number)(e);
}
var VM = {
  top: 5,
  right: 5,
  bottom: 5,
  left: 5
}, KM = BM({
  accessibilityLayer: !0,
  barCategoryGap: "10%",
  barGap: 4,
  layout: "horizontal",
  margin: VM,
  responsive: !1,
  reverseStackOrder: !1,
  stackOffset: "none",
  syncMethod: "index"
}, dg), HM = /* @__PURE__ */ Me(function(t, r) {
  var n, i = et(t.categoricalChartProps, KM), a = t.chartName, o = t.defaultTooltipEventType, u = t.validateTooltipEventTypes, l = t.tooltipPayloadSearcher, c = t.categoricalChartProps, s = {
    chartName: a,
    defaultTooltipEventType: o,
    validateTooltipEventTypes: u,
    tooltipPayloadSearcher: l,
    eventEmitter: void 0
  };
  return /* @__PURE__ */ w.createElement(nM, {
    preloadedState: {
      options: s
    },
    reduxStoreName: (n = c.id) !== null && n !== void 0 ? n : a
  }, /* @__PURE__ */ w.createElement(yD, {
    chartData: c.data
  }), /* @__PURE__ */ w.createElement(aM, {
    layout: i.layout,
    margin: i.margin
  }), /* @__PURE__ */ w.createElement(lM, {
    throttleDelay: i.throttleDelay,
    throttledEvents: i.throttledEvents
  }), /* @__PURE__ */ w.createElement(oM, {
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
  }), /* @__PURE__ */ w.createElement(zM, Tu({}, i, {
    ref: r
  })));
}), GM = ["axis"], YM = /* @__PURE__ */ Me((e, t) => /* @__PURE__ */ w.createElement(HM, {
  chartName: "LineChart",
  defaultTooltipEventType: "axis",
  validateTooltipEventTypes: GM,
  tooltipPayloadSearcher: nk,
  categoricalChartProps: e,
  ref: t
}));
let Du = null;
function qM(e) {
  Du = e;
}
const ki = {
  call: (e, t) => {
    if (!Du) throw new Error("bridge not set");
    return Du.call(e, t);
  }
}, ih = "#8b5cf6";
function XM() {
  const [e, t] = fe([]), [r, n] = fe([]), [i, a] = fe([]), [o, u] = fe([]), [l, c] = fe(!0), [s, f] = fe(null);
  return he(() => {
    (async () => {
      try {
        const d = "demo-buildings", [h, v, p, m] = await Promise.all([
          ki.call("federation.query", { source: d, sql: "SELECT substr(time,1,13) as hour, ROUND(AVG(value),2) as avg_kwh FROM point_reading WHERE time >= '2026-06-05' AND time < '2026-06-06' AND point_id LIKE '%-kwh' GROUP BY 1 ORDER BY 1" }),
          ki.call("federation.query", { source: d, sql: "SELECT s.id, s.name, COUNT(DISTINCT m.id) as mc FROM site s LEFT JOIN meter m ON m.site_id=s.id GROUP BY s.id,s.name ORDER BY s.name" }),
          ki.call("federation.query", { source: d, sql: "SELECT m.id,m.name,m.site_id,p.id as pid,p.name as pn FROM meter m JOIN point p ON p.meter_id=m.id ORDER BY m.id LIMIT 50" }),
          ki.call("federation.query", { source: d, sql: "SELECT * FROM meter_tag LIMIT 20" })
        ]);
        t(h.rows.map((y) => ({ hour: String(y[0]).replace("T", " "), avg_kwh: Number(y[1]) }))), n(v.rows.map((y) => ({ id: String(y[0]), name: String(y[1]), meterCount: Number(y[2]) }))), a(p.rows.map((y) => ({ id: String(y[0]), name: String(y[1]), site_id: String(y[2]), point_id: String(y[3]), point_name: String(y[4]) }))), u(m.rows.map((y) => ({ meter_id: String(y[0]), tag: String(y[1]), kind: String(y[2]), val: String(y[3] ?? "") })));
      } catch (d) {
        f(d instanceof Error ? d.message : String(d));
      } finally {
        c(!1);
      }
    })();
  }, []), l ? /* @__PURE__ */ oe("div", { style: { padding: 24, fontFamily: "system-ui" }, children: "Loading live data from demo-buildings…" }) : s ? /* @__PURE__ */ bt("div", { style: { padding: 24, color: "#dc2626", fontFamily: "system-ui" }, children: [
    "Error: ",
    s
  ] }) : /* @__PURE__ */ bt("div", { style: { padding: 24, display: "flex", flexDirection: "column", gap: 24, fontFamily: "system-ui,-apple-system,sans-serif", background: "hsl(var(--background,210 20% 98.5%))", color: "hsl(var(--foreground,222 30% 16%))", minHeight: "100%" }, children: [
    /* @__PURE__ */ bt("header", { children: [
      /* @__PURE__ */ oe("h2", { style: { margin: 0, fontSize: 20, fontWeight: 700 }, children: "Site Summary — Live from demo-buildings" }),
      /* @__PURE__ */ bt("p", { style: { margin: "4px 0 0", fontSize: 13, opacity: 0.6 }, children: [
        r.length,
        " sites · ",
        i.length,
        " meter points · ",
        e.length,
        " hourly readings (2026-06-05)"
      ] })
    ] }),
    /* @__PURE__ */ bt("section", { style: { border: "1px solid hsl(var(--border,215 16% 86%))", borderRadius: 8, padding: 16, background: "hsl(var(--card,0 0% 100%))" }, children: [
      /* @__PURE__ */ oe("h3", { style: { margin: "0 0 12px", fontSize: 14, fontWeight: 600 }, children: "Hourly Energy (kWh) — 2026-06-05" }),
      /* @__PURE__ */ oe("div", { style: { height: 280, width: "100%" }, children: /* @__PURE__ */ oe(Lx, { width: "100%", height: "100%", children: /* @__PURE__ */ bt(YM, { data: e, margin: { top: 8, right: 16, bottom: 8, left: 0 }, children: [
        /* @__PURE__ */ oe(Wy, { strokeDasharray: "3 3", stroke: "rgba(128,128,128,0.15)" }),
        /* @__PURE__ */ oe(Qy, { dataKey: "hour", stroke: "rgba(128,128,128,0.6)", tick: { fontSize: 11 }, tickLine: !1, axisLine: { stroke: "rgba(128,128,128,0.2)" } }),
        /* @__PURE__ */ oe(Jy, { stroke: "rgba(128,128,128,0.6)", tick: { fontSize: 11 }, tickLine: !1, axisLine: !1 }),
        /* @__PURE__ */ oe(Dk, { contentStyle: { background: "hsl(var(--card,0 0% 100%))", border: "1px solid hsl(var(--border,215 16% 86%))", borderRadius: 6, fontSize: 12 } }),
        /* @__PURE__ */ oe(Xy, { type: "monotone", dataKey: "avg_kwh", stroke: ih, strokeWidth: 2, dot: { r: 3, fill: ih }, activeDot: { r: 5 } })
      ] }) }) })
    ] }),
    /* @__PURE__ */ oe(Bo, { title: "Sites", cols: ["ID", "Name", "Meters"], rows: r.map((d) => [d.id, d.name, String(d.meterCount)]) }),
    /* @__PURE__ */ oe(Bo, { title: "Meters & Points", cols: ["Meter ID", "Meter Name", "Site", "Point ID", "Point Name"], rows: i.map((d) => [d.id, d.name, d.site_id, d.point_id, d.point_name]) }),
    /* @__PURE__ */ oe(Bo, { title: "Meter Tags (sample)", cols: ["Meter", "Tag", "Kind", "Value"], rows: o.map((d) => [d.meter_id, d.tag, d.kind, d.val]) })
  ] });
}
function Bo({ title: e, cols: t, rows: r }) {
  return /* @__PURE__ */ bt("section", { style: { border: "1px solid hsl(var(--border,215 16% 86%))", borderRadius: 8, overflow: "hidden", background: "hsl(var(--card,0 0% 100%))" }, children: [
    /* @__PURE__ */ bt("h3", { style: { margin: 0, padding: "12px 16px", fontSize: 14, fontWeight: 600, borderBottom: "1px solid hsl(var(--border,215 16% 86%))" }, children: [
      e,
      " ",
      /* @__PURE__ */ bt("span", { style: { fontWeight: 400, opacity: 0.5 }, children: [
        "(",
        r.length,
        ")"
      ] })
    ] }),
    /* @__PURE__ */ oe("div", { style: { overflowX: "auto" }, children: /* @__PURE__ */ bt("table", { style: { width: "100%", borderCollapse: "collapse", fontSize: 13 }, children: [
      /* @__PURE__ */ oe("thead", { children: /* @__PURE__ */ oe("tr", { children: t.map((n) => /* @__PURE__ */ oe("th", { style: { textAlign: "left", padding: "8px 16px", fontWeight: 600, fontSize: 11, textTransform: "uppercase", opacity: 0.5, borderBottom: "1px solid hsl(var(--border,215 16% 86%))", whiteSpace: "nowrap" }, children: n }, n)) }) }),
      /* @__PURE__ */ oe("tbody", { children: r.map((n, i) => /* @__PURE__ */ oe("tr", { style: { borderBottom: "1px solid rgba(128,128,128,0.08)" }, children: n.map((a, o) => /* @__PURE__ */ oe("td", { style: { padding: "8px 16px", whiteSpace: "nowrap" }, children: a }, o)) }, i)) })
    ] }) })
  ] });
}
function t$(e, t, r) {
  qM(r);
  const n = Fo(e);
  return n.render(
    /* @__PURE__ */ oe(yg, { children: /* @__PURE__ */ oe("div", { className: "lbx-test-panel", children: /* @__PURE__ */ oe(XM, {}) }) })
  ), () => n.unmount();
}
export {
  t$ as mount
};
