// Dev-only login prefills so the preview is one tap. Mirrors the root Makefile's dev identity
// (`SEED_USER ?= user:ada`, `WS ?= acme`) so the app and the CLI/browser demos share a workspace.
//
// IMPORTANT about the workspace (global-identity, decision #4): whoever logs into an EMPTY
// workspace first bootstraps it as admin; that same user always gets back in; a DIFFERENT user is
// refused with "not a member" until an admin admits them. So these defaults only "just work" when
// this user is the one who owns this workspace on the node you point at. If you ever see
// "not a member", either use YOUR user for this workspace or pick a fresh workspace name — the
// first login into a new one bootstraps you in. `__DEV__` gates this to dev builds only.

declare const __DEV__: boolean;

const isDev = typeof __DEV__ !== 'undefined' ? __DEV__ : true;

/** Prefilled login values in dev; empty in release (never ship a baked-in identity). */
export const devLogin = {
  user: isDev ? 'ada' : '',
  workspace: isDev ? 'acme' : '',
  /** A sensible default node URL for the browser preview; the real device enters its own. */
  nodeUrl: isDev ? 'http://127.0.0.1:8080' : '',
};
