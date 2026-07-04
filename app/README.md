# `app/` — the React Native mobile app (workshop)

The mobile twin of `ui/`: one RN host shell + federated extensions + the shared SDK.
**Status: shell slice SHIPPED** (login → workspace switcher → channels over REST + live
SSE → cap-gated `ext.list` nav; 12/12 real-gateway tests in `sdk/`). Extensions + the
full sdk extraction are still scope. The authoritative ask is `docs/scope/app/README.md`
(read order: shell → extensions → sdk); shipped truth: `docs/public/app/app.md`;
working log: `docs/sessions/app/app-shell-session.md`.

```
shell/                     ← the RN host app (Re.Pack 5 + Module Federation 2)
sdk/                       ← @nube/app-sdk — contract types + verb map + invoke/stream clients
extensions/
  proof-panel-app/         ← mobile companion of rust/extensions/proof-panel (page + widget)
  channel-chat/            ← pure-app extension: channels + in-channel AI agent
docs/                      ← pointer only; authored docs live in docs/scope/app/
```

Ground rules (from the scopes — the short version):

- The app talks to the node **only through the gateway** (REST + SSE, the web verb
  map). No Zenoh session on the phone; no second transport surface.
- App extension remotes are **JS-only** Module Federation containers, signed and
  served by the gateway under `/extensions/{ext}/app/`, mounted as React components
  (`Page` / `Widget`) over the same `ctx`/`bridge` contract as the web.
- The contract source of truth is `sdk/` (`@nube/app-sdk`); the web mirrors are
  checked against it, never the other way around.
- `docs/FILE-LAYOUT.md` applies here exactly as in `ui/` — one component/hook/verb
  per file, ≤400 lines hard.
