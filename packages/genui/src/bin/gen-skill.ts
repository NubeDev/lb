// `pnpm --filter @nube/genui gen:skill` — the codegen chain that crosses three build systems
// (genui-scope "The codegen chain is named"). From the ONE `defineCatalog` source it renders:
//
//   1. the catalog signature block into the marked section of `docs/skills/genui-widget/SKILL.md`
//      (the skill the Rust node embeds + seeds as `skill:core.genui-widget`), and
//   2. the A2UI-style catalog JSON into `rust/crates/host/src/dashboard/genui_catalog.json`
//      (the artifact the host `include_str!`s and validates a saved genui cell against — Decision 6).
//
// CI runs this and fails on a dirty diff, so the node can NEVER embed a skill or a validator that lags
// the catalog. Hand-edits to the skill live ONLY outside the generated markers.
//
// Run with `--check` to only verify freshness (exit 1 on drift) without writing — the CI gate.

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { nubeCatalog } from "../catalog/nubeCatalog";
import { catalogPrompt } from "../catalog/prompt";
import { toCatalogJson } from "../catalog/toJson";
import { IR_VERSION } from "../ir/types";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../../../.."); // packages/genui/src/bin → repo root
const SKILL = resolve(repo, "docs/skills/genui-widget/SKILL.md");
const CATALOG_JSON = resolve(repo, "rust/crates/host/src/dashboard/genui_catalog.json");

const BEGIN = "<!-- BEGIN GENERATED CATALOG (do not edit — `pnpm --filter @nube/genui gen:skill`) -->";
const END = "<!-- END GENERATED CATALOG -->";

/** Render the generated skill catalog section (signature block, fenced). */
function skillBlock(): string {
  const sig = catalogPrompt(nubeCatalog).trimEnd();
  return [BEGIN, "", "```text", sig, "```", "", END].join("\n");
}

/** Splice the generated block into the skill markdown between the markers. */
function spliceSkill(md: string): string {
  const b = md.indexOf(BEGIN);
  const e = md.indexOf(END);
  const block = skillBlock();
  if (b === -1 || e === -1) {
    // First run / markers missing: append the block at the end.
    return md.trimEnd() + "\n\n" + block + "\n";
  }
  return md.slice(0, b) + block + md.slice(e + END.length);
}

function catalogJsonText(): string {
  return JSON.stringify(toCatalogJson(nubeCatalog, IR_VERSION), null, 2) + "\n";
}

function main() {
  const check = process.argv.includes("--check");
  const skillMd = readFileSync(SKILL, "utf8");
  const nextSkill = spliceSkill(skillMd);
  const nextJson = catalogJsonText();
  const curJson = (() => {
    try {
      return readFileSync(CATALOG_JSON, "utf8");
    } catch {
      return "";
    }
  })();

  const skillDirty = nextSkill !== skillMd;
  const jsonDirty = nextJson !== curJson;

  if (check) {
    if (skillDirty || jsonDirty) {
      console.error(
        `genui gen:skill is STALE — run \`pnpm --filter @nube/genui gen:skill\`:` +
          (skillDirty ? "\n  - docs/skills/genui-widget/SKILL.md" : "") +
          (jsonDirty ? "\n  - rust/crates/host/src/dashboard/genui_catalog.json" : ""),
      );
      process.exit(1);
    }
    console.log("genui gen:skill: up to date");
    return;
  }

  if (skillDirty) writeFileSync(SKILL, nextSkill);
  if (jsonDirty) writeFileSync(CATALOG_JSON, nextJson);
  console.log(`genui gen:skill: wrote${skillDirty ? " SKILL.md" : ""}${jsonDirty ? " genui_catalog.json" : ""}${!skillDirty && !jsonDirty ? " (no changes)" : ""}`);
}

main();
