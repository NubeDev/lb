// The footer is the wizard's single control surface: Back on the left of the action cluster, one
// stateful primary on the right. "Forward" means something different on each step (generate/open,
// build/continue, publish/restart) but it's always the same button in the same place — so the user
// never hunts for what to do next. Errors surface inline here, next to where the action was taken.

import {
  ArrowLeft,
  ArrowRight,
  FolderOpen,
  Hammer,
  Loader2,
  Rocket,
  Sparkles,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { StudioWizard } from "./studio.wizard";

interface Primary {
  label: string;
  icon: LucideIcon;
  onClick: () => void;
  disabled: boolean;
}

export function WizardFooter({ w }: { w: StudioWizard }) {
  const primary = resolvePrimary(w);
  const canGoBack = w.step > 1 && !w.published;

  return (
    <div className="flex items-center gap-3 border-t border-border bg-panel/40 px-1 py-3">
      <div className="min-w-0 flex-1 text-xs">
        {w.status === "error" ? (
          <span role="alert" className="text-red-600 dark:text-red-400">
            {w.message}
          </span>
        ) : (
          <ContextHint w={w} />
        )}
      </div>

      {canGoBack && (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={w.back}
          disabled={w.busy}
          className="gap-1.5 text-muted"
        >
          <ArrowLeft size={14} />
          Back
        </Button>
      )}

      <Button
        type="button"
        variant="solid"
        size="sm"
        onClick={primary.onClick}
        disabled={primary.disabled}
        className="gap-1.5"
      >
        {w.busy ? (
          <Loader2 size={14} className="animate-spin" />
        ) : (
          <primary.icon size={14} />
        )}
        {primary.label}
      </Button>
    </div>
  );
}

function resolvePrimary(w: StudioWizard): Primary {
  if (w.step === 1) {
    return w.mode === "new"
      ? {
          label: w.busy ? "Generating…" : "Generate extension",
          icon: Sparkles,
          onClick: () => void w.generate(),
          disabled: w.busy || !w.id.trim(),
        }
      : {
          label: w.busy ? "Opening…" : "Open folder",
          icon: FolderOpen,
          onClick: () => void w.openExisting(),
          disabled: w.busy || !w.path.trim(),
        };
  }

  if (w.step === 2) {
    if (w.built) {
      return {
        label: "Continue to publish",
        icon: ArrowRight,
        onClick: () => w.goTo(3),
        disabled: w.busy,
      };
    }
    const retry = w.status === "error" || w.logs.length > 0;
    return {
      label: w.busy ? "Building…" : retry ? "Retry build" : "Build extension",
      icon: Hammer,
      onClick: () => void w.build(),
      disabled: w.busy,
    };
  }

  if (w.published) {
    return {
      label: "Build another",
      icon: Sparkles,
      onClick: w.reset,
      disabled: false,
    };
  }
  return {
    label: w.busy ? "Publishing…" : "Publish extension",
    icon: Rocket,
    onClick: () => void w.publish(),
    disabled: w.busy,
  };
}

function ContextHint({ w }: { w: StudioWizard }) {
  if (w.step === 1 && w.mode === "new") {
    return (
      <span className="text-muted">
        Scaffolds into{" "}
        <span className="font-mono text-fg">rust/extensions/{w.id || "…"}</span>
      </span>
    );
  }
  if (w.step === 2 && w.path) {
    return (
      <span className="truncate text-muted">
        Building <span className="font-mono text-fg">{w.path}</span>
      </span>
    );
  }
  return null;
}
