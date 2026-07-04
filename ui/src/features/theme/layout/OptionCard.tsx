// A selectable Layout-option card: a titled mini-diagram that highlights when chosen. Shared by the
// three Layout pickers (variant/collapsible/side). Token-bound (accent ring on select). One component
// per file (FILE-LAYOUT).

import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface Props {
  name: string;
  selected: boolean;
  onSelect: () => void;
  "aria-label": string;
  children: ReactNode;
}

export function OptionCard({ name, selected, onSelect, "aria-label": ariaLabel, children }: Props) {
  return (
    <Button
      type="button"
      variant={selected ? "default" : "outline"}
      aria-label={ariaLabel}
      aria-pressed={selected}
      onClick={onSelect}
      className={cn(
        "flex h-auto flex-col gap-2 p-3 text-center",
        selected ? "border-accent bg-accent/10" : undefined,
      )}
    >
      <span className="text-xs font-semibold text-fg">{name}</span>
      {children}
    </Button>
  );
}
