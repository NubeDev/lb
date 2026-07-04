// Convert any CSS color a preset can carry — `#hex`, `oklch(L C H)`, or `hsl(H S% L%)` — into the
// project's base-token format: the bare HSL channel triplet "H S% L%" consumed as `hsl(var(--…))`.
// Presets in the shadcn/tweakcn packs use oklch and hex; a pasted tweakcn theme uses hsl. This is the
// one place that normalization happens, so the adapter and import parser both speak triplets.
//
// One responsibility: color-string → "H S% L%". Dependency-free (no culori) — the math is small and
// keeping it inline avoids a runtime dep for a build-time-ish transform. Returns null on anything it
// cannot parse, so callers fail-closed rather than emit a broken token.

/** Parse any supported CSS color into an `{h, s, l}` in degrees/percent, or null if unrecognized. */
function toHsl(input: string): { h: number; s: number; l: number } | null {
  const c = input.trim().toLowerCase();
  if (c.startsWith("#")) return hexToHsl(c);
  if (c.startsWith("oklch")) return oklchToHsl(c);
  if (c.startsWith("hsl")) return parseHslFn(c);
  return null;
}

/** The public API: a normalized "H S% L%" triplet (rounded, no `hsl(` wrapper), or null. */
export function colorToHslTriplet(input: string): string | null {
  const hsl = toHsl(input);
  if (!hsl) return null;
  const h = Math.round(hsl.h);
  const s = Math.round(hsl.s);
  const l = Math.round(hsl.l);
  return `${h} ${s}% ${l}%`;
}

function hexToHsl(hex: string): { h: number; s: number; l: number } | null {
  let h = hex.slice(1);
  if (h.length === 3) h = h.split("").map((ch) => ch + ch).join("");
  if (h.length !== 6 || /[^0-9a-f]/.test(h)) return null;
  const r = parseInt(h.slice(0, 2), 16) / 255;
  const g = parseInt(h.slice(2, 4), 16) / 255;
  const b = parseInt(h.slice(4, 6), 16) / 255;
  return rgbToHsl(r, g, b);
}

/** Parse `hsl(H S% L%)` / `hsl(H, S%, L%)` / `hsla(...)` → components (alpha ignored — tokens are opaque). */
function parseHslFn(input: string): { h: number; s: number; l: number } | null {
  const m = input.match(/hsla?\(([^)]+)\)/);
  if (!m) return null;
  const parts = m[1].split(/[\s,/]+/).filter(Boolean);
  if (parts.length < 3) return null;
  const h = parseFloat(parts[0]);
  const s = parseFloat(parts[1]);
  const l = parseFloat(parts[2]);
  if ([h, s, l].some((n) => Number.isNaN(n))) return null;
  return { h, s, l };
}

/** Parse `oklch(L C H)` (L in 0..1 or %, C, H in deg) and convert to HSL via linear-sRGB. */
function oklchToHsl(input: string): { h: number; s: number; l: number } | null {
  const m = input.match(/oklch\(([^)]+)\)/);
  if (!m) return null;
  const parts = m[1].split(/[\s,/]+/).filter(Boolean);
  if (parts.length < 3) return null;
  const L = parts[0].endsWith("%") ? parseFloat(parts[0]) / 100 : parseFloat(parts[0]);
  const C = parseFloat(parts[1]);
  const H = parseFloat(parts[2]);
  if ([L, C, H].some((n) => Number.isNaN(n))) return null;

  // OKLCH → OKLab → linear sRGB (Björn Ottosson's constants) → gamma sRGB → HSL.
  const hr = (H * Math.PI) / 180;
  const a = C * Math.cos(hr);
  const bb = C * Math.sin(hr);
  const l_ = L + 0.3963377774 * a + 0.2158037573 * bb;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * bb;
  const s_ = L - 0.0894841775 * a - 1.291485548 * bb;
  const l3 = l_ * l_ * l_;
  const m3 = m_ * m_ * m_;
  const s3 = s_ * s_ * s_;
  const rl = 4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
  const gl = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
  const bl = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.707614701 * s3;
  return rgbToHsl(gamma(rl), gamma(gl), gamma(bl));
}

/** Linear-sRGB channel → gamma-encoded sRGB, clamped to [0,1]. */
function gamma(x: number): number {
  const v = x <= 0.0031308 ? 12.92 * x : 1.055 * Math.pow(x, 1 / 2.4) - 0.055;
  return Math.min(1, Math.max(0, v));
}

/** sRGB (0..1 channels) → HSL (deg, %, %). */
function rgbToHsl(r: number, g: number, b: number): { h: number; s: number; l: number } {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  let h = 0;
  let s = 0;
  const d = max - min;
  if (d !== 0) {
    s = d / (1 - Math.abs(2 * l - 1));
    switch (max) {
      case r:
        h = ((g - b) / d) % 6;
        break;
      case g:
        h = (b - r) / d + 2;
        break;
      default:
        h = (r - g) / d + 4;
    }
    h *= 60;
    if (h < 0) h += 360;
  }
  return { h, s: s * 100, l: l * 100 };
}
