// The layout shell ONLY: Toolbar top, Palette left, SceneCanvas center, PropertyRail
// right, hint bar bottom (never an empty void — opens on the AHU demo,
// builder-ux-scope.md §first-run). Editor/canvas take stores, not App internals
// (thecrew-scope.md §reuse #4).

import { Palette } from "./editor/Palette";
import { PropertyRail } from "./editor/PropertyRail";
import { Toolbar } from "./editor/Toolbar";
import { SceneCanvas } from "./canvas/SceneCanvas";

export function App() {
  return (
    <div className="flex h-screen flex-col bg-[#0a0e14] text-slate-200">
      <Toolbar />
      <div className="flex min-h-0 flex-1">
        <Palette />
        <main className="relative min-w-0 flex-1">
          <SceneCanvas />
        </main>
        <PropertyRail />
      </div>
    </div>
  );
}
