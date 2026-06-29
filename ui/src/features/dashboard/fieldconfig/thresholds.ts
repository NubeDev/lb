// Resolve a CANONICAL value against a `ThresholdsConfig` → a color token (viz field-config scope,
// "thresholds color, not alert"). Thresholds here only COLOR a value (and a line); firing on breach is
// the rules engine's job, never this file. The evaluation is against the canonical value (before any
// prefs unit conversion — field-config scope, Risks: "min/max/thresholds are in canonical units").
//
// One responsibility: value → step color. The color-name → token resolution is `color.ts`'s job.

import type { ThresholdsConfig } from "@/lib/dashboard";
import { resolveColor } from "./color";

/** The active step for `value` — the last step whose `value` ≤ the input (Grafana semantics; the
 *  first step is always `-∞`/`null`). `null` if no steps. `percentage` mode needs the field's
 *  min/max to normalize; callers that lack them pass the raw value and `absolute` is assumed. */
export function activeStep(
  value: number,
  cfg: ThresholdsConfig | undefined,
  bounds?: { min?: number; max?: number },
): { value: number | null; color: string } | null {
  if (!cfg || cfg.steps.length === 0) return null;
  const v =
    cfg.mode === "percentage" && bounds && bounds.min != null && bounds.max != null && bounds.max > bounds.min
      ? ((value - bounds.min) / (bounds.max - bounds.min)) * 100
      : value;
  let active = cfg.steps[0];
  for (const step of cfg.steps) {
    // The base step's value is -∞: null OR absent (the store drops an explicit JSON null, so a
    // round-tripped base step has no `value` key — both mean "always matches"). Later steps match v ≥ value.
    if (step.value == null || v >= step.value) active = step;
  }
  return active;
}

/** The themed color for `value` under `cfg`, or `fallback` when there are no thresholds. */
export function thresholdColor(
  value: number,
  cfg: ThresholdsConfig | undefined,
  fallback = "hsl(var(--accent))",
  bounds?: { min?: number; max?: number },
): string {
  const step = activeStep(value, cfg, bounds);
  return step ? resolveColor(step.color, fallback) : fallback;
}
