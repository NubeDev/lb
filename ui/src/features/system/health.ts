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
  // Semantic tokens (`--success`/`--warning`, globals.css), not raw Tailwind hues — so health tracks
  // the theme's own state vocabulary in every preset/mode instead of a hardcoded emerald/amber.
  ok: {
    label: "Ok",
    dot: "bg-success",
    text: "text-success",
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
    dot: "bg-warning",
    text: "text-warning",
    border: "border-warning/40",
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
