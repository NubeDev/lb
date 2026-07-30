# A cleartext SMTP session ignored its own timeout and hung the relay for 30 seconds

- Area: inbox-outbox (email transport)
- Status: fixed
- First seen: 2026-07-30 (while building the transport — caught by the test written for it)
- Resolved: 2026-07-30
- Session: ../../sessions/inbox-outbox/email-transport-session.md
- Regression test: `rust/crates/mail/tests/smtp_send_test.rs`
  (`a_hung_session_times_out_instead_of_stalling_the_relay`)

## Symptom

A test that scripts the one server behaviour a per-send timeout exists for — accept the TCP connection
and then say **nothing at all** — did not fail the send after the configured 300 ms. It returned after
**30.02 s**, when the *test server's* own sleep expired:

```
thread 'a_hung_session_times_out_instead_of_stalling_the_relay' panicked at
  the send did not respect its timeout (30.023297949s)
```

The `SmtpEndpoint` carried `timeout: 300ms`, and that timeout was handed to `mail-send`'s
`SmtpClientBuilder::timeout(..)`. It had no effect.

## Root cause

`mail-send` bounds *some* of a session, not all of it:

- `SmtpClientBuilder::connect()` (implicit TLS and STARTTLS) wraps the **whole** handshake — TCP,
  TLS, greeting read, EHLO, AUTH — in `tokio::time::timeout`;
- `SmtpClient::cmd()` wraps each command/response round-trip;
- `SmtpClientBuilder::connect_plain()` — the cleartext path — wraps **only the TCP connect**, then calls
  `client.read()` for the greeting. `read()` has no timeout of its own.

So exactly one shape hangs forever: no-TLS submission to a server that accepts the socket and never
greets. A half-open socket, a wedged LAN relay, or a TCP-level proxy holding the connection produces it.

Why it matters more than a slow test: `send_smtp` is called **inside the outbox relay tick**. One hung
session stalls that pass, and every other effect behind it — push notifications included — waits.
"Invites are slow" and "notifications stopped" would have looked like unrelated reports.

## Fix

Stop delegating the guarantee. `send_smtp` now wraps the entire submission in its own
`tokio::time::timeout(endpoint.timeout, …)`, with the library's timeout still set underneath (it produces
better-shaped errors when it does fire):

```rust
pub async fn send_smtp(...) -> MailResult<()> {
    tokio::time::timeout(timeout, submit(endpoint, credentials, message))
        .await
        .unwrap_or_else(|_| Err(MailError::Transient(format!(
            "smtp: session exceeded the {}s timeout", timeout.as_secs_f32()))))
}
```

A timeout classifies as **transient**, so the outbox backs off and retries — a wedged relay is not a
reason to park the mail.

## Lesson

A timeout configured on a dependency is a *claim*, not a mechanism, until something proves it fires. This
one was set correctly, threaded correctly, and did nothing on the code path the tests would otherwise
never have exercised — the plaintext path is the one a LAN relay uses, i.e. the deployment least likely
to have anyone watching.

The test that caught it is the cheap kind worth writing every time an external is involved: script the
pathological server (accept, then silence) rather than only the failing one (refuse, or reply `5xx`). A
connection *refused* returns instantly and would have passed happily.

## Related

- `rust/crates/mail/src/send/smtp.rs` (the wrapper + the comment explaining why it is ours)
- Scope: `docs/scope/inbox-outbox/email-transport-scope.md` (Risks — "Blocking the relay reactor")
