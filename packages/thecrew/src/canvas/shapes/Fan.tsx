// hvac.fan — impeller that SPINS at bound `speed`, status ring emissive
// (`running`/`fault` → theme/materials statusMaterial). Flat: circle + impeller
// glyph · 3D: short cylinder + spinning blades. Anchors: in/out.

import { useRef } from "react";
import type { Group } from "three";
import { useFrame } from "@react-three/fiber";
import { Text } from "@react-three/drei";
import { bodyMaterial, bodyDarkMaterial, hoverMaterial, statusMaterial } from "../../theme/materials";
import { tokens } from "../../theme/tokens";
import type { ShapeComponentProps, SymbolDef } from "./shape-props";

const REDUCED_MOTION =
  typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const BODY_HEIGHT = 12;
const BLADES = 6;
const MAX_SPIN = 4; // rad/s cap — visually calm (look-scope §motion)

export function Fan({ shape, values, hovered }: ShapeComponentProps) {
  const diameter = typeof shape.props.diameter === "number" ? shape.props.diameter : 64;
  const direction = shape.props.direction === "left" ? "left" : "right";
  const label = typeof shape.props.label === "string" ? shape.props.label : "";

  const running = values.running === true;
  const fault = values.fault === true;
  const speed = typeof values.speed === "number" && Number.isFinite(values.speed) ? values.speed : 0;

  const r = diameter / 2;
  const impeller = useRef<Group>(null);

  useFrame((_, delta) => {
    if (!impeller.current || !running || REDUCED_MOTION) return;
    const omega = Math.min(MAX_SPIN, (speed / 60) * 2 * Math.PI * 0.04);
    impeller.current.rotation.z += (direction === "left" ? -1 : 1) * omega * delta;
  });

  const status = fault ? "fault" : running ? "running" : "stopped";

  return (
    <group>
      {hovered && (
        <mesh position={[0, 0, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[diameter + 10, diameter + 10]} />
        </mesh>
      )}
      {/* body: short cylinder, axis rotated onto +Z */}
      <mesh position={[0, 0, BODY_HEIGHT / 2]} rotation={[Math.PI / 2, 0, 0]} material={bodyMaterial()}>
        <cylinderGeometry args={[r, r, BODY_HEIGHT, 48]} />
      </mesh>
      {/* status ring around the rim — the bloom element */}
      <mesh position={[0, 0, BODY_HEIGHT - 2]} material={statusMaterial(status)}>
        <torusGeometry args={[r + 2.5, 1.5, 12, 64]} />
      </mesh>
      {/* impeller glyph, spinning */}
      <group ref={impeller} position={[0, 0, BODY_HEIGHT + 1.5]}>
        <mesh rotation={[Math.PI / 2, 0, 0]} material={bodyDarkMaterial()}>
          <cylinderGeometry args={[r * 0.16, r * 0.16, 3, 24]} />
        </mesh>
        {Array.from({ length: BLADES }, (_, i) => {
          const a = (i / BLADES) * Math.PI * 2;
          const d = r * 0.45;
          return (
            <mesh
              key={i}
              position={[Math.cos(a) * d, Math.sin(a) * d, 0]}
              rotation={[0, 0, a]}
              material={bodyDarkMaterial()}
            >
              <boxGeometry args={[r * 0.72, r * 0.2, 2.5]} />
            </mesh>
          );
        })}
      </group>
      {label !== "" && (
        <Text
          position={[0, -r - 14, 1]}
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

export const fanDef: SymbolDef = {
  type: "hvac.fan",
  label: "Fan",
  component: Fan,
  anchors: (shape) => {
    const d = typeof shape.props.diameter === "number" ? shape.props.diameter : 64;
    const r = d / 2;
    return [
      { name: "in", x: -r - 4, y: 0, dir: Math.PI },
      { name: "out", x: r + 4, y: 0, dir: 0 },
    ];
  },
  bounds: (shape) => {
    const d = typeof shape.props.diameter === "number" ? shape.props.diameter : 64;
    return { w: d + 8, h: d + 8 };
  },
  propSchema: {
    label: { label: "Label", kind: "text" },
    diameter: { label: "Diameter", kind: "number" },
    direction: { label: "Direction", kind: "select", options: ["left", "right"] },
  },
  bindSlots: ["running", "speed", "fault"],
};
