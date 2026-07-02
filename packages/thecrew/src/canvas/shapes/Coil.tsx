// hvac.coil — zigzag coil tinted by medium (chw/hw); binds `valve`, `temp_in`,
// `temp_out`. Flat: zigzag glyph · 3D: finned slab. Anchors: in/out.

import { useMemo } from "react";
import { Text } from "@react-three/drei";
import { bodyMaterial, hoverMaterial, mediumColor, statusMaterial } from "../../theme/materials";
import { tokens } from "../../theme/tokens";
import type { ShapeComponentProps, SymbolDef } from "./shape-props";

const SLAB_HEIGHT = 12;
const SLAB_W = 22; // thin across the airstream (x), spans the duct in y
const ZIGS = 6;

function fmtTemp(v: unknown): string {
  return typeof v === "number" && Number.isFinite(v) ? `${v.toFixed(1)}°` : "";
}

interface Zig {
  cx: number;
  cy: number;
  len: number;
  angle: number;
}

export function Coil({ shape, values, hovered }: ShapeComponentProps) {
  const width = typeof shape.props.width === "number" ? shape.props.width : 48;
  const medium = typeof shape.props.medium === "string" ? shape.props.medium : "chw";
  const label = typeof shape.props.label === "string" ? shape.props.label : "";

  const rawValve = values.valve;
  const valve =
    typeof rawValve === "number" && Number.isFinite(rawValve)
      ? Math.min(100, Math.max(0, rawValve))
      : 0;
  const tempIn = fmtTemp(values.temp_in);
  const tempOut = fmtTemp(values.temp_out);

  const half = width / 2;
  const faceZ = SLAB_HEIGHT + 0.5;

  // zigzag glyph: alternating diagonal runs down the face — the medium cue
  const zigs = useMemo<Zig[]>(() => {
    const amp = SLAB_W * 0.32;
    const top = half - 6;
    const step = ((width - 12) / ZIGS) || 1;
    const out: Zig[] = [];
    for (let i = 0; i < ZIGS; i++) {
      const x0 = i % 2 === 0 ? -amp : amp;
      const x1 = -x0;
      const y0 = top - i * step;
      const y1 = top - (i + 1) * step;
      out.push({
        cx: (x0 + x1) / 2,
        cy: (y0 + y1) / 2,
        len: Math.hypot(x1 - x0, y1 - y0),
        angle: Math.atan2(y1 - y0, x1 - x0),
      });
    }
    return out;
  }, [width, half]);

  const valveMax = width * 0.6;
  const valveH = Math.max(0.5, (valve / 100) * valveMax);

  return (
    <group>
      {hovered && (
        <mesh position={[0, 0, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[width + 10, width + 18]} />
        </mesh>
      )}
      {/* slab body */}
      <mesh position={[0, 0, SLAB_HEIGHT / 2]} material={bodyMaterial()}>
        <boxGeometry args={[SLAB_W, width, SLAB_HEIGHT]} />
      </mesh>
      {/* zigzag glyph tinted by medium — the recognizable cue */}
      {zigs.map((s, i) => (
        <mesh key={i} position={[s.cx, s.cy, faceZ]} rotation={[0, 0, s.angle]}>
          <boxGeometry args={[s.len, 2.4, 1.2]} />
          <meshBasicMaterial color={mediumColor(medium)} transparent opacity={0.9} toneMapped={false} />
        </mesh>
      ))}
      {/* valve indicator: bar height = valve %, glows when open */}
      <mesh
        position={[SLAB_W / 2 + 5, -valveMax / 2 + valveH / 2, SLAB_HEIGHT / 2]}
        material={statusMaterial(valve > 3 ? "running" : "stopped")}
      >
        <boxGeometry args={[4, valveH, 3]} />
      </mesh>
      {tempIn !== "" && (
        <Text
          position={[-width / 4, -half - 10, 1]}
          fontSize={7}
          color={tokens.color.text.value}
          anchorX="center"
          anchorY="middle"
        >
          {tempIn}
        </Text>
      )}
      {tempOut !== "" && (
        <Text
          position={[width / 4, -half - 10, 1]}
          fontSize={7}
          color={tokens.color.text.value}
          anchorX="center"
          anchorY="middle"
        >
          {tempOut}
        </Text>
      )}
      {label !== "" && (
        <Text
          position={[0, -half - 22, 1]}
          fontSize={9}
          color={tokens.color.text.label}
          anchorX="center"
          anchorY="middle"
        >
          {label}
        </Text>
      )}
    </group>
  );
}

export const coilDef: SymbolDef = {
  type: "hvac.coil",
  label: "Coil",
  component: Coil,
  anchors: (shape) => {
    const w = typeof shape.props.width === "number" ? shape.props.width : 48;
    return [
      { name: "in", x: -w / 2 - 4, y: 0, dir: Math.PI },
      { name: "out", x: w / 2 + 4, y: 0, dir: 0 },
    ];
  },
  bounds: (shape) => {
    const w = typeof shape.props.width === "number" ? shape.props.width : 48;
    return { w: w + 8, h: w + 16 };
  },
  propSchema: {
    label: { label: "Label", kind: "text" },
    width: { label: "Width", kind: "number" },
    medium: { label: "Medium", kind: "select", options: ["chw", "hw"] },
  },
  bindSlots: ["valve", "temp_in", "temp_out"],
};
