// The ONE shared destructive-confirm dialog (admin-console scope). EVERY delete / disable / remove /
// uninstall in the admin console routes through this — never a bespoke confirm. It states *what is
// lost* (consequence) and *what is reversible*, and escalates the gate for data loss:
//   - none       → a single Confirm button (reversible: archive, disable, remove).
//   - type-name  → the operator must TYPE the entity name before Confirm enables (hard-delete /
//                  workspace purge). The backend ALSO requires a confirm token == the id + the
//                  `workspace.purge` cap — defense in depth (admin-crud session). This UI gate is
//                  the human safety net, not the security boundary.
//   - second-gate→ a second explicit "I understand" checkbox before Confirm enables.
//
// Cancel performs NOTHING (just closes). The dialog blocks the action until an explicit confirm.
// Markup + local input state only; the caller owns the actual verb call in `onConfirm`.

import { useState } from "react";
import type { ReactNode } from "react";
import { AlertTriangle } from "lucide-react";

export type Escalation = "none" | "type-name" | "second-gate";

export interface ConfirmDestructiveProps {
  /** The action title, e.g. "Delete workspace pilot". */
  title: string;
  /** Human consequence text — what is lost / what is reversible. Shown verbatim. */
  consequence: string;
  /** Reversible (archive/disable) vs irreversible (purge/delete). Drives the badge + copy. */
  reversible: boolean;
  /** The confirm escalation. `type-name` requires typing `confirmName`. */
  escalation: Escalation;
  /** For `type-name`: the exact string the operator must type to enable Confirm. */
  confirmName?: string;
  /** The verb label on the confirm button (default "Confirm"). */
  confirmLabel?: string;
  /** Run the destructive verb. Called only on an explicit, satisfied confirm. */
  onConfirm: () => void;
  /** Close without doing anything. */
  onCancel: () => void;
  /** Optional extra content rendered above the footer (e.g. the live-token revoke lever). */
  extra?: ReactNode;
}

export function ConfirmDestructive({
  title,
  consequence,
  reversible,
  escalation,
  confirmName,
  confirmLabel = "Confirm",
  onConfirm,
  onCancel,
  extra,
}: ConfirmDestructiveProps) {
  const [typed, setTyped] = useState("");
  const [acked, setAcked] = useState(false);

  const gateSatisfied =
    escalation === "none" ||
    (escalation === "type-name" && typed === confirmName) ||
    (escalation === "second-gate" && acked);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="w-96 rounded-lg border border-border bg-panel p-4 shadow-xl">
        <header className="flex items-center gap-2">
          <AlertTriangle size={16} className="text-accent" />
          <h2 className="text-sm font-medium">{title}</h2>
          <span
            className={`ml-auto rounded px-2 py-0.5 text-xs ${
              reversible ? "bg-accent/15 text-accent" : "bg-red-500/15 text-red-400"
            }`}
          >
            {reversible ? "reversible" : "irreversible"}
          </span>
        </header>

        <p className="mt-3 text-xs text-muted" data-testid="consequence">
          {consequence}
        </p>

        {escalation === "type-name" && (
          <label className="mt-3 block text-xs text-muted">
            Type <span className="font-mono text-accent">{confirmName}</span> to confirm:
            <input
              aria-label="type to confirm"
              className="mt-1 w-full rounded bg-bg px-2 py-1 text-sm"
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
            />
          </label>
        )}

        {escalation === "second-gate" && (
          <label className="mt-3 flex items-center gap-2 text-xs text-muted">
            <input
              aria-label="acknowledge"
              type="checkbox"
              checked={acked}
              onChange={(e) => setAcked(e.target.checked)}
            />
            I understand this cannot be undone.
          </label>
        )}

        {extra && <div className="mt-3">{extra}</div>}

        <footer className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            aria-label="cancel"
            onClick={onCancel}
            className="rounded bg-bg px-3 py-1 text-xs"
          >
            Cancel
          </button>
          <button
            type="button"
            aria-label="confirm action"
            disabled={!gateSatisfied}
            onClick={onConfirm}
            className="rounded bg-red-500/80 px-3 py-1 text-xs text-white disabled:opacity-40"
          >
            {confirmLabel}
          </button>
        </footer>
      </div>
    </div>
  );
}
