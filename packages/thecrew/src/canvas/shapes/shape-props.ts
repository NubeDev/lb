// The shared contract every symbol component receives (symbols-scope.md §design-language).
// Symbols are pure: (shape, resolved values, selected) → meshes. They never fetch,
// never pick colors (theme/materials.ts), and declare their anchors here.

import type { SceneShape } from "../../scene/scene.types";

export interface Anchor {
  name: string; // e.g. "in", "out"
  x: number;
  y: number;
  /** outward direction, radians in the ground plane */
  dir: number;
}

export interface ShapeComponentProps {
  shape: SceneShape;
  /** bind results, keyed by prop name — resolved by ShapeNode via the ValueSource seam */
  values: Record<string, unknown>;
  selected: boolean;
  hovered: boolean;
}

export interface SymbolDef {
  type: string; // "hvac.fan"
  component: (props: ShapeComponentProps) => unknown; // JSX.Element | null
  anchors: (shape: SceneShape) => Anchor[];
  /** ≤8 props — drives the PropertyRail (builder-ux-scope.md §tune) */
  propSchema: Record<string, { label: string; kind: "text" | "number" | "select"; options?: string[] }>;
}
