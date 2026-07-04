// Bundle-discipline guards (theme-appearance scope) — the fonts + `motion` must stay behind LAZY seams
// so they never ship in the main bundle and never load unless selected. jsdom can't inspect the built
// chunks cheaply, so we assert the SOURCE invariants that guarantee it (verified against a real `vite
// build`: the main JS bundle references zero font woff2 and no motion engine; each family + motion is
// its own chunk):
//
//   1. `font-loader.ts` imports `@fontsource/*` ONLY via dynamic `import()` (no top-level static import
//      would pull every family into the main bundle).
//   2. `motion` (motion.dev) is imported in EXACTLY ONE file — `lib/motion/motion.ts` — the single seam
//      the scope mandates so the off switch is trustworthy and the engine tree-shakes to one place.

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..", "..");
const FONT_LOADER = join(__dirname, "font-loader.ts");

function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) out.push(...sourceFiles(p));
    else if (/\.tsx?$/.test(e.name)) out.push(p);
  }
  return out;
}

describe("bundle discipline", () => {
  it("font-loader imports @fontsource only via dynamic import()", () => {
    const src = readFileSync(FONT_LOADER, "utf8");
    // No STATIC top-level `import … from "@fontsource…"` — those pull the font into the main chunk.
    const staticImport = /^\s*import\s[^\n]*["']@fontsource/m;
    expect(staticImport.test(src), "font-loader must not statically import @fontsource").toBe(false);
    // It DOES use dynamic import() of @fontsource.
    expect(/import\(\s*["']@fontsource/.test(src)).toBe(true);
  });

  it("imports `motion` (motion.dev) in exactly one file — the lib/motion seam", () => {
    // Match a bare `from "motion"` / `from "motion/react"` (not `lib/motion`, not `useMotionPref`).
    const motionImport = /from\s+["']motion(\/[\w-]+)?["']/;
    const offenders: string[] = [];
    for (const file of sourceFiles(SRC)) {
      if (file.endsWith("font-loader.guard.test.ts")) continue;
      const text = readFileSync(file, "utf8");
      if (motionImport.test(text)) offenders.push(file.replace(SRC, "src"));
    }
    expect(offenders, `motion.dev must be imported only in lib/motion/motion.ts; found: ${offenders.join(", ")}`).toEqual(
      ["src/lib/motion/motion.ts"],
    );
  });
});
