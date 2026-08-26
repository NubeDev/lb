# A workspace admin could register a mailbox and then could not seal its password

- Area: inbox-outbox (mail source) / auth-caps
- Status: fixed
- First seen: 2026-08-26 (first live registration of a mailbox on a running node)
- Resolved: 2026-08-26
- Session: ../../sessions/inbox-outbox/mail-source-session.md
- Regression coverage: `rust/crates/host/src/authz/builtin_roles.rs` unit tests (the admin bundle
  membership assertions) + the live drive in the session doc.

## Symptom

On a live node, a `workspace-admin` token holding every `mcp:mail.source.*:call` capability:

```
$ mcp secret.set '{"path":"mail/meter-mailbox","value":"…","visibility":"workspace"}'
denied
```

…while the very next call succeeded:

```
$ mcp mail.source.register '{"id":"meter-data", …, "secretPath":"mail/meter-mailbox"}'
{"source": { … }}
```

So the admin could register a mailbox that points at a secret path, and could not put anything at
that path. The source then failed every poll with `no credential at secret path
'mail/meter-mailbox'` — a correctly-configured feature that could not be configured.

## Root cause

`mcp:secret.set:call` clears the MCP gate, but `lb_secrets::set` re-checks a **per-path**
`secret:<path>:write` inside. The admin bundle grants exactly three prefixes:

```rust
"secret:agent/*:write",
"secret:federation/*:write",
"secret:webhook/*:write",
```

`mail/…` is not among them, and — the part that makes this a recurring shape — the generic
`store:*:write` / `secret:*:write` wildcards are **single-segment**: they do not span a `mail/foo`
resource path. This is the identical trap `store:media/{id}:read` records in `builtin_roles.rs`:
a verb cap that reaches nothing because the resource cap beside it was never named.

## Fix

`"secret:mail/*:write"` added to `ADMIN_ONLY_CAPS`, beside `mcp:secret.set:call` — the same tier as
`mcp:mail.source.register:call`, because sealing a mailbox credential is exactly as privileged as
pointing a poller at that mailbox.

## Lesson

**A feature that resolves a credential by path must ship the write grant for that path prefix in
the same slice.** The secrets surface is two gates deep (`mcp:secret.set:call`, then
`secret:<path>:write`), and every new credential-consuming feature adds a *third* thing that has to
line up: the path prefix its config will name. Adding the consumer's caps without the producer's
grant leaves a feature that is complete, tested, and unusable — and the suite cannot see it,
because tests seal secrets with a hand-minted principal carrying whatever cap the test needs.

Checklist for the next credential-by-path feature: (1) the verb caps, (2) the `secret:<prefix>/*:write`
grant in the role bundle that will use it, (3) a live seal-then-use run.
