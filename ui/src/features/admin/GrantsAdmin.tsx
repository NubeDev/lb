// Roles & grants administration (admin-console scope): READ a subject's grants + assign/revoke a cap
// or role — NO role editor this slice (the model + define lives in authz-grants). A subject is
// `kind:name` (`user:bob` / `team:eng`); assigning a role is a grant of `role:<name>`. Revoke routes
// through ConfirmDestructive (reversible — re-assign restores; but the freshness asymmetry means the
// drop lands on the subject's next sign-in for the cached half). Markup + wiring only; data lives in
// useGrantsAdmin. The gateway re-checks every verb (and no-widening) — the UI gate is convenience.

import { useState } from "react";
import { KeyRound } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { useGrantsAdmin } from "./useGrantsAdmin";

interface Props {
  ws: string;
}

export function GrantsAdmin({ ws }: Props) {
  const [subject, setSubject] = useState("user:bob");
  const { grants, roles, error, assign, revoke } = useGrantsAdmin(subject);
  const [newCap, setNewCap] = useState("");
  const [pending, setPending] = useState<string | null>(null);

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <KeyRound size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Roles &amp; grants</h1>
        <input
          aria-label="subject"
          className="ml-2 rounded bg-panel px-2 py-1 text-xs"
          value={subject}
          onChange={(e) => setSubject(e.target.value)}
        />
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error && (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error}
        </div>
      )}

      {roles.length > 0 && (
        <div className="border-b border-border px-4 py-1 text-xs text-muted">
          Roles: {roles.join(", ")}
        </div>
      )}

      <ul className="flex-1 overflow-y-auto px-4 py-2">
        {grants.length === 0 ? (
          <li className="text-sm text-muted">No grants for {subject}.</li>
        ) : (
          grants.map((cap) => (
            <li key={cap} className="flex items-center gap-2 py-1 text-sm" role="listitem">
              <span className="font-mono text-xs">{cap}</span>
              <button
                aria-label={`revoke ${cap}`}
                className="ml-auto rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                onClick={() => setPending(cap)}
              >
                Revoke
              </button>
            </li>
          ))
        )}
      </ul>

      <form
        className="flex gap-1 border-t border-border px-4 py-2"
        onSubmit={(e) => {
          e.preventDefault();
          const cap = newCap.trim();
          if (cap) {
            void assign(cap);
            setNewCap("");
          }
        }}
      >
        <input
          aria-label="cap to assign"
          className="min-w-0 flex-1 rounded bg-panel px-2 py-1 text-sm font-mono"
          placeholder="mcp:…:call or role:<name>"
          value={newCap}
          onChange={(e) => setNewCap(e.target.value)}
        />
        <button aria-label="assign" className="rounded bg-accent/15 px-3 text-accent">
          Assign
        </button>
      </form>

      {pending && (
        <ConfirmDestructive
          title={`Revoke ${pending}`}
          consequence={`Removes this grant from ${subject}. Gate-3 access (membership-checked reads) drops immediately; the cached token half drops on ${subject}'s next sign-in. Reversible — re-assign restores it.`}
          reversible
          escalation="none"
          confirmLabel="Revoke"
          onConfirm={() => {
            void revoke(pending);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </section>
  );
}
