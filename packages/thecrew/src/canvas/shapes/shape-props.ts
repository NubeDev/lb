// The shared contract every symbol component receives (symbols-scope.md §design-language).
// Symbols are pure: (shape, resolved values, selected) → meshes. They never fetch,
// never pick colors (theme/materials.ts), and declare their anchors here.
//
// Coordinate convention (whole package): ground plane = XY, +Z is "up" (extrusion).
// Flat mode looks straight down -Z; the 3D tilt orbits with up = +Z.

import type { ReactElement } from "react";
import type { SceneShape } from "../../scene/scene.types";

export interface Anchor {
  name: string; // e.g. "in", "out"
  x: number; // shape-local
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
  /** palette display name */
  label: string;
  component: (props: ShapeComponentProps) => ReactElement | null;
  anchors: (shape: SceneShape) => Anchor[];
  /** shape-local bounding box (ground plane) — drives the selection halo + box select */
  bounds: (shape: SceneShape) => { w: number; h: number };
  /** ≤8 props — drives the PropertyRail (builder-ux-scope.md §tune) */
  propSchema: Record<
    string,
    { label: string; kind: "text" | "number" | "select" | "boolean"; options?: string[] }
  >;
  /** binding slots the rail offers a channel picker for */
  bindSlots: string[];
}
