// The Setup wizard picker (setup scope) — the icon-card grid the Setup tab lands on. One card per
// entry in the catalog; picking one hands its id up so the hub swaps in that wizard. Purely
// presentational (tokens only, no verbs); the hub owns which wizard is active. Responsive grid
// (1→2→3 cols). One responsibility per file (FILE-LAYOUT).

import { ArrowRight } from "lucide-react";

import { WIZARDS, type WizardId } from "./catalog";

interface Props {
  onPick: (id: WizardId) => void;
}

export function WizardPicker({ onPick }: Props) {
  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-4 py-5">
      <div className="mx-auto max-w-4xl">
        <h3 className="text-base font-semibold text-fg">Choose a setup wizard</h3>
        <p className="mt-0.5 text-sm text-muted">
          Guided flows that orchestrate the People / Teams / Roles / Nav verbs into one path.
        </p>
        <div className="mt-5 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {WIZARDS.map((w) => {
            const Icon = w.icon;
            return (
              // eslint-disable-next-line no-restricted-syntax -- a card, not a shadcn Button shape
              <button
                key={w.id}
                type="button"
                onClick={() => onPick(w.id)}
                aria-label={w.title}
                className="group flex flex-col items-start gap-3 rounded-lg border border-border bg-panel p-4 text-left transition-colors hover:border-primary/50 hover:bg-primary/5"
              >
                <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
                  <Icon size={20} />
                </span>
                <div className="min-w-0">
                  <span className="block text-sm font-semibold text-fg">{w.title}</span>
                  <span className="mt-0.5 block text-xs text-muted">{w.blurb}</span>
                </div>
                <span className="mt-auto inline-flex items-center gap-1 text-xs font-medium text-primary opacity-0 transition-opacity group-hover:opacity-100">
                  Start <ArrowRight size={13} />
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
