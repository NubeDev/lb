# Auth-caps scope — login hardening (credential check + role-scoped cap issuance)

Status: scope (the ask). Promotes to `doc-site/content/public/auth-caps/` once shipped.

The `POST /login` dev-shim is the one non-real piece of an otherwise-real auth stack, and it
currently leaks in two ways that a live session surfaced. **(1)** It performs *no credential
check* — any caller who can reach the port mints a valid signed token for any `user` in any
`workspace`, including a workspace-admin. **(2)** Every login — regardless of who logs in — is
minted the **same `member_caps()` set, and that set contains admin-grade capabilities**
(`members.add`, `teams.manage`, `roles.define`, `grants.assign`, `user.manage`,
`workspace.create`). So a nominal "member" (`Role::Member`) is, in practice, a full admin: in a
live test `user:bob` — added only as a plain member — added a member, created a team, and
**granted himself `mcp:workspace.delete:call`**, all `204`. This scope hardens the login seam so
that (a) identity is *proven* before a token is minted and (b) the caps a token carries are
**scoped to the principal's actual role/grants**, not a blanket admin bundle. The token/verify/
capability machinery downstream is already real — this fixes only the credential front-door and
the cap-issuance step behind it.

## Goals

- **Cap issuance follows role + grants, not a blanket bundle.** A `Role::Member` login must mint
  a member's caps; admin caps (`members.*`, `teams.manage`, `roles.*`, `grants.assign`,
  `user.manage`, `workspace.*`, `dashboard.delete_any`, …) must require the `workspace-admin`
  role (or an explicit grant). This closes the privilege-escalation with no dependency on real
  authentication and is the **urgent** half.
- **A real credential check gates token minting.** Replace "trust the request body" with a
  pluggable credential verification (password-hash to start, OIDC/IdP behind the same seam per
  README §6.6) that must pass before `mint`. A bad credential `401`s; the dev-shim becomes an
  explicitly-flagged, non-default dev mode.
- **The dev-login remains usable for local dev/CI** — but is opt-in and clearly labelled, never
  the production default, and even in dev it issues **role-correct** caps (so tests exercise the
  real deny path instead of an all-powerful token).
- **No change to the downstream contract** — `verify`, the workspace wall (§7), and every
  server-side capability re-check stay exactly as they are; this scope only changes what claims a
  token is born with and whether it is born at all.

## Non-goals

- **Full IdP/SSO integration** (SAML, multi-provider OIDC federation, SCIM provisioning) — the
  seam must *accept* an OIDC code exchange, but wiring a specific provider is a later scope.
- **Key rotation / token revocation lists / refresh tokens** — `authz.revoke-tokens` already
  exists; session-lifecycle redesign is out of scope here.
- **Password reset / MFA / account recovery flows** — a real credential store enables these, but
  they are their own surface.
- **Redefining the role→cap catalog itself.** This scope makes issuance *honor* roles; the
  authoritative member/admin cap lists live in `authz-grants-scope.md` and the role catalog — we
  consume them, we don't re-author them here (beyond removing the admin caps wrongly baked into
  the dev member bundle).

## Intent / approach

Two changes at one seam (`rust/role/gateway/src/routes/login.rs` + `session/credentials.rs`),
kept behind the existing `mint`/`verify` boundary so no route changes:

1. **Split the cap bundle by role.** `dev_claims` today hardcodes `Role::Member` **and**
   `member_caps()` where `member_caps()` is really an admin bundle. Replace with: resolve the
   principal's role (from the membership/role record), then mint `caps = base_member_caps ∪
   resolve_caps(store, ws, principal)` — i.e. the *durable grant store already is the source of
   truth* (login.rs already unions `resolve_caps`; the bug is that the **base** bundle is
   over-broad). Trim `base_member_caps` down to genuinely member-level verbs; admin caps come
   **only** from the `workspace-admin` role's grants via `resolve_caps`. A first-principal
   workspace bootstrap still grants that principal the `workspace-admin` role (so the demo's first
   login is still an admin) — but through the grant path, not a blanket base bundle.
   *Alternative rejected:* keeping the broad base bundle and "hiding" admin controls in the UI —
   that is exactly the false-security the live test broke (the UI `caps` list is explicitly *not*
   the boundary; the REST/MCP routes honored bob's caps and executed).

2. **Gate minting on a credential.** Introduce a `CredentialCheck` trait with one method
   (`verify(user, workspace, secret) -> Result<()>`); `login` calls it before `dev_claims`. Ship
   two implementations behind config: `DevTrustAny` (today's behavior, **opt-in** via an explicit
   env flag, for local dev/CI) and `PasswordHash` (argon2 against a hashed credential in the
   store, per-workspace, capability-mediated). OIDC lands later as a third impl behind the same
   trait — no route change. This matches README §6.6 ("OIDC for human login; tokens are JWTs
   carrying a workspace claim and scopes") and the auth-caps non-goal that deferred the *provider*,
   not the *check*.

Sequencing: **change 1 first** (it's the security hole, needs no credential store, and is
independently testable), **change 2 second** (needs a credential store + argon2 + a config flag).

## How it fits the core

- **Tenancy / isolation:** unchanged and reaffirmed — the workspace still comes from the token,
  never the body (login.rs derives `ws` into the claim; every route reads the token's ws). A
  credential is verified *within* a workspace; a password for `(ada, acme)` cannot mint a token
  for `beta`.
- **Capabilities:** this scope's whole point. The deny path becomes real for members: a
  member-role token lacks `mcp:members.add:call` etc., so the route's existing capability check
  `403`s — the same chokepoint, now actually exercised because the token no longer over-grants.
  The base member bundle is trimmed; admin caps ride the `workspace-admin` role through
  `resolve_caps` (`authz-grants-scope.md`).
- **Placement:** either — token minting/verifying is symmetric (edge verifies offline with the
  public key per §6.6). The credential *store* is workspace/identity data → cloud-authoritative
  with an edge read-cache (§6.8); an edge can verify a cached password hash offline. No
  `if cloud {…}`; the `CredentialCheck` impl is selected by config/role, not a code branch.
- **MCP surface:** minimal. `login` stays a plain gateway route (not an MCP verb — it *issues*
  the token everything else is gated by; it cannot itself be capability-gated). Admin management
  of credentials (set/rotate a user's password hash) is an **admin-gated MCP verb**
  (`identity.set_credential`, gated `mcp:identity.manage:call` which already exists) so it goes
  through the same mediated path as every other admin action. API shape (§6.1): a single
  **create/update** verb (`identity.set_credential`) — no list (never enumerate secrets), no
  live-feed, no batch. `login` itself is a bespoke unauthenticated route by nature.
- **Data (SurrealDB):** a per-`(workspace, user)` credential record holding an **argon2 hash**
  (never a plaintext), in the identity directory. State, not motion. The hash is secret material
  → see Secrets below. No new datastore.
- **Bus (Zenoh):** N/A — login is a request/response; no message class involved. (Token-on-the-bus
  for node enrollment is `edge-trust-scope.md`, unaffected.)
- **Sync / authority:** credential records are shared identity data → cloud-authoritative, edge
  read-cache (§6.8); edge-originated credential writes queue through the outbox to the hub. Offline
  edge can *verify* against the cache but *set* queues.
- **Secrets:** the password hash is secret-class. It is written only via the mediated admin verb,
  stored hashed (argon2, per-workspace), and **never** returned by any read — not in `caps`, not
  in a token, not in a log, not in `identity.*` reads (mirrors the §6.7 "values never reach a log/
  page/list" rule). The dev-shim path stores/handles no secret.

## Example flow

**A — the escalation, closed (change 1):**
1. Admin `ada` adds `user:bob` as a plain member (`members.add`, role `member`).
2. `bob` logs in. `login` resolves bob's role = `member`, mints `caps = trimmed_member_caps ∪
   resolve_caps(acme, bob)`. Bob holds no admin grants, so no admin caps are in the token.
3. Bob calls `POST /admin/members` → the route's `mcp:members.add:call` check finds the cap
   absent → **`403`** (today: `204`). Same for `teams.manage`, `grants.assign`, `workspace.*`.
4. Bob calls `dashboard.get` for a dashboard shared to his team → still `200`. Member reach intact.

**B — real credential (change 2):**
1. Admin sets bob's password: `identity.set_credential {user, secret}` (gated
   `mcp:identity.manage:call`) → argon2 hash written to the workspace's identity directory.
2. `POST /login {user:"user:bob", workspace:"acme", secret:"…"}` → `PasswordHash::verify` checks
   argon2 → pass → token minted (with role-correct caps from A). Wrong secret → **`401`**, no token.
3. Local dev: `LB_DEV_LOGIN=1 make dev` selects `DevTrustAny` → today's password-less login, but
   *still role-scoped* — a dev "member" login still can't add members.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Capability deny-tests (required, and the headline here):** a `member`-role token is denied
  `members.add`, `teams.manage`, `grants.assign`, `user.manage`, `workspace.create`,
  `workspace.delete`, `dashboard.delete_any` at the **route/MCP layer** (not just missing from
  the `caps` list) — this is the regression test for the exact live finding (bob's `204`s must
  become `403`s). A `workspace-admin` token still passes all of them.
- **Workspace-isolation (required):** a credential/token for `(user, acme)` cannot mint or
  authorize anything in `beta`; a password set in `acme` does not authenticate in `beta`.
- **Credential check:** correct secret → `200` + token; wrong/absent secret → `401`, **no token
  minted**; `DevTrustAny` only reachable under the explicit dev flag (default build refuses
  password-less login).
- **First-principal bootstrap:** first login into an empty workspace still yields a
  `workspace-admin` (via the role-grant path), and that admin *can* do the admin verbs — proving
  we tightened members without breaking admin bootstrap.
- **No mocks (CLAUDE §9):** all of the above run against the **real** gateway + SurrealDB (`mem://`
  or the dev store) with real seeded identity/credential records. Argon2 is a real dependency, not
  faked. The only permitted fake is a real external IdP (a provider OIDC endpoint) behind the
  `CredentialCheck` trait in one named file — not needed until the OIDC impl, and flagged there.
- **Regression entry:** log the escalation under `debugging/auth-caps/member-token-carries-admin-caps.md`
  with the reproduction (bob adds member / self-grants) and the fix, per `debugging-scope.md`.

## Risks & hard problems

- **Trimming the member bundle is load-bearing and easy to over-trim.** `credentials.rs` has
  several comments documenting caps that are *individually required* because the `mcp:*.<verb>:call`
  wildcards don't cover suffixes like `.catalog`/`.pin` (see the existing `dev_login_carries_the_
  widget_catalog_read` test). Removing admin caps must not remove these member-essential ones — the
  existing unit tests over `member_caps()` are the guardrail; extend them, don't delete them.
- **Bootstrap chicken-and-egg:** the first principal into an empty workspace must become admin, but
  we're removing the blanket admin bundle. The bootstrap must *grant the `workspace-admin` role*
  during `membership_login_resolve` so `resolve_caps` then yields admin caps — get the ordering
  right or the first login mints a powerless token (the mirror of the bug the login.rs comments
  already warn about, where a prefix mismatch made an admin resolve to *no* caps).
- **Every existing dev/E2E test currently relies on the all-powerful member token.** Trimming will
  turn some green tests red because they were exercising admin verbs on a "member" login — that's
  the point, but it means an audit of the test suite, reseeding admins where a test genuinely needs
  one (`signInWithCaps`/seed an admin), not widening the bundle back.
- **Argon2 cost vs. login latency** on edge devices; pick params deliberately.

## Open questions

- **Where does the role→cap mapping live at issuance?** Options: (a) `login` resolves role, then
  unions a small hardcoded `member_caps` with `resolve_caps` (least change); (b) all caps —
  including member baseline — come from role grants seeded at workspace creation, so `dev_claims`
  carries *no* hardcoded caps. (b) is cleaner (one source of truth) but needs the seed to grant a
  `member` role bundle. Recommend (a) for the urgent fix, (b) as the follow-through.
- **Exact trimmed member baseline:** enumerate which of today's `member_caps()` entries are truly
  member-level (channels pub/sub, inbox/outbox member verbs, nav member reads, dashboard reads,
  their own prefs/pins) vs. admin (the six named above + any others). Needs a line-by-line pass
  against `authz-grants-scope.md`'s role catalog.
- **Dev-login default:** hard-refuse password-less login unless `LB_DEV_LOGIN=1`, or warn-and-allow
  with a loud log? Recommend hard-refuse in release builds, allow under the flag in dev/CI.
- **Credential store shape:** a dedicated `credential` table vs. a field on the identity record —
  and how it rides the §6.8 cloud-authoritative sync (does an edge ever *set* a password offline,
  or only verify against cache?).

## Related

- README `§6.6` (Identity, auth & capabilities — the RBAC hierarchy and OIDC-login intent this
  restores), `§3.5` (capability-first, enforcement order), `§7` (the workspace wall), `§6.7`
  (Secrets — how the password hash is mediated).
- Sibling scope: `auth-caps-scope.md` (the grammar + the deferred-OIDC non-goal this picks up),
  `authz-grants-scope.md` (the durable role/grant catalog this issuance must honor),
  `global-identity-scope.md` (membership resolves login; where the credential record joins the
  identity directory), `admin-crud-scope.md` (disable-bites-login, the admin management surface),
  `edge-trust-scope.md` (token-on-the-bus / offline verify — unaffected but adjacent).
- Skill: `skills/auth-caps/SKILL.md` — the implementing session **must** update it (the login
  section currently documents the password-less dev-login as *the* login; it needs the credentialed
  path, the `401` on bad secret, the `LB_DEV_LOGIN` dev flag, and the corrected "a member is not an
  admin" note). A stale skill here is a finding.
- Source: `rust/role/gateway/src/routes/login.rs`, `rust/role/gateway/src/session/credentials.rs`
  (`dev_claims` / `member_caps`), `rust/crates/host/src/authz/` (`resolve_caps`), `rust/crates/auth/`.
