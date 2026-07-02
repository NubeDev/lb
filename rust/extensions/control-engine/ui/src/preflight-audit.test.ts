// Slice-9 regression guard (built-artifact audit). A federated page is injected into the HOST document,
// so its stylesheets must NOT carry Tailwind Preflight — the global `*`/`html`/`body` reset that once
// re-reset the live shell (main nav + sidebar collapsed). See
// docs/debugging/frontend/ce-page-css-preflight-leaks-into-shell.md.
//
// This asserts the BUILT artifacts (the bytes the browser actually loads) contain ZERO Preflight
// signatures. It fails the build if `@tailwind base` (or `@import 'tailwindcss'`'s preflight) ever
// returns to `src/styles/tokens.css` or `packages/ce-wiresheet`'s CSS entry. Rule 9: no mocks — we read
// the real compiled output, not a stand-in. The build must run first (`vite build`, `build:lib`).
import { readFileSync, readdirSync, existsSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

// Preflight's fingerprints — the GLOBAL reset selectors/props Tailwind's `base` layer emits. NONE may
// appear in a library stylesheet. We match the RESET'S structural shape, not lone property names: the
// remoteEntry chunk also bundles component JS (e.g. d3-drag/d3-zoom set `-webkit-tap-highlight-color`
// on the canvas ELEMENT at runtime — legitimate, not a global reset), so a bare `-webkit-*` substring
// would false-positive on JS. Every signature below is a CSS-reset marker that cannot appear in
// well-formed component JS. (The v4 `@layer properties` polyfill uses the single-colon `*,:before,:after`
// form to seed `--tw-*` custom-property fallbacks and applies NO reset — it is inert on the host, so it
// is deliberately not matched; we key on the double-colon box-sizing reset instead.)
const PREFLIGHT_SIGNATURES: RegExp[] = [
  /\*,\s*::before,\s*::after/, // the box-sizing reset selector (double-colon Preflight form)
  /-webkit-text-size-adjust\s*:/, // html{-webkit-text-size-adjust:100%} — Preflight-only, colon-anchored
  /@tailwind\s+base/, // the v3 directive itself (uncompiled, belt-and-suspenders)
  /@layer\s+base\b/, // the compiled base layer
  /layer\(base\)/, // the v4 `@import … layer(base)` form
];

function assertNoPreflight(label: string, css: string) {
  for (const sig of PREFLIGHT_SIGNATURES) {
    expect(sig.test(css), `${label} leaks Preflight signature ${sig}`).toBe(false);
  }
}

const here = path.dirname(new URL(import.meta.url).pathname);
const distDir = path.resolve(here, "../dist");
const wiresheetCss = path.resolve(here, "../../../../../packages/ce-wiresheet/dist/ce-wiresheet.css");

describe("slice-9 built-artifact Preflight audit", () => {
  it("the built remoteEntry chunks carry no Preflight (tokens.css + injected editor CSS)", () => {
    expect(
      existsSync(distDir),
      "control-engine/ui/dist missing — run `vite build` before this test",
    ).toBe(true);
    const chunks = readdirSync(distDir).filter((f) => f.endsWith(".js"));
    expect(chunks.length, "no built JS chunks found").toBeGreaterThan(0);
    for (const f of chunks) {
      assertNoPreflight(`remoteEntry chunk ${f}`, readFileSync(path.join(distDir, f), "utf8"));
    }
  });

  it("the vendored ce-wiresheet lib CSS carries no Preflight (fixed upstream, per S2)", () => {
    expect(
      existsSync(wiresheetCss),
      "ce-wiresheet.css missing — run `pnpm build:lib` in packages/ce-wiresheet first",
    ).toBe(true);
    assertNoPreflight("ce-wiresheet.css", readFileSync(wiresheetCss, "utf8"));
  });
});
