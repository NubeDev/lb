# Channels - standalone docs backfill (session)

- Date: 2026-06-28
- Scope: ../../scope/channels/channels-scope.md
- Stage: S7 collaboration docs backfill
- Status: done

## Goal

Create first-class channel docs because the shipped behavior was documented only inside the broader
collaboration slice. Capture the actual code: durable inbox-backed history, bus motion, registry
records, gateway routes, SSE, presence, UI hooks, and tests.

## What changed

- Added `scope/channels/channels-scope.md` as the topic brief reconstructed from shipped code.
- Added `public/channels/channels.md` as the durable shipped source of truth.
- Added this session note.
- Updated indexes/status/vision to make the topic discoverable.

## Decisions & alternatives

- Documented channels as two layers: message channel plus registry. The registry is list metadata, not
  the source of message truth.
- Kept capability docs on the existing `bus:chan/...` grammar because the code intentionally has no
  new registry capability.
- Called out `?token=` SSE authentication because it is a transport-specific exception that future
  work should not rediscover.

## Tests

Docs-only change. No code tests were run.

Code reviewed for the doc:

- `rust/crates/host/src/channel/`
- `rust/crates/host/src/channel_registry/`
- `rust/role/gateway/src/routes/channel_registry.rs`
- `rust/role/gateway/src/routes/post.rs`
- `rust/role/gateway/src/routes/history.rs`
- `rust/role/gateway/src/routes/stream.rs`
- `ui/src/lib/channel/`
- `ui/src/features/channel/`
- `rust/crates/host/tests/collaboration_test.rs`
- `rust/role/gateway/tests/gateway_test.rs`
- `rust/role/gateway/tests/gateway_routes_test.rs`
- `ui/src/features/channel/*.gateway.test.tsx`

## Debugging

None.

## Public / scope updates

Promoted to `public/channels/channels.md`. Added cross-links from the docs indexes, `STATUS.md`, and
the coding-agent workplace vision note.

## Dead ends / surprises

The docs had the shipped behavior in `public/frontend/collaboration.md`, but there was no topic folder
for channels. That made channel registry and live stream code hard to find from docs alone.

## Follow-ups

- Rich channel metadata.
- Short-lived stream token or cookie for SSE auth.
- Optional registry repair for imported or legacy message histories.
