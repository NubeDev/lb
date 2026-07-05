// The workbench's dock tab (data-studio-10x scope, phase 1) — one tab renderer for every pane kind.
// Parity with the flexlayout-era strip: the title caps + ellipsizes (see datastudio-dock.css) with
// the FULL title as the hover tooltip, double-click renames (the panel's own title, persisted with
// the layout), and an inline close. One responsibility: tab chrome; the pane bodies live elsewhere.

import { useEffect, useState } from "react";
import { X } from "lucide-react";
import type { IDockviewPanelHeaderProps } from "dockview-react";

export function WorkbenchTab({ api }: IDockviewPanelHeaderProps) {
  const [title, setTitle] = useState(api.title ?? "");

  useEffect(() => {
    const d = api.onDidTitleChange((e) => setTitle(e.title));
    return () => d.dispose();
  }, [api]);

  const rename = () => {
    const next = window.prompt("Rename tab", title);
    if (next?.trim()) api.setTitle(next.trim());
  };

  return (
    <div
      className="ds-tab flex h-full min-w-0 items-center gap-1 px-2"
      title={title}
      onDoubleClick={rename}
    >
      <span className="ds-tab-title min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{title}</span>
      <button
        type="button"
        aria-label={`close ${title}`}
        className="ds-tab-close shrink-0 rounded p-0.5 text-muted hover:text-fg"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          api.close();
        }}
      >
        <X size={12} />
      </button>
    </div>
  );
}
