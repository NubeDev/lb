// hvac.damper — blade-in-frame that visibly SWEEPS to bound `position` (0–100)
// (look-scope.md §motion). Flat: blade glyph · 3D: framed rotating vanes.
// Anchors: in/out. Props: width, actuated?.

import { useRef } from "react";
import type { Mesh } from "three";
import { useFrame } from "@react-three/fiber";
import { Text } from "@react-three/drei";
import { bodyMaterial, bodyDarkMaterial, hoverMaterial, statusMaterial } from "../../theme/materials";
import { tokens } from "../../theme/tokens";
import type { ShapeComponentProps, SymbolDef } from "./shape-props";

const REDUCED_MOTION =
  typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const FRAME_HEIGHT = 12;
const OPEN_RAD = (85 * Math.PI) / 180; // 100% open ≈ 85° blade tilt
const VANES = 3;

export function Damper({ shape, values, hovered }: ShapeComponentProps) {
  const width = typeof shape.props.width === "number" ? shape.props.width : 48;
  const label = typeof shape.props.label === "string" ? shape.props.label : "";

  const raw = values.position;
  const position =
    typeof raw === "number" && Number.isFinite(raw) ? Math.min(100, Math.max(0, raw)) : 0;
  const target = (position / 100) * OPEN_RAD;

  const vanes = useRef<(Mesh | null)[]>([]);

  useFrame((_, delta) => {
    for (const v of vanes.current) {
      if (!v) continue;
      if (REDUCED_MOTION) v.rotation.x = target;
      else v.rotation.x += (target - v.rotation.x) * Math.min(1, delta * 6);
    }
  });

  const half = width / 2;
  const vaneLen = width - 10;
  const vaneH = (width - 16) / VANES;
  const z = FRAME_HEIGHT / 2;

  return (
    <group>
      {hovered && (
        <mesh position={[0, 0, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[width + 10, width + 10]} />
        </mesh>
      )}
      {/* open frame: two posts + top/bottom rails */}
      <mesh position={[-half, 0, z]} material={bodyMaterial()}>
        <boxGeometry args={[5, width + 5, FRAME_HEIGHT]} />
      </mesh>
      <mesh position={[half, 0, z]} material={bodyMaterial()}>
        <boxGeometry args={[5, width + 5, FRAME_HEIGHT]} />
      </mesh>
      <mesh position={[0, half, z]} material={bodyMaterial()}>
        <boxGeometry args={[width + 5, 5, FRAME_HEIGHT]} />
      </mesh>
      <mesh position={[0, -half, z]} material={bodyMaterial()}>
        <boxGeometry args={[width + 5, 5, FRAME_HEIGHT]} />
      </mesh>
      {/* vanes: rotate around their long (x) axis by bound position */}
      {Array.from({ length: VANES }, (_, i) => {
        const y = (i - (VANES - 1) / 2) * (vaneH + 3);
        return (
          <mesh
            key={i}
            ref={(m) => {
              vanes.current[i] = m;
            }}
            position={[0, y, z]}
            material={bodyDarkMaterial()}
          >
            <boxGeometry args={[vaneLen, vaneH, 2]} />
          </mesh>
        );
      })}
      {/* status tick: open = running glow, closed = dim */}
      <mesh
        position={[half + 2, half + 2, FRAME_HEIGHT + 1]}
        material={statusMaterial(position > 3 ? "running" : "stopped")}
      >
        <circleGeometry args={[2.5, 16]} />
      </mesh>
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

export const damperDef: SymbolDef = {
  type: "hvac.damper",
  label: "Damper",
  component: Damper,
  anchors: (shape) => {
    const w = typeof shape.props.width === "number" ? shape.props.width : 48;
    return [
      { name: "in", x: -w / 2 - 4, y: 0, dir: Math.PI },
      { name: "out", x: w / 2 + 4, y: 0, dir: 0 },
    ];
  },
  bounds: (shape) => {
    const w = typeof shape.props.width === "number" ? shape.props.width : 48;
    return { w: w + 8, h: w + 8 };
  },
  propSchema: {
    label: { label: "Label", kind: "text" },
    width: { label: "Width", kind: "number" },
    actuated: { label: "Actuated", kind: "boolean" },
  },
  bindSlots: ["position"],
};
