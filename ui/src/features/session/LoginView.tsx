// The login screen — the dev-login form that obtains a real session token (collaboration scope,
// slice 1). The user picks an identity + workspace; `signIn` posts to the gateway `login` route and
// stores the issued token. No password yet (Non-goals); the credential check plugs in server-side.

import { useState } from "react";
import { LogIn } from "lucide-react";

interface Props {
  onSignIn: (user: string, workspace: string) => Promise<void>;
}

export function LoginView({ onSignIn }: Props) {
  const [user, setUser] = useState("user:ada");
  const [workspace, setWorkspace] = useState("acme");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  return (
    <div className="flex h-full items-center justify-center bg-bg">
      <form
        className="w-72 rounded-lg border border-border bg-panel p-5"
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError(null);
          try {
            await onSignIn(user.trim(), workspace.trim());
          } catch (err) {
            setError(err instanceof Error ? err.message : String(err));
          } finally {
            setBusy(false);
          }
        }}
      >
        <h1 className="mb-3 flex items-center gap-2 text-sm font-medium">
          <LogIn size={16} className="text-accent" /> Sign in
        </h1>
        {error && (
          <div role="alert" className="mb-2 text-xs text-accent">
            {error}
          </div>
        )}
        <label className="mb-1 block text-xs text-muted">Identity</label>
        <input
          aria-label="identity"
          className="mb-3 w-full rounded bg-bg px-2 py-1 text-sm"
          value={user}
          onChange={(e) => setUser(e.target.value)}
        />
        <label className="mb-1 block text-xs text-muted">Workspace</label>
        <input
          aria-label="workspace"
          className="mb-4 w-full rounded bg-bg px-2 py-1 text-sm"
          value={workspace}
          onChange={(e) => setWorkspace(e.target.value)}
        />
        <button
          aria-label="sign in"
          disabled={busy}
          className="w-full rounded bg-accent/15 py-1.5 text-sm text-accent disabled:opacity-50"
        >
          {busy ? "Signing in…" : "Sign in"}
        </button>
      </form>
    </div>
  );
}
