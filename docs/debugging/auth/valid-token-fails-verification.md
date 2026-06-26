# A freshly-minted token fails verification with BadToken

- Area: auth
- Status: resolved
- First seen: 2026-06-26
- Resolved: 2026-06-26
- Session: ../../sessions/core/s0-s1-spine-session.md
- Regression test: rust/crates/auth/tests/token_test.rs

## Symptom

`auth::verify` returned `Err(AuthError::BadToken)` for a token that `auth::mint` had just
produced with the *same* `SigningKey`. The round-trip test failed at the verify step:

```
valid token verifies: BadToken
```

The expiry test failed downstream for the same reason (it never got past the signature).

## Reproduce

1. `SigningKey::generate()`, `mint(&key, &claims)`, `verify(&key, &token, now)`.
2. Verify fails immediately with `BadToken`, before the expiry check.

## Investigation

- Ruled out the claims/clock logic: `validate_exp` was already disabled and expiry was
  checked by hand, so the failure was the *signature*, not the timestamp.
- The signature path was `ed25519-dalek` (key material) → `jsonwebtoken` 9 (`from_ed_der`).
- jsonwebtoken 9 signs/verifies EdDSA via **ring**. ring's Ed25519 requires a **PKCS#8 v2**
  DER (private key *plus* the embedded public key). `ed25519_dalek::to_pkcs8_der()` emits
  **PKCS#8 v1** (private key only). ring rejects the v1 blob, so signing/verification never
  agree → `BadToken`. (Classic ring↔dalek PKCS#8-version mismatch.)

## Root cause

Two libraries with incompatible key encodings sat on the same seam: dalek produced PKCS#8
v1; ring (under jsonwebtoken) demanded v2. The seam itself was the defect, not either key.

## Fix

Remove the fragile seam: **sign and verify the JWT with `ed25519-dalek` directly** and drop
`jsonwebtoken`/ring from `auth`. The token is still a standard compact JWS
(`base64url(header).base64url(payload).base64url(sig)`) with `alg: EdDSA`, but we own the
encode/sign/verify with one library — no cross-library key encoding. See
`rust/crates/auth/src/{mint,verify,token}.rs`.

## Verification

`cargo test -p lb-auth` — the round-trip, expiry, wrong-key, and tamper tests pass (output
pasted in the session doc).

## Prevention

The regression test `token_test::round_trips_and_carries_the_workspace_claim` fails-before /
passes-after. Guardrail: `auth` now depends on exactly one crypto library for tokens
(`ed25519-dalek`), so this class of cross-library encoding mismatch cannot recur here. If a
JWT library is reintroduced later, a round-trip test is the canary.
