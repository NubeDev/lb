// The canvas right dock (flow-ui-polish scope) — ONE panel owning the right edge, with Config and
// Debug as tabs, replacing the two side-by-side panels that used to stack up (the clash). Stateless
// chrome: the active tab + contents are props; the config edit buffer stays owned by the canvas so a
// tab switch never drops an edit. Resizable by a pointer-drag handle on its left edge, width
// persisted per-browser (the useSplitPane precedent — presentation state, never authoring state).

import { useCallback, useState } from "react";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

export type DockTab = "config" | "debug";

const WIDTH_KEY = "lb.flows.dock.width";
const MIN_W = 280;
const MAX_W = 640;
const DEFAULT_W = 340;

const clampW = (w: number) => Math.min(MAX_W, Math.max(MIN_W, Math.round(w)));

export interface RightDockProps {
  tab: DockTab;
  onTabChange: (tab: DockTab) => void;
  onClose: () => void;
  config: React.ReactNode;
  debug: React.ReactNode;
}

export function RightDock({ tab, onTabChange, onClose, config, debug }: RightDockProps) {
  const [width, setWidth] = useState(() => {
    const saved = Number(window.localStorage?.getItem(WIDTH_KEY));
    return Number.isFinite(saved) && saved > 0 ? clampW(saved) : DEFAULT_W;
  });

  const commit = useCallback((w: number) => {
    const next = clampW(w);
    setWidth(next);
    try {
      window.localStorage?.setItem(WIDTH_KEY, String(next));
    } catch {
      // storage full/denied — the in-memory width still works for this session
    }
  }, []);

  // Drag the left edge: width = distance from the pointer to the dock's right edge.
  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLElement>) => {
      e.preventDefault();
      const target = e.currentTarget;
      const right = target.parentElement?.getBoundingClientRect().right ?? e.clientX + width;
      target.setPointerCapture(e.pointerId);
      const move = (ev: PointerEvent) => commit(right - ev.clientX);
      const up = () => {
        target.removeEventListener("pointermove", move);
        target.removeEventListener("pointerup", up);
      };
      target.addEventListener("pointermove", move);
      target.addEventListener("pointerup", up);
    },
    [commit, width],
  );

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLElement>) => {
      if (e.key === "ArrowLeft") commit(width + 16);
      else if (e.key === "ArrowRight") commit(width - 16);
      else return;
      e.preventDefault();
    },
    [commit, width],
  );

  return (
    <aside
      aria-label="flow right dock"
      className="relative flex min-h-0 flex-col border-l border-border bg-card/40"
      style={{ width }}
    >
      <div
        role="separator"
        aria-label="resize dock"
        aria-orientation="vertical"
        aria-valuenow={width}
        tabIndex={0}
        onPointerDown={onPointerDown}
        onKeyDown={onKeyDown}
        className="absolute inset-y-0 left-0 w-1 cursor-col-resize transition-colors hover:bg-accent/40 focus-visible:bg-accent/40 focus-visible:outline-none"
      />
      <Tabs
        value={tab}
        onValueChange={(v) => onTabChange(v as DockTab)}
        className="min-h-0 flex-1 gap-0"
      >
        <div className="flex items-center gap-2 border-b border-border px-2 py-1.5">
          <TabsList className="flex-1 border-0 bg-transparent p-0">
            <TabsTrigger value="config">Config</TabsTrigger>
            <TabsTrigger value="debug">Debug</TabsTrigger>
          </TabsList>
          <Button aria-label="close dock" onClick={onClose} variant="ghost" size="sm">
            ✕
          </Button>
        </div>
        <TabsContent value="config" className="flex min-h-0 flex-1 flex-col overflow-y-auto">
          {config}
        </TabsContent>
        <TabsContent value="debug" className="flex min-h-0 flex-1 flex-col">
          {debug}
        </TabsContent>
      </Tabs>
    </aside>
  );
}
