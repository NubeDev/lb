// Section — the ce-wiresheet InspectPanel grouping primitive, ported to shadcn tokens.
// A titled block: an uppercase, letter-spaced, muted label over its children, with the
// dense vertical rhythm the ce panel uses. This is the panel's core structural unit —
// "so many options" reads as a stack of these.
//
// One responsibility: the titled group. Rows/tables/KV live in their own files.

import type { ReactNode } from "react";
import { cn } from "./lib/cn";

export interface SectionProps {
  /** The uppercase group label (e.g. "Properties (12)"). */
  title: ReactNode;
  /** Optional trailing controls on the header row (a button, a count, a toggle). */
  aside?: ReactNode;
  className?: string;
  children: ReactNode;
}

/** A titled, dense group — the ce InspectPanel `Section` look on shadcn tokens. */
export function Section({ title, aside, className, children }: SectionProps) {
  return (
    <section className={cn("mb-4 last:mb-0", className)}>
      <div className="mb-1.5 flex items-center justify-between gap-2">
        <div className="text-[10px] font-semibold uppercase tracking-wide text-lbp-muted">
          {title}
        </div>
        {aside}
      </div>
      {children}
    </section>
  );
}
