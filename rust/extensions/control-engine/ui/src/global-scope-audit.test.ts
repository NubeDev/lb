// Slice-9.1 regression guard — the FULL global-leak audit, broader than the Preflight-only check in
// `preflight-audit.test.ts`. A federated page injects its stylesheets into the HOST document, so beyond
// Preflight (the global reset) it must also ship NO other global writes that collide with the shell:
//
//   • `:root` / `:host` custom-property writes — the editor's `@theme` + token block once wrote
//     `--card`/`--border`/`--foreground`/`--radius-md`/… to `:root`, overriding the shell's OWN tokens
//     of the same name document-wide (visible: shadcn cards/popovers flipped to the editor's dark
//     palette in host light mode). Now scoped to `.ce-wiresheet` by the ce-wiresheet lib build.
//   • bare `.react-flow*` rules — `@xyflow/react`'s ~150 rules once shipped unscoped; the host renders
//     React Flow in its system/data/flows views with its OWN `.react-flow` theming, so the vendored copy
//     collided (last-injected won). Now `.ce-wiresheet`-scoped by the lib build.
//
// See docs/debugging/frontend/ce-page-css-preflight-leaks-into-shell.md (§ "the leak the guard missed").
// Rule 9: reads the REAL built artifacts. Runs after `build:lib` + `vite build`.
import { readFileSync, readdirSync, existsSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const here = path.dirname(new URL(import.meta.url).pathname);
const distDir = path.resolve(here, "../dist");
const wiresheetCss = path.resolve(here, "../../../../../packages/ce-wiresheet/dist/ce-wiresheet.css");

// Strip comments so a `:root` mentioned in a CSS comment isn't a false positive.
const decomment = (css: string) => css.replace(/\/\*[\s\S]*?\*\//g, "");

// A `:root{…}` or `:host{…}` STYLE rule (writes custom properties to the document root). The
// `*,:before,:after,::backdrop{--tw-*}` polyfill uses `*`, not `:root`, so it is not matched.
function rootWrites(css: string): string[] {
  const hits: string[] = [];
  for (const m of decomment(css).matchAll(/(^|[{};])\s*(:root|:host)\s*[,{]/g)) {
    hits.push(m[0].trim().slice(0, 40));
  }
  return hits;
}

// Every selector that references `.react-flow` must also contain `.ce-wiresheet` (scoped). Returns the
// unscoped offenders.
function unscopedReactFlow(css: string): string[] {
  const bad: string[] = [];
  for (const m of decomment(css).matchAll(/([^{}]+)\{/g)) {
    const list = m[1];
    if (list.trimStart().startsWith("@")) continue;
    for (const raw of list.split(",")) {
      const s = raw.trim();
      if (s.includes(".react-flow") && !s.includes(".ce-wiresheet")) bad.push(s.slice(0, 60));
    }
  }
  return bad;
}

describe("slice-9.1 global-scope audit (no :root writes, no unscoped .react-flow)", () => {
  it("the ce-wiresheet lib CSS writes nothing to :root/:host and scopes every .react-flow rule", () => {
    expect(existsSync(wiresheetCss), "ce-wiresheet.css missing — run `pnpm build:lib` first").toBe(
      true,
    );
    const css = readFileSync(wiresheetCss, "utf8");
    expect(rootWrites(css), "ce-wiresheet.css leaks :root/:host token writes to the host").toEqual(
      [],
    );
    expect(
      unscopedReactFlow(css),
      "ce-wiresheet.css leaks unscoped .react-flow rules that collide with the host's canvases",
    ).toEqual([]);
  });

  it("the built remoteEntry chunks carry no unscoped editor :root writes or .react-flow rules", () => {
    expect(existsSync(distDir), "control-engine/ui/dist missing — run `vite build` first").toBe(true);
    const chunks = readdirSync(distDir).filter((f) => f.endsWith(".js"));
    for (const f of chunks) {
      const js = readFileSync(path.join(distDir, f), "utf8");
      // Only flag an EDITOR :root block (carries the editor-only `--cool` token) — the JS bundle also
      // contains d3/other libs that legitimately mention `:root` in unrelated string data.
      const editorRoot = [...decomment(js).matchAll(/:root[^{]*\{[^}]*--cool/g)];
      expect(
        editorRoot.length,
        `${f}: injected editor CSS still writes editor tokens to :root`,
      ).toBe(0);
      expect(
        unscopedReactFlow(js),
        `${f}: injected editor CSS still ships unscoped .react-flow rules`,
      ).toEqual([]);
    }
  });
});
