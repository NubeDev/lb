# Invite accept: credential written before redemption claimed (double-redeem race)

- **Date:** 2026-07-11
- **Area:** auth-caps (invites)
- **Status:** fixed
- **Found by:** peer review of the invites slice (branch `updates-to-core`)

## Symptom

Two concurrent `POST /public/invite/accept` calls on the same token could BOTH pass: the loser's
password overwrote the winner's, so the winner's freshly-set credential silently stopped working
— an unauthenticated caller could reset the invitee's password after redemption.

## Root cause

Ordering + a non-atomic mark. `invites/accept.rs` ran the whole onboarding chain (identity,
**credential write**, membership, grants) FIRST and only then called
`invite_mark_accepted_raw`, which was a plain read → `is_redeemable` → write. Two accepts
interleaving between the read and the write both saw `pending`, both "claimed", and both had
already written their credential — last writer wins. The single-use guarantee lived in a
read-modify-write with no store-level conditionality.

## Fix

Claim-first with a store-level CAS, mutate second:

- `authz/src/invite.rs` — `invite_mark_accepted_raw` now takes an **atomic redemption claim**
  before touching the invite record: `lb_store::create` of an `invite_claim:{token_hash}` row
  (`INVITE_CLAIM_TABLE`). SurrealDB `CREATE` errors on a duplicate id (`StoreError::Conflict`),
  so exactly ONE caller ever gets `true` — the same first-settle primitive the agent's Ask
  decision uses. Added `invite_release_claim_raw` (winner-only, sub-checked) so a post-claim
  onboarding failure returns the invite to `pending` for idempotent retry.
- `host/src/invites/accept.rs` — reordered: verify token + takeover protection (**reads only**)
  → **claim redemption** → onboarding mutations (`onboard()`, one rollback site: release the
  claim on `Err`) → mint session. A concurrent loser is now rejected BEFORE any credential or
  membership mutation.

## Regression test

`host/tests/invites_hardening_test.rs::double_redeem_loses_before_credential_mutation` — second
accept fails with `AlreadyAccepted`, then asserts the winner's password still verifies
(`CredentialCheck::Ok`) and the loser's never took (`BadSecret`). Sequential, but the claim is a
store-level conditional CREATE, so a truly concurrent loser takes the identical reject path —
the ordering is asserted by construction.

## Lesson

A "single-use" guarantee enforced by read-check-write is not single-use. Any pre-auth verb that
mutates credentials must take its uniqueness claim through a store-atomic primitive
(`lb_store::create` first-settle / `increment`) BEFORE the first mutation, not after the last.
