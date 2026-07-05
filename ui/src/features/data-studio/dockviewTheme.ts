// The workbench's Dockview theme handle (data-studio-10x scope, phase 1). Dockview themes are a
// class name + CSS custom properties; the properties themselves live in `datastudio-dock.css`,
// aliased to the shell's shadcn tokens so dark/light parity is automatic (the shell flips its
// tokens, the dock follows). This file only names the theme.

import type { DockviewTheme } from "dockview-react";

export const LB_DOCKVIEW_THEME: DockviewTheme = {
  name: "lazybones",
  className: "dockview-theme-lb",
  // The tab drop indicator as a thin insertion strip — matches the bordered-pill tab styling.
  dndTabIndicator: "line",
};
