//! Role: the **GitHub outbox `Target`** — the real HTTP delivery adapter for the transactional
//! outbox's egress edge. The host owns the `Target` trait (the relay's delivery seam); this crate
//! supplies the concrete GitHub-REST impl that opens PRs and posts comments, filling the last mock
//! behind that seam (the in-test target was the only stub, outbox scope).
//!
//! It is the egress counterpart to `lb-role-github-webhook`'s ingress: a webhook delivery comes IN
//! through `ingest_via_bridge`; an outbox effect goes OUT through this `Target`. Both keep their
//! HTTP/network dependency (`reqwest`) in a role crate, never in core `lb-host` (roles depend on
//! host, never the reverse), exactly like `HttpSource` in `lb-role-registry-host`.
//!
//! The outbox contract holds over the wire: at-least-once delivery + receiver dedup → effectively
//! once. For `create_pr`, GitHub's own `422 "already exists"` is the dedup oracle (a re-delivery is a
//! no-op, never a second PR); the relay's backoff + dead-letter (new this slice) stop a perpetually
//! failing effect from retrying forever. See `../../scope/inbox-outbox/outbox-scope.md`.

mod client;
mod request;

pub use client::GithubTarget;
pub use request::TARGET;
