# Auth-caps (+ files, inbox-outbox, frontend) — cc-app platform-gap scopes (session)

- Date: 2026-07-11
- Scope: the five scopes this session *wrote* (it is a scope-setup session, docs only)
- Status: done

## Goal

The cc-app childcare product (a downstream embedder, `~/code/rust/cc-app`) surfaced seven
platform gaps. Turn the real ones into proper lb scopes so they're built **generically in
lb**, never product-side; fold the ones existing scopes already cover.

## What changed

Five new scope docs + `scope/README.md` index entries for each:

- `scope/auth-caps/entity-scoped-grants-scope.md` — row-level reach: additive `scope`
  selector on the authz-grants record, `check_scoped`/`scope_filter`, SDK host-callback.
- `scope/auth-caps/invites-scope.md` — single-use invite records (role/team intent +
  opaque payload), outbox email delivery, one pre-auth accept route, atomic onboarding.
- `scope/files/media-scope.md` — resumable chunked upload, variant jobs, streaming
  capability-checked serve; SurrealDB buckets (rule 2 intact).
- `scope/inbox-outbox/push-target-scope.md` — push as an outbox `Target`: device records,
  FCM/APNs/WebPush behind one `PushProvider` trait, prefs quiet-hours gate.
- `scope/frontend/minimal-shell-scope.md` — the publishable minimal host for
  100%-extension UIs; retires vendor-the-whole-shell (the rubix-ai compromise).

## Decisions & alternatives

- Gap "kiosk/device principals" → **no new scope**: `api-keys-scope.md` already covers it.
- Gap "cap refresh without re-login" → **no new scope**: `builtin-role-freshness` +
  invites' caps-live-on-first-login + the access-console freshness levers carry it.
- entity-scoped grants: selector-on-grant over a "team per entity" pattern (team explosion,
  console noise, no query-side filter) and over ABAC/policy language (surface, auditability).
- media: chunk protocol over raise-body-limits (the 413 history) and over an external blob
  store (rule 2).
- push: `Target` over a standalone notifier service (the outbox already owns durability).

## Tests

N/A — scope docs only. Each scope names its mandatory deny/isolation categories; the
cross-family matrix in cc-app's `care-authz-scope.md` is the downstream acceptance harness.

## Debugging

None.

## Public / scope updates

`scope/README.md` bullets extended (auth-caps, files, inbox-outbox, frontend). Public
stubs deferred, matching the repo's existing practice for these topics.

## Follow-ups

- Build order: entity-scoped-grants first (cc-app's blocker), then invites → media →
  push-target → minimal-shell; each per HOW-TO-CODE with PR + `node-v*` tag.
- First consumer + acceptance context: `cc-app` `docs/scope/care/care-scope.md` (§lb gaps,
  §Scope map).
