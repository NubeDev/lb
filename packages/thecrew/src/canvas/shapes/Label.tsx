// shape.label — SDF text (drei Text), one typeface, two sizes, tabular numerals for
// bound values; never rotated in flat mode, billboarded in 3D (look-scope.md §recipe).
// v1: inherits ShapeNode rotation (acceptable) — no counter-rotation here.

import { Text } from "@react-three/drei";
import { hoverMaterial } from "../../theme/materials";
import { tokens } from "../../theme/tokens";
import type { SceneShape } from "../../scene/scene.types";
import type { ShapeComponentProps, SymbolDef } from "./shape-props";

function readText(props: Record<string, unknown>): string {
  return typeof props.text === "string" ? props.text : "";
}

function readSize(props: Record<string, unknown>): number {
  const s = props.size;
  return typeof s === "number" && Number.isFinite(s) && s > 0 ? s : 12;
}

/** Bound value display: number → 1 decimal, boolean → ON/OFF, else String(v). */
function formatValue(v: unknown): string {
  if (typeof v === "number") return v.toFixed(1);
  if (typeof v === "boolean") return v ? "ON" : "OFF";
  return String(v);
}

function labelBounds(shape: SceneShape): { w: number; h: number } {
  const text = readText(shape.props);
  const size = readSize(shape.props);
  return { w: Math.max(text.length * size * 0.6, size * 2), h: size * 2.6 };
}

export function Label({ shape, values, hovered }: ShapeComponentProps) {
  const text = readText(shape.props);
  const size = readSize(shape.props);
  const v = values.value;
  const hasValue = v !== undefined && v !== null;
  if (text === "" && !hasValue) return null;
  const b = labelBounds(shape);
  return (
    <group>
      {hovered && (
        <mesh position={[0, 0, -0.3]} material={hoverMaterial()}>
          <planeGeometry args={[b.w + 8, b.h + 8]} />
        </mesh>
      )}
      {text !== "" && (
        <Text
          position={[0, hasValue ? size * 0.8 : 0, 0.8]}
          fontSize={size}
          color={tokens.color.text.label}
          anchorX="center"
          anchorY="middle"
        >
          {text}
        </Text>
      )}
      {hasValue && (
        // live data gets the accent (look-scope §visual-language)
        <Text
          position={[0, text !== "" ? -size * 0.9 : 0, 0.8]}
          fontSize={size * 1.4}
          color={tokens.color.accent}
          anchorX="center"
          anchorY="middle"
        >
          {formatValue(v)}
        </Text>
      )}
    </group>
  );
}

export const labelDef: SymbolDef = {
  type: "plan.label",
  label: "Label",
  component: Label,
  anchors: () => [],
  bounds: labelBounds,
  propSchema: {
    text: { label: "Text", kind: "text" },
    size: { label: "Size", kind: "number" },
  },
  bindSlots: ["value"],
};
