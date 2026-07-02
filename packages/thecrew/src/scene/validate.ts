// Total validation + normalization of a SceneDoc (thecrew-scope.md testing plan).
// Unknown type / missing transform / bad bind never crash a render: they normalize to
// a placeholder-renderable shape and are reported. The framework's AI-drawing loop
// depends on these errors being *teaching* (name the failing shape and why).

import type { CameraMode, SceneDoc, SceneShape, Transform, ValueRef } from "./scene.types";

export interface SceneIssue {
  shapeId: string;
  problem: string;
}

export interface ValidatedScene {
  doc: SceneDoc;
  issues: SceneIssue[];
}

const CAMERA_MODES: CameraMode[] = ["ortho-top", "persp"];

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function num(v: unknown, fallback: number): number {
  return typeof v === "number" && Number.isFinite(v) ? v : fallback;
}

/** Normalize a transform; missing/garbage fields default so the shape still renders. */
function normalizeTransform(t: unknown, shapeId: string, issues: SceneIssue[]): Transform {
  if (!isRecord(t)) {
    issues.push({ shapeId, problem: "missing transform `t` — placed at origin" });
    return { x: 0, y: 0 };
  }
  const out: Transform = { x: num(t.x, 0), y: num(t.y, 0) };
  if (t.x !== undefined && !Number.isFinite(t.x as number)) {
    issues.push({ shapeId, problem: "transform x is not a finite number — set to 0" });
  }
  if (t.y !== undefined && !Number.isFinite(t.y as number)) {
    issues.push({ shapeId, problem: "transform y is not a finite number — set to 0" });
  }
  if (typeof t.z === "number" && Number.isFinite(t.z)) out.z = t.z;
  if (typeof t.r === "number" && Number.isFinite(t.r)) out.r = t.r;
  if (typeof t.sx === "number" && Number.isFinite(t.sx)) out.sx = t.sx;
  if (typeof t.sy === "number" && Number.isFinite(t.sy)) out.sy = t.sy;
  return out;
}

/** Keep only well-formed binds ({ channel: string }); report + drop the rest. */
function normalizeBind(
  bind: unknown,
  shapeId: string,
  issues: SceneIssue[],
): Record<string, ValueRef> | undefined {
  if (bind === undefined) return undefined;
  if (!isRecord(bind)) {
    issues.push({ shapeId, problem: "bind is not an object — dropped" });
    return undefined;
  }
  const out: Record<string, ValueRef> = {};
  for (const [prop, ref] of Object.entries(bind)) {
    if (isRecord(ref) && typeof ref.channel === "string" && ref.channel.length > 0) {
      out[prop] = { channel: ref.channel };
    } else {
      issues.push({
        shapeId,
        problem: `bind "${prop}" is not { channel: string } — dropped`,
      });
    }
  }
  return Object.keys(out).length > 0 ? out : undefined;
}

function normalizeShape(raw: unknown, shapeId: string, issues: SceneIssue[]): SceneShape {
  if (!isRecord(raw)) {
    issues.push({ shapeId, problem: "shape is not an object — replaced with placeholder" });
    return { type: "unknown", t: { x: 0, y: 0 }, props: {} };
  }
  let type: string;
  if (typeof raw.type === "string" && raw.type.length > 0) {
    type = raw.type; // unknown types are FINE — ShapeNode renders the placeholder
  } else {
    issues.push({ shapeId, problem: "missing shape type — rendering placeholder" });
    type = "unknown";
  }
  const props = isRecord(raw.props) ? raw.props : {};
  if (raw.props !== undefined && !isRecord(raw.props)) {
    issues.push({ shapeId, problem: "props is not an object — reset to {}" });
  }
  const shape: SceneShape = {
    type,
    t: normalizeTransform(raw.t, shapeId, issues),
    props,
  };
  const bind = normalizeBind(raw.bind, shapeId, issues);
  if (bind) shape.bind = bind;
  return shape;
}

/** Total: any input produces a renderable SceneDoc + a list of teaching issues. */
export function validateScene(input: unknown): ValidatedScene {
  const issues: SceneIssue[] = [];
  if (!isRecord(input)) {
    issues.push({ shapeId: "(doc)", problem: "document is not an object — empty scene" });
    return { doc: { v: 1, camera: "ortho-top", shapes: {} }, issues };
  }
  if (input.v !== 1) {
    issues.push({ shapeId: "(doc)", problem: `unknown schema version ${JSON.stringify(input.v)} — treated as v1` });
  }
  let camera: CameraMode = "ortho-top";
  if (CAMERA_MODES.includes(input.camera as CameraMode)) {
    camera = input.camera as CameraMode;
  } else if (input.camera !== undefined) {
    issues.push({ shapeId: "(doc)", problem: `unknown camera ${JSON.stringify(input.camera)} — using ortho-top` });
  }
  const shapes: Record<string, SceneShape> = {};
  if (isRecord(input.shapes)) {
    for (const [id, raw] of Object.entries(input.shapes)) {
      shapes[id] = normalizeShape(raw, id, issues);
    }
  } else if (input.shapes !== undefined) {
    issues.push({ shapeId: "(doc)", problem: "shapes is not an object — empty scene" });
  }
  const doc: SceneDoc = { v: 1, camera, shapes };
  if (isRecord(input.bg) && typeof input.bg.asset === "string") {
    doc.bg = { asset: input.bg.asset };
  }
  return { doc, issues };
}
