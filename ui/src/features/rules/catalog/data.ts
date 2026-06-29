// The Data verb family — `source`/`query`/`history`/`span`/`last`/`param`. Mirrors
// `rust/crates/rules/src/verbs/data.rs` exactly (rules-editor-ux scope). These seed a base grid from a
// named source (the host resolves the name through the allowlist + workspace pin) or read a point's
// history; `span`/`last` are typed window constructors; `param` reads a bound input.

import type { CatalogGroup } from "./catalog.types";

export const DATA_GROUP: CatalogGroup = {
  category: "data",
  label: "Data",
  blurb: "Seed a grid from a named source, or read a point's history.",
  entries: [
    {
      name: "source",
      signature: "source(name)",
      summary: "The uniform entry — a base grid reading all rows of a named source.",
      snippet: 'source("series")',
      category: "data",
    },
    {
      name: "query",
      signature: "query(source, sql)",
      summary: "A hand-written query against a named source (re-validated at the host).",
      snippet: 'query("series", "SELECT * FROM series LIMIT 100")',
      category: "data",
    },
    {
      name: "history",
      signature: 'history(source, point, span) | history(source, point, "24h")',
      summary: "The (ts, value) rows of a named point within a window.",
      snippet: 'history("series", "<point>", "24h")',
      category: "data",
    },
    {
      name: "span",
      signature: "span(s)",
      summary: "A typed window constructor (validated duration), e.g. span(\"24h\").",
      snippet: 'span("24h")',
      category: "data",
    },
    {
      name: "last",
      signature: "last(s)",
      summary: "A typed trailing window, e.g. last(\"7d\").",
      snippet: 'last("7d")',
      category: "data",
    },
    {
      name: "param",
      signature: "param(name)",
      summary: "Read a bound input by name (also available as a scope var).",
      snippet: 'param("threshold")',
      category: "data",
    },
  ],
};
