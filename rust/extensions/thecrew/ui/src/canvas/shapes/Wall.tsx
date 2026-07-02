// plan.wall — wall chain (polyline, same drawing gesture as ducts). Flat: crisp
// double-line · 3D: extruded wall (2.7 m default). Anchors: endpoints + midpoints
// (doors snap INTO segments).

import { useMemo } from "react";
import * as THREE from "three";
import type { SceneShape } from "../../scene/scene.types";
import { bodyMaterial, hoverMaterial } from "../../theme/materials";
import { tokens } from "../../theme/tokens";
import type { Anchor, ShapeComponentProps, SymbolDef } from "./shape-props";

const WALL_HEIGHT = 20; // Z extrusion — the phase-4 tilt gets real walls for free

type Pt = [number, number];

/** Tolerant read: points must be ≥2 finite [x,y] pairs, else null (render nothing). */
function readPoints(props: Record<string, unknown>): Pt[] | null {
  const raw = props.points;
  if (!Array.isArray(raw) || raw.length < 2) return null;
  const pts: Pt[] = [];
  for (const p of raw) {
    if (!Array.isArray(p) || !Number.isFinite(p[0]) || !Number.isFinite(p[1])) return null;
    pts.push([p[0] as number, p[1] as number]);
  }
  return pts;
}

function readThickness(props: Record<string, unknown>): number {
  const t = props.thickness;
  return typeof t === "number" && Number.isFinite(t) && t > 0 ? t : 8;
}

interface Segment {
  mid: Pt;
  len: number;
  angle: number;
}

function segmentsOf(pts: Pt[]): Segment[] {
  const out: Segment[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const [ax, ay] = pts[i];
    const [bx, by] = pts[i + 1];
    const dx = bx - ax;
    const dy = by - ay;
    const len = Math.hypot(dx, dy);
    if (len < 1e-6) continue;
    out.push({ mid: [(ax + bx) / 2, (ay + by) / 2], len, angle: Math.atan2(dy, dx) });
  }
  return out;
}

function bbox(pts: Pt[]): { cx: number; cy: number; w: number; h: number } {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const [x, y] of pts) {
    minX = Math.min(minX, x); maxX = Math.max(maxX, x);
    minY = Math.min(minY, y); maxY = Math.max(maxY, y);
  }
  return { cx: (minX + maxX) / 2, cy: (minY + maxY) / 2, w: maxX - minX, h: maxY - minY };
}

/** Thin lighter edge lines along both faces of every segment (the crisp double-line
 * read in ortho-top), drawn on the wall's top face. */
function edgeGeometry(pts: Pt[], thickness: number): THREE.BufferGeometry {
  const half = thickness / 2;
  const z = WALL_HEIGHT + 0.3;
  const verts: number[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const [ax, ay] = pts[i];
    const [bx, by] = pts[i + 1];
    const len = Math.hypot(bx - ax, by - ay);
    if (len < 1e-6) continue;
    const px = (-(by - ay) / len) * half; // perpendicular offset
    const py = ((bx - ax) / len) * half;
    verts.push(ax + px, ay + py, z, bx + px, by + py, z);
    verts.push(ax - px, ay - py, z, bx - px, by - py, z);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(verts, 3));
  return g;
}

export function Wall({ shape, hovered }: ShapeComponentProps) {
  const pts = useMemo(() => readPoints(shape.props), [shape.props]);
  const thickness = readThickness(shape.props);
  const segs = useMemo(() => (pts ? segmentsOf(pts) : []), [pts]);
  const edges = useMemo(() => (pts ? edgeGeometry(pts, thickness) : null), [pts, thickness]);
  const joints = useMemo(() => {
    if (!pts) return [];
    const closed =
      pts.length > 2 && pts[0][0] === pts[pts.length - 1][0] && pts[0][1] === pts[pts.length - 1][1];
    // interior corners keep chains continuous; a closed loop's seam is a corner too
    const j = pts.slice(1, -1);
    if (closed) j.push(pts[0]);
    return j;
  }, [pts]);
  if (!pts || segs.length === 0) return null;
  const box = bbox(pts);
  return (
    <group>
      {hovered && (
        <mesh position={[box.cx, box.cy, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[box.w + thickness + 8, box.h + thickness + 8]} />
        </mesh>
      )}
      {segs.map((s, i) => (
        <mesh
          key={`s${i}`}
          position={[s.mid[0], s.mid[1], WALL_HEIGHT / 2]}
          rotation={[0, 0, s.angle]}
          material={bodyMaterial()}
        >
          <boxGeometry args={[s.len, thickness, WALL_HEIGHT]} />
        </mesh>
      ))}
      {joints.map(([x, y], i) => (
        // square joint boxes so corner miters read as one continuous wall
        <mesh key={`j${i}`} position={[x, y, WALL_HEIGHT / 2]} material={bodyMaterial()}>
          <boxGeometry args={[thickness, thickness, WALL_HEIGHT + 0.2]} />
        </mesh>
      ))}
      {edges && (
        <lineSegments geometry={edges}>
          <lineBasicMaterial color={tokens.color.text.label} transparent opacity={0.35} />
        </lineSegments>
      )}
    </group>
  );
}

function wallAnchors(shape: SceneShape): Anchor[] {
  const pts = readPoints(shape.props);
  if (!pts) return [];
  const anchors: Anchor[] = [];
  for (let i = 0; i < pts.length; i++) {
    // endpoint dir: along the adjacent segment, pointing outward
    const [ax, ay] = pts[i];
    const [nx, ny] = i === 0 ? pts[1] : pts[i - 1];
    const dir = Math.atan2(ay - ny, ax - nx);
    anchors.push({ name: `p${i}`, x: ax, y: ay, dir });
  }
  for (let i = 0; i < pts.length - 1; i++) {
    const [ax, ay] = pts[i];
    const [bx, by] = pts[i + 1];
    // midpoint dir: perpendicular to the segment (doors snap INTO segments)
    const dir = Math.atan2(by - ay, bx - ax) + Math.PI / 2;
    anchors.push({ name: `m${i}`, x: (ax + bx) / 2, y: (ay + by) / 2, dir });
  }
  return anchors;
}

export const wallDef: SymbolDef = {
  type: "plan.wall",
  label: "Wall",
  component: Wall,
  anchors: wallAnchors,
  bounds: (shape) => {
    const pts = readPoints(shape.props);
    const t = readThickness(shape.props);
    if (!pts) return { w: t, h: t };
    const box = bbox(pts);
    return { w: box.w + t, h: box.h + t };
  },
  propSchema: {
    label: { label: "Label", kind: "text" },
    thickness: { label: "Thickness", kind: "number" },
  },
  bindSlots: [],
};
