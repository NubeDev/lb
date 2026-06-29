// The function-palette catalog types (rules-editor-ux scope). The palette is a STATIC typed mirror of
// the Rhai verbs the `lb-rules` engine registers (`rust/crates/rules/src/verbs/*`) — the registered set
// is compile-time known, so a host "list functions" verb would be a needless backend change. Each entry
// carries the exact signature + a one-line summary (lifted from the crate `///` docs) + the snippet a
// click inserts at the cursor. ONE CATEGORY DATA FILE PER VERB FAMILY (FILE-LAYOUT) — never a dump.
//
// Accuracy is the contract: an entry that lies about a signature is worse than none. Keep these in lock-
// step with the crate verbs (they live in the same repo, next to each other).

/** The verb families, surfaced as palette groups. `timeseries` is first-class (the user asked for it). */
export type CatalogCategory = "data" | "grid" | "timeseries" | "ai" | "output";

/** One palette entry — a registered Rhai verb. */
export interface FnEntry {
  /** The verb name as registered in the engine (e.g. `history`, `rollup`). */
  name: string;
  /** The exact signature, mirroring the crate registration (e.g. `history(source, point, span)`). */
  signature: string;
  /** A one-line description (from the crate `///` docs). */
  summary: string;
  /** The text inserted at the cursor on click — a runnable snippet with `<…>` placeholders. */
  snippet: string;
  category: CatalogCategory;
}

/** One palette group: a category + its entries (rendered as a section). */
export interface CatalogGroup {
  category: CatalogCategory;
  /** The human label shown on the section + tab. */
  label: string;
  /** A one-line "what is this family" blurb for the section header. */
  blurb: string;
  entries: FnEntry[];
}
