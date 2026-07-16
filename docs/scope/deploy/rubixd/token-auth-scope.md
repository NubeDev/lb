# rubixd scope — boot token, one-time claim, REST auth

Status: scope (the ask). Slice 2 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

rubixd gets a local HTTP surface (REST now, the Bootstrap UI in slice 7). Access is one
**admin token, generated on boot, claimable exactly once from the UI**, and presented as
`Authorization: Bearer <token>` on every REST call thereafter. The mechanics live in the
shared **`fleet-auth`** crate so rartifacts (which has the same requirement) reuses them
verbatim — write once here, consume there.

## Goals

- **`fleet-auth` crate** (shared): `token.rs` (generate 32 random bytes → `rbd_<base62>`
  / `rfa_<base62>` display form, prefix per service), `claim.rs` (the state machine),
  `bearer.rs` (an axum extractor verifying a SHA-256 token hash in constant time),
  `store.rs` (persist *hash + claimed flag only* — plaintext is never written to disk).
- **Boot behavior**: if no admin-token record exists (or `--reset-token`), generate one;
  persist its hash with `claimed = false`; hold the plaintext **in memory only**; log
  `admin token ready — claim it once at http://<bind>/claim` (the URL, **never** the
  token) to stdout/journal.
- **One-time claim**: `POST /api/claim` returns `{ token: "rbd_…" }` exactly once,
  flips `claimed = true`, and zeroizes the in-memory plaintext. Every later call →
  `410 Gone`. If the process restarts *unclaimed*, a fresh token replaces the old hash
  (the old one was never seen by anyone).
- **REST auth**: every route except `POST /api/claim` and `GET /health` requires
  `Authorization: Bearer` matching the stored hash; failure → `401` with no body detail.
  The same token drives the UI (stored client-side after claim) and any curl/script.
- **`GET /health`** (open — the fleet health contract, decided in
  [`../containerize-scope.md`](../containerize-scope.md) §The health contract): `200
  {"status":"ok","version":…,"detail":{…}}` when the ledger is open; `503
  {"status":"degraded",…}` when it is not. **`/health`, never `/healthz`.** It reads
  **in-memory state only** — no store query, no disk I/O, no network call, and it must
  **never block on a dependency** (a health check that can hang is one that lies).
  `detail` carries `{"ledger":…,"backends":{"docker":…,"systemd":…}}` — the same
  backend-availability facts `status` prints, machine-readable for a probe. **A backend
  being unavailable is NOT degraded** — a docker-only box is correctly configured and
  returns 200; degraded means "cannot do the job at all". `detail` names *which* subsystem
  is down, never a path or key.
- **Recovery**: `rubixd --reset-token` (CLI, requires local root) regenerates and
  re-opens the claim window — lost tokens are recovered at the box, never over HTTP.
- REST surface gated by it (this slice ships the server + these read verbs; later
  slices add theirs): `GET /api/status`, `GET /api/instances`,
  `GET /api/instances/{name}`.

## Non-goals

- No multi-user, no roles, no token expiry/rotation schedule — one machine, one admin
  token, rotate via `--reset-token`. (rartifacts' richer identity — agent/publisher
  principals as lb api-keys — is rartifacts' auth scope; it consumes this crate only
  for the one-time claim bootstrap.)
- No TLS termination in v1 — rubixd binds `127.0.0.1` by default; exposing it on a LAN
  is an explicit config choice documented with a "put TLS/a tunnel in front" warning.
- No sessions/cookies — the bearer token is the whole story; the UI keeps it in
  `localStorage` (localhost-bound risk accepted, recorded below).

## Intent / approach

- Claim-race posture: whoever reaches `/api/claim` first owns the box's agent. Default
  `127.0.0.1` bind makes that "someone with local access" — acceptable. On a non-local
  bind, boot **additionally** logs a 6-digit claim code and `POST /api/claim` requires
  it (`{"code": "…"}`); wrong code 3× re-locks claiming until restart. This keeps the
  UI flow ("open page, click claim") on localhost and adds one field when exposed.
- Hashing: SHA-256 of the 32-byte secret is sufficient (high-entropy random, not a
  password) — no argon2 dependency. Constant-time compare via `subtle`.
- Alternative rejected: printing the token itself to stdout (the Portainer/HA pattern
  variant) — the user's ask is claim-from-UI, and journals get shipped to log
  aggregators; a URL + short code leaks nothing durable.

## How it fits the core

Capability-first translated: the deny path is first-class — 401 unauthenticated, 410
re-claim, 423 locked after bad codes; each has a test. Secrets: only hashes at rest
(house secrets posture). Everything else N/A (not an lb node).

## Example flow

1. Fresh install boots → journal: `claim it once at http://127.0.0.1:9420/claim`.
2. Operator opens the URL (slice 7 UI; until then `curl -X POST /api/claim`), receives
   `rbd_9fK…` once, stores it.
3. `curl -H "Authorization: Bearer rbd_9fK…" :9420/api/status` → 200.
4. Second `POST /api/claim` → 410. Token lost later → `sudo rubixd --reset-token` →
   new claim URL in journal.

## Testing plan

Real axum server, real embedded store (no mocks):

- claim returns token once; second claim 410; restart-after-claim still 410 and old
  token still works (hash persisted).
- restart *before* claim → old plaintext invalid, new claim works.
- 401 on missing/garbage/truncated bearer for every registered route; `GET /health`
  and `POST /api/claim` reachable unauthenticated (deny + allow both asserted).
- `GET /health`: 200 + `{"status":"ok","version":…}` against a real open ledger; **503 +
  `{"status":"degraded"}` when the ledger cannot open** (the liveness/readiness split —
  the process still *answers*, which is what proves it is alive rather than restart-worthy);
  `detail.backends` reflects a real probe; **no key/path material in any body**.
- non-local bind: claim without code 400, wrong code 401, third wrong 423, correct
  code + claim works.
- `--reset-token`: old bearer 401s, new claim succeeds.

## Risks & hard problems

- `localStorage` token on a LAN-exposed UI is exfiltratable by any XSS — the UI slice
  must ship zero third-party JS beyond bundled Bootstrap, and this is re-reviewed there.
- Plaintext lives in RAM until claimed — `zeroize` on claim; accepted residue risk.

## Decisions (no open questions)

- **6-digit claim code on localhost — decided: no.** Keep localhost one-click; require the
  code only on a non-local bind (§Intent). A localhost claim already implies local access,
  which is the threat the code defends against — charging every fresh install a journal
  read to defend against nothing is friction that teaches operators to script around the
  claim flow. *Reopen if*: the container posture makes non-local binds the common case in
  practice (see below) — uniformity would then be worth the one read.
- **Container posture interaction — decided: the code is required in-container.** A
  containerized rubixd sets `RUBIXD_BIND_ADDR=0.0.0.0:9420`
  ([`../containerize-scope.md`](../containerize-scope.md)), which **is** a non-local bind —
  so the 6-digit code path is automatically active there, by the rule above and with no
  container-specific branch. This is the rule working as designed, not an exception to it.

## Related

[`embedded-ui-scope.md`](embedded-ui-scope.md) (the claim page) ·
[`../rartifacts/token-auth-scope.md`](../rartifacts/token-auth-scope.md) (the other
`fleet-auth` consumer — build this slice first) ·
[`../containerize-scope.md`](../containerize-scope.md) (the `/health` body contract this
slice implements; `RUBIXD_BIND_ADDR`, which makes the claim-code path active in-container).
