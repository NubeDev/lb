// The workbench's dock tab (data-studio-10x scope, phase 1) — one tab renderer for every pane kind.
// Parity with the flexlayout-era strip: the title caps + ellipsizes (see datastudio-dock.css) with
// the FULL title as the hover tooltip, double-click renames (the panel's own title, persisted with
// the layout), and an inline close. The leading icon matches the SURFACE the pane is showing — a
// view pane (`flows`, `rules`, …) renders the SAME icon the sidebar rail does (sourced from
// `SURFACE_DEF`), so the dock tab and the rail read as one. A builder pane gets a chart glyph.
// One responsibility: tab chrome; the pane bodies live elsewhere.

import { useEffect, useState } from "react";
import { LineChart, X } from "lucide-react";
import type { IDockviewPanelHeaderProps } from "dockview-react";

import { SURFACE_DEF } from "@/features/shell/surfaceDefs";
import type { CoreSurface } from "@/features/shell";
import type { BuilderConfig, ViewPaneConfig } from "./workbenchModel";

/** Resolve the leading icon for a tab from its params: a view pane maps to its surface's icon
 *  (the same one the sidebar uses — single source in `SURFACE_DEF`); a builder pane gets a chart. */
function iconForParams(params: BuilderConfig | ViewPaneConfig | undefined) {
  if (params && typeof params === "object" && "kind" in params && typeof params.kind === "string") {
    return SURFACE_DEF[params.kind as CoreSurface]?.icon ?? LineChart;
  }
  return LineChart;
}

export function WorkbenchTab({ api, params }: IDockviewPanelHeaderProps) {
  const [title, setTitle] = useState(api.title ?? "");

  useEffect(() => {
    const d = api.onDidTitleChange((e) => setTitle(e.title));
    return () => d.dispose();
  }, [api]);

  const rename = () => {
    const next = window.prompt("Rename tab", title);
    if (next?.trim()) api.setTitle(next.trim());
  };

  const Icon = iconForParams(params);

  return (
    <div
      className="ds-tab flex h-full min-w-0 items-center gap-1.5 px-3"
      title={title}
      onDoubleClick={rename}
    >
      <Icon size={14} className="shrink-0 text-muted" />
      <span className="ds-tab-title min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{title}</span>
      <button
        type="button"
        aria-label={`close ${title}`}
        className="ds-tab-close shrink-0 rounded-sm p-0.5 text-muted hover:text-fg"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          api.close();
        }}
      >
        <X size={13} />
      </button>
    </div>
  );
}
