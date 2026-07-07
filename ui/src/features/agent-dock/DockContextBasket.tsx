// The context-basket CHIP ROW (agent-context-basket scope) — presentation only. Shows the gathered
// refs above the composer so the user sees exactly what the next ask will carry; each chip removes
// its ref, and clear-all empties the basket. Hidden when empty (byte-identical dock).

import { Paperclip, X } from "lucide-react";

import type { Item } from "@/lib/channel/channel.types";
import { Button } from "@/components/ui/button";
import type { ContextBasket } from "./useContextBasket";
import { refLabel } from "./contextBasket";

interface Props {
  basket: ContextBasket;
  /** The session items — chips label refs by their payload kind, not the raw id. */
  items: Item[];
}

export function DockContextBasket({ basket, items }: Props) {
  if (basket.ids.length === 0) return null;
  return (
    <div
      aria-label="context basket"
      className="flex flex-wrap items-center gap-1.5 border-t border-border bg-panel-2/40 px-3 py-2"
    >
      <Paperclip size={12} className="shrink-0 text-muted" />
      <span className="text-xs text-muted">Next ask includes:</span>
      {basket.ids.map((id) => (
        <span
          key={id}
          data-testid="context-chip"
          className="flex items-center gap-1 rounded-md border border-border bg-bg px-2 py-0.5 text-xs"
        >
          <span className="font-medium">{refLabel(items, id)}</span>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={`remove ${id} from context`}
            className="h-4 w-4 text-muted hover:text-fg"
            onClick={() => basket.toggle(id)}
          >
            <X size={11} />
          </Button>
        </span>
      ))}
      <Button
        type="button"
        variant="ghost"
        size="sm"
        aria-label="clear context"
        className="ml-auto h-6 px-2 text-xs text-muted"
        onClick={basket.clear}
      >
        Clear
      </Button>
    </div>
  );
}
