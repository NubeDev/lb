// Step 1 — the one branch in the whole flow, made explicit. Two paths that used to compete as stacked
// panels are now a deliberate either/or: generate a fresh extension, or open a folder you already have.
// Picking a path reveals only its inputs; the footer's primary action runs it and advances to Build.

import { FolderOpen, Sparkles } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { DevkitFeature, DevkitTier } from "@/lib/devkit/devkit.api";
import { DirectoryPicker } from "./DirectoryPicker";
import { RecentBuilds } from "./RecentBuilds";
import type { StudioWizard } from "./studio.wizard";

const FEATURE_LABELS: Record<DevkitFeature, string> = {
  ui: "UI",
  "series-read": "Series read",
  ingest: "Ingest",
  kv: "KV",
};

const TIER_HINT: Record<DevkitTier, string> = {
  wasm: "Sandboxed, portable. The default.",
  native: "A native sidecar. More power, more trust.",
};

export function CreateStep({ w }: { w: StudioWizard }) {
  return (
    <div className="flex flex-col gap-5">
      <RecentBuilds
        recent={w.recent}
        busy={w.busy}
        onRun={w.openRecent}
        onForget={w.forgetRecent}
      />

      <div className="grid gap-3 sm:grid-cols-2">
        <PathCard
          selected={w.mode === "new"}
          onSelect={() => w.setMode("new")}
          icon={Sparkles}
          title="Generate new"
          desc="Scaffold a fresh extension from a template."
        />
        <PathCard
          selected={w.mode === "existing"}
          onSelect={() => w.setMode("existing")}
          icon={FolderOpen}
          title="Open existing"
          desc="Point at a folder you've already scaffolded."
        />
      </div>

      {w.mode === "new" ? (
        <div className="flex flex-col gap-4">
          <Field
            label="Extension ID"
            hint="Lowercase, dotted or dashed. This names the artifact."
          >
            <Input
              value={w.id}
              onChange={(e) => w.setId(e.target.value)}
              className="font-mono"
              spellCheck={false}
            />
          </Field>

          <Field label="Tier">
            <div className="grid grid-cols-2 gap-2">
              {(["wasm", "native"] as DevkitTier[]).map((item) => (
                <Button
                  key={item}
                  type="button"
                  variant="outline"
                  onClick={() => w.setTier(item)}
                  aria-pressed={w.tier === item}
                  className={cn(
                    "h-auto flex-col items-start justify-start gap-0.5 px-3 py-2 text-left font-normal",
                    w.tier === item
                      ? "border-accent bg-accent/10 text-fg ring-1 ring-accent/20 hover:bg-accent/10"
                      : "text-muted hover:border-accent/40 hover:bg-bg hover:text-fg",
                  )}
                >
                  <span className="font-mono text-xs font-medium">{item}</span>
                  <span className="text-[11px] leading-tight text-muted">
                    {TIER_HINT[item]}
                  </span>
                </Button>
              ))}
            </div>
          </Field>

          {w.availableFeatures.length > 0 && (
            <Field label="Features" hint="What the host grants this extension.">
              <div className="flex flex-wrap gap-2">
                {w.availableFeatures.map((feature) => {
                  const on = w.features.includes(feature);
                  return (
                    <Button
                      key={feature}
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => w.toggleFeature(feature)}
                      aria-pressed={on}
                      className={cn(
                        "h-auto rounded-full px-3 py-1",
                        on
                          ? "border-accent/30 bg-accent/15 text-accent hover:bg-accent/20"
                          : "text-muted hover:border-accent/40 hover:bg-bg hover:text-fg",
                      )}
                    >
                      {FEATURE_LABELS[feature]}
                    </Button>
                  );
                })}
              </div>
            </Field>
          )}
        </div>
      ) : (
        <Field label="Extension folder" hint="Browse to a folder that holds an extension.toml.">
          <DirectoryPicker value={w.path} onPick={w.setPath} />
        </Field>
      )}
    </div>
  );
}

function PathCard({
  selected,
  onSelect,
  icon: Icon,
  title,
  desc,
}: {
  selected: boolean;
  onSelect: () => void;
  icon: typeof Sparkles;
  title: string;
  desc: string;
}) {
  return (
    <Button
      type="button"
      variant="outline"
      onClick={onSelect}
      aria-pressed={selected}
      className={cn(
        "h-auto items-start justify-start gap-3 rounded-lg p-3 text-left font-normal",
        selected
          ? "border-accent bg-accent/10 ring-1 ring-accent/20 hover:bg-accent/10"
          : "hover:border-accent/40 hover:bg-bg",
      )}
    >
      <span
        className={cn(
          "flex h-8 w-8 shrink-0 items-center justify-center rounded-md border",
          selected
            ? "border-accent/30 bg-accent/15 text-accent"
            : "border-border bg-panel text-muted",
        )}
      >
        <Icon size={16} />
      </span>
      <span className="min-w-0">
        <span className="block text-sm font-medium text-fg">{title}</span>
        <span className="mt-0.5 block text-xs leading-snug text-muted">
          {desc}
        </span>
      </span>
    </Button>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-xs font-medium text-fg">{label}</span>
      {children}
      {hint && (
        <span className="text-[11px] leading-tight text-muted">{hint}</span>
      )}
    </label>
  );
}
