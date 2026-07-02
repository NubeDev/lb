// plan.door — snaps into a wall segment, cuts it. Flat: arc-swing glyph · 3D: wall
// gap + leaf. Props: width, swing side. Hinge at shape origin; the leaf stands open
// along +Y, the quarter-arc sweeps from the closed position (+X) to the leaf.

import { useMemo } from "react";
import * as THREE from "three";
import { bodyMaterial, hoverMaterial } from "../../theme/materials";
import { tokens } from "../../theme/tokens";
import type { ShapeComponentProps, SymbolDef } from "./shape-props";

function readWidth(v: unknown): number {
  return typeof v === "number" && Number.isFinite(v) && v > 0 ? v : 0;
}

export function Door({ shape, hovered }: ShapeComponentProps) {
  const width = readWidth(shape.props.width);
  const swing = shape.props.swing === "right" ? "right" : "left";
  const arc = useMemo(() => {
    const w = width || 32;
    // thin ring segment: the swing path, from closed (+X) to open (+Y)
    return new THREE.RingGeometry(Math.max(w - 2, 1), w, 24, 1, 0, Math.PI / 2);
  }, [width]);
  if (!width) return null;
  const mirror = swing === "right" ? -1 : 1;
  return (
    <group>
      {hovered && (
        <mesh position={[0, 0, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[width + 8, width + 8]} />
        </mesh>
      )}
      <group scale={[1, mirror, 1]}>
        <mesh geometry={arc} position={[0, 0, 0.6]}>
          <meshBasicMaterial
            color={tokens.color.text.label}
            transparent
            opacity={0.5}
            toneMapped={false}
            depthWrite={false}
            side={THREE.DoubleSide}
          />
        </mesh>
        {/* the door leaf, standing open from the hinge — reads as gap + leaf in 3D */}
        <mesh position={[0, width / 2, 4]} material={bodyMaterial()}>
          <boxGeometry args={[3, width, 8]} />
        </mesh>
      </group>
    </group>
  );
}

export const doorDef: SymbolDef = {
  type: "plan.door",
  label: "Door",
  component: Door,
  anchors: () => [{ name: "hinge", x: 0, y: 0, dir: 0 }],
  bounds: (shape) => {
    const w = readWidth(shape.props.width) || 32;
    return { w: w + 8, h: w + 8 };
  },
  propSchema: {
    label: { label: "Label", kind: "text" },
    width: { label: "Width", kind: "number" },
    swing: { label: "Swing", kind: "select", options: ["left", "right"] },
  },
  bindSlots: [],
};
