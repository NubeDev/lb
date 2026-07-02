// Top toolbar: flat/3D camera toggle (Tab), grid-snap toggle (G), undo/redo, demo
// switcher (AHU / floor plan / blank), `?` keyboard overlay. If a control isn't used
// by a benchmark, it doesn't ship (builder-ux-scope.md anti-goals).
//
// Keyboard split (documented in use-drag-place.ts too): tool/camera keys (V/D/G/Tab/?)
// live HERE; selection keys (Esc/Del/arrows/⌘D) in use-selection.ts; undo keys in
// use-undo.ts; chain-tool Enter/Esc inside use-drag-place.ts.

import { useEffect, useState, type ReactNode } from "react";
import { Box, Square, Magnet, Undo2, Redo2, Keyboard } from "lucide-react";
import clsx from "clsx";
import { useSceneStore, type DemoName } from "../state/scene-store";

const DEMOS: { name: DemoName; label: string }[] = [
  { name: "ahu", label: "AHU" },
  { name: "plan", label: "Floor plan" },
  { name: "blank", label: "Blank" },
];

// builder-ux-scope.md §keyboard
const KEYMAP: [string, string][] = [
  ["V", "select tool"],
  ["D", "duct / wall chain tool"],
  ["Esc", "cancel / clear selection"],
  ["Del", "delete selection"],
  ["⌘Z / ⇧⌘Z", "undo / redo"],
  ["⌘D", "duplicate"],
  ["G", "toggle grid snap"],
  ["Tab", "toggle flat / 3D"],
  ["← ↑ → ↓", "nudge (⇧ = ×4)"],
  ["?", "this overlay"],
];

function IconButton({
  title,
  active,
  disabled,
  onClick,
  children,
}: {
  title: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={clsx(
        "rounded p-1.5 outline-none transition-colors focus-visible:ring-1 focus-visible:ring-[var(--tc-accent)]",
        active ? "text-[var(--tc-accent)]" : "text-slate-400 hover:text-slate-200",
        disabled && "cursor-default opacity-30 hover:text-slate-400",
      )}
    >
      {children}
    </button>
  );
}

export function Toolbar() {
  const demo = useSceneStore((s) => s.demo);
  const camera = useSceneStore((s) => s.doc.camera);
  const snapEnabled = useSceneStore((s) => s.snapEnabled);
  const canUndo = useSceneStore((s) => s.past.length > 0);
  const canRedo = useSceneStore((s) => s.future.length > 0);
  const [help, setHelp] = useState(false);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
      if (e.key === "Escape") {
        setHelp(false); // Esc's cancel/clear duties live in use-selection.ts
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const s = useSceneStore.getState();
      switch (e.key) {
        case "v":
        case "V":
          s.armType(null);
          s.setTool("select");
          break;
        case "d":
        case "D":
          // arm a chain-able type for the current demo, then enter the chain tool
          // (armType resets tool to "select", so arm first, then setTool)
          if (s.armedType !== "hvac.duct" && s.armedType !== "plan.wall") {
            s.armType(s.demo === "plan" ? "plan.wall" : "hvac.duct");
          }
          useSceneStore.getState().setTool("chain");
          break;
        case "g":
        case "G":
          s.toggleSnap();
          break;
        case "Tab":
          e.preventDefault();
          s.toggleCamera();
          break;
        case "?":
          setHelp((h) => !h);
          break;
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const store = useSceneStore.getState();

  return (
    <header className="relative z-30 flex h-11 shrink-0 items-center gap-3 border-b border-[var(--tc-hairline)] bg-[var(--tc-panel)] px-3 backdrop-blur-md">
      <span className="text-sm font-medium tracking-wide text-slate-200">thecrew</span>

      <nav className="flex items-center gap-1 text-xs">
        {DEMOS.map((d) => (
          <button
            key={d.name}
            type="button"
            onClick={() => store.loadDemo(d.name)}
            className={clsx(
              "rounded px-2 py-1 outline-none transition-colors focus-visible:ring-1 focus-visible:ring-[var(--tc-accent)]",
              demo === d.name ? "text-[var(--tc-accent)]" : "text-slate-400 hover:text-slate-200",
            )}
          >
            {d.label}
          </button>
        ))}
      </nav>

      <div className="flex-1" />

      <div className="flex items-center gap-1">
        <IconButton
          title={camera === "ortho-top" ? "Flat view — Tab tilts to 3D" : "3D view — Tab returns to flat"}
          active={camera === "persp"}
          onClick={() => store.toggleCamera()}
        >
          {camera === "ortho-top" ? <Square size={16} /> : <Box size={16} />}
        </IconButton>
        <IconButton title="Grid snap (G)" active={snapEnabled} onClick={() => store.toggleSnap()}>
          <Magnet size={16} />
        </IconButton>
        <IconButton title="Undo (⌘Z)" disabled={!canUndo} onClick={() => store.undo()}>
          <Undo2 size={16} />
        </IconButton>
        <IconButton title="Redo (⇧⌘Z)" disabled={!canRedo} onClick={() => store.redo()}>
          <Redo2 size={16} />
        </IconButton>
        <IconButton title="Keyboard shortcuts (?)" active={help} onClick={() => setHelp((h) => !h)}>
          <Keyboard size={16} />
        </IconButton>
      </div>

      {help && (
        // NOT a modal: a transparent click-catcher dismisses; Esc / ? also close.
        <div className="fixed inset-0 z-40" onClick={() => setHelp(false)}>
          <div
            className="absolute right-3 top-12 w-64 rounded-md border border-[var(--tc-hairline)] bg-[var(--tc-panel)] p-3 backdrop-blur-md"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-2 text-xs font-medium text-slate-200">Keyboard</div>
            <dl className="space-y-1 text-xs">
              {KEYMAP.map(([key, what]) => (
                <div key={key} className="flex items-baseline justify-between gap-2">
                  <dt className="font-mono text-[var(--tc-accent)]">{key}</dt>
                  <dd className="text-right text-slate-400">{what}</dd>
                </div>
              ))}
            </dl>
          </div>
        </div>
      )}
    </header>
  );
}
