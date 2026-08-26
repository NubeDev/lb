# Inbox / outbox

Shipped so far:

- **[Email transport](./email-transport)** — a real mailer behind the outbox's `email` target: SMTP
  (TLS, `PLAIN`/`LOGIN`/`XOAUTH2` with token refresh) or the Postmark API, selected by name in boot
  config, with credentials resolved per send from `secrets/` and honest transient-vs-permanent delivery
  outcomes.
- **[Mail source](./mail-source)** — the receive half: a watched IMAP mailbox whose arriving messages
  become workspace assets (the raw message and every attachment), series samples (attachments decoded
  through an opaque format registry), and an item in the lb inbox. Credentials by secrets path, a
  sender allowlist, and a per-message ledger that makes re-delivery a no-op.

TODO: the rest is filled as the features ship. See `docs/scope/inbox-outbox/` for the asks
(`inbox-outbox-scope.md`, `outbox-scope.md`, `push-target-scope.md`, `mail-source-scope.md`,
`email-transport-scope.md`).
