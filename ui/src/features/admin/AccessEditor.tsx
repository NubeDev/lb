// The access editor (access-console scope) — shows and edits ONE subject's access. The PRIMARY grant
// path is now the catalog-driven `CapabilityPicker` (no-widening: only caps the admin holds); the
// raw-string entry stays as an "Advanced" escape hatch for power-users / non-`mcp:` surfaces. Revoke
// actions show honest timing AND offer the live-token `RevokeTokensLever` ("Apply now — end active
// sessions"). Built on shadcn primitives (Button/Input/Badge/Card) per ui-standards. Assigning a
// role is a grant of `role:<name>`; the gateway re-checks every verb. Markup + wiring; data in
// useSubjectGrants. One responsibility per file (FILE-LAYOUT).

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Select } from "@/components/ui/select";
import { ConfirmDestructive } from "@/features/confirm";
import { CapabilityPicker } from "./access/CapabilityPicker";
import { RevokeTokensLever } from "./access/RevokeTokensLever";
import { useSubjectGrants } from "./useSubjectGrants";

interface Props {
  /** The `kind:name` subject, or null when nothing is selected. */
  subject: string | null;
  /** Every role defined in the workspace (for the assign dropdown). */
  availableRoles: string[];
  /** The admin's session caps — gates the picker catalog + the revoke lever. */
  caps?: string[] | undefined;
}

type Pending = { kind: "role" | "cap"; value: string } | null;

export function AccessEditor({ subject, availableRoles, caps }: Props) {
  const { roles, caps: granted, error, assignRole, revokeRole, assignCap, revokeCap, refresh } =
    useSubjectGrants(subject);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [newCap, setNewCap] = useState("");
  const [pending, setPending] = useState<Pending>(null);

  if (!subject) return null;
  const unassigned = availableRoles.filter((r) => !roles.includes(r));

  return (
    <div className="space-y-4">
      {error && (
        <p className="text-xs text-destructive" role="alert">
          {error}
        </p>
      )}

      <CapabilityPicker
        alreadyGranted={granted}
        onPick={(cap) => void assignCap(cap)}
      />

      <div>
        <h3 className="mb-1 text-xs font-medium uppercase tracking-wide text-muted">Roles</h3>
        {roles.length === 0 ? (
          <p className="text-xs text-muted">No roles assigned.</p>
        ) : (
          <ul className="flex flex-wrap gap-1.5">
            {roles.map((r) => (
              <li key={r}>
                <Badge variant="secondary" className="gap-1">
                  {r}
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-auto px-1 py-0 text-muted hover:text-destructive"
                    aria-label={`revoke role ${r} from ${subject}`}
                    onClick={() => setPending({ kind: "role", value: r })}
                  >
                    ×
                  </Button>
                </Badge>
              </li>
            ))}
          </ul>
        )}
        {unassigned.length > 0 && (
          <div className="mt-2 flex flex-col gap-2 sm:flex-row sm:items-center">
            <Select
              aria-label={`assign a role to ${subject}`}
              className="sm:max-w-sm"
              value=""
              onChange={(e) => {
                if (e.target.value) void assignRole(e.target.value);
              }}
            >
              <option value="">Assign a role…</option>
              {unassigned.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </Select>
          </div>
        )}
      </div>

      <div>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="text-xs text-muted"
          onClick={() => setShowAdvanced((s) => !s)}
        >
          {showAdvanced ? "▾" : "▸"} Advanced: direct capabilities ({granted.length})
        </Button>
        {showAdvanced && (
          <div className="mt-2 space-y-2">
            {granted.length === 0 ? (
              <p className="text-xs text-muted">No direct capabilities.</p>
            ) : (
              <ul className="space-y-1">
                {granted.map((c) => (
                  <li key={c} className="flex items-center gap-2 text-xs">
                    <span className="break-all font-mono">{c}</span>
                    <Button
                      type="button"
                      variant="destructive"
                      size="sm"
                      className="ml-auto"
                      aria-label={`revoke ${c}`}
                      onClick={() => setPending({ kind: "cap", value: c })}
                    >
                      Revoke
                    </Button>
                  </li>
                ))}
              </ul>
            )}
            <form
              className="flex flex-col gap-2 sm:flex-row"
              onSubmit={(e) => {
                e.preventDefault();
                const cap = newCap.trim();
                if (cap) {
                  void assignCap(cap);
                  setNewCap("");
                }
              }}
            >
              <Input
                aria-label="capability to assign"
                className="font-mono sm:max-w-sm"
                placeholder="store:…/…:read (advanced)"
                value={newCap}
                onChange={(e) => setNewCap(e.target.value)}
              />
              <Button type="submit" size="sm" disabled={!newCap.trim()}>
                Assign
              </Button>
            </form>
          </div>
        )}
      </div>

      {pending && (
        <ConfirmDestructive
          title={pending.kind === "role" ? `Revoke role ${pending.value}` : `Revoke ${pending.value}`}
          consequence={`Removes this ${pending.kind === "role" ? "role" : "capability"} from ${subject}. Membership-checked access drops immediately; the cached token half drops on the subject's next sign-in. Reversible — re-assign restores it.`}
          reversible
          escalation="none"
          confirmLabel="Revoke"
          onConfirm={() => {
            if (pending.kind === "role") void revokeRole(pending.value);
            else void revokeCap(pending.value);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
          extra={
            <RevokeTokensLever
              subject={subject}
              caps={caps}
              context={pending.value}
              onApplied={() => void refresh()}
            />
          }
        />
      )}
    </div>
  );
}
