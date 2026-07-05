# Webhooks — a first-class inbound-HTTP surface, keyed and mediated (session)

- Date: 2026-07-05
- Scope: ../../scope/ingest/webhooks-scope.md
- Stage: S8 (data plane) — no new stage; builds on the shipped ingest buffer + API keys + flows
- Status: done (Rust core + admin + inbound route + tests green; UI + flow node are named follow-ups)

## Goal

Build the platform's generic, workspace-walled, credential-protected inbound-HTTP surface — the
thing the scope calls "API-key ⊕ ingest-producer ⊕ flow-source, glued by the gateway route." A
third-party service `POST`s to a stable URL we issued; we authenticate per the hook's own
credential; every accepted hit becomes exactly one ingest `Sample` on the hook's series. The
service is **the producer**, never a second store; the endpoint + credential + URL outlive any one
flow and are reachable by anything that subscribes to the series.

This slice ships the **core service + the gateway routes + the integration tests**. The admin-UI
wizard and the flow `webhook` source node are named follow-ups (the latter depends on a deeper
flow-integration change — see "Decisions" below).

## What changed

### Backend — `rust/crates/host/src/webhook/` (NEW module, one verb per file, FILE-LAYOUT)

- **`model.rs`** — the `webhook` record + `WebhookView` (credential-free), the `AuthMode` enum
  (`bearer` | `signature`), the derived `series_for(ws, id)` / `producer_for(id)` / `secret_path(id)`
  helpers, and the constants (`TABLE` = `"webhook"`, `TOMBSTONE_STATUS` = `"__revoked__"`,
  `HMAC_SCHEME` = `"hmac-sha256"`, `DEFAULT_HMAC_HEADER` = `"X-Signature"`).
- **`error.rs`** — `WebhookError` with the opaque `NotFound`/`Invalid`/`Revoked` surface the route
  collapses to a bare 404/410 (no existence leak), plus `Denied`/`BadInput`/`Widen`/`Store` for
  the admin verbs. `From<IngestError>` so the accept path threads ingest errors through.
- **`create.rs`** — `webhook_create` (both modes). The no-widening guard runs up front (the
  creator must hold `mcp:ingest.write:call`). `bearer` mode delegates to `apikey_create` with one
  narrowed cap and stores `bearer_key_id` on the record. `signature` mode generates a shared
  secret, stores it in `lb-secrets` at `webhook/{id}` under `Visibility::Workspace`, and stores
  `secret_ref` + `hmac_header`. Returns `CreatedWebhook { id, url_path, secret, auth_mode,
  hmac_header }` — the secret is the ONLY egress of the raw credential.
- **`list.rs` / `get.rs`** — credential-free enumeration (re-uses `lb_store::list` with
  `kind_discrim`, like apikeys).
- **`revoke.rs`** — tombstone + revoke the linked apikey (bearer) + cache-bust. Idempotent.
- **`rotate.rs`** — `bearer` mode delegates to `apikey_rotate` (same keyid, fresh secret, cache
  busted); `signature` mode overwrites the `lb-secrets` shared secret. INSTANT — no overlap.
- **`verify.rs`** — the `signature`-mode HMAC verifier (constant-time compare over the raw body,
  `sha256=<hex>` shape; mirrors the vetted `lb_role_github_webhook::verify` discipline).
- **`auth.rs`** — `webhook_resolve`: the inbound auth path. Loads the record (404 opaque),
  per-mode verify (bearer → linked apikey path with the **linkage check** that the presented keyid
  == `bearer_key_id`; signature → mediate-read the `lb-secrets` shared secret + HMAC over raw
  body). Returns `(record, principal)`.
- **`accept.rs`** — `webhook_accept`: builds the `Sample` (raw body as payload, JSON-or-string),
  calls `ingest_write`, drains workspace staging, publishes motion, bumps `last_hit_at`.
- **`secret.rs`** — shared-secret generator (re-uses `lb_apikey::generate_secret`).
- **`mod.rs`** — module doc (the rule-10 invariant restated) + re-exports.

### Gateway — `rust/role/gateway/src/routes/`

- **`webhook.rs`** (NEW) — the public inbound `POST /hooks/{ws}/{id}`. Captures the raw `Bytes`
  body BEFORE any JSON parse (load-bearing for HMAC), extracts the bearer header value, passes a
  header-lookup closure to `webhook_resolve`, then calls `webhook_accept`. Replies `202 Accepted
  { id, series, seq }`. Every auth failure collapses to the same opaque `404` (unknown id,
  disabled, wrong-secret, cross-ws URL all look identical — no oracle); a revoked hook is `410
  Gone`.
- **`admin_webhooks.rs`** (NEW) — the admin surface (`/admin/webhooks` list/create + `/{id}`
  get/revoke/rotate), mirroring `admin_apikeys.rs` 1:1.
- **`routes/mod.rs`** + **`server.rs`** — wired the new routes. The inbound `POST /hooks/{ws}/{id}`
  is registered FIRST (the only unauthenticated-by-session route in the gateway — a third-party
  caller presents the hook's own credential, not a session token).

### Caps — `rust/role/gateway/src/session/credentials.rs`

Added `mcp:webhook.manage:call`, `mcp:ingest.write:call`, `secret:webhook/*:write` to the dev-login
admin set. The first gates the admin verbs; the second is the cap a hook's inbound principal
resolves to (and therefore the cap the no-widening guard demands of the creator); the third is re-
checked by `lb_secrets::set_with` during `signature`-mode create/rotate. The public inbound route
takes NO session token — these caps gate the admin surface only.

### Cargo deps

`lb-host` gained `hmac`, `sha2`, `rand` (workspace-deps, already pinned) — the HMAC verify + the
shared-secret generator. `lb-role-gateway` gained `hmac`, `sha2` as **dev-deps** (the test's own
independent signer; production HMAC verify lives in `lb-host`).

## Decisions & alternatives

### 1. `bearer` mode reuses the apikey record verbatim (the scope's leaning #4 — ADOPTED)

The webhook secret IS an apikey: `webhook.create(auth_mode=bearer)` calls `apikey_create` with
`label = webhook:{id}`, no role, one narrowed cap `mcp:ingest.write:call`. The webhook row carries
`bearer_key_id` so revoke/rotate reach the linked apikey. **Why:** one credential path, free
rotate/revoke, free peppered-hash discipline, free cache. **Rejected:** an inline hash on the
webhook row — a parallel credential path that duplicates the apikey discipline for no gain (rule
1/9).

### 2. `signature` mode stores the shared secret in `lb-secrets` under Workspace visibility

`webhook/{id}`, set via `lb_secrets::set_with(..., Visibility::Workspace)`. The host's verify path
reads it via `secret::get_workspace` (the mediated host-read of a Workspace secret). **Why this
visibility:** the synthetic webhook principal built on verify holds NO `secret:*:get` cap; a
`Private` secret would deny the host and break every inbound verify. The wall still holds: gate 1
(workspace) by `ws`, and the value is for an authorized direct consumer (the host on the workspace's
behalf), never a caller-visible surface.

### 3. The signature scheme is fixed `hmac-sha256` (header `sha256=<hex>`), admin-picked header name

The header NAME is admin config (default `X-Signature`); the verifier is generic HMAC. **Rejected:**
provider-aware parsing (auto-detect GitHub/Slack) — rule 10. The admin supplies the header name +
the shared secret in the wizard; the scheme stays generic so a swap to an equivalent provider
needs zero core-crate change. `hmac-sha1` / timestamped variants are deferred (scope open
question, leaning "add only if a real caller needs them, still generic").

### 4. The inbound route is the ONLY unauthenticated-by-session route in the gateway

A third-party caller is not a workspace member; it presents the hook's own credential (a `lbk_…`
bearer or an HMAC signature), not a session token. The route calls `webhook_resolve` directly
(NOT `session::authenticate`). **Why:** the session path is for human/agent workspace members; the
webhook's auth model is its own (composed of apikey + HMAC), and the route must accept a request
with no `Authorization: Bearer <jwt>` header at all (signature mode has only the signature
header). Every failure collapses to the same opaque 404 so the public route is not a webhook-id
oracle.

### 5. The presented keyid must match `bearer_key_id` (linkage check)

A webhook is authenticated by ITS issued key, not any workspace apikey that happens to resolve to
`mcp:ingest.write:call`. A leaked sibling key cannot impersonate the hook; rotate replaces THIS
id; revoke kills THIS id. The apikey namespace wall already enforces ws-scoping; the linkage check
is the per-hook refinement on top.

### 6. `seq` is a wall-clock-ms timestamp (monotonic-ish per `(series, producer)`)

Two hits in the same millisecond would dedup (one upserts the other) — a known v1 limit. **Why
accepted:** the ingest dedup identity is `(series, producer, seq)`; the producer stamp
(`webhook:{id}`) is constant per hook, so a same-ms collision would lose one event. For high-volume
producers the user should use distinct producers or a future per-hook counter (deferred — its own
scope). The likelihood is low for the typical webhook rate (events/sec, not events/ms).

### 7. The flow `webhook` source node is DEFERRED — the trigger covers the use case today

The scope names a built-in source node with config `{ webhook_id }`. The existing `trigger` node
with `mode = event` + `series = webhook:{ws}:{id}` already lets any flow consume webhook hits
today (the series is the contract). Adding a dedicated `webhook` node with functional arming
requires teaching `flows/source.rs::arm_source` to derive the series from node config (so the
webhook node resolves `webhook:{ws}:{webhook_id}` instead of the default `flow:{ws}:{flow}:{node}`)
— a generic source-series-templating change that should be its own slice (a per-node-type branch
in `arm_source` would be a rule-10 leak). **Follow-up:** `scope/flows/webhook-source-node-scope.md`.

### 8. `202 Accepted` (not `200 OK`) on a successful hit

The route replies `202 Accepted { id, series, seq }` — decouples the sender from the buffer's
commit (the sample is staged + drained synchronously, but the contract is "accepted, here's the
coordinate," not "fully committed"). A sender can poll `series.read`/`latest` to confirm.
**Resolved** the scope's open question 2 (`202` vs `200`).

### 9. Always-narrow the route principal to `mcp:ingest.write:call`

The synthetic webhook principal carries exactly one cap, `mcp:ingest.write:call` — least-
privilege (scope open question 3, leaning always-narrowed — ADOPTED). The route constructs the
Sample with the hook's own series (the principal never chooses the series), so the cap is the only
authority the principal needs.

## Tests

Real store + real gateway + real caps + real ingest buffer, seeded through the real write path
(rule 9 — no mocks, no `*.fake.ts`). **34 tests, all green:**

- **Host units (`crates/host/src/webhook/*`, 18 tests):** `verify_signature` (correct / tampered
  body / wrong secret / missing / malformed / whitespace), `parse_bearer_key_id` (3-field split +
  non-lbk + 2-field rejected), `body_to_payload` (JSON / non-JSON / empty / invalid UTF-8 lossy),
  `WebhookRecord` derivation, `AuthMode` round-trip, `WebhookView` no-credential.
- **Gateway integration (`role/gateway/tests/webhook_routes_test.rs`, 16 tests):**

Mandatory categories:
- **Capability-deny:** `management_verbs_denied_without_webhook_manage`,
  `escalation_denied_when_creator_lacks_ingest_write` (the no-widening guard).
- **Workspace isolation:** `cross_workspace_url_is_opaque_404` (ws-B URL → ws-A webhook is 404),
  `ws_b_admin_cannot_see_ws_a_webhooks` (list returns ws-B's own only),
  `bearer_mode_wrong_ws_in_bearer_refused` (a forged ws-mismatched bearer is refused).
- **No-secret-leak:** `list_and_get_carry_no_secret_material` (dumps the JSON; asserts neither the
  bearer secret nor the shared secret nor any field named `secret`/`hash`/`bearer_key_id`/
  `secret_ref` appears).

Behavior:
- **Bearer end-to-end:** `bearer_mode_create_post_sample_committed` (create → POST with bearer →
  202 → `series.read` returns the sample — the round-trip), `bearer_mode_wrong_secret_is_opaque_404`.
- **Signature end-to-end:** `signature_mode_create_post_sample_committed` (sign raw body → POST →
  202 → `series.read`), `signature_mode_wrong_signature_is_opaque_404`,
  `signature_mode_missing_header_is_opaque_404`.
- **Raw-body invariant (the most-common-webhook-bug pin):**
  `signature_mode_body_tamper_breaks_signature` (sign the compact body, post the pretty-printed
  body with the same JSON value — must NOT verify; a re-serialized body would break every real
  signature, and any middleware that re-parses JSON before the verify step does exactly this).
- **Rotate:** `rotate_signature_old_dead_new_works` (rotate → old secret's signature 404s, new
  secret's works).
- **Revoke:** `revoke_then_route_410s_no_further_samples` (pre-revoke works → revoke → post-
  revoke 410s, no further samples).
- **Create-reply shape:** `create_signature_returns_hmac_header_for_the_wizard` (default
  `X-Signature` + admin-picked echo), `unknown_auth_mode_is_bad_input`.

### Green command output

```
$ cargo test -p lb-host --lib webhook
running 18 tests
... (all webhook::* units)
test result: ok. 18 passed; 0 failed; 0 ignored

$ cargo test -p lb-role-gateway --test webhook_routes_test
running 16 tests
test bearer_mode_wrong_secret_is_opaque_404 ... ok
test escalation_denied_when_creator_lacks_ingest_write ... ok
... (all 16)
test result: ok. 16 passed; 0 failed; 0 ignored

$ cargo test -p lb-role-gateway --test apikey_routes_test   # regression
test result: ok. 8 passed; 0 failed; 0 ignored

$ cargo build --workspace --features lb-role-gateway/test-harness
Finished `dev` profile [unoptimized + debuginfo] target(s)
```

`cargo fmt` clean. No pre-existing red surfaced.

## Debugging

None — nothing non-trivially broke. The two iterations during the build:

1. The route returned `200 OK` (the `Json` default) instead of `202 Accepted`. Fixed by returning
   `(StatusCode::ACCEPTED, Json(...))` explicitly.
2. `parse_bearer_key_id("lbk_acme.k7f3a")` succeeded on a 2-field bearer (no secret field) — the
   parser only checked for the keyid. Tightened to require the third field.

Both were caught by the test suite itself; no debug entry needed.

## Public / scope updates

- Promoted to **`docs/public/ingest/webhooks.md`** (the durable, trimmed truth — what shipped, the
  two auth modes, the URL shape, the cap grammar, the named follow-ups).
- Updated **`docs/public/ingest/ingest.md`** with a one-line cross-link to the new sibling.
- Updated **`docs/scope/ingest/webhooks-scope.md`** open questions (all resolved — see the file's
  refreshed "Open questions" section).

## Skill docs

n/a: no agent-/API-drivable surface. The webhook is a **third-party → platform** inlet; the
platform-side surface is the admin verbs (already covered by the public doc) + the inbound route
(not agent-driven — it's an external HTTP inlet). A future `webhook.normalize` extension tool
would warrant a skill entry; none ships here.

## Dead ends / surprises

- **The flow node is harder than it looks.** The obvious "add a `webhook` source descriptor"
  would either (a) be a palette-only affordance with no functional arming (the existing
  `trigger+mode=event` already covers the use case) or (b) require teaching `flows/source.rs` to
  derive a node's series from its config — a generic source-series-templating change that should
  be its own slice (a per-node-type branch in `arm_source` would be a rule-10 leak). Deferred as a
  named follow-up rather than shipping a half-working node.
- **The signature-header lookup needed a closure.** The route does not know `record.hmac_header`
  ahead of time (the admin picks it per hook), so passing a single header value didn't fit. The
  cleanest seam: the route passes a `Fn(&str) -> Option<String>` over its header map, and the host
  looks up the exact header the record names. Keeps the host transport-agnostic (no `HeaderMap`
  dep) and the route free of provider knowledge.
- **`Principal::for_key` for the synthetic signature-mode principal.** The dedicated constructor
  (not the co-trust `routed`) — the host builds it after a verified delivery, scoped to exactly
  `mcp:ingest.write:call`. Mirrors the apikey principal's trust model (verified server-side, not
  caller-asserted).

## Follow-ups

- **Flow `webhook` source node** — `scope/flows/webhook-source-node-scope.md` (generic source-
  series templating; today a `trigger` with `mode=event` + `series=webhook:{ws}:{id}` covers it).
- **Admin UI wizard** — `scope/frontend/admin-console-scope.md` extension (the Webhooks tab beside
  API Keys: name → auth-mode picker → generate-secret → copy-once → URL → test-hit). Backend is
  ready; UI is the next slice.
- **Per-hook `seq` counter** — `now_ms` is the v1 floor (a same-ms collision dedups); a SurrealQL
  MAX+1 counter per `(series, producer)` is the upgrade path if a real caller hits the limit.
- **Multi-node revoke cache-bust broadcast** — same honest bound as apikeys (instant at the
  authority + on any node that got the bus cache-bust; sync + cache-TTL is the floor). The bus
  broadcast is a v1 nicety, not built this slice (lazy expiry + local bust are the security
  floor, both tested).
- **HMAC scheme variants** — `hmac-sha1` / timestamped-signature if a real caller needs them
  (still generic, never provider-named).
