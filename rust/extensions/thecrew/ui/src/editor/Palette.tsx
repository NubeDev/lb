// The symbol palette (builder-ux-scope.md §palette): HVAC + Floor-plan categories,
// search, big LIVE thumbnails (the real component rendered small — fans spin on
// hover). Drag-out and click-to-arm both start use-drag-place.
//
// Each tile is a tiny r3f <Canvas> (frameloop "demand" until hovered — hover demos
// the symbol with live-ish values). HTML5 drag sets "thecrew/type" AND arms the type
// so the on-canvas ghost renders the right symbol during dragover (use-drag-place).

import { useMemo, useState } from "react";
import { Canvas } from "@react-three/fiber";
import { Search } from "lucide-react";
import clsx from "clsx";
import { SYMBOLS } from "../canvas/ShapeNode";
import type { SymbolDef } from "../canvas/shapes/shape-props";
import { defaultShape } from "../scene/defaults";
import { useSceneStore } from "../state/scene-store";

/** hover-demo values — enough to make every symbol animate (fan spins, damper sweeps) */
const DEMO_VALUES = { running: true, speed: 900, position: 65, dp: 120, valve: 60, temp: 24.2, flow: 900 };

/** point-based shapes (duct/wall) draw from local origin — center them in the tile */
function centerOffset(shape: ReturnType<typeof defaultShape>): [number, number] {
  const pts = shape.props.points as [number, number][] | undefined;
  if (!Array.isArray(pts) || pts.length === 0) return [0, 0];
  const xs = pts.map((p) => p[0]);
  const ys = pts.map((p) => p[1]);
  return [-(Math.min(...xs) + Math.max(...xs)) / 2, -(Math.min(...ys) + Math.max(...ys)) / 2];
}

function Tile({ def }: { def: SymbolDef }) {
  const [hovered, setHovered] = useState(false);
  const armed = useSceneStore((s) => s.armedType === def.type);
  const armType = useSceneStore((s) => s.armType);
  const shape = useMemo(() => defaultShape(def.type), [def.type]);
  const bounds = def.bounds(shape);
  const offset = centerOffset(shape);
  // fit def.bounds into the ~184×80px tile viewport, top-down ortho
  const zoom = Math.min(170 / Math.max(bounds.w, 1), 66 / Math.max(bounds.h, 1)) * 0.85;

  return (
    <button
      type="button"
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("thecrew/type", def.type);
        e.dataTransfer.effectAllowed = "copy";
        armType(def.type); // ghost follows during dragover (use-drag-place.ts)
      }}
      onDragEnd={() => {
        // drop already disarmed on success; this catches a cancelled drag
        if (useSceneStore.getState().armedType === def.type) armType(null);
      }}
      onClick={() => armType(armed ? null : def.type)} // click again disarms
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      title={armed ? `${def.label} — armed, click the canvas to place` : `${def.label} — click to arm, or drag onto the canvas`}
      className={clsx(
        "block w-full cursor-grab rounded-md border text-left outline-none transition-colors focus-visible:ring-1 focus-visible:ring-[var(--tc-accent)]",
        armed
          ? "border-transparent ring-1 ring-[var(--tc-accent)]"
          : "border-[var(--tc-hairline)] hover:border-[var(--tc-accent)]/40",
      )}
    >
      <div className="pointer-events-none h-20 w-full">
        <Canvas
          orthographic
          dpr={1}
          frameloop={hovered ? "always" : "demand"}
          camera={{ position: [0, 0, 100], up: [0, 1, 0], zoom }}
          gl={{ antialias: true, alpha: true }}
        >
          <ambientLight intensity={0.8} />
          <directionalLight position={[60, 80, 120]} intensity={1.1} />
          <group position={[offset[0], offset[1], 0]}>
            <def.component shape={shape} values={hovered ? DEMO_VALUES : {}} selected={false} hovered={false} />
          </group>
        </Canvas>
      </div>
      <div
        className={clsx(
          "px-2 pb-1.5 text-xs",
          armed ? "text-[var(--tc-accent)]" : "text-[var(--tc-text-muted)]",
        )}
      >
        {def.label}
      </div>
    </button>
  );
}

function Category({ title, defs }: { title: string; defs: SymbolDef[] }) {
  if (defs.length === 0) return null;
  return (
    <section className="space-y-2">
      <h2 className="text-[10px] font-medium uppercase tracking-widest text-[var(--tc-text-muted)]">{title}</h2>
      {defs.map((def) => (
        <Tile key={def.type} def={def} />
      ))}
    </section>
  );
}

export function Palette() {
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  const defs = Object.values(SYMBOLS).filter((d) => !q || d.label.toLowerCase().includes(q));
  const hvac = defs.filter((d) => d.type.startsWith("hvac."));
  const plan = defs.filter((d) => d.type.startsWith("plan."));

  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-[var(--tc-hairline)] bg-[var(--tc-panel)] backdrop-blur-md">
      <div className="relative shrink-0 p-2">
        <Search size={13} className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-[var(--tc-text-muted)]" />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search symbols"
          className="w-full rounded border border-[var(--tc-hairline)] bg-transparent py-1 pl-7 pr-2 text-sm text-[var(--tc-text)] placeholder:text-[var(--tc-text-muted)] outline-none focus-visible:ring-1 focus-visible:ring-[var(--tc-accent)]"
        />
      </div>
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-2 pb-3">
        <Category title="HVAC" defs={hvac} />
        <Category title="Floor plan" defs={plan} />
        {defs.length === 0 && <p className="pt-2 text-xs text-[var(--tc-text-muted)]">no symbols match “{query}”</p>}
      </div>
    </aside>
  );
}
