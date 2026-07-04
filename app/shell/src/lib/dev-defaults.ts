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

/** The shape of the login prefill the LoginScreen reads. */
export interface DevLogin {
  user: string;
  workspace: string;
  /**
   * A sensible default node URL for the browser preview; the real device enters its own.
   * Default 8080 = the root `make dev` node (the common dev loop). Override with `?node=` — e.g.
   * `?node=http://127.0.0.1:8087` for the app's own throwaway `test_gateway` (`make -C app dev`).
   *
   * Historically this defaulted to 8087 to dodge a `make dev` login 403: the bare handle `ada` was
   * treated as a distinct principal from the seeded `user:ada`, so `ada` was "not a member" of the
   * persistent `acme`. That is fixed at the login edge — the gateway now canonicalizes a bare handle
   * to the `user:<name>` principal, so `ada` resolves to `user:ada` on any node (an empty in-memory
   * node still bootstraps it). Either port works; 8080 matches the default `make dev`.
   */
  nodeUrl: string;
}

/**
 * Prefilled login values in dev; empty in release (never ship a baked-in identity).
 *
 * On NATIVE the `__DEV__` gate is authoritative (Metro sets it true in dev, false in release).
 * The react-native-web PREVIEW is different: `vite-plugin-react-native-web` hardcodes `__DEV__`
 * false, so the web entry (`web/index.web.tsx`, preview-only code) calls `setDevLogin(...)` to
 * seed ada/acme explicitly. Keeping the gate here means a real release app still ships empty
 * fields — the override lives only in preview code that never enters a device bundle.
 */
export let devLogin: DevLogin = {
  user: isDev ? 'ada' : '',
  workspace: isDev ? 'acme' : '',
  nodeUrl: isDev ? 'http://127.0.0.1:8080' : '',
};

/** Preview-only: seed the login prefill (the web entry uses this; native relies on `__DEV__`). */
export function setDevLogin(next: Partial<DevLogin>): void {
  devLogin = { ...devLogin, ...next };
}
