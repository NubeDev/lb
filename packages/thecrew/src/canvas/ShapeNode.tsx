// type → symbol component dispatch, plus the three cross-cutting behaviors every
// shape gets for free: the labeled PLACEHOLDER box for unknown types (never a crash —
// symbols-scope.md §placeholder), the cyan selection halo, and bound-value resolution
// (subscribes shape.bind channels via the ValueSource seam, passes values down).

import { useMemo } from "react";
import * as THREE from "three";
import { Text } from "@react-three/drei";
import type { ThreeEvent } from "@react-three/fiber";
import type { SceneShape } from "../scene/scene.types";
import { useValues } from "../data/use-values";
import { useSceneStore } from "../state/scene-store";
import { haloMaterial } from "../theme/materials";
import { tokens } from "../theme/tokens";
import type { SymbolDef } from "./shapes/shape-props";
import { ductDef } from "./shapes/Duct";
import { fanDef } from "./shapes/Fan";
import { damperDef } from "./shapes/Damper";
import { filterDef } from "./shapes/Filter";
import { coilDef } from "./shapes/Coil";
import { casingDef } from "./shapes/AhuCasing";
import { wallDef } from "./shapes/Wall";
import { roomDef } from "./shapes/Room";
import { doorDef } from "./shapes/Door";
import { labelDef } from "./shapes/Label";

/** The symbol registry — the playground's hand-written "symbol pack".
 * Palette + PropertyRail read this too. */
export const SYMBOLS: Record<string, SymbolDef> = Object.fromEntries(
  [ductDef, fanDef, damperDef, filterDef, coilDef, casingDef, wallDef, roomDef, doorDef, labelDef].map(
    (d) => [d.type, d],
  ),
);

/** Unknown type → dashed-feel outline + type name. The framework's
 * missing-symbol-pack behavior, proven here. Never a crash. */
function Placeholder({ type }: { type: string }) {
  const edges = useMemo(() => new THREE.EdgesGeometry(new THREE.PlaneGeometry(64, 48)), []);
  return (
    <group>
      <mesh position={[0, 0, 0.5]}>
        <planeGeometry args={[64, 48]} />
        <meshBasicMaterial color={tokens.color.steel} transparent opacity={0.12} />
      </mesh>
      <lineSegments position={[0, 0, 0.6]} geometry={edges}>
        <lineBasicMaterial color={tokens.color.text.label} />
      </lineSegments>
      <Text position={[0, 0, 0.7]} fontSize={9} color={tokens.color.text.label} anchorX="center">
        {type}
      </Text>
    </group>
  );
}

export function ShapeNode({ id, shape }: { id: string; shape: SceneShape }) {
  const values = useValues(shape.bind);
  const selected = useSceneStore((s) => s.selection.includes(id));
  const hovered = useSceneStore((s) => s.hovered === id);
  const select = useSceneStore((s) => s.select);
  const setHovered = useSceneStore((s) => s.setHovered);

  const def = SYMBOLS[shape.type];
  const bounds = def ? def.bounds(shape) : { w: 64, h: 48 };

  function onClick(e: ThreeEvent<MouseEvent>) {
    e.stopPropagation();
    select([id], e.shiftKey);
  }

  return (
    <group
      position={[shape.t.x, shape.t.y, 0]}
      rotation={[0, 0, shape.t.r ?? 0]}
      scale={[shape.t.sx ?? 1, shape.t.sy ?? 1, 1]}
      onClick={onClick}
      onPointerOver={(e) => {
        e.stopPropagation();
        setHovered(id);
      }}
      onPointerOut={() => setHovered(null)}
    >
      {selected && (
        <mesh position={[0, 0, -0.4]} material={haloMaterial()}>
          <planeGeometry args={[bounds.w + 14, bounds.h + 14]} />
        </mesh>
      )}
      {hovered &&
        def?.anchors(shape).map((a) => (
          // anchors show on hover only (symbols-scope §design-language)
          <mesh key={a.name} position={[a.x, a.y, 2]}>
            <circleGeometry args={[3, 16]} />
            <meshBasicMaterial color={tokens.color.accent} transparent opacity={0.7} />
          </mesh>
        ))}
      {def ? <def.component shape={shape} values={values} selected={selected} hovered={hovered} /> : <Placeholder type={shape.type} />}
    </group>
  );
}
