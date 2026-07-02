// `@nube/genui` — the RENDER stratum public surface (the entry every viewer loads). Parser-free,
// normalize-free, deterministic: `ir/` pure ops + `catalog/` + `react/` `<GenUiSurface>`. The AUTHORING
// stratum (Lang adapter + normalize) lives behind the separate `@nube/genui/authoring` entry so a
// viewer never bundles the parser (genui-scope "The render path carries no adapter").
//
// Consumed two ways: the lb `ui/` dashboard (`view:"genui"`), and — later — a standalone channel/
// extension host. Self-themed via `--gu-*` tokens scoped to `.gu-root`; imports nothing from `ui/src`.

import "./genui.css";

// IR — the versioned contract + pure ops.
export * from "./ir/index";

// Catalog — the component contract + prompt/JSON generation + the v1 catalog.
export { defineCatalog } from "./catalog/defineCatalog";
export type { Catalog, CatalogEntry, PropSpec, RenderProps } from "./catalog/defineCatalog";
export { toCatalogJson, catalogNames } from "./catalog/toJson";
export type { CatalogJson, CatalogJsonComponent } from "./catalog/toJson";
export { catalogPrompt } from "./catalog/prompt";
export { nubeCatalog } from "./catalog/nubeCatalog";

// React — the render surface + bridge context.
export { GenUiSurface } from "./react/GenUiSurface";
export type { GenUiSurfaceProps } from "./react/GenUiSurface";
export { GenUiProvider, useGenUiBridge } from "./react/GenUiContext";
export type { GenUiBridge, GenUiCtx } from "./react/GenUiContext";
