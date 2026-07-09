// The login screen — the dev-login form that obtains a real session token (collaboration scope,
// slice 1). The user picks an identity + workspace; `signIn` posts to the gateway `login` route and
// stores the issued token. No password yet (Non-goals); the credential check plugs in server-side.
//
// Pre-auth branding: the sign-in card paints the workspace's brand (login heading / sub-heading /
// logo) from the workspace-keyed localStorage boot cache (`loadCachedBrand`, workspace-branding
// scope). No token exists here, so we cannot call `prefs.resolve`; the cache is the last brand this
// browser resolved for the entered workspace (populated after any prior authenticated visit). It
// re-reads as the visitor edits the workspace field, and falls back to the neutral defaults for a
// never-visited workspace. The full pre-auth public read route stays the deferred slice.

import { useMemo, useState } from "react";
import { LogIn } from "lucide-react";

import { loadCachedBrand, BRANDING_PLACEHOLDERS } from "@/lib/branding";

interface Props {
  onSignIn: (user: string, workspace: string) => Promise<void>;
}

export function LoginView({ onSignIn }: Props) {
  const [user, setUser] = useState("user:ada");
  const [workspace, setWorkspace] = useState("acme");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // The cached brand for the workspace currently entered — re-read on every workspace keystroke so
  // the card rebrands live as the visitor types. `null` (never-visited workspace) → neutral defaults.
  const brand = useMemo(() => loadCachedBrand(workspace.trim()), [workspace]);
  const heading = brand?.loginHeading || BRANDING_PLACEHOLDERS.loginHeading;
  const subheading = brand?.loginSubheading || BRANDING_PLACEHOLDERS.loginSubheading;
  const logo = brand?.loginLogoDataUri;

  return (
    <div className="flex h-full items-center justify-center bg-bg px-4">
      <form
        className="w-full max-w-sm rounded-lg border border-border bg-panel p-5 shadow-lg shadow-black/10"
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
        <div className="mb-5 flex items-start gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-md border border-accent/20 bg-accent/10 text-accent">
            {logo ? (
              <img src={logo} alt="" className="h-full w-full object-contain" />
            ) : (
              <LogIn size={17} />
            )}
          </div>
          <div>
            <h1 className="text-sm font-semibold text-fg">{heading}</h1>
            <p className="mt-0.5 text-xs leading-5 text-muted">{subheading}</p>
          </div>
        </div>
        {error && (
          <div role="alert" className="mb-3 rounded-md border border-red-500/25 bg-red-500/10 px-3 py-2 text-xs text-red-600 dark:text-red-300">
            {error}
          </div>
        )}
        <label className="mb-1.5 block text-xs font-medium text-muted">Identity</label>
        <input
          aria-label="identity"
          className="control-field mb-3 w-full"
          value={user}
          onChange={(e) => setUser(e.target.value)}
        />
        <label className="mb-1.5 block text-xs font-medium text-muted">Workspace</label>
        <input
          aria-label="workspace"
          className="control-field mb-4 w-full"
          value={workspace}
          onChange={(e) => setWorkspace(e.target.value)}
        />
        <button
          aria-label="sign in"
          disabled={busy}
          className="soft-button w-full"
        >
          {busy ? "Signing in..." : "Sign in"}
        </button>
      </form>
    </div>
  );
}
