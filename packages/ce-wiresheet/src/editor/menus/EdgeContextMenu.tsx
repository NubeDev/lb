import { useEffect } from "react";
import { EdgeMenuItem } from "./EdgeMenuItem";

// Right-click menu for a wire. `isLoopBack` flips the primary action between
// "Reevaluate" (loopback edges don't auto-fire) and "Set as loopback".
export function EdgeContextMenu({
  x,
  y,
  isLoopBack,
  onPrimary,
  onDelete,
  onClose,
}: {
  x: number;
  y: number;
  isLoopBack: boolean;
  onPrimary: () => void;
  onDelete: () => void;
  onClose: () => void;
}) {
  useEffect(() => {
    const dismiss = (e: MouseEvent) => {
      const el = e.target as Element | null;
      if (el && el.closest("[data-ce-edge-menu]")) return;
      onClose();
    };
    // Capture phase + pointerdown: React Flow's pane (d3-zoom) calls
    // stopImmediatePropagation on pointer/mouse down, so a bubble-phase
    // document listener never sees outside clicks. Capture fires first.
    document.addEventListener("pointerdown", dismiss, true);
    document.addEventListener("contextmenu", dismiss, true);
    return () => {
      document.removeEventListener("pointerdown", dismiss, true);
      document.removeEventListener("contextmenu", dismiss, true);
    };
  }, [onClose]);
  // Loopback edges break feedback cycles for the engine — they don't auto-fire
  // on source change, so the only way to push a value through them is a manual
  // reevaluate. Non-loopback edges auto-flow, so the useful action there is
  // promoting them to loopback (one-way: the engine never clears the flag).
  const primaryLabel = isLoopBack ? "Reevaluate" : "Set as loopback";
  return (
    <div
      data-ce-edge-menu
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: "fixed",
        left: x,
        top: y,
        zIndex: 100,
        background: "hsl(var(--card))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 4,
        padding: 4,
        minWidth: 160,
        boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
        fontSize: 12,
        color: "hsl(var(--foreground))",
        fontFamily: "-apple-system, system-ui, sans-serif",
      }}
    >
      <EdgeMenuItem label={primaryLabel} onClick={onPrimary} />
      <EdgeMenuItem label="Delete" onClick={onDelete} danger />
    </div>
  );
}
