// The ONE shared destructive-confirm dialog (admin-console scope). EVERY delete / disable / remove /
// uninstall in the admin console routes through this — never a bespoke confirm. It states *what is
// lost* (consequence) and *what is reversible*, and escalates the gate for data loss:
//   - none       → a single Confirm button (reversible: archive, disable, remove).
//   - type-name  → the operator must TYPE the entity name before Confirm enables (hard-delete /
//                  workspace purge). The backend ALSO requires a confirm token == the id + the
//                  `workspace.purge` cap — defense in depth (admin-crud session). This UI gate is
//                  the human safety net, not the security boundary.
//   - second-gate→ a second explicit "I understand" toggle before Confirm enables.
//
// Cancel performs NOTHING (just closes). The dialog blocks the action until an explicit confirm.
// Markup + local input state only; the caller owns the actual verb call in `onConfirm`.
//
// On the shadcn `Dialog` primitive (ui-standards-scope) — focus trapping + Escape/overlay dismissal
// come for free; dismissing any way routes through `onCancel` (which does nothing but close).

import { useState } from "react";
import type { ReactNode } from "react";
import { AlertTriangle } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";

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
    <Dialog open onOpenChange={(o) => (o ? undefined : onCancel())}>
      <DialogContent showClose={false} className="max-w-sm gap-3">
        <DialogHeader>
          <div className="flex items-center gap-2">
            <AlertTriangle size={16} className="text-accent" />
            <DialogTitle>{title}</DialogTitle>
            <Badge
              variant={reversible ? "outline" : "destructive"}
              className={reversible ? "ml-auto border-accent/25 bg-accent/10 text-accent" : "ml-auto"}
            >
              {reversible ? "reversible" : "irreversible"}
            </Badge>
          </div>
        </DialogHeader>

        <DialogDescription data-testid="consequence">{consequence}</DialogDescription>

        {escalation === "type-name" && (
          <label className="block text-xs text-muted">
            Type <span className="font-mono text-accent">{confirmName}</span> to confirm:
            <Input
              aria-label="type to confirm"
              className="mt-1 h-8"
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
            />
          </label>
        )}

        {escalation === "second-gate" && (
          <label className="flex items-center gap-2 text-xs text-muted">
            <Switch
              aria-label="acknowledge"
              checked={acked}
              onCheckedChange={setAcked}
            />
            I understand this cannot be undone.
          </label>
        )}

        {extra && <div>{extra}</div>}

        <DialogFooter>
          <Button type="button" variant="outline" size="sm" aria-label="cancel" onClick={onCancel}>
            Cancel
          </Button>
          <Button
            type="button"
            variant="destructive"
            size="sm"
            aria-label="confirm action"
            disabled={!gateSatisfied}
            onClick={onConfirm}
          >
            {confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
