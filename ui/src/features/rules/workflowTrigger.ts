// Pure presenters for a flow's trigger (rules-workflow-convergence scope). The trigger's config is
// a fixed shape owned by the `trigger` node's schema (`{mode, cron?, series?, inject_mode?}` — see
// `TriggerConfigFields`). These helpers turn that config into the icon + label + caption a row
// paints, with no React — pure data in, presentational data out (FILE-LAYOUT: presenters in their
// own file, view imports them).

import type { LucideIcon } from "lucide-react";
import {
  Clock,
  Hand,
  Power,
  Radio,
  SquareTerminal,
  Zap,
} from "lucide-react";

export type TriggerMode = "manual" | "cron" | "event" | "inject" | "boot";

export interface TriggerView {
  mode: TriggerMode;
  icon: LucideIcon;
  /** A short noun: "Manual", "Schedule", "Event", "Inject", "On boot". */
  label: string;
  /** The mode-specific value (the cron string, the series name, the inject behaviour). */
  caption: string | null;
}

const MODE_LABEL: Record<TriggerMode, string> = {
  manual: "Manual",
  cron: "Schedule",
  event: "Event",
  inject: "Inject",
  boot: "On boot",
};

function modeOf(mode: unknown): TriggerMode {
  if (mode === "cron" || mode === "event" || mode === "inject" || mode === "boot") return mode;
  return "manual";
}

/** A cron string → "in 12m / in 2h / tomorrow" style hint, but kept loose: a precise instant is the
 *  row's own job (it has `nextAttemptTs`); this is just the schedule's slogan. */
export function cronSlogan(cron: string | null | undefined): string | null {
  if (!cron) return null;
  // The cron builder writes the standard 5-field form. A few friendly reads for the common shapes;
  // anything else falls through to the raw string (still legible to an author who wrote it).
  if (cron === "* * * * *") return "every minute";
  if (/^\*\/(\d+) \* \* \* \*$/.test(cron)) {
    const m = /^\*\/(\d+)/.exec(cron);
    return m ? `every ${m[1]}m` : cron;
  }
  if (/^0 \* \* \* \*$/.test(cron)) return "hourly";
  if (/^0 (\d+) \* \* \*$/.test(cron)) {
    const m = /^0 (\d+)/.exec(cron);
    return m ? `daily at ${m[1]}:00` : cron;
  }
  return cron;
}

/** Read a trigger node's config and return the presentational view. Falls back to "Manual" when the
 *  config is missing or the node type isn't a trigger (a flow with no trigger is treated as manual). */
export function readTrigger(config: Record<string, unknown> | null | undefined): TriggerView {
  const cfg = config ?? {};
  const mode = modeOf(cfg.mode);
  switch (mode) {
    case "cron":
      return { mode, icon: Clock, label: MODE_LABEL.cron, caption: cronSlogan(cfg.cron as string) };
    case "event":
      return {
        mode,
        icon: Radio,
        label: MODE_LABEL.event,
        caption: (cfg.series as string) || null,
      };
    case "inject": {
      const injectMode = (cfg.inject_mode as string | undefined) ?? "fire";
      return {
        mode,
        icon: Zap,
        label: MODE_LABEL.inject,
        caption: injectMode === "retain" ? "retain" : "fire",
      };
    }
    case "boot":
      return { mode, icon: Power, label: MODE_LABEL.boot, caption: null };
    case "manual":
    default:
      return { mode: "manual", icon: Hand, label: MODE_LABEL.manual, caption: null };
  }
}

/** The trigger icon for a row that has NO trigger node yet (a hand-built flow with no entry point).
 *  Reads as a manual / unstarted flow. */
export function fallbackTriggerIcon(): LucideIcon {
  return SquareTerminal;
}
