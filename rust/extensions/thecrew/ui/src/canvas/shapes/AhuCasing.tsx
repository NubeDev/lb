// hvac.ahu-casing — the unit enclosure + name plate; `status` rollup emissive.
// Flat: rounded rect outline · 3D: low extruded enclosure, open top. Container: no anchors.

import { useEffect, useMemo } from "react";
import * as THREE from "three";
import { Text } from "@react-three/drei";
import { bodyMaterial, bodyDarkMaterial, hoverMaterial, statusMaterial } from "../../theme/materials";
import { tokens } from "../../theme/tokens";
import type { ShapeComponentProps, SymbolDef } from "./shape-props";

const WALL = 6; // ring wall thickness
const WALL_HEIGHT = 10;

/** Rounded-rect path centered on the origin (absarc corners). */
function roundedRect(w: number, h: number, r: number): THREE.Shape {
  const s = new THREE.Shape();
  const x = -w / 2;
  const y = -h / 2;
  s.moveTo(x + r, y);
  s.lineTo(x + w - r, y);
  s.absarc(x + w - r, y + r, r, -Math.PI / 2, 0, false);
  s.lineTo(x + w, y + h - r);
  s.absarc(x + w - r, y + h - r, r, 0, Math.PI / 2, false);
  s.lineTo(x + r, y + h);
  s.absarc(x + r, y + h - r, r, Math.PI / 2, Math.PI, false);
  s.lineTo(x, y + r);
  s.absarc(x + r, y + r, r, Math.PI, Math.PI * 1.5, false);
  return s;
}

export function AhuCasing({ shape, values, hovered }: ShapeComponentProps) {
  const w = typeof shape.props.w === "number" ? shape.props.w : 320;
  const h = typeof shape.props.h === "number" ? shape.props.h : 128;
  const name = typeof shape.props.name === "string" ? shape.props.name : "";
  const status = values.status === true;

  // wall ring: outer rounded rect minus inner rounded rect, extruded thin
  const ring = useMemo(() => {
    const outer = roundedRect(w, h, 10);
    outer.holes.push(roundedRect(w - WALL * 2, h - WALL * 2, 6));
    return new THREE.ExtrudeGeometry(outer, { depth: WALL_HEIGHT, bevelEnabled: false });
  }, [w, h]);

  useEffect(() => () => ring.dispose(), [ring]);

  const plateW = Math.min(72, w - WALL * 2 - 8);
  const plateX = -w / 2 + WALL + plateW / 2 + 6;
  const plateY = h / 2 - WALL - 14;

  return (
    <group>
      {hovered && (
        <mesh position={[0, 0, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[w + 6, h + 6]} />
        </mesh>
      )}
      <mesh geometry={ring} material={bodyMaterial()} />
      {/* name plate: dark plate + name + status dot, top-left inside the outline */}
      <group position={[plateX, plateY, WALL_HEIGHT + 0.5]}>
        <mesh position={[0, 0, 0]} material={bodyDarkMaterial()}>
          <boxGeometry args={[plateW, 18, 2]} />
        </mesh>
        <mesh position={[-plateW / 2 + 8, 0, 1.6]} material={statusMaterial(status ? "running" : "stopped")}>
          <circleGeometry args={[2.5, 16]} />
        </mesh>
        {name !== "" && (
          <Text
            position={[4, 0, 1.6]}
            fontSize={11}
            color={tokens.color.text.value}
            anchorX="center"
            anchorY="middle"
          >
            {name}
          </Text>
        )}
      </group>
    </group>
  );
}

export const casingDef: SymbolDef = {
  type: "hvac.casing",
  label: "AHU casing",
  component: AhuCasing,
  anchors: () => [], // container — no anchors
  bounds: (shape) => ({
    w: typeof shape.props.w === "number" ? shape.props.w : 320,
    h: typeof shape.props.h === "number" ? shape.props.h : 128,
  }),
  propSchema: {
    name: { label: "Name", kind: "text" },
    w: { label: "Width", kind: "number" },
    h: { label: "Height", kind: "number" },
  },
  bindSlots: ["status"],
};
