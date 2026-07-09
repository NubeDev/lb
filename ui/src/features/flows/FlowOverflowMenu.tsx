// The header's `⋯` overflow menu (flow-ui-polish scope) — the occasional operator actions moved out
// of the always-visible toolbar: Enable/Disable, Live values, Undo, Export…, Import…, Delete. The
// lightweight outside-click popover discipline (RunHistoryMenu precedent), no new dependency. One
// responsibility: render + dispatch; every action + piece of state is a prop.

import { useEffect, useRef, useState } from "react";
import {
  Download,
  MoreHorizontal,
  Power,
  Radio,
  RotateCcw,
  Trash2,
  Upload,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

export interface FlowOverflowMenuProps {
  /** The durable enabled flag (from node_state; the flow record until it loads). */
  enabled: boolean;
  /** Live-value painting on/off (SSE watch + node_state/runs poll). */
  liveValues: boolean;
  /** Undo is available (the stack is non-empty). */
  canUndo: boolean;
  onToggleEnabled: () => void;
  onToggleLiveValues: (next: boolean) => void;
  onUndo: () => void;
  /** Open the transfer dialog on the given tab. */
  onTransfer: (tab: "export" | "import") => void;
  onDelete: () => void;
}

export function FlowOverflowMenu({
  enabled,
  liveValues,
  canUndo,
  onToggleEnabled,
  onToggleLiveValues,
  onUndo,
  onTransfer,
  onDelete,
}: FlowOverflowMenuProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  // Close on any outside click (the usual lightweight popover discipline).
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const pick = (fn: () => void) => () => {
    setOpen(false);
    fn();
  };

  return (
    <div ref={rootRef} className="relative">
      <Button
        aria-label="more flow actions"
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={() => setOpen((o) => !o)}
        variant="ghost"
        size="sm"
        title="More actions"
      >
        <MoreHorizontal size={15} />
      </Button>
      {open && (
        <div
          role="menu"
          aria-label="flow actions"
          className="absolute right-0 top-full z-50 mt-1 w-52 rounded-md border border-border bg-panel p-1 shadow-lg"
        >
          <MenuItem
            label={enabled ? "disable flow" : "enable flow"}
            onClick={pick(onToggleEnabled)}
            title={
              enabled
                ? "Disable: the flow stops firing (durable — survives restart)"
                : "Enable: the flow fires on its triggers again"
            }
          >
            <Power size={13} />
            {enabled ? "Disable flow" : "Enable flow"}
          </MenuItem>
          {/* A toggle row, not a pick-and-close action — flipping it keeps the menu open. */}
          <label
            className="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-xs text-fg hover:bg-bg"
            title="Paint each wire's current value (SSE + poll)"
          >
            <Radio size={13} className={cn(liveValues && "text-accent")} aria-hidden />
            <span className="flex-1">Live values</span>
            <Switch
              aria-label="toggle live values"
              checked={liveValues}
              onCheckedChange={onToggleLiveValues}
            />
          </label>
          <MenuItem label="undo" onClick={pick(onUndo)} disabled={!canUndo}>
            <RotateCcw size={13} />
            Undo
          </MenuItem>
          <div className="my-1 h-px bg-border" role="separator" />
          <MenuItem label="export flow" onClick={pick(() => onTransfer("export"))}>
            <Download size={13} />
            Export…
          </MenuItem>
          <MenuItem label="import flow" onClick={pick(() => onTransfer("import"))}>
            <Upload size={13} />
            Import…
          </MenuItem>
          <div className="my-1 h-px bg-border" role="separator" />
          <MenuItem
            label="delete flow"
            onClick={pick(onDelete)}
            className="text-destructive hover:text-destructive"
          >
            <Trash2 size={13} />
            Delete flow
          </MenuItem>
        </div>
      )}
    </div>
  );
}

function MenuItem({
  label,
  className,
  children,
  ...props
}: React.ComponentProps<typeof Button> & { label: string }) {
  return (
    <Button
      type="button"
      variant="ghost"
      role="menuitem"
      aria-label={label}
      className={cn("h-auto w-full justify-start gap-2 rounded-md px-2 py-1.5 text-xs", className)}
      {...props}
    >
      {children}
    </Button>
  );
}
