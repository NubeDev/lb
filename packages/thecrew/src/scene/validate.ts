// Total validation + normalization of a SceneDoc (thecrew-scope.md testing plan).
// Unknown type / missing transform / bad bind never crash a render: they normalize to
// a placeholder-renderable shape and are reported. The framework's AI-drawing loop
// depends on these errors being *teaching* (name the failing shape and why).

import type { SceneDoc } from "./scene.types";

export interface SceneIssue {
  shapeId: string;
  problem: string;
}

export interface ValidatedScene {
  doc: SceneDoc;
  issues: SceneIssue[];
}

export function validateScene(_input: unknown): ValidatedScene {
  // TODO(phase 1): shape-map structural checks, transform defaults, bind channel
  // shape check, unknown-type pass-through (ShapeNode renders the placeholder).
  throw new Error("TODO(phase 1): validateScene");
}
