# minimal-shell: every SSE subscription silently 401'd (subscribe POST unauthenticated)

**Area:** frontend / minimal-shell · **Date:** 2026-07-11 · **Found in:** peer review of 3c20433

## Symptom

The minimal shell's event hub opened the stream fine (`GET /events/stream?token=` — token in
the query, because `EventSource` can't set headers) but no subject ever delivered a frame.

## Cause

`POST /events/{sid}/subscribe` is **header-authed** (`rust/role/gateway/src/routes/events.rs`
— "Header-authed; the subject's gate re-runs server-side"). `packages/minimal-shell/src/events.ts`
sent the subscribe/re-declare POSTs with only `Content-Type`, so the gateway answered 401 and
the fire-and-forget `fetch` swallowed it. The stream stayed open and healthy-looking with zero
subjects attached — a silent no-op.

## Fix

Spread `authHeaders()` into both subscribe call sites (the `hello` re-declare loop and
`subscribe()`).

Two sibling shell bugs fixed in the same pass (regression-tested in `App.test.tsx`):
- 401 handling cleared `lb.session` from localStorage without notifying the session store —
  UI stayed "logged in" until reload. Now `ipc.ts` fires `lb.session.cleared` and `session.ts`
  re-emits.
- `getSession` returned a fresh `JSON.parse` object per call — `useSyncExternalStore` snapshot
  instability (React "getSnapshot should be cached" loop) once a session exists. Now cached by
  raw string.

## Lesson

The events API is deliberately split-auth (query-token stream, header-authed control verbs);
any new client must copy both halves. A fire-and-forget `fetch` on a control verb hides the
deny — log or surface non-OK responses.
