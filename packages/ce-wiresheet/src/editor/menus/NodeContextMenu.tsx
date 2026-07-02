import { useEffect } from "react";
import { EdgeMenuItem } from "./EdgeMenuItem";
import { CopyUid, CopyText, ellipsizePath, stripRoot } from "../../components/FunctionBlock";

// Right-click menu on a node (or a multi-selection). Header shows the target's
// name/uid/path; actions vary by whether one or many nodes are selected and by
// what the type supports (UX, actions, rename).
export function NodeContextMenu({
  x,
  y,
  hasActions,
  canRename,
  name,
  uid,
  path,
  count,
  uxAvailable,
  onRename,
  onDetails,
  onOpenUx,
  onInspect,
  onGroup,
  onMoveInto,
  onAction,
  onClose,
}: {
  x: number;
  y: number;
  hasActions: boolean;
  canRename: boolean;
  name?: string;
  uid?: number;
  path?: string;
  count: number;
  uxAvailable: boolean;
  onRename: () => void;
  onDetails: () => void;
  onOpenUx: () => void;
  onInspect: () => void;
  onGroup: () => void;
  onMoveInto: () => void;
  onAction: () => void;
  onClose: () => void;
}) {
  useEffect(() => {
    const dismiss = (e: MouseEvent) => {
      const el = e.target as Element | null;
      if (el && el.closest("[data-ce-node-menu]")) return;
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
  return (
    <div
      data-ce-node-menu
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
      <div
        style={{ padding: "4px 8px", color: "hsl(var(--muted-foreground))", borderBottom: "1px solid hsl(var(--border))", marginBottom: 4 }}
      >
        {uid != null ? name || "component" : `${count} components`}
        <div
          style={{
            fontSize: 9,
            color: "hsl(var(--muted-foreground))",
            fontFamily: "var(--font-mono)",
            marginTop: 2,
          }}
        >
          {uid != null ? <CopyUid label="comp" value={uid} /> : `${count} selected`}
        </div>
        {uid != null && path && (
          <div
            style={{
              fontSize: 9,
              color: "hsl(var(--muted-foreground))",
              fontFamily: "var(--font-mono)",
              marginTop: 2,
              maxWidth: 240,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {/* Left-truncated path (keeps node + nearest parents); copies the full path. */}
            <CopyText display={ellipsizePath(path)} value={stripRoot(path)} title="click to copy path" />
          </div>
        )}
      </div>
      {uxAvailable && uid != null && <EdgeMenuItem label="Open UX" onClick={onOpenUx} />}
      {uid != null && <EdgeMenuItem label="Inspect" onClick={onInspect} />}
      {canRename && <EdgeMenuItem label="Rename…" onClick={onRename} />}
      {canRename && <EdgeMenuItem label="Configure…" onClick={onDetails} />}
      {count >= 2 && <EdgeMenuItem label={`Group ${count} into folder`} onClick={onGroup} />}
      <EdgeMenuItem label="Move into…" onClick={onMoveInto} />
      {hasActions && <EdgeMenuItem label="Action…" onClick={onAction} />}
    </div>
  );
}
