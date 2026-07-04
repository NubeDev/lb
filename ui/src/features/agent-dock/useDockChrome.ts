// The dock CHROME state (agent-dock scope) — open/closed + width, persisted across reloads, plus the
// mobile auto-close floor. UI-only state (client-side per scope: no server surface). One responsibility:
// own the panel's presentation state; the message data lives in `useDockSession`.
//
// Persistence mirrors the sidebar idiom (a small localStorage key). Width is clamped to the panel's
// [min,max]; below the mobile breakpoint the dock force-closes (a resizable right panel can't share a
// phone viewport — ui-standards-scope) and the launcher still toggles it back for a tablet.

import { useCallback, useEffect, useState } from "react";

import { useIsMobile } from "@/hooks/use-mobile";

const OPEN_KEY = "lb.agent-dock.open";
const WIDTH_KEY = "lb.agent-dock.width";

/** Panel width bounds (px). `INITIAL` is the comfortable default; the resize handle clamps to these. */
export const DOCK_MIN_WIDTH = 320;
export const DOCK_MAX_WIDTH = 640;
export const DOCK_INITIAL_WIDTH = 384;

function readOpen(): boolean {
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem(OPEN_KEY) === "true";
}

function readWidth(): number {
  if (typeof localStorage === "undefined") return DOCK_INITIAL_WIDTH;
  const raw = Number(localStorage.getItem(WIDTH_KEY));
  if (!Number.isFinite(raw) || raw <= 0) return DOCK_INITIAL_WIDTH;
  return Math.min(DOCK_MAX_WIDTH, Math.max(DOCK_MIN_WIDTH, raw));
}

export interface DockChrome {
  /** Whether the dock is open. Force-`false` on a mobile viewport regardless of the stored pref. */
  open: boolean;
  /** The persisted panel width (px), kept in sync with the resize handle. */
  width: number;
  toggle: () => void;
  close: () => void;
  setWidth: (w: number) => void;
}

/** Drive the dock's open/width chrome with reload persistence + a mobile auto-close floor. */
export function useDockChrome(): DockChrome {
  const [open, setOpen] = useState<boolean>(readOpen);
  const [width, setWidthState] = useState<number>(readWidth);
  const isMobile = useIsMobile();

  // Persist open-state — but only the user's INTENT (not the mobile-forced close), so returning to a
  // wide viewport restores the panel the user had open.
  useEffect(() => {
    if (typeof localStorage !== "undefined") localStorage.setItem(OPEN_KEY, String(open));
  }, [open]);

  const setWidth = useCallback((w: number) => {
    setWidthState(w);
    if (typeof localStorage !== "undefined") localStorage.setItem(WIDTH_KEY, String(w));
  }, []);

  const toggle = useCallback(() => setOpen((o) => !o), []);
  const close = useCallback(() => setOpen(false), []);

  // Mobile floor: never RENDER open on a phone (a resizable right dock can't share the viewport). The
  // stored intent is untouched, so widening the window reopens what the user had.
  return { open: open && !isMobile, width, toggle, close, setWidth };
}
