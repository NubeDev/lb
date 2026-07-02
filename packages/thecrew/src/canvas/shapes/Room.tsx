// plan.room — zone fill + name; `temp` binding tints the fill through a threshold
// ramp (the floor-plan money shot), `occupied` badges. Flat: tinted fill · 3D: floor
// slab tint under tilted walls.

import { useRef } from "react";
import * as THREE from "three";
import { Text } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { hoverMaterial, statusMaterial } from "../../theme/materials";
import { tokens } from "../../theme/tokens";
import type { ShapeComponentProps, SymbolDef } from "./shape-props";

/** Reduced motion: skip the breathing lerp, snap the tint (look-scope §motion). */
const REDUCED_MOTION =
  typeof window !== "undefined" &&
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

// The threshold-ramp stops — all from tokens, never local hexes.
const COLD = new THREE.Color(tokens.color.medium.chw); // ≤ 21 °C
const NEUTRAL = new THREE.Color(tokens.color.steel).multiplyScalar(0.7); // ~23 °C comfortable
const WARM = new THREE.Color(tokens.color.medium.hw); // ≥ 25 °C

/** cold ≤21 → comfortable 23 → warm ≥25, linear between the stops. */
function tempColor(temp: number, out: THREE.Color): THREE.Color {
  if (temp <= 21) return out.copy(COLD);
  if (temp >= 25) return out.copy(WARM);
  if (temp <= 23) return out.copy(COLD).lerp(NEUTRAL, (temp - 21) / 2);
  return out.copy(NEUTRAL).lerp(WARM, (temp - 23) / 2);
}

function readSize(v: unknown): number {
  return typeof v === "number" && Number.isFinite(v) && v > 0 ? v : 0;
}

export function Room({ shape, values, hovered }: ShapeComponentProps) {
  const w = readSize(shape.props.w);
  const h = readSize(shape.props.h);
  const name = typeof shape.props.name === "string" ? shape.props.name : "";
  const temp =
    typeof values.temp === "number" && Number.isFinite(values.temp) ? values.temp : undefined;
  const occupied = values.occupied === true;

  const matRef = useRef<THREE.MeshBasicMaterial>(null);
  const target = useRef(new THREE.Color().copy(NEUTRAL));
  if (temp !== undefined) tempColor(temp, target.current);
  else target.current.copy(NEUTRAL);

  useFrame(() => {
    const m = matRef.current;
    if (!m) return;
    if (REDUCED_MOTION) m.color.copy(target.current);
    else m.color.lerp(target.current, 0.06); // the floor "breathes" toward the target
  });

  if (!w || !h) return null;
  return (
    <group>
      {hovered && (
        <mesh position={[0, 0, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[w + 8, h + 8]} />
        </mesh>
      )}
      {/* the floor slab — translucent zone fill just above ground */}
      <mesh position={[0, 0, 1.2]}>
        <boxGeometry args={[w, h, 2]} />
        <meshBasicMaterial
          ref={matRef}
          color={`#${NEUTRAL.getHexString()}`}
          transparent
          opacity={0.35}
          toneMapped={false}
          depthWrite={false}
        />
      </mesh>
      {name !== "" && (
        <Text
          position={[0, 8, 3]}
          fontSize={10}
          color={tokens.color.text.label}
          anchorX="center"
          anchorY="middle"
        >
          {name}
        </Text>
      )}
      {temp !== undefined && (
        <Text
          position={[0, -6, 3]}
          fontSize={8}
          color={tokens.color.text.value}
          anchorX="center"
          anchorY="middle"
        >
          {`${temp.toFixed(1)}°`}
        </Text>
      )}
      {/* occupied dot in a corner — status is a slot, not a style */}
      <mesh
        position={[w / 2 - 10, h / 2 - 10, 3]}
        material={statusMaterial(occupied ? "running" : "stopped")}
      >
        <circleGeometry args={[3, 16]} />
      </mesh>
    </group>
  );
}

export const roomDef: SymbolDef = {
  type: "plan.room",
  label: "Room",
  component: Room,
  anchors: () => [],
  bounds: (shape) => ({ w: readSize(shape.props.w) || 96, h: readSize(shape.props.h) || 80 }),
  propSchema: {
    name: { label: "Name", kind: "text" },
    w: { label: "Width", kind: "number" },
    h: { label: "Height", kind: "number" },
  },
  bindSlots: ["temp", "occupied"],
};
