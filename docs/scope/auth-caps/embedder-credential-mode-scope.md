# Auth-caps scope — embedder credential mode (BootConfig-selectable login check)

Status: scope (the ask). Promotes to `doc-site/content/public/auth-caps/` once shipped.

`login-hardening-scope.md` gave the gateway a **pluggable credential check** — `DevTrustAny`
(password-less, opt-in) and `PasswordHash` (argon2 against the stored `(ws, user)` credential),
selected at the standalone binary boundary by `credential_check_from_env()` (`LB_DEV_LOGIN`).
That selection **only reaches the standalone `Gateway::boot()` path**. The embed seam —
`lb_node::boot_full(BootConfig)`, used by every embedder (`rubix-ai`, `cc-app`) — builds its
gateway with `Gateway::new_live()` (`rust/node/src/builder.rs`), which **hardwires
`DevTrustAny`** and deliberately does *not* call `Gateway::boot()` (that would open a second
store handle — see the builder module doc). `BootConfig` carries no credential-mode field.

**The consequence, verified live on an embedded node:** a `boot_full` gateway accepts *any*
password. `POST /login {user, workspace, secret:"WRONG"}` returns `200` with a valid token — the
argon2 credential that `invite.accept` / `identity.set_credential` writes is never checked. An
embedder therefore **cannot run real password login at all**; the entire `PasswordHash` half of
login-hardening is unreachable below the embed seam. This scope closes that: it makes the
credential check an **embedder-selectable `BootConfig` field**, applied through the credential-check
seam that already exists — no new check, no route change, no second store handle.

## Goals

- **`boot_full` can run `PasswordHash`.** Add an additive `credential_mode` to `BootConfig` so an
  embedder selects the same argon2 check the standalone binary gets from `LB_DEV_LOGIN` unset. A
  wrong/absent secret then `401`s on an embedded node exactly as it does on the standalone one.
- **Reuse the existing seam.** Apply the mode through `Gateway::with_credential_check(...)` in
  `builder.rs` — the builder method login-hardening already added for exactly this. No new trait,
  no new impl, no change to `login.rs` or any route.
- **A `PasswordHash` node can bootstrap a first admin.** `identity.set_credential` is admin-gated,
  but no admin can authenticate until a credential exists — the bootstrap paradox. An additive
  `BootConfig::seed_credential` argon2-seeds the dev [`seed_user`]'s credential at boot (alongside
  the existing membership seed), so the first admin can log in with a real password. `None` (the
  default) seeds nothing — correct for `DevTrustAny` nodes and for embedders that provision
  credentials their own way.
- **`boot_full`'s default is unchanged (back-compat).** The field defaults to `DevTrustAny`, so
  every existing embedder and every `boot_full`-based test keeps today's password-less behaviour
  until it opts in. `Default::default()` and `from_env()` both stay behaviour-preserving (see
  below).
- **The standalone binary is untouched.** `BootConfig::from_env()` maps `LB_DEV_LOGIN` → the mode
  (set/non-empty → `DevTrustAny`, unset → `PasswordHash`), reproducing today's `node` binary
  exactly. `credential_check_from_env()` stays as the standalone `Gateway::boot()`'s source of
  truth; `from_env()` merely mirrors its rule into the field.

## Non-goals

- **Any change to the credential check itself.** `DevTrustAny` / `PasswordHash` / the
  `CredentialCheck` trait / argon2 params / `identity.set_credential` are all as
  `login-hardening-scope.md` shipped them. This scope only makes the *selection* reachable from
  `BootConfig`.
- **A new login mode (OIDC, MFA, magic-link).** Those remain the deferred login-hardening
  non-goals; when an OIDC impl lands behind the same trait it will be selectable through the same
  field with no further embed-seam work.
- **Reworking `builder.rs`'s `new_live` vs `boot` decision.** The builder keeps `new_live` (one
  store handle — the load-bearing constraint); we add one builder call after it, we do not switch
  to `Gateway::boot()`.
- **Deciding cc-app's (or any embedder's) dev-vs-prod policy.** How an embedder chooses the mode
  (an env var, a config file, always-on in release) is the embedder's concern above its own binary
  boundary — this scope only exposes the knob.

## Intent / approach

One additive field, applied at one seam, mirrored in the two `BootConfig` constructors:

1. **`BootConfig::credential_mode`** — a small `CredentialMode` enum in `rust/node/src/config.rs`
   (`DevTrustAny` | `PasswordHash`). `#[non_exhaustive]` `BootConfig` already permits additive
   fields. `Default` → `DevTrustAny` (the embed-friendly, back-compat default, matching every
   other `boot_full` posture today). `from_env()` → derived from `LB_DEV_LOGIN` with the *same*
   rule `credential_check_from_env()` uses, so the standalone binary is byte-for-byte unchanged.

2. **Apply it in `builder.rs`.** Immediately after `let mut gw = Gateway::new_live(...)` in the
   `GatewayMode::Addr` arm, map `cfg.credential_mode` to the concrete `Arc<dyn CredentialCheck>`
   (`DevTrustAny` → `session::DevTrustAny`, `PasswordHash` → `session::PasswordHash`) and call
   `gw = gw.with_credential_check(check)`. `GatewayMode::Off` is headless (no login route) — the
   field is irrelevant there, applied only in the `Addr` arm.

   *Mapping placement — rejected alternative:* have `builder.rs` call
   `credential_check_from_env()` directly. Rejected: it re-reads env below the boot seam, breaking
   the `BootConfig` invariant that "no library code below the boot seam reads `LB_*` from env"
   (config.rs module doc). The env read stays at the binary boundary (`from_env`); the field
   carries it down.

3. **`lb-node` re-exports** whatever the embedder needs to name the mode (`CredentialMode`), same
   as it already re-exports `BootConfig` / `GatewayMode` / `SigningKey`.

Sequencing: the field + the two constructor arms + the builder apply land together (they're one
change); the test proving `PasswordHash` bites through `boot_full` lands with them.

## How it fits the core

- **Tenancy / isolation:** unchanged. The credential is still verified *within* the token's
  workspace (login-hardening §"How it fits"); this scope changes *whether* it is verified on an
  embedded node, never *across what boundary*.
- **Capabilities:** unchanged. Cap issuance (role-scoped, login-hardening change 1) is orthogonal
  and already reaches `boot_full` (it lives in `login.rs`, not the check). This scope only affects
  the credential front-door, exactly the half login-hardening left unreachable below the seam.
- **Placement:** symmetric. `BootConfig` is the embedder's boot contract; the field is selected by
  config, never a code branch on a role or a name (rule 10). Every embedder — cloud or edge — reads
  the same field.
- **MCP surface:** none. `login` stays a bespoke unauthenticated route; `identity.set_credential`
  (the admin write) is unchanged. No new verb.
- **Data (SurrealDB):** none. Uses the existing per-`(ws, user)` argon2 credential record.
- **Bus / sync / secrets:** unchanged from login-hardening — the password hash is still
  secret-class, written only via the mediated admin verb, never read back.

## Example flow

**Embedded node, `PasswordHash` selected:**
1. Embedder fills `BootConfig { credential_mode: PasswordHash, .. }` (or `from_env()` with
   `LB_DEV_LOGIN` unset) and calls `boot_full`.
2. `builder.rs` builds `Gateway::new_live(...).with_credential_check(PasswordHash)`.
3. A person accepts an invite (`POST /public/invite/accept` → argon2 credential written +
   membership + session) and sets password `p`.
4. `POST /login {user:"user:ana@x", workspace, secret:"p"}` → `PasswordHash::verify` argon2-checks
   → **`200`** + token. `secret:"WRONG"` → **`401`**, no token (today on an embedded node: `200`).

**Embedded node, default (`DevTrustAny`):** step 2 builds `.with_credential_check(DevTrustAny)`
(the explicit default; today it is the implicit hardwired value) → `POST /login` with any secret
`200`s. Byte-for-byte today's behaviour — no embedder that doesn't opt in is affected.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Credential check through `boot_full` (the headline):** boot an embedded node
  (`BootConfig { gateway: Addr, credential_mode: PasswordHash, .. }`, real gateway + `mem://`
  store), seed a `(ws, user)` argon2 credential via the real `identity.set_credential` path (or an
  invite-accept), then over the real `POST /login`: correct secret → `200` + token; wrong secret →
  `401`, **no token**; absent credential → `401`. This is the exact regression for the live
  finding (`WRONG` currently `200`s on an embedded node).
- **Back-compat default:** a `boot_full` node built with `BootConfig::default()` (or
  `credential_mode: DevTrustAny`) still password-less-`200`s — proving no existing embedder breaks.
- **`from_env` parity:** `BootConfig::from_env()` with `LB_DEV_LOGIN` unset yields `PasswordHash`;
  set/non-empty yields `DevTrustAny` — matching `credential_check_from_env()` so the standalone
  binary is unchanged (assert the field, and that the standalone login behaviour is untouched).
- **Workspace-isolation (reaffirmed, not re-implemented):** a password set in `acme` does not
  authenticate in `beta` on an embedded node — inherited from login-hardening; assert it still
  holds once the check is reachable through the seam.
- **No mocks (CLAUDE §9):** all against the real gateway + real SurrealDB + real argon2, seeded via
  the real write path. No fake credential store.
- **Regression entry:** log the embed-seam gap under
  `debugging/auth-caps/boot-full-hardwires-devtrustany.md` with the live reproduction (wrong
  password `200`s on a `boot_full` node) and the fix, per `debugging-scope.md`.

## Risks & hard problems

- **The default must stay `DevTrustAny` or every `boot_full`-based test that logs in
  password-less turns red.** Unlike the standalone binary (where login-hardening made
  `PasswordHash` the *unset* default, a deliberate prod-safe choice), the embed default must
  preserve `boot_full`'s current behaviour. `Default::default()` = `DevTrustAny`; only
  `from_env()` mirrors the `LB_DEV_LOGIN`-unset → `PasswordHash` rule (because `from_env` exists to
  reproduce the standalone binary). Getting these two constructors' defaults *different on purpose*
  is the subtle part — document it at the field.
- **One store handle stays load-bearing.** The fix must not reintroduce `Gateway::boot()` in
  `builder.rs` (double store open). Apply the mode via the builder method on the `new_live`
  instance only.
- **Env reads stay at the boundary.** Do not call `credential_check_from_env()` from `builder.rs`
  (below the seam). The env→mode mapping lives in `from_env()`; the field carries it down. This
  keeps the `BootConfig` "no env below the seam" invariant intact.
- **Absent-credential `401` in prod mode** means an embedder that flips to `PasswordHash` without
  seeding credentials for its bootstrap identities locks them out (e.g. cc-app's dev `user:ada`
  has no credential). `BootConfig::seed_credential` resolves this for the dev `seed_user` (the boot
  seed sets its argon2 credential); any *other* identity an embedder needs to log in under
  `PasswordHash` must likewise get a credential (via invite-accept, or `identity.set_credential`
  from an already-authenticated admin). The seed writes the credential raw (no principal — it IS
  the provisioning seam), mirroring the invite-accept onboarding write, so it never needs a
  chicken-and-egg admin token.

## Open questions

- **Enum vs bool.** `CredentialMode { DevTrustAny, PasswordHash }` vs a `password_login: bool`.
  Recommend the enum: it names the two impls the trait already has and extends cleanly when an
  OIDC impl lands (a third variant), where a bool would not.
- **Should `from_env()` read `LB_DEV_LOGIN` or a new embed-specific var?** Recommend reuse
  `LB_DEV_LOGIN` (one login-mode knob across standalone + embed; `from_env` exists precisely to
  reproduce the standalone binary's env contract).

## Related

- Sibling scope: `login-hardening-scope.md` (defines the `CredentialCheck` trait +
  `DevTrustAny`/`PasswordHash` + `Gateway::with_credential_check` this scope makes
  embedder-selectable; this is its embed-seam completion), `auth-caps-scope.md` (the grammar).
- README `§6.6` (identity/auth — the credentialed login this makes reachable for embedders),
  `§7` (the workspace wall — the credential is verified within it).
- Source: `rust/node/src/config.rs` (`BootConfig` / `from_env` / `Default`),
  `rust/node/src/builder.rs` (`Gateway::new_live` → `.with_credential_check`),
  `rust/role/gateway/src/session/credential.rs` (`DevTrustAny` / `PasswordHash` /
  `credential_check_from_env`), `rust/role/gateway/src/state.rs`
  (`Gateway::with_credential_check` / `boot`).
- Consumer: `cc-app` — the embedder that surfaced this (its `boot_full` node cannot enforce
  passwords); it bumps the `lb-node` pin once this ships and selects the mode at its own binary
  boundary.
