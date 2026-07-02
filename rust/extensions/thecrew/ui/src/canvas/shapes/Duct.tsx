// hvac.duct — the duct run: polyline with styled corners + ANIMATED chevron flow
// (speed bound to `flow`), the single most important "it's alive" cue
// (look-scope.md §recipe). Flat: styled polyline · 3D: shallow rectangular channel.
// Anchors: every endpoint. Props: path points, width, medium.

import { useEffect, useMemo } from "react";
import { useFrame } from "@react-three/fiber";
import { ductBodyMaterial, ductFlowMaterial, hoverMaterial } from "../../theme/materials";
import type { Anchor, ShapeComponentProps, SymbolDef } from "./shape-props";

const REDUCED_MOTION =
  typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const CHANNEL_HEIGHT = 6;

type Pt = [number, number];

/** Coerce the untyped `points` prop into a valid polyline (≥2 points) or null. */
function readPoints(raw: unknown): Pt[] | null {
  if (!Array.isArray(raw)) return null;
  const pts: Pt[] = [];
  for (const p of raw) {
    if (Array.isArray(p) && typeof p[0] === "number" && typeof p[1] === "number") {
      pts.push([p[0], p[1]]);
    }
  }
  return pts.length >= 2 ? pts : null;
}

interface Segment {
  cx: number;
  cy: number;
  len: number;
  angle: number;
}

function toSegments(pts: Pt[]): Segment[] {
  const segs: Segment[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const [x0, y0] = pts[i];
    const [x1, y1] = pts[i + 1];
    const dx = x1 - x0;
    const dy = y1 - y0;
    const len = Math.hypot(dx, dy);
    if (len < 1e-6) continue;
    segs.push({ cx: (x0 + x1) / 2, cy: (y0 + y1) / 2, len, angle: Math.atan2(dy, dx) });
  }
  return segs;
}

export function Duct({ shape, values, hovered }: ShapeComponentProps) {
  const points = readPoints(shape.props.points);
  const width = typeof shape.props.width === "number" ? shape.props.width : 40;
  const medium = typeof shape.props.medium === "string" ? shape.props.medium : "air";
  const flow = typeof values.flow === "number" && Number.isFinite(values.flow) ? values.flow : 0;

  const segments = useMemo(() => (points ? toSegments(points) : []), [JSON.stringify(points)]);

  // One flow material+texture per segment (created via the theme factory — the only
  // sanctioned way to get an animated chevron), repeat scaled to segment length.
  const flows = useMemo(
    () =>
      segments.map((s) => {
        const f = ductFlowMaterial(medium);
        f.texture.repeat.x = s.len / width;
        return f;
      }),
    [segments, medium, width],
  );

  useEffect(
    () => () => {
      for (const f of flows) {
        f.texture.dispose();
        f.material.dispose();
      }
    },
    [flows],
  );

  useFrame((_, delta) => {
    const scrolling = flow > 0 && !REDUCED_MOTION;
    for (const f of flows) {
      f.material.opacity = flow > 0 ? 0.75 : 0.22;
      if (scrolling) f.texture.offset.x -= (flow / 900) * delta * 1.5;
    }
  });

  if (!points) return null; // garbage/missing path → render nothing, never crash

  const interior = points.slice(1, -1);
  const bounds = ductBounds(points, width);

  return (
    <group>
      {hovered && (
        <mesh position={[bounds.cx, bounds.cy, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[bounds.w + 6, bounds.h + 6]} />
        </mesh>
      )}
      {segments.map((s, i) => (
        <group key={i} position={[s.cx, s.cy, 0]} rotation={[0, 0, s.angle]}>
          <mesh position={[0, 0, CHANNEL_HEIGHT / 2]} material={ductBodyMaterial()}>
            <boxGeometry args={[s.len, width, CHANNEL_HEIGHT]} />
          </mesh>
          <mesh position={[0, 0, CHANNEL_HEIGHT + 0.4]} material={flows[i]?.material}>
            <planeGeometry args={[s.len, width]} />
          </mesh>
        </group>
      ))}
      {interior.map(([x, y], i) => (
        // rounded joint discs so corners look styled (builder-ux polyline-with-styled-corners)
        <mesh
          key={`j${i}`}
          position={[x, y, CHANNEL_HEIGHT / 2]}
          rotation={[Math.PI / 2, 0, 0]}
          material={ductBodyMaterial()}
        >
          <cylinderGeometry args={[width / 2, width / 2, CHANNEL_HEIGHT, 24]} />
        </mesh>
      ))}
    </group>
  );
}

function ductBounds(pts: Pt[], width: number) {
  let minX = Infinity,
    minY = Infinity,
    maxX = -Infinity,
    maxY = -Infinity;
  for (const [x, y] of pts) {
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x);
    maxY = Math.max(maxY, y);
  }
  return {
    w: maxX - minX + width,
    h: maxY - minY + width,
    cx: (minX + maxX) / 2,
    cy: (minY + maxY) / 2,
  };
}

export const ductDef: SymbolDef = {
  type: "hvac.duct",
  label: "Duct run",
  component: Duct,
  anchors: (shape) => {
    const pts = readPoints(shape.props.points);
    if (!pts) return [];
    const first = pts[0];
    const second = pts[1];
    const last = pts[pts.length - 1];
    const prev = pts[pts.length - 2];
    const a: Anchor[] = [
      { name: "p0", x: first[0], y: first[1], dir: Math.atan2(first[1] - second[1], first[0] - second[0]) },
      { name: "p1", x: last[0], y: last[1], dir: Math.atan2(last[1] - prev[1], last[0] - prev[0]) },
    ];
    return a;
  },
  bounds: (shape) => {
    const pts = readPoints(shape.props.points);
    const width = typeof shape.props.width === "number" ? shape.props.width : 40;
    if (!pts) return { w: width, h: width };
    const b = ductBounds(pts, width);
    return { w: b.w, h: b.h };
  },
  propSchema: {
    label: { label: "Label", kind: "text" },
    width: { label: "Width", kind: "number" },
    medium: { label: "Medium", kind: "select", options: ["air", "chw", "hw"] },
  },
  bindSlots: ["flow"],
};
