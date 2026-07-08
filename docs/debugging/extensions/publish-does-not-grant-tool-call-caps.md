# Publish does not grant the new extension's tool call caps (and the scaffold's `[ui] scope` own-tool name was wrong for hyphenated ids)

**Status:** open (the cap-grant gap is a recorded follow-up; the scaffold `[ui] scope` name bug is fixed).
**Area:** extensions / authz.
**Surfaced:** 2026-07-08, during the agent-authored-extension E2E (`docs/scope/external-agent/agent-ext-authoring-scope.md` G5a/G5b).

## Symptom

A freshly published extension's own tools are **not callable** by the publisher (or anyone) over
`POST /mcp/call`, even though publish returned `204` and the extension is `enabled + running`:

```
$ curl -X POST $GW/mcp/call … -d '{"tool":"energy-dashboard.ping","args":{}}'
denied
```

Even `hello.echo` (the S1 spine extension, loaded at boot) returns `denied` for the dev-login
admin. The catalog (`tools.catalog`) never lists any extension tool whose cap the caller doesn't
already hold, so the tool looks absent to the agent too.

A second, related defect: the scaffolded `[ui] scope` listed the extension's own tool under a
**kebab→dot-converted** name (`energy.dashboard.ping`) while the runtime resolves `<ext>.<tool>`
by splitting on the FIRST dot with the id **verbatim** (`energy-dashboard.ping`). So even if the
cap existed, a hyphenated-id page that called its own tool through the bridge would be scope-denied.

## Root cause

1. **Publish grants OUT-bound caps only.** `ext_publish` → `install_extension` persists
   `granted = requested ∩ admin_approved` — the caps the EXTENSION may call out through the host
   callback (`federation.query`, etc.). It does **not** grant any CALLER the cap to call the
   extension's own tools (`mcp:<id>.<tool>:call`). Those call caps are never added to any
   principal's set. The dev-login token is minted from a STATIC list
   (`session/credentials.rs::member_caps`); the `mcp:*.{get,list,write,create,update,delete,post}:call`
   wildcards it carries do not match `.ping`/`.derive`/`.simulate`, and there is no
   `mcp:*.ping:call`. The shipped `proof-panel` tools work only because they are hardcoded into
   `member_caps` (`mcp:proof-panel.proof.derive:call`, `…simulate:call`) — a path that does not
   exist for a freshly published extension (and would violate rule 10 if it did).
2. **`grants.assign` cannot rescue it.** `grants_assign` enforces no-widening: the assigner must
   HOLD the cap. No principal holds `mcp:energy-dashboard.ping:call`, so no one can grant it.
3. **Durable grants are not folded into the dev-login token.** `dev_claims` sets
   `caps: member_caps()` (static); it never reads the grants store. So even an
   `lb_authz::grant_assign` durable record would not change the dev token's effective caps — the
   MCP authorize gate only consults `principal.caps()` from the token.
4. **The scaffold `[ui] scope` own-tool name was wrong.** `devkit/src/scaffold.rs::tool_name`
   did `request.id.replace('-', ".")`, producing `energy.dashboard.ping` for id `energy-dashboard`.
   The runtime (`mcp/src/call/resolve.rs`) does `split_once('.')` and the registry key is the
   manifest id verbatim, so the call name is `energy-dashboard.ping`. The scope entry never
   matched.

## What was fixed here

- **The scaffold `[ui] scope` name bug** (`rust/crates/devkit/src/scaffold.rs`): the own-tool
  entry is now `format!("{}.ping", request.id)` — id verbatim, no kebab→dot swap. A hyphenated-id
  extension that calls its own tool through the bridge now matches the runtime name.
  `scaffold_test.rs` stays green; the devkit `write_file_test.rs` (5 green) added alongside.

## What is still open (the cap-grant gap)

The publish→call-tool flow is the load-bearing gap for **G5b** (the WASM-tool E2E, where the tool
itself IS the deliverable). Two viable directions (decision deferred — both touch the trust model):

- **(A) `ext_publish` auto-grants the caller the call caps for every `[[tools]]` the manifest
  declares**, directly into the grants store (`lb_authz::grant_assign(Subject::User(sub), …)`).
  Generic + rule-10-clean (the id is opaque data). BUT the dev-login token doesn't fold durable
  grants, so this needs `dev_claims` to consult the grants store too (or the caller must re-login
  before the cap is visible).
- **(B) Fold durable grants into the dev-login token at `dev_claims`** (read
  `resolve_user_caps` for the `sub` and union with `member_caps`). This is the more correct
  standalone fix — the grants store exists to be consulted — and unblocks (A) as well as any
  runtime grant a test/seed writes.

Until one lands, the **UI/data-dashboard** E2E (G5a) is unaffected: the page reaches data through
`bridge.call("federation.query", …)`, and `mcp:federation.query:call` IS in `member_caps`. The
page's data tool IS the tool the user sees; the scaffold's `ping` is boilerplate. The WASM-tool
E2E (G5b) is the one that needs the cap-grant fix.

## Repro

```bash
make cloud                          # node on :8080
TOKEN=$(curl -s -X POST localhost:8080/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
# Publish any built wasm ext with a [[tools]] entry (e.g. the devkit-scaffolded energy-dashboard).
curl -s -o /dev/null -w "%{http_code}\n" -X POST localhost:8080/extensions \
  -H "authorization: Bearer $TOKEN" -d '{"path":"…/rust/extensions/energy-dashboard"}'   # 204
# The tool is enabled + running…
curl -s -H "authorization: Bearer $TOKEN" localhost:8080/extensions | jq '.[]|select(.ext=="energy-dashboard")|{enabled,running}'
# …but calling it is denied:
curl -s -X POST localhost:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"energy-dashboard.ping","args":{}}'      # denied
```

## Lesson

A publish that approves an extension's OUT-bound authority but never its IN-bound callability is
half an approval. The cap model has the seams (`grants.assign`, durable grant store, `member_caps`
wildcards) but none of them connect publish→caller-caps, and the dev-login shortcut bypasses the
grants store entirely. The shipped built-ins hide this because their tool caps are hand-listed in
`member_caps` — a freshly published extension has no such special-case (and must not).
