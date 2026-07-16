# Auth-caps scope — email/password login + workspace selection (the Slack front door)

Status: scope (the ask). Promotes to `doc-site/content/public/auth-caps/` once shipped.
Downstream consumer: `rubix-ai` (`docs/scope/frontend/email-login-scope.md` — the UI half).

Today a human logs in by typing a raw principal (`user:ada`) **and** a workspace name, and the
password check — when enabled — is verified against a **per-`(workspace, user)`** hash. That is
backwards from the model the platform already committed to: `global-identity-scope.md` (shipped)
says **one person authenticates ONCE as a global identity, then picks a workspace** — the Slack
pattern — and its decision #7 explicitly deferred "the credential joins the global identity" to
this slice. This scope is that slice: a real **email + password** front door that authenticates
the global identity in `_lb_identity`, resolves the workspaces the person belongs to, **auto-enters
the workspace when there is exactly one**, and shows a picker only when there are several. No user
ever types a workspace name or a `user:` principal again. The token contract is untouched — one
token, one `ws` claim, same caps grammar; this scope only changes how a person *arrives* at a mint.

> **Pre-production — no legacy human path.** lb has not shipped to production, so this scope
> **replaces** the identity+workspace `POST /login` and the per-`(workspace, user)` credential
> outright rather than living alongside them. `/auth/*` becomes the **only** human login and the
> global credential the **only** human credential. The old `POST /login` route and the per-ws
> `Credential` + `identity.set_credential` are **removed**. There is no back-compat surface to
> preserve and no migration to run.
>
> **Machines don't log in — they carry a key.** Agents, AI sessions, the CLI, appliances, and raw
> API callers do **not** use `/auth/*` or a password. They authenticate with a **lb API key**
> (`api-keys-scope.md`) — a non-human `Subject` in the shipped authz model, workspace-walled,
> scoped, and instantly revocable through the same `caps::check` chokepoint. Removing the human
> legacy path therefore takes **nothing** away from machine callers; API keys are their front door
> and are untouched by this scope. The one exception is dev/CI convenience — see Non-goals.

## Goals

- **Email is the human login handle.** The global identity record gains a globally-unique,
  case-insensitive `email`; login is `email + password`, resolved to the existing `sub`
  (`user:ada`). The `sub` stays the grant key (global-identity decision #6) — email is a lookup
  handle, never a grant subject.
- **The credential is global, on the identity.** One argon2id password hash per *person*, stored
  in `_lb_identity` beside (not inside) the identity record — the `cred_ref` seam decision #7
  reserved. A person has one password across all their workspaces, exactly like Slack.
- **Authenticate once, then choose.** `POST /auth/login {email, password}` verifies the credential,
  enumerates effective memberships, and:
  - **0 workspaces** → `403 "not a member of any workspace"` (decision #4 — no token).
  - **1 workspace** → mints the full workspace token immediately (the auto-skip; no picker step
    exists on the wire for this person).
  - **N workspaces** → returns the membership list + a short-lived **select-token** that is good
    for exactly one thing: `POST /auth/select {workspace}` → the full token.
- **Switching needs no password.** `POST /auth/switch {workspace}` with a *valid full token*
  re-mints into another workspace the same `sub` is an effective member of (membership +
  user-disabled re-checked at switch time). This makes the workspace switcher real without the
  client storing a password.
- **The credential check stays pluggable.** The global check is a trait with the same three faces
  as the shipped per-ws seam: `GlobalPasswordHash` (production default), `DevTrustAny` (opt-in via
  `LB_DEV_LOGIN`, dev/CI only), OIDC later behind the same seam — no route change (README §6.6).
- **Nothing downstream moves.** `verify`, the `ws` hard wall, role/grant/teams/nav-reach cap
  resolution at mint, the three gates — all byte-identical. API-key auth for machines
  (`api-keys-scope.md`) is untouched. This scope changes only the **human** front door.

## Non-goals

- **No email verification, password-reset email, MFA, or account recovery** — same deferrals as
  `login-hardening-scope.md`. A real credential store enables them; they are their own surfaces.
  (Password *set/change* is in scope; the emailed reset link is not.)
- **No OIDC/SSO provider wiring.** The `IdentityCredentialCheck` seam must leave room for a code
  exchange, but no provider lands here.
- **No refresh tokens / session-lifecycle redesign.** The 12h human TTL stands; expiry = log in
  again. (The select-token is a mint-flow artifact, not a session mechanism.)
- **No machine-auth change.** Agents/AI/CLI/appliances/raw API callers authenticate with **lb API
  keys** (`api-keys-scope.md`), not `/auth/*` and not a password — that surface is out of scope
  here and untouched. Removing the human legacy path takes nothing from them.
- **No legacy human `POST /login`.** Pre-production: the identity+workspace `POST /login` route and
  the per-`(workspace, user)` `Credential` + `identity.set_credential` are **removed**, not kept
  alongside. The *only* concession is dev/CI: a password-less path stays reachable **only** under
  the explicit `LB_DEV_LOGIN` flag (via `DevTrustAny` behind the new `/auth/login`), never in a
  release build — so tests need no seeded hashes but production has exactly one human door.
- **No org tier, no multi-hub identity** (unchanged from global-identity non-goals).

## Intent / approach

**Move the human credential to where the human identity already lives, and split the mint into
authenticate → choose → mint.** The building blocks all exist: `_lb_identity` (identity),
`membership` rows + `identity.workspaces` ("drives the login picker + the switcher" — its doc
comment has been waiting for this scope), the `CredentialCheck` pluggable pattern, argon2id
hashing, and the pre-principal gate sequence in `routes/login.rs`. This scope recombines them:

1. **Identity gains `email`** — optional field on `identity:{sub}`, unique index (lowercased) in
   `_lb_identity`. Set at `identity.create` (additive arg) or via a new admin verb
   `identity.set_email` (gated `mcp:identity.manage:call`). Two identities cannot share an email.
2. **A global credential record** — `identity_credential:{sub}` = `{sub, kind:"password", phc,
   set_ts}` in `_lb_identity`, mirroring the shipped per-ws `Credential` shape. Secret-class:
   written only via mediated verbs, never returned by any read (§6.7). Set by admin verb
   `identity.set_password(sub, secret)` (gated `mcp:identity.manage:call`) or self-service
   `POST /auth/password {old, new}` (bearer full token; verifies `old` first — authenticated-self,
   no admin cap).
3. **Three new gateway routes**, all going through the same gate ladder the shipped login built:
   - `POST /auth/login {email, password}` → email→sub lookup + `IdentityCredentialCheck::verify`
     (**one uniform `401 "invalid credentials"`** whether the email is unknown or the password
     wrong — no account enumeration) → enumerate effective memberships, dropping any workspace
     where the per-ws `user_login_check` refuses (disabled there ≠ disabled everywhere) → 0/1/N
     branch above. The 1-branch runs today's full mint path (membership resolve, role-correct
     caps, nav-reach union) unchanged.
   - `POST /auth/select {workspace}` — bearer **select-token** → re-verify membership → mint.
   - `POST /auth/switch {workspace}` — bearer **full token** → re-verify membership +
     `user_login_check` in the target → re-mint under the same `sub`.
   Replies reuse `LoginReply` (`{token, principal, workspace, caps}`) plus a `workspaces:
   [{ws, name}]` list so the client learns the roster in the same round trip.
4. **The select-token is a deliberately powerless JWT**: same signer, `sub` set, `ws: ""`,
   `caps: []`, a `constraint: "ws-select"` claim, TTL ~5 minutes. Every verb/route gate already
   refuses it (empty ws + no caps fail the first two gates); only `/auth/select` accepts it, by
   checking the constraint explicitly. One token type, one new acceptor — no parallel session store.

**Rejected alternatives:**
- *Client re-sends the password with the chosen workspace (one route, two calls).* Rejected — the
  client must hold the plaintext across the picker UI, and every workspace choice replays the
  credential. The select-token keeps the plaintext's lifetime to one request.
- *A multi-workspace token (list of `ws` claims).* Rejected — "one token, one workspace" is the
  hard wall's load-bearing shape (claims.rs calls it out); every route derives THE workspace from
  the token. A ws-list would force every gate to re-decide which wall applies per request.
- *Email/password verified per-workspace (extend the shipped per-ws credential).* Rejected — it
  cannot express "authenticate once, then choose": you must pick the workspace *before* you can
  verify, which is exactly today's UX this scope exists to kill. It also means N passwords per
  person. The global-identity scope already placed identity on the hub; the credential follows it.
- *Auto-enter the last-used workspace for N>1 users (server-side).* Rejected — the server has no
  business remembering UI preference; the client remembers last-used and preselects it in the
  picker (the rubix-ai scope owns that).

## How it fits the core

- **Tenancy / isolation:** the workspace wall is untouched — the full token still carries exactly
  one `ws`, and every route still derives the workspace from the token. The new pre-mint reads
  (email lookup, credential verify, membership enumeration) run in `_lb_identity` + each
  workspace's own `membership` table, the same pre-principal-but-workspace-scoped discipline the
  shipped login gates use. `auth.switch` cannot reach a workspace the `sub` is not an effective
  member of — re-checked server-side at switch time, never trusted from the client roster.
- **Capabilities:** `/auth/login` and `/auth/select` are pre-principal by nature (they *issue* the
  token) — bespoke unauthenticated routes like `POST /login`, hardened by the credential gate, the
  uniform 401, and rate limiting. `/auth/switch` and `/auth/password` require a valid full token.
  Admin surfaces (`identity.set_email`, `identity.set_password`) ride the existing
  `mcp:identity.manage:call`. Cap issuance at mint is byte-identical to the shipped role-correct
  path (login-hardening) — nav-reach and role/grant unions included, per selected workspace.
- **Placement:** identity is hub-authoritative (global-identity decision #8), so `/auth/*` mounts
  where the identity directory is writable/readable authoritatively — decided by the node's role
  config on `BootConfig`, never a code branch. Edges keep offline token verify; they do not run
  the email front door. `DevTrustAny` selection stays env-driven at the binary boundary
  (`LB_DEV_LOGIN`), same as the shipped seam.
- **MCP surface / API shape (§6.1):** create/update only — `identity.set_email`,
  `identity.set_password` (no list/read of secrets, ever; email appears in `identity.get/list` for
  admins). The three `/auth/*` routes are bespoke gateway routes by nature, like `/login`.
  `identity.workspaces` is unchanged and stays admin-gated — the login flow reaches the same
  internals *behind* the credential gate, not through the admin verb.
- **Data (SurrealDB):** two additions in `_lb_identity` only — the `email` field + unique index on
  `identity:{sub}`, and the `identity_credential:{sub}` record. No new tenant-namespace tables.
  State, not motion.
- **Secrets (§6.7):** the global hash is secret-class — argon2id PHC only, written via mediated
  verbs, never in any read/list/log/token. The plaintext exists only inside the two verify calls
  and the set calls. Rate limiting on `/auth/login` (per-email fixed window) blunts online
  guessing; the uniform 401 blunts enumeration.
- **Sync / authority:** identity + credential are hub-authoritative system data (same §6.8
  discipline as the identity directory itself). Edges cache identity read-only; a password set
  reaches edges as system-data sync, and an edge never accepts a password *set* while partitioned.
- **Invites compose, not re-scope:** the shipped invite flow is the natural onboarding path — an
  invite-accept for a new person creates the identity, sets email + password (via the same
  mediated set path), and adds the membership, landing them in the 1-workspace auto-enter branch.
  Wiring that UX is the invite scope's follow-up, noted here so nobody builds a second door.

## Example flow

1. **Provision** — admin creates Ada: `identity.create("user:ada", email: "ada@acme.com")`,
   `identity.set_password("user:ada", …)` (or Ada arrives via an invite that does both). Ada is a
   global identity with a credential, in however many workspaces she's been added to.
2. **One workspace (the common case, the ask's headline)** — Ada is a member of `acme` only.
   `POST /auth/login {email:"ada@acme.com", password:"…"}` → credential verified → memberships =
   `[acme]` → the reply IS the full token (`{token, principal:"user:ada", workspace:"acme", caps,
   workspaces:[{ws:"acme",…}]}`). She never saw a workspace field. **One round trip, signed in.**
3. **Many workspaces** — Bob belongs to `acme` and `globex`. Same call → `401`-safe verify →
   reply is `{select_token, workspaces:[{acme…},{globex…}]}`, no full token. He picks `globex` →
   `POST /auth/select {workspace:"globex"}` (bearer select-token) → membership re-checked → full
   token for `globex`, with globex's independent caps.
4. **Switch** — Bob, working in `globex`, opens the switcher and picks `acme` →
   `POST /auth/switch {workspace:"acme"}` (bearer his globex token) → he is an effective member →
   re-mint `(user:bob, acme, acme's caps)`. No password re-entry. Had an admin removed his `acme`
   membership an hour ago, the switch is `403` — the roster in his client was a lens, not authority.
5. **Wrong password / unknown email** — both return the same `401 "invalid credentials"` after the
   same code path shape; five rapid failures for one email hit the rate-limit window.
6. **Zero memberships** — Carol authenticates fine but was removed from her last workspace →
   `403 "not a member of any workspace"`, no token of any kind (decision #4).
7. **Disabled in one, not all** — Dave is disabled in `acme` (per-ws `user_login_check`) but
   active in `globex` → his login lists only `globex`; an `auth.switch` to `acme` is refused.
8. **Machine caller** — an AI agent / CLI / appliance presents its **lb API key**
   (`api-keys-scope.md`), not `/auth/*` — authenticated as a non-human `Subject`, scoped and
   revocable, entirely outside this human front door.
9. **Dev/CI** — `LB_DEV_LOGIN=1` selects `DevTrustAny` behind `/auth/login` — password-less but
   still role-correct caps, so tests need no seeded hashes. Off in release builds: production has
   exactly one human door and it demands a password.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all over the **real** gateway + store,
seeded via real verbs (no mocks; argon2 real):

- **Capability deny** — a non-admin is refused `identity.set_email`/`identity.set_password`;
  a select-token is refused by every normal route/verb (empty-ws + empty-caps + constraint —
  probe `/mcp/call`, admin routes, SSE); a full token is refused at `/auth/select`.
- **Workspace isolation** — `auth.switch` to a non-member workspace → `403`; the login reply's
  roster lists only effective memberships; ws-B's admin actions can't affect what ws-A's login
  enumerates. A minted token's caps are the *target* workspace's caps only.
- **Credential correctness** — right password → token; wrong password and unknown email → the SAME
  `401` body; absent global credential → `401` (not a crash, not a fall-through to per-ws);
  `identity.set_password` rotates (old fails, new passes); self-service change requires the old
  password; the hash never appears in any read/log/reply.
- **The 0/1/N branch** — 0 → `403` no token; 1 → full token, `select_token` absent; N →
  `select_token` + roster, full token absent; select-token expiry (~5 min) → `401` at
  `/auth/select`; select honors the roster (a workspace not in the person's memberships → `403`
  even with a valid select-token).
- **Email uniqueness** — second identity with the same (case-folded) email is refused; lookup is
  case-insensitive.
- **Legacy path is gone** — `POST /login` no longer exists (or `404`s); no per-ws credential verb
  remains. A test asserts the removed route is unreachable and that the login-hardening tests which
  depended on it were retired or ported to `/auth/*`, not left dangling.
- **Machine auth intact** — an API-key caller still authenticates and authorizes unchanged (a smoke
  test that the removal didn't touch the `apikey:` path).
- **Switch freshness** — membership removed → next `auth.switch` refused; user disabled in the
  target → refused (mirror of `user_login_check` at login).

## Sequencing (implementing session, 2026-07-16)

**Build the replacement first, remove `/login` in a tracked follow-up.** The end-state is unchanged
— `/auth/*` is the only human door and the global credential the only human credential — but the
`/login` removal cascades into the CLI login, the invite-accept mint, and ~10 session-seeding test
suites. To keep the tree green at every step, this session lands the **additive** half:
`identity_credential` + `email` on the identity, `identity.set_email`/`identity.set_password`, the
`GlobalCredentialCheck` seam, the select-token, and `/auth/login|select|switch|password` — fully
tested — **alongside** the existing `/login`. The **removal sweep** (delete `POST /login`, the per-ws
`Credential` + `identity.set_credential`; re-point the CLI + machine callers to API keys; retire/port
the legacy suites) is the immediate next slice, tracked here so it is not forgotten. Until it lands,
`/login` still exists but is no longer the intended human door.

## Risks & hard problems

- **One credential store, cleanly.** The per-ws `Credential` and `identity.set_credential` are slated
  for **deletion** (the removal sweep above) — the global credential in `_lb_identity` becomes the
  single source of truth for human auth. The risk is an incomplete removal: a lingering `POST /login`
  handler or per-ws verify call would be a second, silent door. The removal must be total and the
  "legacy path is gone" test pins it. (Machine auth via API keys is a separate, intact store — do not
  touch it in the sweep.) **Until the sweep, the two credential stores coexist** — the global one is
  what `/auth/*` uses; the per-ws one still backs `/login`. They never consult each other.
- **Account enumeration via timing.** The uniform 401 must also be uniform-ish in *time* — an
  unknown email that skips argon2 returns in µs while a wrong password burns the argon2 cost.
  Verify against a dummy hash on unknown email (the standard fix); test asserts both arms return
  the same status/body.
- **Rate limiting is new surface for the gateway.** Keep it a dumb per-email fixed window in
  memory (per-node); a distributed limiter is over-scope. Name the constant, test the lockout
  window, and let ops raise it.
- **`_lb_identity` gains a unique index.** The reserved-namespace store path so far holds plain
  records; the email uniqueness check must be race-safe (create-if-absent semantics on the index),
  not read-then-write.
- **Select-token abuse.** It must be useless everywhere except `/auth/select` — the deny tests
  above are the guardrail; get them in before the route ships, not after.
- **Bootstrap of the very first admin.** With `POST /login` gone, the old "first login into an
  empty workspace bootstraps a workspace-admin" trick disappears too — so this scope **owns** the
  first-admin story. The blessed path: an operator seeds at provision time with `identity.create`
  (+ `email`) → `identity.set_password` → `create_workspace` (which auto-grants that identity
  `workspace-admin`, decision #3), after which they log in via `/auth/login` normally. Name this in
  the runbook; it is the only bootstrap and it must not fail-open (no password → no admin).

## Open questions (RESOLVED — implementing session, 2026-07-16)

- **Does `/auth/login` honor `LB_DEV_LOGIN` (DevTrustAny) too?** → **YES.** The global check is a
  trait (`GlobalCredentialCheck`) selected by `LB_DEV_LOGIN` at the binary boundary: set →
  `GlobalDevTrustAny` (password-less, dev/CI), unset → `GlobalPasswordHash` (argon2 against the
  `identity_credential` record). CI for the new flow needs no seeded hashes; a release build ignores
  the flag and always demands a password. The minted token is still role-correct.
- **Rate-limit constants** → **10 failures / 15 min per email, per-email only, v1.** Reuses the
  shipped `FixedWindowLimiter` (`routes/rate_limit.rs`) keyed by the lower-cased email. No per-IP
  dimension in v1 (behind a proxy the XFF hop is spoofable and the per-email window already blunts
  online guessing). A per-node in-memory window (not distributed) — ops can raise it.
- **Should `workspaces[]` carry display names?** → **YES, `{ws, name}`** — reuses the existing
  `IdentityWorkspace {ws, name}` resolution row (`identity.workspaces`), so the login reply's roster
  is one round trip and the rubix-ai picker renders the workspace name. Richer branding stays the
  brand endpoint's job (the picker fetches it lazily, not in the login reply).

## Related

- `global-identity-scope.md` — the shipped Slack-model substrate; decisions #4 (zero memberships),
  #6 (human-handle sub), #7 (credential deferred to THIS slice), #8 (hub-only identity writes).
- `login-hardening-scope.md` — the shipped credential seam + role-correct issuance this reuses;
  its non-goals (MFA/reset/refresh) stay non-goals here.
- `api-keys-scope.md` — the **machine** front door (agents/AI/CLI/appliances/raw API). Untouched by
  this scope; it is why removing the human legacy `POST /login` costs machine callers nothing.
- `invites-scope.md` — the onboarding path that composes with this front door.
- `auth-caps-scope.md`, `authz-grants-scope.md`, `nav-reach-scope.md` — issuance inputs, unchanged.
- README §6.6 (OIDC intent — the seam this leaves open), §7 (the wall), §6.7 (secrets), §6.8
  (hub-authoritative sync).
- Downstream: `rubix-ai` `docs/scope/frontend/email-login-scope.md` — the login screen, picker,
  auto-skip, and switcher UI over these routes.
- Source anchors: `rust/role/gateway/src/routes/login.rs` (the gate ladder to mirror),
  `rust/role/gateway/src/session/credential.rs` (the pluggable pattern to mirror globally),
  `rust/crates/authz/src/identity.rs` + `rust/crates/host/src/identity/` (`_lb_identity`, the
  record this extends), `rust/crates/host/src/credential/` (the per-ws shape to mirror),
  `rust/crates/auth/src/claims.rs` (`constraint` — the select-token rides it).
