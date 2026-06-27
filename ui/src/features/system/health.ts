// The one health → token mapping shared by the status grid and the topology graph (system-map scope),
// so the two surfaces colour a subsystem identically (they must never disagree). `idle` greys (up,
// nothing flowing — not a fault); `degraded`/`down` use the destructive/amber tokens. One
// responsibility per file (FILE-LAYOUT): the colour vocabulary, nothing else.

import type { Health } from "@/lib/system/system.types";

export interface HealthStyle {
  /** Short status word for the card/node. */
  label: string;
  /** A status dot background. */
  dot: string;
  /** Status text colour. */
  text: string;
  /** Card/node border accent. */
  border: string;
}

export const HEALTH_STYLES: Record<Health, HealthStyle> = {
  ok: {
    label: "Ok",
    dot: "bg-emerald-500",
    text: "text-emerald-600 dark:text-emerald-400",
    border: "border-border",
  },
  idle: {
    label: "Idle",
    dot: "bg-muted/60",
    text: "text-muted",
    border: "border-border",
  },
  degraded: {
    label: "Degraded",
    dot: "bg-amber-500",
    text: "text-amber-600 dark:text-amber-400",
    border: "border-amber-500/40",
  },
  down: {
    label: "Down",
    dot: "bg-destructive",
    text: "text-destructive",
    border: "border-destructive/40",
  },
};

/** Sort rank so the cards/nodes that want attention surface first (degraded → down → ok → idle). */
export function healthRank(h: Health): number {
  switch (h) {
    case "degraded":
      return 0;
    case "down":
      return 1;
    case "ok":
      return 2;
    case "idle":
      return 3;
  }
}
