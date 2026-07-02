// `@nube/panel` — public surface.
//
// A reusable, right-docked, RESIZABLE side panel: the ce-wiresheet InspectPanel look
// rebuilt on shadcn/ui primitives, self-themed via scoped `hsl(var(--lbp-*))` tokens
// (host-overridable by re-declaring them under `.lb-panel`). Data-driven — the host
// composes <Section>/<PropTable>/<KV> into the panel body. Drag the left edge (or focus
// it + arrow keys) to widen and reveal more option columns.
//
// The stylesheet is a SEPARATE import (theme + utilities only, NO preflight — a library
// must not reset its host): `import '@nube/panel/style.css'`. The panel's section rail
// is @nube/nav-rail's NavMenu — the host imports `@nube/nav-rail/style.css` too.
import "./panel.css";

export { Panel, type PanelProps } from "./Panel";
export { Section, type SectionProps } from "./Section";
export { PropTable, type PropTableProps, type PropColumn, type PropRow } from "./PropTable";
export { KV, type KVProps } from "./KV";
export { ResizeHandle, type ResizeHandleProps } from "./ResizeHandle";
export { useResizable, type Resizable, type UseResizableOptions } from "./useResizable";

// Re-export the section rail so hosts get one import for "panel + its nav".
export { NavMenu, type NavItem, type NavMenuProps } from "@nube/nav-rail";
