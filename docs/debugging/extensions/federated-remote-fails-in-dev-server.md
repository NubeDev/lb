# Federated extension remote fails to load under the Vite dev server

**Symptom (browser):** opening an installed extension's page in the live shell shows
`Could not load <ext>: getUrl(...).then is not a function`. The sidebar nav entry appears
(manifest `[ui]` block is registered) and the gateway serves `remoteEntry.js` with HTTP 200,
but `mount` never runs.

**Affected:** every federated remote (proof-panel, fleet-monitor) — not extension-specific.

**Environment:** the shell served by `make dev` / `make ui` (`cd ui && pnpm run dev`) on
`localhost:5173`.

## Root cause

`@originjs/vite-plugin-federation` is a **build-time** plugin. The federation **host runtime**
— the `_virtual___federation__-*.js` chunk that implements `__federation_method_setRemote` /
`__federation_method_getRemote` / the `getUrl().then(...)` import machinery — is only emitted by
`vite build`. Under `vite dev` (the dev server) that runtime is absent / stubbed, so the shell's
dynamic remote registration (`ui/src/features/ext-host/federation.ts`, which the shell uses because
remotes are gateway-served and unknown at build time) calls into a `getUrl` that does not return a
Promise → `getUrl(...).then is not a function`.

Confirmed by:
- `localhost:5173` is a dev server (`@vite/client`, `/src/main` served raw).
- `cd ui && vite build` emits `dist/assets/_virtual___federation__-*.js`; `vite dev` does not.
- The gateway Vitest suite passes (57 green) because it never goes through the dev server.

## Fix / workaround

Serve a **production build** of the shell, not the dev server:

```
cd ui && pnpm exec vite build
VITE_GATEWAY_URL=http://127.0.0.1:8080 pnpm exec vite preview --port 4173
# open http://127.0.0.1:4173/  (NOT the 5173 dev server)
```

Proper fix (follow-up): add a `make ui-preview` (build + preview) target and document that
**extension pages require the built shell** — the dev server cannot host federated remotes. The
`dev`/`ui` targets should either switch to build+preview or print a warning that extension pages
won't load under them.

## Regression coverage

The real-gateway Vitest (`ProofPanel.gateway.test.tsx`) already exercises the remote against a
built/host-runtime path and stays green; it does not catch this because it never uses `vite dev`.
A true regression test would assert the shipped shell **build** contains the federation host
runtime chunk — tracked as a follow-up (a build-artifact assertion, not a unit test).

## Related

- `ui/src/features/ext-host/federation.ts` (dynamic remote registration — needs the build-time runtime).
- `ui/vite.config.ts` (`federation({ name: "shell", remotes: {}, shared: [...] })` — host config).
- Sibling finding: `bridge-cannot-dispatch-host-native-series.md`.
