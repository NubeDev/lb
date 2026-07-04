# Webhooks scope — a first-class inbound-HTTP surface, keyed and mediated

Status: scope (the ask). Promotes to `public/ingest/` once the first slice proves it end to end.
Target stage: builds on the shipped **ingest buffer** + **API keys** + **flows** — no new stage.

> Read with: `ingest-scope.md` (the `Sample`/`series` contract and the buffer this lands into —
> a webhook is a *producer* of ingest samples, never a new datastore), `../auth-caps/api-keys-scope.md`
> (the credential model this reuses verbatim — a webhook secret is an **API-key-shaped bearer
> credential**, not a new auth system), `../auth-caps/auth-caps-scope.md` (the capability grammar +
> the reserved `key:` subject prefix), `../flows/triggers-lifecycle-scope.md` and
> `../flows/extension-nodes-scope.md` (the flow **source-node** arm/disarm pattern this node wraps),
> `../flows/flow-message-envelope-scope.md` (the envelope a webhook fires into a flow),
> `../../README.md` §6.6 (identity/auth/caps), §7 (tenancy), §6.11 (tags), `../testing/testing-scope.md`
> §2 (the mandatory deny + isolation tests this must satisfy).

We want the platform to **receive inbound HTTP** from the outside world — a third-party service
calling us — as a **first-class, standalone core capability**, not a one-off flow node. A webhook is
a **named, workspace-walled, credential-protected inbound endpoint**: an admin creates one through a
wizard (naming it, generating a signing/auth secret), the platform exposes a stable URL, and every
authenticated hit becomes a **generic ingest `Sample`** on that webhook's series. Anything that wants
to *react* to it — a flow, a rule, a dashboard — subscribes to that series through the seams we
already own. The flow **webhook node is a thin wrapper** that selects an existing webhook and binds
its series as a flow **source trigger**; the endpoint, the credential, and the delivery guarantee all
live in the **core webhook service**, so a webhook is useful with or without a flow.

**The hard constraint (mirrors ingest): this must NOT become a per-integration surface.** A webhook is
a **generic authenticated HTTP inlet that emits a `Sample`**. There is **no Slack webhook, no GitHub
webhook, no Stripe webhook** in core — those are *shapes of payload a caller sends*, normalized (if at
all) by an **out-of-core extension** (the `github-bridge`/protocol-bridge pattern from `ingest-scope.md`),
never a branch in the host. If a provider name appears in a core crate, the scope has failed (rule 10).

## Goals

- A **durable `webhook` record** — `{ id, ws, name, series, secret_ref, auth_mode, status, created_ts,
  last_hit_at? }` — created/listed/revoked/rotated from an admin wizard, workspace-walled.
- A **stable inbound URL** per webhook: `POST /hooks/{ws}/{webhook_id}` on the gateway, resolvable
  O(1) with no scan (the `{ws}` segment selects the namespace, exactly like the API-key `lbk_{ws}.…`
  grammar).
- **Credential-protected ingress, reusing the API-key model verbatim.** A webhook's secret is an
  **API-key-shaped bearer credential** (`§api-keys`): stored only as `HMAC-SHA256(pepper, secret)`,
  shown once at creation, instantly revocable, rotatable. **Two auth modes**, admin-selected per hook:
  - `bearer` — caller sends `Authorization: Bearer <secret>` (our issued key). Simplest; for callers
    we control.
  - `signature` — caller signs the raw body with a **shared secret** using an **HMAC scheme the admin
    picks from a fixed core set** (`hmac-sha256` header-based, the near-universal shape); we verify the
    signature over the raw bytes. For third-parties that sign (the generic form of what GitHub/Stripe
    do) — **without naming any of them**: the admin supplies the header name + the shared secret in the
    wizard; the *scheme* is generic HMAC, the *provider* is opaque config.
- **Every accepted hit becomes exactly one ingest `Sample`** on the webhook's series
  `webhook:{ws}:{id}` — `{ series, producer: "webhook:{id}", ts, seq, payload: <the request body>,
  labels: { source:webhook, method, … } }` — landing in the **existing ingest buffer** (`ingest.write`
  path), so backpressure/dedup/durability are inherited, not rebuilt. The webhook service is a
  **producer**, not a second store.
- **A flow `webhook` source node that wraps the service** — the node's config is just *"which webhook"*
  (a picker over `webhook.list`); arming the node subscribes the flow to that webhook's series via the
  **already-shipped flow source arm/disarm** (`flows/source.rs`), and each hit fires a run with the
  `Sample` payload as the flow message envelope. The node adds **no endpoint and no credential** — it
  reuses the core webhook's. Disarming/deleting the flow leaves the webhook (and its URL/secret)
  intact.
- **An admin wizard** (Admin → Webhooks → New): name it → pick auth mode → **generate secret / API key**
  (shown once, copy button, "you won't see this again") → get the URL → optionally test with a sample
  hit. Plus list / revoke / rotate, mirroring the API-keys tab.
- **Instant revoke + lazy checks** for the credential (inherited from `§api-keys`): a revoked webhook
  refuses the very next inbound request; a disabled webhook returns `404`/`410` with no existence leak.

## Non-goals (the guardrails that keep this generic)

- **No provider-specific webhooks in core.** No Slack/GitHub/Stripe/Discord/Twilio node, route, or
  parse step in any core crate or the core UI shell (rule 10). Provider payload shapes are normalized by
  **out-of-core bridge extensions** (the `github-bridge` pattern) or by a downstream **flow parse/rhai
  node** the user wires — never by the webhook service.
- **No new auth system.** The webhook secret **is** the API-key credential (`§api-keys`) — same hash,
  same pepper, same instant-revoke, same one-time-reveal. We do not invent a second bearer format.
  (`bearer` mode literally issues an `apikey` record scoped to the hook; `signature` mode stores a
  shared secret in `lb-secrets` and verifies HMAC over the body.)
- **No new datastore / queue.** A hit is an **ingest `Sample`**; the ingest buffer is the durability +
  backpressure layer (`ingest-scope.md`). No parallel webhook-event table, no separate delivery log —
  the series *is* the log.
- **No outbound webhooks here.** "Call an external URL *from* a flow" is the **outbox `Target`** path
  (the generic durable-delivery sink from the flows/workflow convergence). This scope is **inbound
  only**. Name them apart so they never merge.
- **No new transport.** The existing gateway HTTP + the Zenoh series stream + SSE. No new broker/port.
- **No per-webhook rate limiting in v1** (inherits the ingest buffer's overflow policy; a dedicated
  limiter is its own scope, as in `§api-keys`).
- **No replay/retry of *inbound* requests.** We are the receiver; delivery guarantees are the *sender's*
  concern. Once accepted, durability is the ingest buffer's job; we do not re-request. (An accepted hit
  that fails to commit dead-letters via the ingest buffer — not a re-fetch.)
- **No SDK/WIT change.** The webhook service is host-native + a gateway route; the flow node is a
  built-in descriptor. Bridge extensions (if any) use the existing `normalize`-style tool WIT.

## Intent / approach

**A webhook is `API-key ⊕ ingest-producer ⊕ flow-source`, glued by the gateway route.** Every piece
already exists; this scope is the *composition*, not new machinery:

1. **The credential** is the API-key model (`§api-keys`). `bearer`-mode creation mints an `apikey`
   record whose subject is `key:webhook:{id}` and returns the one-time `lbk_{ws}.…` secret;
   `signature`-mode stores a shared secret in `lb-secrets` (`secret_ref`) and records the HMAC scheme +
   header name. Verification, revoke, rotate, and lazy-expiry are the API-key seams unchanged.
2. **The endpoint** is one gateway route `POST /hooks/{ws}/{webhook_id}`. It: resolves the namespace
   from `{ws}`, loads `webhook:{ws}:{id}` (404 if absent/disabled — opaque, no existence leak),
   **authenticates per the hook's `auth_mode`** (bearer → the API-key verify path; signature →
   constant-time HMAC over the **raw body** against the `lb-secrets` shared secret), builds a verified
   `Principal::for_key` scoped to that hook's caps, and — gated by `mcp:ingest.write:call` — calls the
   **existing `ingest.write`** with one `Sample`. The route holds **no business logic**; it is an
   auth-and-normalize edge.
3. **The reaction** is whatever subscribes to `webhook:{ws}:{id}`. The **flow webhook source node**
   subscribes via the shipped `flows/source.rs` arm/disarm (identical to how MQTT-in and the generic
   event trigger consume a series), firing a run per hit. A rule, a dashboard, or a plain `series.read`
   can consume it too — the webhook doesn't know or care.

**Why a first-class service and not "just a flow node" (the user's explicit ask):** the endpoint,
the credential, and the URL must **outlive any one flow** and be reachable by **anything** (a rule,
a dashboard, a second flow, a raw `series.read`). Binding the URL + secret to a single flow node would
(a) destroy the endpoint when the flow is edited/deleted, (b) prevent fan-out to multiple consumers,
and (c) bury credential management inside the canvas instead of the admin console where revoke/rotate
belong. So the **service owns the endpoint + credential**; the **node is a subscriber wrapper**. This
is the same split as ingest (the buffer is core; a producer is a caller) and API keys (the credential
is core; a consumer is whoever holds it).

**Rejected alternatives:**
- *Endpoint + secret live on the flow node.* Rejected — the user's requirement is explicitly that the
  webhook is "part of the core system as a stand-alone thing"; a node-owned endpoint dies with the flow
  and can't fan out (above).
- *A new webhook-event table + delivery log.* Rejected — that's a second store for what the ingest
  `series` already is (rule 2). The series is the durable, tag-queryable, replayable log.
- *A new bearer-credential format for webhooks.* Rejected — duplicates `lb-auth`/`lb-authz`/`lb-secrets`
  and the API-key work (rules 1/9). A webhook secret **is** an API key with a hook-scoped subject.
- *Provider-aware parsing in the route (auto-detect GitHub/Slack).* Rejected — rule 10. The route emits
  the raw body as a `Sample`; any provider shaping is a downstream extension or flow node the user opts
  into.

## How it fits the core

- **Tenancy / isolation:** the record is `webhook:{ws}:{id}`, written via `lb_store::write(store, ws, …)`;
  the URL carries `{ws}`; the emitted series is `webhook:{ws}:{id}`. A ws-B caller can never hit, list,
  revoke, or read a ws-A webhook — `caps::check` gate 1 rejects any mismatch, and the route 404s a
  cross-ws URL with no existence signal. **(Mandatory isolation test.)**
- **Capabilities:** management verbs (`webhook.create/list/get/revoke/rotate`) are gated
  `mcp:webhook.manage:call` (a workspace-admin cap); the inbound route's principal is gated
  `mcp:ingest.write:call` (narrowable to the hook's series). Both deny paths are the standard **opaque
  `Denied`/404** — an un-granted or wrong-secret caller learns nothing about which webhooks exist.
  **No-widening (inherited from `§api-keys`):** `webhook.create` must confirm the hook's effective caps
  (`ingest.write` on its series) ⊆ the creating admin's caps. **(Mandatory deny test — including the
  wrong-secret and cross-ws-URL refusals.)**
- **Placement:** **either** — a webhook is created and served on any node (an edge node can expose a LAN
  webhook for local callers; the cloud exposes public ones). No `if cloud {…}`. The record and its
  credential are shared, cloud-authoritative workspace data and **sync like API keys / grants**
  (`§api-keys` Sync/authority) — an edge holds a read-cache; revoke is a `__revoked__` tombstone,
  instant on the observing node and bounded by sync + cache-TTL elsewhere (same honest guarantee as
  API-key revoke — do not imply globally-instant).
- **MCP surface (§6.1):**
  - **CRUD:** `webhook.create` (returns the URL always + the secret **once** in `bearer` mode),
    `webhook.revoke` (tombstone → next hit refused), `webhook.rotate` (new secret, same URL/series, old
    secret dead). No secret `update` — rotate replaces it.
  - **Get / list:** `webhook.list` / `webhook.get` — return id, name, series, auth_mode, URL, status,
    created/last-hit; **never** the hash/shared-secret.
  - **Live feed:** the *hits* are the webhook's **series** (motion) — consumed via the existing series
    stream / `flows.watch`, not a new SSE. Lifecycle is administrative (`list` refresh), no watch.
- **Data (SurrealDB):** one table `webhook`, record `webhook:{ws}:{id}`
  `{ id, name, series, auth_mode, secret_ref, hmac_header?, status, created_ts, last_hit_at? }`. In
  `bearer` mode the credential is the linked `apikey:{ws}:{keyid}` record (reused, not duplicated); in
  `signature` mode `secret_ref` points at an `lb-secrets` entry holding the shared secret — **never the
  raw secret on the webhook record**. Tombstone `__revoked__`. The hits themselves are **not** a webhook
  table — they are ingest `series` rows (state), fed by the Zenoh sample stream (motion). Tag the series
  `source:webhook name:{name}` via the ingest tag-graph so it's discoverable alongside every other
  series.
- **Bus (Zenoh):** the accepted hit publishes as an ingest `Sample` on `ws/{id}/series/webhook:{id}`
  (motion), drained by the ingest buffer to committed series state — **the ingest path unchanged**. The
  webhook service adds no new subject grammar.
- **Secrets:** `bearer` secret handling is the API-key discipline (hash-only, pepper from `lb-secrets`,
  one-time reveal, never logged). `signature` shared-secret lives in `lb-secrets` behind `secret_ref`,
  read only at verify time, never returned by `get`/`list`. **(Focused secret-handling review + a test
  asserting `list`/`get` carry no secret material — same as `§api-keys`.)**
- **Flow integration (the node):** a built-in `webhook` source descriptor
  (`crates/flows/src/builtins/…`, `kind: Source`) whose config is `{ webhook_id }` (a picker over
  `webhook.list`). Arming = `flows/source.rs` subscribe to `webhook:{ws}:{id}`; each hit fires a run with
  the `Sample` as the message envelope (`flow-message-envelope-scope.md`); disarm = unsubscribe, leaving
  the webhook intact. **No named provider** — the node is generic "webhook trigger"; provider shaping is
  a downstream rhai/parse node the user adds. This is the **only** flow-facing surface; there is
  deliberately **no `slack` and no `github` node** (rule 10 + the user's explicit constraint).
- **SDK/WIT impact:** none — host-native service + gateway route + built-in flow descriptor.

## Example flow

1. A workspace-admin opens **Admin → Webhooks → New**, names it `plant-alerts`, picks `auth_mode:
   signature`, sets the HMAC header to `X-Signature` and pastes/generates the shared secret. (Or picks
   `bearer` and clicks **Generate key** → the one-time `lbk_acme.…` secret appears with a copy button.)
2. The UI calls `webhook.create`. The host checks effective caps ⊆ admin caps, writes
   `webhook:acme:wh_9f2…` with `series: webhook:acme:wh_9f2…`, stores the shared secret in `lb-secrets`
   (`secret_ref`), and returns the stable URL `POST https://…/hooks/acme/wh_9f2…`. The wizard shows the
   URL (always) and the secret (once, in `bearer` mode).
3. The external service `POST`s a JSON body to that URL with `X-Signature: sha256=…`.
4. The gateway route resolves ns `acme`, loads `webhook:acme:wh_9f2…` (404 if disabled), reads the
   shared secret via `secret_ref`, **constant-time-verifies `HMAC-SHA256(secret, raw_body)`** against
   the header (or, in `bearer` mode, runs the API-key verify path), builds `Principal::for_key` scoped
   to the hook, and — gated `mcp:ingest.write:call` — calls `ingest.write` with one `Sample`
   `{ series: webhook:acme:wh_9f2…, producer: "webhook:wh_9f2…", ts, seq, payload: <body>,
   labels:{source:webhook, method:POST} }`. The buffer commits it to series state and streams it as
   motion.
5. A flow with a **`webhook` source node** configured to `wh_9f2…` is armed → it's subscribed to that
   series → the hit **fires a flow run** with the body as the envelope, chaining into rhai/rule nodes
   downstream. Meanwhile a dashboard tile reading the same series updates live — **two consumers, one
   webhook**.
6. The admin rotates the secret (`webhook.rotate`): same URL and series, new secret, old signature
   refused on the next hit. Later they revoke it (`webhook.revoke`): the URL now 410s, the flow node's
   subscription goes quiet, no data lost.

## Testing plan

Real store + real gateway + real `caps::check` + real ingest buffer, seeded with real records — **no
mocks, no `*.fake.ts`** (CLAUDE §9). Mandatory categories from `testing-scope.md` §2:

- **Capability-deny (mandatory):**
  - `webhook.create/revoke/rotate/list` refused without `mcp:webhook.manage:call`.
  - **No-widening:** an admin lacking `mcp:ingest.write:call` on the hook's series is refused when
    creating a webhook that would resolve to it (the `§api-keys` effective-caps ⊆ creator check, applied
    to `webhook.create`).
  - The inbound route with a **wrong/absent secret** (both modes) is refused with an opaque error and
    writes **no** sample; a valid secret writes exactly one.
- **Workspace-isolation (mandatory):** a `POST /hooks/{wsB}/{id}` for a ws-A webhook 404s; `webhook.list/
  get/revoke` in ws B never see ws A's webhooks; a ws-A `bearer` secret cannot authenticate a ws-B hook.
- **Offline/sync (mandatory):** `webhook.revoke` tombstone applies idempotently across nodes (re-apply
  no-op), mirroring API-key/grant revoke; a revoked hook refuses on the observing node immediately and
  on a peer after sync + cache-TTL.
- **Unit:**
  - HMAC `signature` verify over the **raw body** (not a re-serialized form) with a fixed shared secret;
    constant-time compare; a body-tamper flips the result. Assert verification uses the bytes as
    received, since re-serializing JSON would break real signatures.
  - `bearer` mode reuses the API-key verify path unchanged (hash input = secret field only).
  - URL parse: valid `{ws}/{id}` split; reject malformed / cross-ws / unknown id → 404 opaque.
  - `Sample` construction from a hit: series/producer/labels correct; body preserved as typed payload;
    `seq` monotonic per `(series, producer)` (the ingest dedup identity — a duplicate delivery upserts
    once).
  - `list`/`get` output asserted to contain **no** hash / shared-secret field.
- **Integration (real gateway + ingest):** full example — create (both modes) → POST → sample committed
  to series → `series.read` returns it → rotate (old refused, new works) → revoke (route 410s, no
  further samples). A **flow** with a `webhook` source node fires a run on the hit end-to-end.
- **UI (`pnpm test` + `pnpm test:gateway`):** the wizard shows the URL always + the secret once (bearer);
  list never renders a secret; revoke updates status; the tab is cap-gated (hidden without
  `webhook.manage`); the flow node's picker lists only this workspace's webhooks.

## Risks & hard problems

- **Raw-body signature verification is fragile.** HMAC must run over the **exact received bytes** —
  any middleware that re-parses/re-serializes the body before the verify step breaks every real
  signature. The route must capture the raw body **before** JSON parsing and verify on those bytes. This
  is the single most common webhook-integration bug; pin it with a body-tamper + re-serialize test.
- **Secret discipline (inherited but re-stated).** The `bearer` secret leaves the host once; the
  `signature` shared secret never leaves `lb-secrets`. Neither is ever logged or returned by
  `list`/`get`. One leaked log line defeats the endpoint.
- **Endpoint enumeration / existence leak.** A disabled/wrong/cross-ws URL must be indistinguishable
  from a never-existed one (opaque 404/410) so the public route doesn't become a webhook-id oracle.
- **Multi-node revoke is not globally instant** — same honest bound as `§api-keys` (authority + bus
  cache-bust immediate; a peer that missed the bust is bounded by sync + cache-TTL). Don't let the
  wizard imply otherwise.
- **Resisting provider creep.** The pressure to "just special-case the GitHub signature header" or
  "auto-parse Slack" will be constant. Hold the line: the scheme is generic HMAC + admin-supplied
  header; provider shaping is downstream (extension or flow node). The moment a provider name enters a
  core crate, rule 10 is broken.
- **Inbound bursts.** A webhook can be hit hard; acceptance rides the ingest buffer's overflow/
  backpressure policy (`ingest-scope.md`), not a new limiter — verify a burst degrades per that policy
  rather than OOMing the route.

## Open questions

- **`signature` HMAC scheme set** — v1 ships `hmac-sha256` (header-based) only, admin picks the header
  name. Add `hmac-sha1`/timestamped-signature variants later only if a real caller needs them (still
  generic, never provider-named)?
- **Sync vs async acceptance response** — return `202 Accepted` the moment the sample is buffered
  (recommended — decouples the sender from commit, matches ingest's accept-vs-commit split), vs `200`
  only after commit? Leaning `202`.
- **Per-hook `ingest.write` cap narrowing** — always narrow the route principal to
  `mcp:ingest.write:call?series=webhook:{ws}:{id}` (recommended, least-privilege) vs the broad
  `ingest.write`? Leaning always-narrowed.
- **`bearer` secret as its own `apikey` record vs an inline hash on the webhook record** — reusing a
  real `apikey:{ws}:{keyid}` record (recommended — one credential path, free rotate/revoke) vs a
  hash field on the webhook row (fewer records, but a parallel credential path). Leaning reuse.

## Related

- README `§6.6` (identity/auth/caps), `§7` (tenancy), `§6.7` (secrets), `§6.11` (tags), `§6.1` (MCP
  surface shape).
- Sibling scope: `ingest-scope.md` (the `Sample`/`series`/buffer this produces into — a webhook is a
  producer), `../auth-caps/api-keys-scope.md` (the credential model reused verbatim — the webhook
  secret **is** an API key; the wizard's "generate key" is `apikey.create`),
  `../auth-caps/auth-caps-scope.md` (the `key:` subject prefix + cap grammar),
  `../flows/triggers-lifecycle-scope.md` + `../flows/extension-nodes-scope.md` + `../flows/flow-message-envelope-scope.md`
  (the source-node arm/disarm + envelope the flow node wraps), `../secrets/secrets-scope.md` (the
  `signature` shared-secret home), `../frontend/admin-console-scope.md` (where the Webhooks wizard/tab
  lands — beside API Keys).
- Implementation seams (for the building session): the API-key verify path + `Principal::for_key`
  (`§api-keys` — reused for `bearer`), `lb-secrets` (the `signature` shared secret + pepper), the
  ingest `ingest.write`/`Sample` path (`ingest-scope.md`), the flow source arm/disarm
  (`crates/host/src/flows/source.rs`) + a new built-in `webhook` source descriptor
  (`crates/flows/src/builtins/`), the gateway route module (`role/gateway/src/routes/` — the new
  `/hooks/{ws}/{id}` handler capturing the **raw body**), and the admin UI shell
  (`ui/src/features/admin/`).
