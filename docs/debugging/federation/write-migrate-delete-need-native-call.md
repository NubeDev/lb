# `federation.write` / `migrate` / `delete` demanded the native control-plane cap

- Date: 2026-10-06
- Area: federation (`host/src/federation/{write,migrate,delete}.rs`)
- Status: fixed

## Symptom

An extension granted `mcp:federation.write:call` / `mcp:federation.migrate:call` was still refused
(`denied`) unless it ALSO held `mcp:native.call:call` — the supervisor control-plane cap that lets a
caller drive any native child. Found designing an extension that creates and writes its own tables
through federation.

## Root cause

`federation.query` was moved to `native::call_sidecar_mediated` (no control-plane check, the verb's own
cap is the gate) — and that function's doc names `federation.write` as a mediated verb — but `write`,
`migrate` and `delete` still dispatched through `native::call_sidecar`, which checks
`mcp:native.call:call` first.

## Fix

All three dispatch through `call_sidecar_mediated`. Each still authorizes on its OWN cap first, so the
gate is not weakened (a reader still cannot write, migrate or delete).

## Regression test

`host/tests/federation_mediated_writes_test.rs` — a caller with only the data verbs migrates, writes
and deletes (failed before: verified); a caller without them is still denied on each.
