// Promotion-checklist SOURCE-level invariants (genui-scope Decision 1). Item 3 (no prop is evaluated as
// code) and item 1 (no `dangerouslySetInnerHTML`) are grep-style assertions over the catalog + react
// source: a regression that introduced `eval`/`new Function`/`dangerouslySetInnerHTML` into the render
// path would fail CI here. Also the gen:skill freshness gate: the generated catalog JSON + skill block
// must match `defineCatalog` (a dirty diff means the node would embed a stale skill/validator).
import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve, join } from "node:path";
import { nubeCatalog } from "../catalog/nubeCatalog";
import { toCatalogJson } from "../catalog/toJson";
import { catalogPrompt } from "../catalog/prompt";
import { IR_VERSION } from "../ir/types";

const here = dirname(fileURLToPath(import.meta.url));
const srcRoot = resolve(here, "..");
const repo = resolve(here, "../../../..");

function allSourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.name.includes(".test.")) continue;
    const p = join(dir, e.name);
    if (e.isDirectory()) out.push(...allSourceFiles(p));
    else if (/\.tsx?$/.test(e.name)) out.push(p);
  }
  return out;
}

describe("promotion checklist (source invariants)", () => {
  const files = allSourceFiles(srcRoot);

  it("item 1: no `dangerouslySetInnerHTML` USE in the render path", () => {
    // Match actual JSX/prop use (`dangerouslySetInnerHTML=` or `:`), not the word in a doc comment
    // (the markdown renderer's comment says it NEVER uses it — that mention is fine).
    for (const f of files) {
      expect(readFileSync(f, "utf8")).not.toMatch(/dangerouslySetInnerHTML\s*[=:]/);
    }
  });

  it("item 3: no `eval(` or `new Function(` in the render path", () => {
    for (const f of files) {
      const src = readFileSync(f, "utf8");
      expect(src).not.toMatch(/\beval\s*\(/);
      expect(src).not.toMatch(/new\s+Function\s*\(/);
    }
  });
});

describe("gen:skill freshness gate", () => {
  it("the embedded host catalog JSON matches defineCatalog", () => {
    const generated = JSON.stringify(toCatalogJson(nubeCatalog, IR_VERSION), null, 2) + "\n";
    const checkedIn = readFileSync(resolve(repo, "rust/crates/host/src/dashboard/genui_catalog.json"), "utf8");
    expect(checkedIn).toBe(generated); // dirty diff → run `pnpm --filter @nube/genui gen:skill`
  });

  it("the SKILL.md generated catalog block matches the prompt", () => {
    const md = readFileSync(resolve(repo, "docs/skills/genui-widget/SKILL.md"), "utf8");
    // Match the block INSIDE the BEGIN/END markers (the prose has other ```text examples).
    const b = md.indexOf("<!-- BEGIN GENERATED CATALOG");
    const e = md.indexOf("<!-- END GENERATED CATALOG -->");
    expect(b).toBeGreaterThan(-1);
    expect(e).toBeGreaterThan(b);
    const marked = md.slice(b, e);
    const fenceStart = marked.indexOf("```text");
    const fenceEnd = marked.indexOf("```", fenceStart + 6);
    const block = marked.slice(fenceStart + 7, fenceEnd).trim();
    expect(block).toBe(catalogPrompt(nubeCatalog).trim());
  });
});
