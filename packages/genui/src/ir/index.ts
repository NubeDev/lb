// The IR barrel — the render-stratum pure ops + types. No parsers, no normalize (those are authoring).
export * from "./types";
export { resolveBindings, resolveValue, resolvePointer } from "./resolveBindings";
export { applyPatch, emptySpec } from "./applyPatch";
export { validate, errors, warnings } from "./validate";
export { migrate } from "./migrate";
