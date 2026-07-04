// The global `mod+j` toggle for the dock (agent-dock scope, resolved decision 4) — a SINGLE shell
// keydown listener, no hotkey library. `mod` is ⌘ on macOS, Ctrl elsewhere (matching the shadcn
// sidebar's `mod+b`, with which `mod+j` deliberately does not collide). One responsibility: bind the
// one shortcut; the toggle action is a callback.
//
// A modifier combo is safe to fire even while a text field is focused (it can't be a literal keystroke),
// so we do NOT skip inputs — the dock must toggle from anywhere, including the composer.

import { useEffect } from "react";

/** Bind `mod+j` to `toggle` for the lifetime of the mounting component (the shell). */
export function useDockHotkey(toggle: () => void): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "j") {
        e.preventDefault();
        toggle();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle]);
}
