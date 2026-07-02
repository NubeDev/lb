// Slice-9 regression guard (source-compile audit). Compile `src/styles/tokens.css` through the SAME
// Tailwind-v3 + PostCSS pipeline the build uses, and assert:
//   1. no `@layer base` / Preflight is emitted (belt-and-suspenders with the built-artifact audit — this
//      one fails at the SOURCE the moment someone re-adds `@tailwind base`, before a build even runs), and
//   2. every generated utility/component rule is scoped under `.ce-page` — no bare `.flex`/`.border` at
//      the rule root that could leak into the host document.
// Mirrors the nav-rail/panel scoped-utility assertions from `library-css-leaks-global-utilities.md`.
// Rule 9: no mocks — we run the real compiler over the real stylesheet.
import { readFileSync } from "node:fs";
import path from "node:path";
import postcss from "postcss";
import tailwindcss from "tailwindcss";
import autoprefixer from "autoprefixer";
import { describe, expect, it } from "vitest";

const uiRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const tokensCss = readFileSync(path.join(uiRoot, "src/styles/tokens.css"), "utf8");
// The extension's own tailwind config (the content globs + token color map).
const tailwindConfig = path.join(uiRoot, "tailwind.config.ts");

async function compileTokens(): Promise<string> {
  const result = await postcss([
    // Force a content source that USES a spread of utilities so Tailwind actually emits them — otherwise
    // the JIT emits nothing and the "is it scoped" assertion is vacuous.
    tailwindcss({
      presets: [],
      config: tailwindConfig,
      content: [
        {
          raw: '<div class="ce-page flex h-full flex-col border border-border rounded-md bg-bg text-fg items-center gap-3 w-full hidden grid absolute relative px-4 py-2"></div>',
        },
      ],
    }),
    autoprefixer(),
  ]).process(tokensCss, { from: path.join(uiRoot, "src/styles/tokens.css") });
  return result.css;
}

describe("slice-9 tokens.css scope audit", () => {
  it("emits no Preflight / @layer base", async () => {
    const css = await compileTokens();
    expect(/@layer\s+base\b/.test(css), "tokens.css emitted a base layer (Preflight)").toBe(false);
    expect(/-webkit-text-size-adjust/.test(css), "tokens.css emitted an html reset").toBe(false);
    expect(/\*,\s*::before,\s*::after/.test(css), "tokens.css emitted the box-sizing reset").toBe(
      false,
    );
  });

  it("actually emits utilities (guard is not vacuous)", async () => {
    const css = await compileTokens();
    expect(css.includes(".flex"), "no .flex compiled — content globs wrong, test is vacuous").toBe(
      true,
    );
  });

  it("scopes every generated utility under .ce-page — no bare .flex/.border at rule root", async () => {
    const css = await compileTokens();
    const root = postcss.parse(css);
    const leaked: string[] = [];
    // A structural Tailwind utility that MUST NOT appear unscoped at the top level.
    const utility = /^\.(flex|grid|hidden|border|rounded[a-z-]*|w-full|absolute|relative|items-|gap-|px-|py-|flex-|h-full|bg-|text-)/;
    root.walkRules((rule) => {
      // Only top-level rules (a scoped rule lives at depth 1 too, but its selector STARTS with .ce-page).
      for (const sel of rule.selectors) {
        const s = sel.trim();
        if (utility.test(s) && !s.startsWith(".ce-page")) {
          leaked.push(s);
        }
      }
    });
    expect(leaked, `unscoped utilities leaked to the host: ${leaked.join(", ")}`).toEqual([]);
  });
});
