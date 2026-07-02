// The layout shell ONLY: Toolbar top, Palette left, SceneCanvas center, PropertyRail
// right, hint bar bottom (never an empty void — opens on the AHU demo,
// builder-ux-scope.md §first-run). Editor/canvas take stores, not App internals
// (thecrew-scope.md §reuse #4).

import clsx from "clsx";
import { Palette } from "./editor/Palette";
import { PropertyRail } from "./editor/PropertyRail";
import { Toolbar } from "./editor/Toolbar";
import { useEditorKeys } from "./editor/use-selection";
import { useUndoKeys } from "./editor/use-undo";
import { SceneCanvas } from "./canvas/SceneCanvas";
import { useSceneStore } from "./state/scene-store";

export function App() {
  useEditorKeys();
  useUndoKeys();
  const empty = useSceneStore((s) => Object.keys(s.doc.shapes).length === 0);
  const crosshair = useSceneStore((s) => !!s.armedType || s.tool === "chain");

  return (
    <div className="flex h-screen flex-col bg-[var(--tc-canvas)] text-[var(--tc-text)]">
      <Toolbar />
      <div className="flex min-h-0 flex-1">
        <Palette />
        <main className={clsx("relative min-w-0 flex-1", crosshair && "cursor-crosshair")}>
          <SceneCanvas />
          {empty && (
            // blank page shows a centered ghost-text prompt, not darkness (§first-run)
            <p className="pointer-events-none absolute inset-0 flex items-center justify-center text-sm text-[var(--tc-text-muted)]">
              drag a symbol from the palette to start
            </p>
          )}
        </main>
        <PropertyRail />
      </div>
      <footer className="flex h-7 shrink-0 items-center justify-center border-t border-[var(--tc-hairline)] bg-[var(--tc-panel)] text-xs text-[var(--tc-text-muted)] backdrop-blur-md">
        drag from the palette · double-click ends a duct run · Tab tilts to 3D · ? for keys
      </footer>
    </div>
  );
}
