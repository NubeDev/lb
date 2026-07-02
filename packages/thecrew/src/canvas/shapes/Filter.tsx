// hvac.filter — hatched panel; `dp` (pressure drop) tints toward "dirty".
// Flat: hatched panel · 3D: thin slab with hatch texture. Anchors: in/out.

import { Text } from "@react-three/drei";
import { bodyMaterial, bodyDarkMaterial, hoverMaterial, statusMaterial } from "../../theme/materials";
import { tokens } from "../../theme/tokens";
import type { ShapeComponentProps, SymbolDef } from "./shape-props";

const SLAB_HEIGHT = 12;
const HATCH_BARS = 7;
const DP_CLEAN = 40; // Pa — no dirt tint below this
const DP_DIRTY = 220; // Pa — full dirt tint

export function Filter({ shape, values, hovered }: ShapeComponentProps) {
  const width = typeof shape.props.width === "number" ? shape.props.width : 48;
  const stages = typeof shape.props.stages === "number" ? Math.max(1, shape.props.stages) : 1;
  const label = typeof shape.props.label === "string" ? shape.props.label : "";

  const dp = typeof values.dp === "number" && Number.isFinite(values.dp) ? values.dp : 0;
  const fault = values.fault === true;

  // dirt tint: clean (transparent) → amber as dp climbs 40→220 Pa
  const dirt = Math.min(1, Math.max(0, (dp - DP_CLEAN) / (DP_DIRTY - DP_CLEAN)));

  // thin across the airstream (x), spans the duct in y; each extra stage deepens the slab
  const slabW = 14 + stages * 6;
  const half = width / 2;
  const barLen = slabW * 1.9;
  const faceZ = SLAB_HEIGHT + 0.4;

  return (
    <group>
      {hovered && (
        <mesh position={[0, 0, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[width + 10, width + 18]} />
        </mesh>
      )}
      {/* slab body */}
      <mesh position={[0, 0, SLAB_HEIGHT / 2]} material={bodyMaterial()}>
        <boxGeometry args={[slabW, width, SLAB_HEIGHT]} />
      </mesh>
      {/* hatch glyph: thin diagonal bars across the face */}
      {Array.from({ length: HATCH_BARS }, (_, i) => {
        const y = (i - (HATCH_BARS - 1) / 2) * (width / HATCH_BARS);
        return (
          <mesh
            key={i}
            position={[0, y, faceZ]}
            rotation={[0, 0, Math.PI / 4]}
            material={bodyDarkMaterial()}
          >
            <boxGeometry args={[barLen, 2, 1.2]} />
          </mesh>
        );
      })}
      {/* dirt tint overlay — token amber, opacity mapped 0 → 0.35 */}
      <mesh position={[0, 0, faceZ + 1]}>
        <planeGeometry args={[slabW, width]} />
        <meshBasicMaterial
          color={tokens.color.status.fault}
          transparent
          opacity={dirt * 0.35}
          depthWrite={false}
          toneMapped={false}
        />
      </mesh>
      {fault && (
        <mesh position={[slabW / 2 + 4, half + 4, faceZ + 1]} material={statusMaterial("fault")}>
          <circleGeometry args={[2.5, 16]} />
        </mesh>
      )}
      {label !== "" && (
        <Text
          position={[0, -half - 14, 1]}
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

export const filterDef: SymbolDef = {
  type: "hvac.filter",
  label: "Filter",
  component: Filter,
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
    stages: { label: "Stages", kind: "number" },
  },
  bindSlots: ["dp", "fault"],
};
