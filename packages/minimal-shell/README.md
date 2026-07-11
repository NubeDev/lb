# @nube/minimal-shell

The publishable minimal host for 100%-extension UIs. Auth screens (login + invite-accept),
`ext.list` discovery, full-screen scoped mount via the `@nube/ext-ui-sdk` federation seam,
SSE/event-stream wiring, theme-token provider, PWA manifest. No lb chrome.

## Usage

```sh
pnpm install
VITE_GATEWAY_URL=http://127.0.0.1:8080 pnpm dev
```

### Config (env vars at build time)

- `VITE_GATEWAY_URL` — the gateway URL (default `http://127.0.0.1:8080`).
- `VITE_HOME_EXT` — the extension id to mount as "home" (opaque config data — rule 10). If unset,
  the shell discovers the first extension with a UI via `ext.list`.
- `VITE_HOME_ENTRY` — the remote entry filename (default `remoteEntry.js`).
- `VITE_HOME_SCOPE` — comma-separated tool names the bridge allows (defense in depth).

### What it does

1. Branded login (pre-auth cache from localStorage).
2. Session token stored in localStorage; `Authorization: Bearer` on every request.
3. Full-screen mount of the configured/discovered extension page via `defineRemote`'s host
   counterpart.
4. SSE hub (one EventSource per tab, refcounted).
5. Theme-token cascade (extensions inherit `--bg`/`--accent`/… via CSS).
6. PWA manifest (installable).

### What it doesn't do (non-goals)

- No sidebar, no dock, no admin console (that's the full shell).
- No nav framework (one extension owns the viewport).
- No UI kit (components come from the extension + SDK presets).

If you need any of these, you've outgrown the minimal shell — take the full `ui/` shell instead.
