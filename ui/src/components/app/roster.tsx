// RosterRail — the canonical "inner sidebar" LIST used by the full-screen surfaces (Dashboards is
// the reference): an `AppRail` with an optional inline-create header, an optional minimize control,
// and a selectable item list with opt-in hover actions (inline rename, delete). Extracted from the
// dashboard roster so every surface shares one maintained look; a feature opts into behavior purely
// by wiring props:
//   - `onCreate`    → the header grows a "New …" field + create button
//   - `onCollapse`  → the header grows a minimize control (host supplies the symmetric expand)
//   - `onRename`    → items grow a hover pencil that swaps to an inline title editor
//   - `onRemove`    → items grow a hover trash; the CALLER owns the destructive confirm (this is a
//                     chrome component — it never imports a feature, so the confirm gate stays with
//                     the feature per the components→features layering rule)
// `noun` seeds every aria-label ("select dashboard …", "new flow title", …) so tests and screen
// readers speak the surface's own vocabulary. One component per file (FILE-LAYOUT).

import { useState, type ComponentType, type CSSProperties } from "react";
import { Check, PanelLeftClose, Pencil, Plus, Trash2, X } from "lucide-react";

import { AppRail } from "@/components/app/rail";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { resolveIcon } from "@/lib/icons/resolve";

export interface RosterItem {
  id: string;
  title: string;
  /** Small trailing caption (e.g. a visibility tier, a version tag). Omit for none. */
  badge?: string;
  /** Optional per-item leading icon name (resolved via `@/lib/icons`) — overrides the rail's shared
   *  `icon` for this row when set (e.g. a dashboard's page-settings icon). Omit ⇒ the shared icon. */
  icon?: string;
  /** Optional accent colour for this row's icon (any CSS colour). Omit ⇒ the default muted/accent tint. */
  iconColor?: string;
}

interface RosterRailProps<T extends RosterItem> {
  /** The surface's noun, used in every aria-label and default copy: "dashboard", "rule", "flow". */
  noun: string;
  items: T[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  /** Per-item leading icon. */
  icon: ComponentType<{
    size?: number | string;
    className?: string;
    style?: CSSProperties;
  }>;
  /** Empty-roster copy. Defaults to "No {noun}s yet." */
  emptyText?: string;
  /** Inline-create: wiring this grows the header field + button. Receives the trimmed title. */
  onCreate?: (title: string) => void;
  createPlaceholder?: string;
  /** Minimize the rail (host renders the symmetric expand control when collapsed). */
  onCollapse?: () => void;
  /** Opt-in hover rename → inline title editor. */
  onRename?: (id: string, title: string) => void;
  /** Opt-in hover delete. The caller owns any destructive-confirm gate. */
  onRemove?: (item: T) => void;
}

export function RosterRail<T extends RosterItem>({
  noun,
  items,
  selectedId,
  onSelect,
  icon: Icon,
  emptyText,
  onCreate,
  createPlaceholder,
  onCollapse,
  onRename,
  onRemove,
}: RosterRailProps<T>) {
  const [title, setTitle] = useState("");
  // The item currently being renamed inline, and the draft title.
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  const create = () => {
    const t = title.trim();
    if (!t) return;
    onCreate?.(t);
    setTitle("");
  };

  const commitEdit = () => {
    if (editingId && draft.trim()) onRename?.(editingId, draft);
    setEditingId(null);
    setDraft("");
  };

  const cancelEdit = () => {
    setEditingId(null);
    setDraft("");
  };

  const header =
    onCreate || onCollapse ? (
      <>
        {onCreate && (
          <>
            <Input
              aria-label={`new ${noun} title`}
              placeholder={createPlaceholder ?? `New ${noun}…`}
              className="h-8 min-w-0 flex-1 text-xs"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && create()}
            />
            <Button
              aria-label={`create ${noun}`}
              variant="outline"
              size="sm"
              className="h-8 px-2"
              onClick={create}
            >
              <Plus size={14} />
            </Button>
          </>
        )}
        {onCollapse && (
          <Button
            aria-label={`minimize ${noun} rail`}
            variant="ghost"
            size="icon"
            className="h-8 w-8 shrink-0"
            title="Minimize"
            onClick={onCollapse}
          >
            <PanelLeftClose size={14} />
          </Button>
        )}
      </>
    ) : undefined;

  return (
    <AppRail label={`${noun} rail`} header={header}>
      <ul className="space-y-1">
        {items.length === 0 && (
          <li className="rounded-md border border-dashed border-border bg-bg/60 px-3 py-3 text-xs text-muted">
            {emptyText ?? `No ${noun}s yet.`}
          </li>
        )}
        {items.map((item) => {
          const active = selectedId === item.id;
          if (editingId === item.id) {
            return (
              <li key={item.id} className="flex items-center gap-1">
                <Input
                  aria-label={`rename ${noun} ${item.id}`}
                  className="h-8 min-w-0 flex-1 text-sm"
                  autoFocus
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitEdit();
                    if (e.key === "Escape") cancelEdit();
                  }}
                />
                <Button
                  aria-label={`confirm rename ${item.id}`}
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 shrink-0"
                  onClick={commitEdit}
                >
                  <Check size={14} />
                </Button>
                <Button
                  aria-label={`cancel rename ${item.id}`}
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 shrink-0"
                  onClick={cancelEdit}
                >
                  <X size={14} />
                </Button>
              </li>
            );
          }
          return (
            <li key={item.id} className="group flex items-center gap-1">
              <Button
                aria-label={`select ${noun} ${item.id}`}
                variant="ghost"
                onClick={() => onSelect(item.id)}
                className={cn(
                  "h-auto min-w-0 flex-1 justify-start gap-2 px-2.5 py-1.5 text-left text-[13px] font-normal",
                  active
                    ? "bg-accent/10 text-accent hover:bg-accent/10"
                    : "text-fg/90 hover:bg-fg/[0.06] hover:text-fg",
                )}
              >
                {(() => {
                  // Per-item icon override (e.g. a dashboard's page-settings icon) falls back to the
                  // rail's shared icon; a per-item colour tints it, else the default muted/accent.
                  const RowIcon = resolveIcon(item.icon) ?? Icon;
                  return (
                    <RowIcon
                      size={14}
                      className={cn(
                        "shrink-0",
                        item.iconColor ? "" : active ? "" : "text-muted",
                      )}
                      style={item.iconColor ? { color: item.iconColor } : undefined}
                    />
                  );
                })()}
                <span className="min-w-0 flex-1 truncate">{item.title}</span>
                {item.badge && (
                  <span className="shrink-0 text-[10px] font-medium text-muted/80">{item.badge}</span>
                )}
              </Button>
              {(onRename || onRemove) && (
                <div className="flex shrink-0 items-center opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                  {onRename && (
                    <Button
                      aria-label={`rename ${noun} ${item.id}`}
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8"
                      title="Rename"
                      onClick={() => {
                        setEditingId(item.id);
                        setDraft(item.title);
                      }}
                    >
                      <Pencil size={13} />
                    </Button>
                  )}
                  {onRemove && (
                    <Button
                      aria-label={`delete ${noun} ${item.id}`}
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-muted hover:text-danger"
                      title="Delete"
                      onClick={() => onRemove(item)}
                    >
                      <Trash2 size={13} />
                    </Button>
                  )}
                </div>
              )}
            </li>
          );
        })}
      </ul>
    </AppRail>
  );
}
