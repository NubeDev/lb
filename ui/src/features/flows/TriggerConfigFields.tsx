// Trigger-node config fields (flows-canvas scope). The trigger's config is a fixed shape (`mode` +
// mode-specific fields) the generic `SchemaForm` renders as raw inputs, which is poor for `cron` —
// most authors do not read a 5-field cron string. This renders the trigger config purposefully: a
// mode `Select`, the existing visual `CronBuilder` (react-js-cron, already used by reminders) when
// `mode=cron`, a series picker when `mode=event`, and the fire|retain `inject_mode` when
// `mode=inject`. It writes the SAME config keys the descriptor's schema validates, so ajv validity
// still holds — this is a presenter over the same buffer the canvas owns.

import { CronBuilder } from "@/features/reminders/CronBuilder";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";

const MODES = [
  { value: "manual", label: "Manual" },
  { value: "cron", label: "Schedule (cron)" },
  { value: "event", label: "Event (series)" },
  { value: "inject", label: "Inject" },
  { value: "boot", label: "On boot" },
];

const INJECT_MODES = [
  { value: "fire", label: "Fire (one-shot run)" },
  { value: "retain", label: "Retain (hold the value)" },
];

interface Props {
  config: Record<string, unknown>;
  onChange: (next: Record<string, unknown>) => void;
  disabled?: boolean;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-xs">
      <span className="font-medium text-fg">{label}</span>
      {children}
    </label>
  );
}

export function TriggerConfigFields({ config, onChange, disabled }: Props) {
  const mode = (config.mode as string | undefined) ?? "manual";
  const cron = (config.cron as string | undefined) ?? "";
  const series = (config.series as string | undefined) ?? "";
  const injectMode = (config.inject_mode as string | undefined) ?? "fire";

  const set = (patch: Record<string, unknown>) => onChange({ ...config, ...patch });

  return (
    <div className="flex flex-col gap-3">
      <Field label="Trigger mode">
        <Select
          aria-label="trigger mode"
          value={mode}
          disabled={disabled}
          onChange={(e) => set({ mode: e.target.value })}
        >
          {MODES.map((m) => (
            <option key={m.value} value={m.value}>
              {m.label}
            </option>
          ))}
        </Select>
      </Field>

      {mode === "cron" ? (
        <Field label="Schedule">
          <div className="rounded-md border border-border bg-bg p-2">
            <CronBuilder value={cron} onChange={(v) => set({ cron: v })} />
          </div>
          <span className="mt-1 font-mono text-[10px] text-muted">
            cron: <span className="text-fg">{cron || "—"}</span>
          </span>
        </Field>
      ) : null}

      {mode === "event" ? (
        <Field label="Series to watch">
          <Input
            aria-label="trigger series"
            placeholder="series name"
            value={series}
            disabled={disabled}
            onChange={(e) => set({ series: e.target.value })}
          />
        </Field>
      ) : null}

      {mode === "inject" ? (
        <Field label="Inject behaviour">
          <Select
            aria-label="trigger inject mode"
            value={injectMode}
            disabled={disabled}
            onChange={(e) => set({ inject_mode: e.target.value })}
          >
            {INJECT_MODES.map((m) => (
              <option key={m.value} value={m.value}>
                {m.label}
              </option>
            ))}
          </Select>
        </Field>
      ) : null}
    </div>
  );
}
