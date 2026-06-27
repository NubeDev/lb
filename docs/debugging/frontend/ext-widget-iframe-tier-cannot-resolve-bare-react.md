# An installed extension's widget tile renders blank — the iframe tier can't resolve its bare `react` imports

- Area: frontend
- Status: resolved
- First seen: 2026-06-28
- Resolved: 2026-06-28
- Session: ../../sessions/frontend/widget-palette-session.md
- Regression test: ui/src/features/dashboard/builder/widgetBuilder.test.ts (`trust-tier routing`) + ui/src/features/dashboard/builder/widgetBuilder.gateway.test.tsx (`packaged-tile palette round-trip`, `palette trust-tier routing`)

## Symptom

A packaged `[[widget]]` tile added from the dashboard palette (e.g. `proof-panel`'s **Proof
Ping**) rendered **blank**. Loading the tile's remote directly — the exact snippet the iframe
tier runs —

```js
const m = await import("http://127.0.0.1:8080/extensions/proof-panel/ui/remoteEntry.js");
const mount = m.mountWidget || m.mount;
if (typeof mount === "function") mount(el, { workspace: "", binding: {}, options: {} }, bridge, "proof-ping");
else el.textContent = "remote has no mountWidget";
```

throws `Failed to resolve module specifier "react"` (and `"react-dom/client"`,
`"react/jsx-runtime"`). The served `remoteEntry.js` begins with bare imports:
`import { jsx } from "react/jsx-runtime"; import { createRoot } from "react-dom/client";`.

## Root cause

Two compounding facts:

1. **The remote is built to be resolved by the shell's import map.** `proof-panel/ui/vite.config.ts`
   externalizes `react`/`react-dom`/`react-dom/client`/`react/jsx-runtime` (the rubix-cube
   import-map pattern, deliberately NOT bundling React) so the page renders **in-process against the
   host's single React** — no second copy, no "Invalid hook call". Those bare specifiers survive
   into the output and **only resolve where an import map exists: the shell `index.html`** (the
   trusted in-process tier).
2. **`ExtWidget` routed `proof-panel` to the iframe tier** because `trust.ts`'s allow-list
   (`VITE_TRUSTED_WIDGET_KEYS`) defaults empty, so any non-allow-listed key → `iframe`. The
   opaque-origin sandbox (`WidgetIframe`/`iframeRuntime`) has **no import map**, a CSP that doesn't
   allow the gateway origin, and a `template`-engine path that does `root.innerHTML = code` (it
   injects the JS as HTML text — it never executes a federated React remote at all). So a federated
   extension widget could **never** render in the iframe tier; the tier was a dead end for it.

The deeper modelling error: the iframe sandbox exists for **untrusted author code** (the scripted
`plot`/`d3`/`template` views a *dashboard editor types*). An **installed extension** is a different
trust class — installing one already requires the publish/install capability (a developer/admin
decision to run that code on the node). Treating an installed widget as "untrusted by default → iframe"
was both wrong (it can't work — the bundle isn't built for it) and unnecessary (the install was the
trust gate).

## Fix

**Installed extension widgets always render in-process; the iframe tier is reserved for scripted
author code.** In `trust.ts`, `extWidgetTier()` now returns `"in-process"` for any installed
extension widget (the install is the trust decision), and `scriptedTier()` stays `"iframe"`
(author code never runs in-process). `ExtWidget` drops its dead iframe branch and always federates
the remote via `loadRemoteWidgetMount` — resolving React through the shell import map, the exact
tier the bundle was built for. The `VITE_TRUSTED_WIDGET_KEYS` allow-list and `WidgetIframe`/
`iframeRuntime` remain, now used **only** by the scripted views.

Rejected alternative: teach the iframe runtime to load ext remotes (inject an esm.sh import map +
relax CSP + an `ext` engine). It would force a **second React copy** into the sandbox and add real
surface area to support a tier no installed widget needs — the install cap is already the gate.

## Proven fixed

Live, against the running node with `proof-panel` published: the in-process tier mounts the remote
and renders the real `proof.demo` value (see the session doc's live-e2e output). Tests: the unit
`trust-tier routing` case now asserts an installed ext widget → `in-process` (scripted → `iframe`);
the gateway `palette trust-tier routing` + round-trip assert the `in-process` mount path.

## Guardrail

`extWidgetTier` no longer has an input that yields `iframe` for an installed widget — the class of
"federated ext widget routed to a tier that can't load it" is now unrepresentable. The
allow-list/iframe machinery is typed to the scripted-view path only.
