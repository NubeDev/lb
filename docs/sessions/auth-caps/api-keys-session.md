# API keys — long-lived machine credentials over the existing authz model (session)

- Date: 2026-06-29
- Scope: ../../scope/auth-caps/api-keys-scope.md
- Stage: core capability (post-S8 platform)
- Status: done

## Goal

Ship the full API-keys feature end to end in one session: the `lb-authz` `Subject::Key` variant, the
host `apikey` service + management verbs, the gateway bearer-auth middleware, the two built-in roles,
lazy expiry + outbox housekeeping, and the admin-console "API Keys" tab — all over the **existing**
grant/role/chokepoint model, with NO new permission grammar. Target: the scope's testing plan green
(cap-deny, workspace-isolation, offline/sync revoke idempotency, unit, integration, UI).

## What changed

**New pure crate `lb-apikey`** (`rust/crates/apikey/`, one responsibility per file) — the store-less
half the host + gateway share:
- `hash.rs` — `key_hash = HMAC-SHA256(pepper, secret_field)` (hex), `verify_hash` constant-time, and
  `hash_matches` (constant-time hex compare for the cache). The hash input is the **secret field
  alone**, never the full bearer (pinned in a unit test). Constant-time = the vetted XOR-accumulate the
  github-webhook verifier uses.
- `crockford.rs` / `secret.rs` — Crockford-base32 id (8 bytes) + secret (32 bytes), no `_`/`.` so the
  grammar is delimiter-safe.
- `token.rs` — `BearerKey { ws, key_id, secret }`, `parse_bearer`/`format_bearer`, prefix `lbk_`,
  three dot-separated fields (a fixed split rejecting any field containing a `.`).
- `roles.rs` — `apikey-read`/`apikey-write` cap bundles + the `read-only|read-write|custom` badge. The
  write role uses **action-named** tool-call wildcards (`*.write`, `*.create`, …), NOT `mcp:*.*:call`,
  so a data key can never match the `apikey.manage` resource and escalate into key administration.

**`lb-authz`** — added `Subject::Key(String)` (wire `key:{id}`, the prefix auth-caps-scope reserved)
and generalized the resolver: `resolve_subject_caps(store, ws, &Subject, &mut caps)` is the new
load-bearing seam (direct grants + role expansion); `resolve_caps(&str)` keeps wrapping it for users
+ teams. A key calls `resolve_subject_caps` directly — it joins NO teams.

**`lb-auth`** — `Principal::for_key(sub, ws, caps)`: the dedicated constructor for a verified machine
principal (NOT the co-trust `routed` path — a bearer key from an untrusted appliance is a different
trust context; the gateway resolves the caps server-side after verifying the secret).

**Host `apikey` service** (`rust/crates/host/src/apikey/`, one verb/file + a cache + a seed):
- `model.rs` — `ApiKeyRecord` (carries `key_hash`, NEVER the secret; `status` tombstone `__revoked__`;
  `kind` label; `kind_discrim` list filter), credential-free `ApiKeyView` + `ApiKeyFull` (get adds the
  resolved caps).
- `create.rs` — gen id+secret, **effective-cap no-widening check** (role caps ∪ extra caps ⊆ creator),
  store the hash, assign the role + caps to `Subject::Key`, enqueue an outbox expiry effect, return the
  one-time bearer.
- `revoke.rs` — tombstone (idempotent) + `ApiKeyCache::bust` (instant local revoke) +
  `revoke_subject` (a re-created id inherits nothing).
- `rotate.rs` — fresh secret, old hash overwritten (dead instantly), cache busted, new bearer returned.
- `list.rs` / `get.rs` — credential-free views + the badge; `get` resolves caps.
- `auth.rs` — `apikey_authenticate`: cache → O(1) ws-scoped read → constant-time verify → status +
  lazy-expiry (`now >= expires_at`) → `resolve_subject_caps` → `Principal::for_key` → cache.
- `cache.rs` — `ApiKeyCache` (hash→principal, 5s TTL, bust-on-revoke; a cached entry also misses past
  the record's expiry so the lazy check stays authoritative). Held on `Node` so revoke/rotate bust the
  entry the auth path reads.
- `seed.rs` — `ensure_builtin_roles` (idempotent `apikey-read`/`apikey-write` on first create).

**Gateway** (`rust/role/gateway/`):
- `session/authenticate.rs` — the chokepoint now branches: `lbk_` prefix → async API-key path
  (`authenticate_apikey` → `lb_host::apikey_authenticate`), else JWT. `authenticate`/`verify_token`
  are now `async`; every call site gained `.await`. Every auth failure collapses to the same opaque
  `401`.
- `state.rs` — `pepper: Arc<[u8]>` (from `LB_APIKEY_PEPPER`; dev default per-process random; tests via
  `with_pepper`).
- `routes/admin_apikeys.rs` + `server.rs`/`routes/mod.rs` — `/admin/apikeys` (list/create),
  `/admin/apikeys/{id}` (get), `…/{id}/revoke`, `…/{id}/rotate`; each gated `mcp:apikey.manage:call`.
- `session/credentials.rs` — dev `member_caps` gains `mcp:apikey.manage:call` + the role cap bundles
  (so the dev admin holds them → the no-widening guard lets it mint keys under either built-in role).

**Admin UI** (`ui/`):
- `lib/admin/apikeys.api.ts` (one call per export, co-located types), `features/admin/useApiKeys.ts`
  (the one-time secret surfaced via state, never persisted), `features/admin/ApiKeysAdmin.tsx` (table +
  create form + the show-secret-once banner + revoke/rotate). Wired into `AdminView.tsx` (a cap-gated
  "API Keys" tab), the barrel, `admin-caps.ts` (`CAP.apikeyManage` + `ADMIN_SECTION_CAPS`), and the
  `http.ts` verb→route switch.

## Decisions & alternatives

- **Key format + Principal seam:** the prompt's LOCKED decision #1 (`lb_{ws}_{keyid}_{secret}` +
  `Principal::routed`) contradicted the approved scope doc (`lbk_{ws}.{keyid}.{secret}` dot-delimited +
  `Principal::for_key`). One targeted clarification → **followed the scope doc**: dot-delimited is
  delimiter-safe (the doc explicitly rejects the `_`-delimited form for id-collision), and `for_key`
  states the trust invariant `routed`'s co-trust caveat doesn't cover. Rejected the `_`/`routed` forms.
- **Reaching the management verbs:** followed the **admin-route** pattern (users/grants/roles/teams) —
  dedicated `/admin/apikeys*` routes calling the host service directly, gated `mcp:apikey.manage:call`
  — rather than adding `apikey.*` to the `POST /mcp/call` host-native dispatch. Keeps admin verbs out
  of the data-plane bridge and avoids double-gating.
- **apikey-write caps are action-named** (`*.write`/`*.create`/…), not `mcp:*.*:call`: a blanket `*.*`
  would match `apikey.manage` and let a data key administer keys. Pinned in a unit test.
- **Outbox housekeeping:** the enqueue-at-create (durable expiry intent via `next_attempt_ts`) is the
  scheduled-tombstone path; security is the auth-time lazy check (tested at the `now ==
  expires_at` / `now > expires_at` boundary). The running housekeeping tick is config wiring (future);
  `revoke` tombstone idempotency IS tested (the offline/sync property).
- **Multi-node revoke:** local bust + lazy expiry are the security floor (tested); the bus
  cache-bust broadcast + a two-gateway test are the scope's "v1 nicety", deferred this slice — the
  honest guarantee (instant at the authority + on a node that got the bust; elsewhere bounded by sync +
  the 5s TTL) is documented, not implied as globally instant.

## Tests

Real store + real gateway + real `caps::check`, seeded via the real write path — **no mocks**. Green:

```
lb-apikey             : test result: ok. 20 passed (hash round-trip · constant-time compare ·
                        secret-field-only hash · parse + reject malformed · validity · expiry n/a)
lb-authz resolve_key  : test result: ok. 3 passed (key resolves grants+role · the zero-caps guard
                        for resolve_caps(&str) · revoked grant contributes nothing)
lb-host apikey cache  : test result: ok. 5 passed (TTL hit · wrong-secret miss · bust immediate ·
                        cached entry expires at record expiry · stale after TTL)
gateway apikey_routes : test result: ok. 8 passed (cap-deny per verb · escalation deny incl. the
                        role path · list/get carry no hash or secret · create→auth→allow→deny→
                        revoke→refused · revoke idempotent · rotate old-dead/new-works · lazy-expiry
                        now== & now> expires_at · ws-isolation incl. forged-ws bearer · **cache-bust
                        immediate, not after the TTL**)
UI pnpm test          : 147 passed
UI pnpm test:gateway  : ApiKeysAdmin 2 + AdminView cap-gate 4 passed (create-shows-secret-once ·
                        list renders no hash/secret · revoke→revoked · tab hidden without apikey.manage)
```

Gates: `cargo build --workspace` ✅ · `cargo fmt --check` ✅ · `cargo test --workspace` — every binary
green except `github_bridge_normalize_test` (missing the **pre-built** `github_bridge_ext.wasm`
artifact in this environment — environmental, not this change; same class as the proof-panel wasm the
UI suite notes). `pnpm test` ✅ · `pnpm test:gateway` — API-Keys tests green; the only other failures
are ProofPanel (same missing-wasm class) and two shared-gateway timing flakes (App/SystemView) that
pass on re-run.

## Debugging

None — nothing this slice touched broke. The workspace-test failures observed
(`github_bridge_normalize_test`, ProofPanel) are pre-existing/environmental (a wasm extension artifact
that must be built separately and is absent in this sandbox), not regressions; no debug entry opened.

## Public / scope updates

- Promoted shipped truth into `public/auth-caps/auth-caps.md` (replaced the "scoped, not yet shipped"
  note with what shipped).
- Resolved every open question in `scope/auth-caps/api-keys-scope.md` (cache TTL = fixed 5s +
  bust-on-revoke; `last_used_at` deferred; rotation instant; built-in roles ensured on first create;
  list = role names + badge; format/Principal seam = the scope doc's).
- `STATUS.md`: added the shipped "API keys" slice row + a Current-stage note.

## Dead ends / surprises

- The `cargo` linker (`zigcc`) and its target-triple-rewrite wrapper were missing in this sandbox —
  recreated the wrapper (rewrites `x86_64-unknown-linux-gnu` → `x86_64-linux-gnu` ONLY in target-flag
  values, never inside `-L` sysroot paths, which was the bug in a naive global rewrite).
- Making `authenticate` async rippled to ~90 gateway call sites + the SSE `verify_token` callers + a
  sync `util` helper in `prefs.rs` + the `test_gateway_seed` `auth` helper — all mechanical `.await`
  additions. Worth it: API keys now authenticate the SAME routes a JWT does (the whole gateway), which
  is the point (an appliance calls `/series`/`/mcp/call` with its bearer).

## Follow-ups

- `last_used_at` (throttled per-request write) — a later phase.
- The bus cache-bust broadcast + a two-gateway cross-node revoke test (the multi-node instant-revoke
  nicety; the sync+TTL floor is shipped + documented).
- A running housekeeping tick that calls the outbox-scheduled expiry tombstone (config wiring in the
  `node` binary).
- STATUS.md updated: yes (slice row + Current-stage note).
