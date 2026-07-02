// THE scene document schema — the framework contract (thecrew-scope.md §reuse #1).
// Mirrors docs/scope/frontend/graphics-canvas-scope.md; schema changes here must be
// pushed back to that scope, never forked locally.

/** Playground binding: a simulator channel. The framework replaces this with its
 * Target/source vocabulary at the same seam (data/value-source.ts). */
export type ValueRef = { channel: string };

export interface Transform {
  x: number;
  y: number;
  /** reserved for 3D; flat scenes keep 0/undefined */
  z?: number;
  /** rotation in the ground plane, radians */
  r?: number;
  sx?: number;
  sy?: number;
}

export interface SceneShape {
  /** symbol type, e.g. "hvac.fan", "plan.wall" — unknown types render a placeholder */
  type: string;
  t: Transform;
  props: Record<string, unknown>;
  /** prop name → live value source */
  bind?: Record<string, ValueRef>;
}

export type CameraMode = "ortho-top" | "persp";

export interface SceneDoc {
  v: 1;
  camera: CameraMode;
  /** optional underlay (image/SVG asset) — floor plans */
  bg?: { asset?: string };
  /** flat id-referenced shape map — easy for an LLM to patch incrementally */
  shapes: Record<string, SceneShape>;
}
