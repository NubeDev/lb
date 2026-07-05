# Webhooks — a first-class inbound-HTTP surface, keyed and mediated

Status: shipped (Rust core + gateway routes + integration tests; UI wizard + flow node are named
follow-ups). Part of the S8 data plane — builds on the shipped **ingest buffer** + **API keys** +
**flows**, no new stage.

> Sibling: [`ingest.md`](ingest.md) (the `Sample`/`series` contract this produces into — a webhook
> is a *producer* of ingest samples, never a new datastore). Scope:
> [`../../scope/ingest/webhooks-scope.md`](../../scope/ingest/webhooks-scope.md). Session:
> [`../../sessions/ingest/webhooks-session.md`](../../sessions/ingest/webhooks-session.md).

A webhook is a **named, workspace-walled, credential-protected inbound HTTP endpoint**: an admin
creates one (naming it, picking an auth mode, generating a secret), the platform exposes a stable
URL `POST /hooks/{ws}/{id}`, and every authenticated hit becomes a **generic ingest `Sample`** on
that webhook's series `webhook:{ws}:{id}`. Anything that wants to *react* to it — a flow (a
`trigger` with `mode=event` watching that series today; a dedicated `webhook` source node is a
named follow-up), a rule, a dashboard tile, or a raw `series.read` — subscribes through the seams
we already own. The webhook service is a **producer**, not a second store.

**The hard constraint (rule 10):** a webhook is a **generic authenticated HTTP inlet that emits a
`Sample`**. There is **no Slack webhook, no GitHub webhook, no Stripe webhook** in the core — those
are *shapes of payload a caller sends*, normalized (if at all) by an **out-of-core bridge
extension** (`lb-role-github-webhook`), never a branch in the host. Provider shaping is a
downstream flow `rhai`/`parse` node the user wires.

## The two auth modes (admin-selected per hook)

- **`bearer`** — the caller sends `Authorization: Bearer lbk_{ws}.{keyid}.{secret}`. The credential
  IS a real `apikey:{ws}:{keyid}` record (reused, not duplicated): `webhook.create` mints it with
  label `webhook:{id}`, no role, one narrowed cap `mcp:ingest.write:call`. The webhook row carries
  `bearer_key_id` so revoke/rotate reach the linked apikey. The presented keyid MUST match
  `bearer_key_id` (linkage check) — a leaked sibling key cannot impersonate the hook. For callers
  we control.

- **`signature`** — the caller signs the raw body with a **shared secret** using HMAC-SHA256 and
  sends the result in an admin-picked header (`X-Signature` by default), value `sha256=<64 hex>`.
  We verify the signature over the **raw bytes** (constant-time compare). The shared secret lives
  in `lb-secrets` at `webhook/{id}` under Workspace visibility (so the host can mediate-read it on
  verify). For third-parties that sign (the generic form of what GitHub/Stripe do) — **without
  naming any of them**: the admin supplies the header name + the shared secret; the scheme is
  generic HMAC, the provider is opaque config.

## How a hit flows

1. Admin creates a webhook via `POST /admin/webhooks` → returns `{ id, url_path, secret,
   auth_mode, hmac_header }` (the secret is shown ONCE; never recoverable).
2. External service `POST`s to `/hooks/{ws}/{id}` with the credential.
3. The gateway route captures the **raw body** before any JSON parse (load-bearing — HMAC verify
   runs over the exact received bytes), then:
   - `webhook_resolve` loads the record, per-mode verifies, builds a `Principal` scoped to
     `mcp:ingest.write:call`. Every failure (unknown id / disabled / wrong-secret / cross-ws URL)
     collapses to the same opaque `404` — the public route is **not a webhook-id oracle**.
   - `webhook_accept` builds one `Sample { series: webhook:{ws}:{id}, producer: webhook:{id},
     payload: <body>, labels: {source:webhook, method:POST} }`, writes it through the existing
     `ingest.write` path, drains workspace staging, publishes motion on the series subject, bumps
     `last_hit_at`.
4. Route replies `202 Accepted { id, series, seq }`. The buffer's acceptance IS the durability
   promise; a sender can poll `series.read`/`latest` to confirm commit.
5. A revoked hook returns `410 Gone` on the next hit; rotate returns a fresh one-time secret
   (old dead instantly); revoke tombstones + revokes the linked apikey + cache-busts.

## How it fits the core

- **Tenancy / isolation:** the record is `webhook:{ws}:{id}` in the workspace's own namespace; the
  URL carries `{ws}`; the emitted series is `webhook:{ws}:{id}`. A ws-B caller cannot hit, list,
  revoke, or read a ws-A webhook — the namespace wall + the cap gate refuse, and a cross-ws URL
  404s with no existence signal.
- **Capabilities:** management verbs (`webhook.create/list/get/revoke/rotate`) are gated
  `mcp:webhook.manage:call`; the inbound route's principal is gated `mcp:ingest.write:call`
  (always narrowed — least-privilege). **No-widening:** `webhook.create` confirms the hook's
  effective cap (`mcp:ingest.write:call`) ⊆ the creating admin's caps (the same guard
  `apikey.create` runs, applied symmetrically across both modes).
- **Placement:** either — a webhook is created and served on any node (an edge node can expose a
  LAN webhook for local callers; the cloud exposes public ones). The record + its credential are
  shared, cloud-authoritative workspace data and sync like API keys / grants. Revoke is a
  `__revoked__` tombstone — instant on the observing node, bounded by sync + cache-TTL elsewhere
  (the same honest guarantee as apikey revoke — do not imply globally-instant).
- **Data:** one table `webhook`, record `{ id, ws, name, series, auth_mode, bearer_key_id?,
  secret_ref?, hmac_header?, status, created_ts, last_hit_at? }`. **Never** the raw secret or the
  apikey hash on the webhook row. The hits themselves are NOT a webhook table — they are ingest
  `series` rows (state), fed by the Zenoh sample stream (motion).
- **Bus:** the accepted hit publishes as an ingest `Sample` on `ws/{id}/series/webhook:{id}`
  (motion), drained by the ingest buffer to committed series state — the ingest path unchanged.
- **Secrets:** `bearer` secret handling is the apikey discipline (hash-only, pepper from
  `lb-secrets`, one-time reveal, never logged). `signature` shared-secret lives in `lb-secrets`
  behind `secret_ref`, read only at verify time, never returned by `get`/`list`.

## The MCP surface (§6.1)

- **CRUD:** `webhook.create` (returns the URL always + the secret **once** in both modes),
  `webhook.revoke` (tombstone → next hit 410s), `webhook.rotate` (new secret, same URL/series, old
  dead).
- **Get / list:** `webhook.list` / `webhook.get` — return id, name, series, auth_mode, URL,
  status, created/last-hit; **never** the hash / shared-secret / `bearer_key_id` / `secret_ref`.
- **Live feed:** the *hits* are the webhook's **series** (motion) — consumed via the existing
  series stream / `series.read` / a flow `trigger` watching the series, not a new SSE.

## The inbound route (the only unauthenticated-by-session route)

`POST /hooks/{ws}/{id}` is the **only** route in the gateway that does NOT take a workspace session
token. A third-party caller is not a workspace member; it presents the hook's own credential (a
bearer apikey or an HMAC signature), not a JWT. The route calls `webhook_resolve` directly (NOT
`session::authenticate`), then `webhook_accept`. The route holds **no business logic** — it is an
auth-and-normalize edge.

## What's deferred (named follow-ups)

- **Admin UI wizard** — the Webhooks tab in the admin console beside API Keys (name → auth-mode
  picker → generate-secret → copy-once → URL → test-hit). Backend is ready; UI is the next slice.
- **Flow `webhook` source node** — a dedicated palette entry whose config is `{ webhook_id }` and
  whose arming derives the series `webhook:{ws}:{id}` from config. Needs a generic source-series-
  templating change in `flows/source.rs` (a per-node-type branch would be a rule-10 leak). Today a
  `trigger` with `mode=event` + `series=webhook:{ws}:{id}` covers the use case.
- **Per-hook `seq` counter** — `now_ms` is the v1 floor; a same-ms collision dedups (data loss of
  one event). A SurrealQL MAX+1 per `(series, producer)` is the upgrade if a real caller hits it.
- **HMAC variants** — `hmac-sha1` / timestamped signatures if a real caller needs them (still
  generic, never provider-named).
- **Multi-node revoke cache-bust broadcast** — same honest bound as apikeys (the bus broadcast is a
  v1 nicety; lazy expiry + local bust are the security floor, both tested).

## Rejected alternatives

- *Endpoint + secret live on the flow node* — the user's requirement (the scope's Intent) is that
  the webhook is "part of the core system as a stand-alone thing"; a node-owned endpoint dies with
  the flow and can't fan out (a rule, a dashboard, a second flow all want the same series).
- *A new webhook-event table + delivery log* — that's a second store for what the ingest `series`
  already is (rule 2). The series is the durable, tag-queryable, replayable log.
- *A new bearer-credential format for webhooks* — duplicates `lb-auth`/`lb-authz`/`lb-secrets` and
  the apikey work (rules 1/9). A webhook secret **is** an apikey with a hook-scoped subject.
- *Provider-aware parsing in the route* — rule 10. The route emits the raw body as a `Sample`; any
  provider shaping is a downstream extension or flow node the user opts into.

## Open questions (all resolved this slice)

- **`signature` HMAC scheme set** — v1 ships `hmac-sha256` only (admin picks the header name).
  `hmac-sha1`/timestamped variants later only if a real caller needs them (still generic).
- **Sync vs async acceptance response** — `202 Accepted` (decouples the sender from commit,
  matches ingest's accept-vs-commit split).
- **Per-hook `ingest.write` cap narrowing** — always narrowed to `mcp:ingest.write:call`
  (least-privilege; the route constructs the Sample, so the principal never chooses the series).
- **`bearer` secret as its own `apikey` record vs an inline hash** — reuses a real
  `apikey:{ws}:{keyid}` record (one credential path, free rotate/revoke). ADOPTED.
