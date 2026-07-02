// A single body-level container for the editor's portaled overlays — context
// menus, the marquee, value pickers, the diagnostics drawer. These render via
// createPortal to ESCAPE the graph pane's `transform` (which would otherwise be
// the containing block for their `position: fixed`, breaking viewport tracking).
//
// They must still inherit the editor's design tokens (`.ce-wiresheet`), so instead
// of portaling to bare `document.body` we portal into this container, which carries
// the scope class. CeEditor keeps its theme (`.theme-light`) in sync.

let root: HTMLElement | null = null;

export function wiresheetPortalRoot(): HTMLElement {
  if (root && root.isConnected) return root;
  root = document.createElement("div");
  root.className = "ce-wiresheet";
  root.setAttribute("data-ce-wiresheet-portals", "");
  document.body.appendChild(root);
  return root;
}

// Keep the portal container's light/dark in step with the editor.
export function setWiresheetPortalTheme(mode: "dark" | "light"): void {
  wiresheetPortalRoot().classList.toggle("theme-light", mode === "light");
}
