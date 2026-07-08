// The trigger picker shared by the Add and Edit workflow dialogs (rules-workflow-convergence scope).
// Two parts: a card grid that picks the WHEN (the mode — schedule / event / boot / inject / manual),
// then the mode-specific field (a CronBuilder, a series input, or the fire/retain toggle). Writes
// the SAME config keys the `trigger` node's descriptor schema validates, so ajv validity holds at
// `flows.save` / `flows.node.update` time — this is a presenter over the canonical shape, not a
// parallel one (mirrors `TriggerConfigFields` on the canvas).
//
// Extracted so Add and Edit read identically: an author who learned the picker in one dialog knows
// the other on sight. One component per file (FILE-LAYOUT).

import { Clock, Hand, Power, Radio, Zap } from "lucide-react";

import { CronBuilder } from "@/features/reminders/CronBuilder";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import type { TriggerConfig, TriggerMode } from "./workflowTrigger";

interface TriggerChoice {
  mode: TriggerMode;
  icon: typeof Clock;
  title: string;
  blurb: string;
}

// Order matters — the modes an author reaches for first (Schedule, Event) lead; Manual is last
// because picking it amounts to "I'll press Run myself".
const CHOICES: TriggerChoice[] = [
  { mode: "cron", icon: Clock, title: "On a schedule", blurb: "Hourly, daily, or a custom cron." },
  { mode: "event", icon: Radio, title: "On an event", blurb: "When a series is published." },
  { mode: "boot", icon: Power, title: "On boot", blurb: "Once, when the node starts." },
  { mode: "inject", icon: Zap, title: "On inject", blurb: "When a value is pushed in." },
  { mode: "manual", icon: Hand, title: "Manual", blurb: "Only when you press Run." },
];

interface WorkflowTriggerPickerProps {
  /** The current config — the picker is fully controlled. */
  config: TriggerConfig;
  /** Replace the config — the parent owns the state, the picker renders + emits edits. */
  onChange: (next: TriggerConfig) => void;
  /** Disable all controls (e.g. while submitting). */
  disabled?: boolean;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

export function WorkflowTriggerPicker({ config, onChange, disabled }: WorkflowTriggerPickerProps) {
  const set = (patch: Partial<TriggerConfig>) => onChange({ ...config, ...patch });

  return (
    <fieldset className="flex flex-col gap-3" disabled={disabled}>
      <div className="grid grid-cols-1 gap-1.5 sm:grid-cols-2">
        {CHOICES.map((c) => {
          const Icon = c.icon;
          const active = config.mode === c.mode;
          return (
            <Button
              key={c.mode}
              type="button"
              variant="outline"
              aria-pressed={active}
              disabled={disabled}
              onClick={() => set({ mode: c.mode })}
              className={cn(
                "h-auto items-start gap-2.5 whitespace-normal px-3 py-2 text-left",
                active ? "border-accent/40 bg-accent/5 hover:bg-accent/5" : "hover:bg-accent/[0.03]",
              )}
            >
              <Icon size={16} className={cn("mt-0.5 shrink-0", active ? "text-accent" : "text-muted")} />
              <span className="min-w-0 flex-1">
                <span className="block text-xs font-medium text-fg">{c.title}</span>
                <span className="mt-0.5 block text-[11px] leading-4 text-muted">{c.blurb}</span>
              </span>
            </Button>
          );
        })}
      </div>

      {config.mode === "cron" ? (
        <Field label="Schedule">
          <div className="rounded-md border border-border bg-bg p-2">
            <CronBuilder value={config.cron ?? ""} onChange={(v) => set({ cron: v })} />
          </div>
          <span className="font-mono text-[10px] text-muted">
            cron: <span className="text-fg">{config.cron || "—"}</span>
          </span>
        </Field>
      ) : null}

      {config.mode === "event" ? (
        <Field label="Series to watch">
          <Input
            aria-label="trigger series"
            placeholder="series name"
            value={config.series ?? ""}
            disabled={disabled}
            onChange={(e) => set({ series: e.target.value })}
          />
        </Field>
      ) : null}

      {config.mode === "inject" ? (
        <Field label="Inject behaviour">
          <div className="flex gap-2">
            {(["fire", "retain"] as const).map((m) => (
              <Button
                key={m}
                type="button"
                variant="outline"
                aria-pressed={(config.inject_mode ?? "fire") === m}
                disabled={disabled}
                onClick={() => set({ inject_mode: m })}
                className={cn(
                  "flex-1 px-3 py-1.5 text-xs font-medium",
                  (config.inject_mode ?? "fire") === m
                    ? "border-accent/40 bg-accent/5 text-accent hover:bg-accent/5"
                    : "text-muted hover:bg-accent/[0.03]",
                )}
              >
                {m === "fire" ? "Fire (one-shot run)" : "Retain (hold the value)"}
              </Button>
            ))}
          </div>
        </Field>
      ) : null}
    </fieldset>
  );
}
