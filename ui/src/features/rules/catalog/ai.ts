// The AI verb family — `ask`/`complete`/`classify`/`embed` on the `ai` handle. Mirrors
// `rust/crates/rules/src/verbs/ai.rs` exactly (rules-editor-ux scope). Every call charges the per-run AI
// budget meter (a loop can't overspend); `ai.ask`'s proposed SQL is re-validated through the same fence
// `query()` uses. A model-less workspace returns the honest "AI not configured" error (rendered as such).

import type { CatalogGroup } from "./catalog.types";

export const AI_GROUP: CatalogGroup = {
  category: "ai",
  label: "AI",
  blurb: "Model-backed verbs (budget-metered; needs a configured model).",
  entries: [
    {
      name: "ask",
      signature: "ai.ask(question)",
      summary: "Propose + run a fenced query from a natural-language question → a grid.",
      snippet: 'ai.ask("which coolers ran hot today?")',
      category: "ai",
    },
    {
      name: "complete",
      signature: "ai.complete(prompt) | ai.complete(prompt, context) | ai.complete(prompt, grid)",
      summary: "A text completion, optionally with a string or a grid as context.",
      snippet: 'ai.complete("summarize", grid)',
      category: "ai",
    },
    {
      name: "classify",
      signature: "ai.classify(grid, [labels])",
      summary: "Label each grid row with one of the given labels → an array of {…row, label}.",
      snippet: 'ai.classify(grid, ["ok", "alert"])',
      category: "ai",
    },
    {
      name: "embed",
      signature: "ai.embed(text)",
      summary: "An embedding vector (array of floats) for the text.",
      snippet: 'ai.embed("cooler temperature")',
      category: "ai",
    },
  ],
};
