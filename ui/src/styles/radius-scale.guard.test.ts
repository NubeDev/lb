// Build-level guard for the radius bug (theme-appearance scope, slice 0). jsdom cannot see Tailwind's
// compiled output, so this asserts on the SOURCE instead — two invariants that together mean "a radius
// nudge re-rounds the whole app":
//
//   1. The `@theme` block in globals.css derives EVERY used rounded stop from the one `--radius` token
//      (the shipped bug was that only sm/md/lg derived; bare `rounded`/`rounded-xl`/`rounded-2xl` used
//      Tailwind's static defaults and never moved). We check `--radius-DEFAULT` + the full ladder.
//   2. No `.tsx` under src/ uses the bare `rounded` utility (which maps to the un-derived Tailwind
//      DEFAULT unless we pin it) — every corner goes through a token-derived stop instead. The allowlist
//      is `rounded-full` / `rounded-none` (deliberate pills/squares, literal by intent).
//
// If either regresses, the radius control silently goes back to "does nothing" for most of the UI.

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..");
const GLOBALS = join(SRC, "styles", "globals.css");

/** Recursively collect every `.tsx` file under `dir`. */
function tsxFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...tsxFiles(p));
    else if (entry.name.endsWith(".tsx")) out.push(p);
  }
  return out;
}

describe("radius scale — build guard", () => {
  const css = readFileSync(GLOBALS, "utf8");

  it("derives the full rounded ladder from var(--radius) in @theme", () => {
    // Each stop must map off `--radius`, not a static Tailwind default. Match the DECLARATION line
    // (a `--radius-x:` followed by a value on the same line), not prose in a comment that names the key.
    for (const key of ["xs", "sm", "DEFAULT", "md", "lg", "xl", "2xl", "3xl"]) {
      const decl = new RegExp(`^\\s*--radius-${key}:\\s*[^;]*var\\(--radius\\)`, "m");
      expect(decl.test(css), `--radius-${key} must derive from var(--radius) in globals.css`).toBe(true);
    }
  });

  it("pins bare `rounded` (--radius-DEFAULT) so it tracks the token", () => {
    // The bug was that `rounded` (Tailwind DEFAULT) ignored `--radius`. Pinning DEFAULT fixes it even
    // if a stray bare `rounded` slips back in.
    expect(css).toMatch(/--radius-DEFAULT:\s*[^;]*var\(--radius\)/);
  });

  it("re-asserts the radius ladder in a high-specificity cascade-last override", () => {
    // The load-bearing fix: `tw-animate-css` re-imports Tailwind's default theme AFTER our `@theme`
    // and would win with the static `.375rem`. The `:root:root` (specificity 0,2,0) block beats it.
    // If this block is dropped, the compiled `.rounded-md` silently reverts to the static default and
    // the radius control does nothing again — the exact shipped bug. (Live-verified in Chromium; jsdom
    // can't compute `var()`, so we assert the source guard here.)
    expect(css, "missing the :root:root radius override (the tw-animate-css cascade fix)").toMatch(
      /:root:root\s*\{[^}]*--radius-md:\s*[^;]*var\(--radius\)/,
    );
  });

  it("has no bare `rounded` utility in any .tsx (swept onto token-derived stops)", () => {
    const bare = /\brounded\b(?!-)/;
    const offenders: string[] = [];
    for (const file of tsxFiles(SRC)) {
      const text = readFileSync(file, "utf8");
      text.split("\n").forEach((line, i) => {
        if (bare.test(line)) offenders.push(`${file.replace(SRC, "src")}:${i + 1}  ${line.trim()}`);
      });
    }
    expect(offenders, `bare \`rounded\` must be a token-derived stop (rounded-md/-lg/etc.):\n${offenders.join("\n")}`).toEqual(
      [],
    );
  });
});
