// The seam between the host shell's CSS theme and the 3D canvas. thecrew is a federated remote
// mounted inside the shell, so the host's tokens (ui/src/styles/globals.css:
// --bg/--panel/--border/--fg/--muted/--accent, as `H S% L%` triples, swapped by the `.dark` class
// and `[data-theme-accent]` on <html>) are in scope on document.documentElement. CSS can read them
// directly (styles.css), but three.js needs concrete color values — so here we RESOLVE the host vars
// at runtime into hex, derive thecrew's canvas palette from them, and notify on theme change.
//
// ONE responsibility: host-CSS-var → canvas color resolution + change subscription. There are NO
// hardcoded surface/accent/text colors anywhere else; this is the single source (look-scope.md
// §visual-language: "the framework binds these to shell tokens" — this is that binding).

/** Read one host token (`H S% L%`) off :root and convert to a `#rrggbb` hex string for three.js.
 *  Falls back to a mid-grey if the var is absent (e.g. a headless test with no shell stylesheet). */
function hostHex(name: string, fallback: string): string {
  if (typeof window === "undefined" || typeof getComputedStyle !== "function") return fallback;
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  if (!raw) return fallback;
  const m = raw.match(/^([\d.]+)\s+([\d.]+)%\s+([\d.]+)%$/);
  if (!m) return raw.startsWith("#") ? raw : fallback; // tolerate an already-hex value
  return hslToHex(Number(m[1]), Number(m[2]), Number(m[3]));
}

/** HSL (h∈[0,360), s,l∈[0,100]) → `#rrggbb`. */
function hslToHex(h: number, s: number, l: number): string {
  const sf = s / 100;
  const lf = l / 100;
  const c = (1 - Math.abs(2 * lf - 1)) * sf;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = lf - c / 2;
  const [r, g, b] =
    h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] :
    h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  const to = (v: number) => Math.round((v + m) * 255).toString(16).padStart(2, "0");
  return `#${to(r)}${to(g)}${to(b)}`;
}

/** Mix two hex colors by ratio t∈[0,1] (0 = a, 1 = b) — for deriving equipment/grid shades that sit
 *  between the panel and the fg, so they track the theme instead of being fixed hexes. */
function mix(a: string, b: string, t: number): string {
  const pa = [1, 3, 5].map((i) => parseInt(a.slice(i, i + 2), 16));
  const pb = [1, 3, 5].map((i) => parseInt(b.slice(i, i + 2), 16));
  const to = (v: number) => Math.round(v).toString(16).padStart(2, "0");
  return `#${pa.map((v, i) => to(v + (pb[i] - v) * t)).join("")}`;
}

/** The canvas palette, fully derived from host tokens. Surfaces/accent/text follow the theme;
 *  the medium (chw/hw/air) and status hues stay fixed semantic colors — they encode meaning
 *  (chilled vs hot, running vs fault), not chrome, so they must NOT drift with the accent swatch. */
export interface CanvasColors {
  canvas: string; // scene background + ground = host --bg
  duct: string; // duct bodies, just off the background
  grid: string; // ground grid, between bg and border
  steel: string; // equipment bodies (desaturated), between panel and fg
  accent: string; // live data + selection = host --accent
  textLabel: string; // secondary labels = host --muted
  textValue: string; // live values = host --fg
}

export function readCanvasColors(): CanvasColors {
  const bg = hostHex("--bg", "#0a0e14");
  const panel = hostHex("--panel", "#101620");
  const border = hostHex("--border", "#242a34");
  const fg = hostHex("--fg", "#e2e8f0");
  const muted = hostHex("--muted", "#94a3b8");
  const accent = hostHex("--accent", "#22d3ee");
  return {
    canvas: bg,
    duct: mix(bg, panel, 0.5), // barely lighter than the background
    grid: mix(bg, border, 0.5), // faint grid that reads in both light + dark
    steel: mix(panel, fg, 0.35), // desaturated equipment body derived from the surface ramp
    accent,
    textLabel: muted,
    textValue: fg,
  };
}

/** Subscribe to host theme changes (the `.dark` class and `data-theme-accent` on <html> flip the
 *  tokens). Returns an unsubscribe. Fires once synchronously is NOT done — call readCanvasColors()
 *  for the initial value; this only notifies on subsequent changes. */
export function subscribeThemeChange(cb: () => void): () => void {
  if (typeof MutationObserver === "undefined") return () => {};
  const obs = new MutationObserver((records) => {
    if (records.some((r) => r.attributeName === "class" || r.attributeName === "data-theme-accent")) {
      cb();
    }
  });
  obs.observe(document.documentElement, { attributes: true, attributeFilter: ["class", "data-theme-accent"] });
  return () => obs.disconnect();
}
