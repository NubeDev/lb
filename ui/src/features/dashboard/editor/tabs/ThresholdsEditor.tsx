// The thresholds editor (viz field-config scope: `ThresholdsConfig`). Edits the step list — the first
// step is always `-∞` (value null) per Grafana, so it shows as the base step and isn't removable. Steps
// are bounded by the shared cap (`fieldconfig/caps.ts`, mirroring the host) so the editor refuses a
// 65th. Colors are Grafana semantic names resolved to theme tokens at RENDER (`fieldconfig/color.ts`),
// not painted raw here. One responsibility: author the threshold steps.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { ThresholdsConfig } from "@/lib/dashboard";
import { MAX_THRESHOLD_STEPS } from "../../fieldconfig/caps";
import { resolveColor } from "../../fieldconfig/color";

const COLORS = ["green", "yellow", "orange", "red", "blue", "purple"];

interface Props {
  value: ThresholdsConfig | undefined;
  onChange: (next: ThresholdsConfig | undefined) => void;
}

/** The default base threshold (Grafana's): a single `-∞ → green` step. */
function baseThresholds(): ThresholdsConfig {
  return { mode: "absolute", steps: [{ value: null, color: "green" }] };
}

export function ThresholdsEditor({ value, onChange }: Props) {
  const cfg = value ?? baseThresholds();
  const setStep = (idx: number, next: Partial<{ value: number | null; color: string }>) => {
    const steps = cfg.steps.map((s, i) => (i === idx ? { ...s, ...next } : s));
    onChange({ ...cfg, steps });
  };
  const addStep = () => {
    if (cfg.steps.length >= MAX_THRESHOLD_STEPS) return;
    const last = cfg.steps[cfg.steps.length - 1];
    const value = (last.value ?? 0) + 10;
    onChange({ ...cfg, steps: [...cfg.steps, { value, color: "red" }] });
  };
  const removeStep = (idx: number) => {
    if (idx === 0) return; // the base -∞ step stays
    onChange({ ...cfg, steps: cfg.steps.filter((_, i) => i !== idx) });
  };

  return (
    <div className="grid gap-1.5" aria-label="thresholds editor">
      {cfg.steps.map((step, idx) => (
        <div key={idx} className="flex items-center gap-1.5">
          <span
            className="inline-block h-3 w-3 shrink-0 rounded-full"
            style={{ background: resolveColor(step.color) }}
            aria-hidden
          />
          <Select
            aria-label={`threshold ${idx} color`}
            className="h-8 w-24"
            value={step.color}
            onChange={(e) => setStep(idx, { color: e.target.value })}
          >
            {COLORS.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </Select>
          {idx === 0 ? (
            <span className="text-xs text-muted">base (−∞)</span>
          ) : (
            <>
              <Input
                aria-label={`threshold ${idx} value`}
                type="number"
                className="h-8 w-24 text-xs"
                value={step.value ?? 0}
                onChange={(e) => setStep(idx, { value: Number(e.target.value) })}
              />
              <Button
                variant="ghost"
                aria-label={`remove threshold ${idx}`}
                className="h-auto px-1.5 text-xs text-muted hover:text-red-500"
                onClick={() => removeStep(idx)}
              >
                ×
              </Button>
            </>
          )}
        </div>
      ))}
      <Button
        variant="outline"
        size="sm"
        aria-label="add threshold"
        className="h-auto justify-self-start px-2 py-0.5 text-[11px] text-muted hover:text-fg"
        onClick={addStep}
      >
        + Add threshold
      </Button>
    </div>
  );
}
