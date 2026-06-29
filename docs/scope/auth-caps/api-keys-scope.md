# API keys scope — machine principals over the existing authz model

Status: scope (the ask). Promotes to `public/auth-caps/` once the first slice proves it end to end.

> Read with: `auth-caps-scope.md` (the token + capability grammar — the `key:` **subject
> prefix** is reserved there; note `auth:key:{id}` in that doc is the token-*signing* public-key
> record, a different concept — this scope's credential record is `apikey:{ws}:{id}`),
> `authz-grants-scope.md` (the durable
> grants/roles/teams model + `resolve_caps`), `admin-crud-scope.md` (the disable/delete +
> revoke seam this mirrors), `secrets/secrets-scope.md` (mediated secret material),
> `inbox-outbox/outbox-scope.md` (the `next_attempt_ts` housekeeping gate),
> `../../README.md` §6.6 (identity/auth/caps), §7 (tenancy),
> `../testing/testing-scope.md` §2 (the mandatory deny + isolation tests this must satisfy).

We want **long-lived, non-human credentials** — for appliances, a future CLI, raw API
callers, and AI agents — that authenticate against the platform and carry a **scoped,
revocable** set of permissions: read-only vs read-write, which tools they may call, which
dashboard pages/routes they may reach. The credential must be **instantly revocable**, may
**expire**, and is managed from an admin UI. The whole point: an API key is **not a new
permission system** — it is a non-human `Subject` in the authz model we already shipped,
authenticated by a hashed secret and authorized through the one `caps::check` chokepoint.

## Goals

- Issue a workspace-walled API key with a secret shown **exactly once**, never recoverable.
- A key's permissions come from the **same** grant/role/team machinery as a user
  (`resolve_caps`) — so "read-only", "tool allowlist", and "page allowlist" are all just
  *which caps the key resolves to*, enforced at the existing chokepoint with no new mechanism.
- **Instant revoke** (tombstone → refused on the very next request) and **lazy expiry**
  (checked at authentication, can never fail-open if a scheduler misses).
- A `kind` label (`appliance | cli | api | agent`) for filtering, audit, and per-kind
  defaults — **labelling, not a security boundary**.
- An admin-console surface to create / list / revoke / rotate keys.
- AI-agent keys compose with the S5 `agent ∩ caller` delegation so a key can never exceed
  the human who provisioned it.

## Non-goals

- **No external API-key/auth crate.** There is no dominant Rust library that fits a
  workspace-walled, capability-first, SurrealDB-backed, single-Ed25519 model; adopting one
  (or `jsonwebtoken`, or a generic auth framework) duplicates `lb-auth`/`lb-authz`/`lb-caps`/
  `lb-secrets` and violates rules 1/2/9. **We build native on the seams we already own.**
- **No new permission grammar, surface, or action.** Read-only/read-write is the existing
  `Read`/`Get` vs `Write`/`Call` actions; tool/page limits are existing caps.
- **No OAuth/OIDC client-credentials flow, no scoped OAuth tokens** — deferred; a key is a
  bearer credential, not an OAuth grant.
- **No per-key rate limiting** in v1 (tracked separately; a rate limiter is its own scope).
- **No cross-workspace keys.** A key authorizes exactly one workspace (the hard wall).
- **No IP allowlists / mTLS-bound keys** in v1 (node identity lives in `edge-trust-scope.md`).

## Intent / approach

A key is a durable record plus a hashed secret. **Three decisions, all settled toward
reuse:**

**1. The key is the bearer credential, verified per request (not exchanged for a token).**
The presented secret has an **unambiguous, delimiter-safe grammar**: `lbk_{ws}.{keyid}.{secret}`
— three dot-separated fields after the `lbk_` prefix, where `keyid` and `secret` are
base32 (Crockford, no padding) and `ws` is the workspace slug, so **no field can contain a
`.`** and parsing is a fixed split (never the old `_`-delimited form, which collided with `_`
inside ids). The `{ws}.{keyid}` lets the gateway do an O(1) ws-scoped lookup with no scan.
Gateway auth middleware sees the `lbk_` prefix, splits the three fields, fetches
`apikey:{ws}:{keyid}`, **constant-time-compares `HMAC-SHA256(pepper, secret_field)` to the
stored `key_hash`** (the hash input is the **`secret` field alone**, never the full bearer
string), checks active + not-expired, resolves caps, and builds a **verified `Principal` via a
dedicated `Principal::for_key(sub, ws, caps)` constructor** — no minting. A small
hash→`Principal` cache (TTL of seconds) keeps the hot path cheap; **revoke invalidates the
cache** so a tombstone bites on the next request *on that node*.

> *Why a dedicated `Principal::for_key`, not `Principal::routed`:* `routed` is the S5 **co-trust**
> path (`principal.rs`), documented as carrying *unsigned* caller caps that are only sound
> because edge and hub are co-trusted in-process. A bearer key from an untrusted appliance is a
> different trust context. It is mechanically sound here only because the **gateway resolves the
> caps server-side after verifying the secret** (the principal's inputs are trusted even though
> the caller is not) — so we give it its own named constructor that states that invariant,
> rather than silently inheriting a caveat whose justification doesn't apply. (`routed` also
> hardcodes `Role::Member`; `for_key` can set the role explicitly if an admin key is ever wanted.)

> *Rejected:* exchange-the-key-for-a-short-token (Model A). It reuses the verify path
> unchanged but leaves a revoked key live until its token TTL lapses. For machine identity,
> **instant revoke beats minting convenience** — the per-request cost is one HMAC + a cached
> O(1) lookup, which is acceptable.

**2. Permissions are grants on a `Subject`, resolved by a generalized resolver.** The key's
subject is `key:{keyid}` (the `sub` prefix `auth-caps-scope.md` already reserves). We add one
`Subject` variant — `Key(String)` — so `grant_assign`/`grant_revoke`/`role_define` apply to a
key exactly as to a user. **One small generalization is required, not "unchanged":**
`resolve_caps(store, ws, user: &str)` is hardwired to user semantics — it wraps its arg in
`Subject::User(...)` and resolves team membership by matching the arg against member lists
(`resolve.rs:32-44`). Passing `"key:…"` would build `Subject::User("key:…")` → resolve to
**zero caps → silently deny everything.** So we expose the existing inner helper as
`resolve_subject_caps(store, ws, &Subject)` (direct grants + role expansion — the logic already
in `add_subject_caps`, `resolve.rs:52`) and the gateway calls **that** for keys. A key gets
**direct grants + roles only — no team-membership edge** (the member edge matches *user* names;
roles already cover bundling, so keys join no teams in v1). Two built-in roles ship the common
case in one click:
- `apikey-read` — read/get/list caps only (`store:*:read`, `mcp:*.get:call`, `mcp:*.list:call`).
- `apikey-write` — `apikey-read` plus write/call caps.

A key may also be granted finer custom caps. **Read-only is then enforced for free**:
`caps::check` denies any `Write`/`Call` a read-only key doesn't hold. **Tool limits** are
`mcp:<ext>.<tool>:call` grants; **page limits** are the same caps the UI surface gate
(`allowed.ts`) already reads.

**3. Expiry is a lazy check at auth; the scheduler only does housekeeping.** `expires_at` is
compared at authentication (mirroring how `verify` enforces `Claims.exp`) — security never
depends on a job firing. The **outbox** (`next_attempt_ts`) is used **only** to tombstone
expired records and emit "expires in 7 days" notifications. (Today the outbox's
`next_attempt_ts` is the only *implemented* future-scheduler; `lb-jobs` advertises `run_at`
but hasn't built it — so the rationale is "use the one that exists," not "jobs can't ever do
this.") Manual revoke is a `__revoked__` tombstone (the same pattern grants/users already
use), immediate under decision 1 *on the node that processes it* (see Sync/authority below for
the multi-node bound).

The secret is high-entropy random (32 bytes), shown once, stored only as a
**`key_hash = HMAC-SHA256(pepper, secret)`** — a *peppered* hash whose pepper lives in
`lb-secrets`/env, **never in the DB**, so a DB-only leak yields no usable key. High entropy
means a fast keyed hash is correct here (Stripe/GitHub do the SHA-256 form) and keeps
per-request auth cheap; a slow password KDF (argon2/bcrypt) would be wrong on the hot path. The
`cred_ref`→`lb-secrets` seam stays available if we later move the hash off the admin-readable
record.

## How it fits the core

- **Tenancy / isolation:** the key record is `apikey:{ws}:{id}`, written via
  `lb_store::write(store, ws, …)` so namespace selection isolates it physically; the
  presented credential carries its `{ws}`, and `caps::check` gate 1 rejects any request whose
  `ws` ≠ the key's. Workspace B can never see, use, or revoke workspace A's keys. **(Mandatory
  isolation test.)**
- **Capabilities:** management verbs are gated `mcp:apikey.manage:call` (a workspace-admin
  cap); the **deny path** is the standard opaque `Denied`. A key's *own* authority is its
  resolved caps — and crucially a key can never resolve to a cap the **granting admin** doesn't
  already hold. **This is NOT covered by the existing grant path and must be enforced in
  `apikey.create` itself:** `grants_assign` exempts `role:` grants from no-widening
  (`grants.rs:28` — role caps were bounded at `roles.define` time), but `apikey.create` assigns
  a *built-in* role (`apikey-write`) that was seeded by the system, not bounded by this admin. A
  workspace-admin who lacks `store:*:write` could otherwise mint a key that has it. So
  `apikey.create` computes the key's **effective** resolved caps (built-in role + any custom
  caps) and rejects creation unless that set ⊆ the creator's own caps. **(Mandatory deny test —
  including the role-grant escalation path, not just plain caps.)**
- **Placement:** **either** — no `if cloud {…}`. Keys are issued and verified on any node;
  an edge appliance authenticates to its local node the same way the cloud does.
- **MCP surface (API shape, §6.1):**
  - **CRUD:** `apikey.create` (returns the secret **once**), `apikey.revoke` (tombstone),
    `apikey.rotate` (new secret, same grants, old secret dead). No `update` of the secret —
    rotation replaces it; grant changes go through the existing `grant_assign`/`grant_revoke`
    on subject `key:{id}` (one grant path, no parallel verb).
  - **Get / list:** `apikey.list` (ws-scoped, **never returns the hash or secret** — returns
    id, label, kind, role/cap summary, created/expires/last-used, status), `apikey.get` by id.
  - **Live feed:** **N/A** — key lifecycle is administrative; a `list` refresh suffices, no
    `watch`/SSE (state, not motion).
  - **Batch:** **N/A** in v1 — keys are created one at a time. (If bulk appliance enrolment is
    ever needed it becomes a job per §6.1; explicitly out of scope now.)
- **Data (SurrealDB):** one table `apikey`, records `apikey:{ws}:{id}`:
  `{ id, label, kind, key_hash, prefix, expires_at?, created_ts, last_used_at?, status }`
  where `key_hash = HMAC-SHA256(pepper, secret)` and `prefix` is the non-secret display stub
  (e.g. `lbk_acme.k7f3a…`) for the list view, tombstone `__revoked__`. Grants/roles for the key
  live in the existing `grant`/`role` tables under subject `key:{id}`. **This record is shared,
  cloud-authoritative workspace data — it syncs like grants** (the key's grants under
  `key:{id}` already sync §6.8; a non-syncing key record would orphan them on peers). State only
  — no motion.
- **Bus (Zenoh):** **N/A** for the credential itself. The only motion is the optional outbox
  effect (expiry tombstone + notification), which is must-deliver and therefore goes through
  the **outbox**, never raw pub/sub.
- **Sync / authority:** the key record and its grants are **shared, cloud-authoritative**
  workspace data (like grants — *not* node-local; a node-local record would orphan the synced
  grants). An edge node holds a **read-cache** of the record. Revoke is a `__revoked__`
  tombstone → idempotent offline apply (same as `grant_revoke`). **Revoke is therefore instant
  only on the node that observes the tombstone**; on another gateway it bites once the tombstone
  syncs *and* that node's hash→`Principal` cache TTL lapses (seconds). For true cross-node
  instant revoke we broadcast a cache-bust on the bus (a fire-and-forget `apikey/revoked/{id}`
  message peers subscribe to) — **in scope as a v1 nicety**, with the sync+TTL bound as the
  correctness floor if a peer misses the broadcast. The headline "instant revoke" means
  *at the authority + on any node that got the bust*, never "globally instant regardless of
  sync."
- **Secrets:** the key's secret is the secret material. It is **never** returned after
  creation and **never** logged; only the hash is stored. The `cred_ref`/`lb-secrets` seam is
  the documented upgrade path if the hash must leave the admin-readable record.
- **SDK/WIT impact:** none — this is host-native, not a guest ABI change. The new
  `Subject::Key` variant is internal to `lb-authz`; the wire form is `key:{name}`, consistent
  with the existing `user:`/`team:`/`role:` subjects.

## Example flow

1. A workspace-admin opens **Admin → API Keys → New**, names it `rooftop-hvac`, picks kind
   `appliance`, selects role `apikey-write`, sets expiry `+180d`, and (optionally) adds a
   narrowing cap `mcp:series.write:call` only.
2. The UI calls `apikey.create`. The host generates id `k7f3a…` (base32, no `_`), a 32-byte
   base32 secret, **first checks the key's effective resolved caps ⊆ the admin's caps** (else
   `Denied`), stores `apikey:acme:k7f3a…` with `key_hash = HMAC-SHA256(pepper, secret)`, assigns
   role `apikey-write` to subject `key:k7f3a…` via the existing `grant_assign`, and returns the
   one-time string `lbk_acme.k7f3a….<secret>`. The UI shows it once with a copy button and a
   "you won't see this again" warning.
3. The appliance stores the string and calls the gateway with
   `Authorization: Bearer lbk_acme.k7f3a….<secret>`.
4. Gateway auth middleware sees `lbk_`, splits the three dot fields → ws `acme`, id `k7f3a…`,
   secret, fetches the record, constant-time-compares `HMAC-SHA256(pepper, secret_field)` to
   `key_hash`, confirms `status==active` and `now < expires_at`, runs
   `resolve_subject_caps(store, "acme", &Subject::Key("k7f3a…"))`, and builds
   `Principal::for_key(sub="key:k7f3a…", ws="acme", caps)`. It caches hash→principal briefly.
5. The appliance calls `series.write` → `caps::check` passes (gate 1 ws match, gate 2 the
   resolved write cap). It calls `series.delete` → **denied** (not in its caps). A read-only
   key would be denied `series.write` too — read-only is just the cap set.
6. The admin revokes the key: `apikey.revoke` writes `__revoked__`, busts this node's cache, and
   broadcasts `apikey/revoked/k7f3a…` on the bus. This node refuses the appliance's next request
   immediately; peer gateways refuse once they get the bust (or, as the floor, once the
   tombstone syncs and their cache TTL lapses).
7. Separately, 7 days before `expires_at`, an outbox effect fires a "key expiring" inbox
   notification; at `expires_at` an effect tombstones the record. Even if both effects are
   delayed, step 4's `now < expires_at` check already refuses the key — **housekeeping, not
   the security gate**.

## Testing plan

Real store + real gateway + real `caps::check`, seeded with real records — **no mocks, no
`*.fake.ts`** (CLAUDE §9). Mandatory categories from `testing-scope.md` §2:

- **Capability-deny (mandatory):**
  - `apikey.create`/`revoke`/`rotate`/`list` refused without `mcp:apikey.manage:call`.
  - **Effective-cap no-widening (the role-grant escalation path):** an admin who lacks
    `store:foo:write` is **refused** when creating a key that would resolve to it — *both* via a
    custom cap *and* via assigning a built-in role (`apikey-write`) that contains it. This is the
    case the existing `role:`-exempt `grants_assign` does NOT cover, so it must be tested
    explicitly against `apikey.create`.
  - A read-only (`apikey-read`) key is **denied** every `write`/`call`; an `apikey-write` key
    is allowed them — the same key denied the verbs outside its grants.
- **Workspace-isolation (mandatory):** a key minted in ws A cannot authenticate against ws B;
  `apikey.list`/`get`/`revoke` in ws B never see ws A's keys; an `lbk_{ws}.…` whose ws field
  mismatches the record's ws is refused.
- **Offline/sync (mandatory):** `apikey.revoke` tombstone applies idempotently across nodes
  (re-apply is a no-op), mirroring `grant_revoke`. **Two-gateway revoke:** a key accepted on
  gateway A is revoked on A, and gateway B refuses it after the cache-bust broadcast (and, with
  the broadcast suppressed, after sync + cache-TTL) — proving the multi-node bound is honest, not
  "globally instant."
- **Unit:**
  - `Subject::Key` serde round-trip — `parse(as_key()) == Some(Key(..))` and a stored `key:…`
    grant deserializes (a missed `"key"` arm in `Subject::parse` would silently resolve every key
    to no caps — pin it).
  - `resolve_subject_caps` for a `Subject::Key`: direct grants + role expansion resolve; confirm
    a key passed to the *old* `resolve_caps(&str)` resolves to **zero** caps (the bug we're
    avoiding, asserted as a guard).
  - HMAC hash round-trip with a fixed pepper; **hash input is the `secret` field only**, not the
    full bearer string (assert a full-string hash does NOT match); constant-time compare.
  - `lbk_{ws}.{keyid}.{secret}` parse: valid splits, and reject malformed (wrong field count, a
    `.` inside a field, wrong prefix).
  - Lazy-expiry boundary with an **injected clock** (never wall-clock) — `now == expires_at` and
    `now > expires_at` both refused.
- **Integration (real gateway):** the full example flow — create → authenticate → allowed
  call → denied call → revoke → refused-next-request; rotate → old secret dead + new works;
  the cache invalidation on revoke is observable (refused immediately, not after a TTL).
- **Performance (the request-time resolve):** `resolve_subject_caps` on the auth hot path is a
  per-request cost (the deliberate trade for revocability vs the user token's cached projection);
  a bench over direct-grant and role-expansion paths confirms the cached fast-path keeps it off
  the flame graph.
- **UI (`pnpm test` + `pnpm test:gateway`):** create-shows-secret-once, list never renders a
  hash/secret, revoke updates status, the tab is cap-gated (hidden without `apikey.manage`).

## Risks & hard problems

- **The per-request lookup + cache is the hot path.** Under-caching costs a store read per
  call; over-caching delays revoke. The cache TTL (seconds) + explicit bust-on-revoke is the
  balance to get right and to **test** (revoke must bite immediately, not after the TTL).
- **Secret handling discipline.** The raw secret must never be logged, never returned by
  `list`/`get`, and only ever leave the host once at `create`. One leak in a log line defeats
  the whole feature — this needs a focused review and a test asserting `list` output carries
  no secret/hash field.
- **`Subject::Key` ripple.** Adding a variant touches `Subject::parse`/`as_key`, `grant`,
  `revoke_subject`, and — critically — the **resolver**: `resolve_caps(&str)` cannot be reused
  for keys (it wraps the arg in `Subject::User`); the new `resolve_subject_caps(&Subject)` is the
  load-bearing seam and the one most likely to be skipped. The `revoke_subject` seam must also
  tombstone a deleted key's grants so a re-created id can't inherit stale caps.
- **The grant path's `role:` no-widening exemption is a foot-gun for keys.** Because
  `grants_assign` trusts built-in roles, the privilege check has to move into `apikey.create`
  (effective resolved caps ⊆ creator's caps). Miss this and a narrow admin mints a broad key.
- **Multi-node revoke is not globally instant.** The honest guarantee is instant at the
  authority + on any node that received the bus cache-bust; a peer that missed the broadcast is
  bounded by sync + cache TTL. Don't let the UI or docs imply otherwise.
- **Constant-time compare** must actually be constant-time (use a vetted primitive, not `==`),
  and the **pepper** must come from `lb-secrets`/env, never the DB or a committed constant.
- **Per-kind defaults vs explicit caps.** Defaults are convenience; the security boundary is
  always the resolved caps. Don't let a `kind` ever imply authority — keep it labelling.

## Open questions

- **Cache TTL value** — start at a few seconds and measure, or make it config? Recommend a
  small fixed TTL + explicit bust-on-revoke for v1; revisit if the store read shows on a flame
  graph.
- **`last_used_at` write-through cost** — updating it on every auth is a write per request.
  Throttle (write at most every N seconds per key) or defer the field to a phase-2? Recommend
  **defer `last_used_at` to phase 2** so v1 ships without a per-request write; add it throttled
  once the auth path is proven.
- **Rotation grace window** — does `rotate` kill the old secret instantly (recommended,
  simplest, instant) or allow a short overlap for zero-downtime appliance updates? Recommend
  instant for v1; a grace window is a later enhancement if a real appliance needs it.
- **Where the `apikey-read`/`apikey-write` built-in roles are seeded** — at workspace creation
  (like other built-ins) vs lazily on first key. Resolve against how existing built-in roles
  are seeded in `authz`.
- **Cap summary in `list`** — show the resolved cap set, the assigned role names, or both?
  Recommend role names + a "read-only / read-write / custom" badge, with full caps on `get`.

## Related

- README `§6.6` (identity/auth/caps), `§7` (tenancy), `§6.7` (secrets), `§6.10` (jobs/outbox).
- Sibling scope: `auth-caps-scope.md` (the `key:` **subject prefix** this fulfils — *not*
  `auth:key:{id}`, which is that doc's token-signing-key record, a different concept),
  `authz-grants-scope.md` (`Subject`, `resolve_caps`, the freshness asymmetry),
  `admin-crud-scope.md` (the disable/revoke seam this mirrors), `edge-trust-scope.md`
  (node identity — the future mTLS-bound-key path), `secrets/secrets-scope.md`,
  `inbox-outbox/outbox-scope.md` (the housekeeping effect path),
  `frontend/admin-console-scope.md` (where the API Keys tab lands),
  `../cli/operator-cli-scope.md` (the operator CLI — this scope's **named first consumer**; the CLI
  authenticates with the dev-login token in v1 and switches to API keys when this ships).
- Implementation seams (for the building session): `Subject` `subject.rs` (+ the new `Key`
  variant), the new `resolve_subject_caps` generalized out of `add_subject_caps` `resolve.rs:52`,
  `revoke_subject` `revoke.rs`, the new `Principal::for_key` `principal.rs` (vs the co-trust
  `routed`), `host/src/authz/grants.rs:28` (the `role:` no-widening exemption `apikey.create`
  must compensate for), the host-native verb dispatch `host/src/tool_call.rs`, the `users`
  service as the verb-file template `host/src/users/`, the gateway login/auth path
  `role/gateway/src/routes/`, and the admin UI shell `ui/src/features/admin/AdminView.tsx`.
