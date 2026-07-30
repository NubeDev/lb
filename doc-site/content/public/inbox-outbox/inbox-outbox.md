# Inbox / outbox

Shipped so far:

- **[Email transport](./email-transport)** — a real mailer behind the outbox's `email` target: SMTP
  (TLS, `PLAIN`/`LOGIN`/`XOAUTH2` with token refresh) or the Postmark API, selected by name in boot
  config, with credentials resolved per send from `secrets/` and honest transient-vs-permanent delivery
  outcomes.

TODO: the rest is filled as the features ship. See `docs/scope/inbox-outbox/` for the asks
(`inbox-outbox-scope.md`, `outbox-scope.md`, `push-target-scope.md`, `mail-source-scope.md`,
`email-transport-scope.md`).
