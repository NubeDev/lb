// The library-panel client surface (library-panels scope) — types, the `panel.*` API, the
// Panel↔Cell bridge, and the shared demo `Cell` builders + starter gallery, exported from one barrel
// so features import from `@/lib/panel` (no cross-feature import into features/admin/setup/*).
export * from "./panel.types";
export * from "./panel.api";
export * from "./panel.cell";
export * from "./demoCells";
export * from "./demoGallery";
