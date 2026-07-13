//! The **subject-scoped** `bus.watch` gate (bus-watch-subject-scope, issue #49). Runs AFTER the
//! coarse `mcp:bus.watch:call` capability gate ([`authorize_bus`](super::authorize::authorize_bus))
//! and narrows it per subject — converging the generic `bus.watch` path with the channel service's
//! `bus:chan/*:sub` subject-cap grammar onto ONE subject-scoped cap model (`Surface::Bus`, a
//! wildcard-capable resource, action [`Action::Watch`]).
//!
//! The rule is "present ⇒ required, absent ⇒ open" — the same additive-narrowing idiom the
//! entity-scoped `{table,ids}` grants use (`Scope::All` default = today's behaviour), applied to the
//! subject-cap grammar instead of record rows (a bus subject is a *string*, not a `{table,id}` pair):
//!
//!   - Resolve the caller's **live** caps from the store (fresh — not the token — so a
//!     `grants.revoke` takes effect on the next check, which is what the revoke-terminates-stream
//!     re-check (Gap 2) relies on, and a `grants.assign` after login authorizes without a re-mint).
//!   - If the caller holds **no** `bus:*:watch` grant ⇒ **allow** (backward-compatible: every
//!     existing caller and every unscoped subject behaves exactly as today).
//!   - If the caller holds **at least one** `bus:*:watch` grant ⇒ **require** one that matches this
//!     subject (`bus:care.feed.*:watch` and `bus:care.feed.leo:watch` both authorize
//!     `care.feed.leo`, via the same `/`- and `.`-segment wildcard grammar the whole cap system uses).
//!
//! Workspace isolation is NOT re-checked here — it is Gate 1, already enforced by the coarse
//! `authorize_bus` before this runs, so a cross-workspace subject never reaches this function.

use lb_auth::Principal;
use lb_caps::{matches, Action, Request, Surface};
use lb_store::Store;

use super::error::BusError;
use crate::authz::resolve_caps_live;

/// The action a subject-scoped bus-watch grant authorizes. The cap string is `bus:<subject>:watch`.
const WATCH: Action = Action::Watch;

/// The authorization mode a `bus.watch` subscribe was allowed under — the discriminant the
/// stream-lifetime re-check (Gap 2) needs so a revoke of the *last* grant cannot silently re-open a
/// subject in back-compat mode. Returned by [`authorize_subject_scoped`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchMode {
    /// The caller held no `bus:*:watch` grant at all — today's behaviour, every subject reachable.
    /// A subsequently-added scoped grant that does NOT match this subject tightens an OPEN stream to
    /// denied (the re-check catches it), so open mode is not a permanent bypass.
    Open,
    /// The subscribe was authorized by a matching `bus:<subject>:watch` grant. The re-check requires
    /// that grant to PERSIST — revoking it closes the stream (Gap 2) and, because the requirement is
    /// anchored to the grant (not re-derived from "does any grant exist"), revoking the caller's last
    /// grant denies rather than re-opening the subject.
    Scoped,
}

/// Authorize `subject` for `principal` in `ws` against the caller's live subject-scoped bus grants.
/// Call AFTER the coarse `mcp:bus.watch:call` gate. Returns the [`WatchMode`] it was allowed under
/// (for the stream re-check), or opaque [`BusError::Denied`].
///
/// Rule: a caller with **no** `bus:*:watch` grant is [`WatchMode::Open`] (back-compat — every
/// subject reachable). A caller with **any** such grant is in scoped enforcement: this subject needs
/// a matching grant → [`WatchMode::Scoped`], else `Denied`.
///
/// `subject` is the caller-supplied subject BEFORE the `ext/` wall — the grant grammar names the
/// subject the caller asked for (`care.feed.leo`), not the internal `ext/care.feed.leo` key, so the
/// grant an extension mints reads the same as the subject it watches.
pub async fn authorize_subject_scoped(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &str,
) -> Result<WatchMode, BusError> {
    let user = bare_user(principal.sub());
    let caps = resolve_caps_live(store, ws, user)
        .await
        .map_err(|e| BusError::Bus(e.to_string()))?;

    let holds_any = holds_any_watch_grant(&caps);
    let matches_subject = matches(&caps, &Request::new(ws, Surface::Bus, subject, WATCH));

    match (holds_any, matches_subject) {
        // No `bus:*:watch` grant at all ⇒ today's behaviour (open).
        (false, _) => Ok(WatchMode::Open),
        // Holds a watch grant AND one matches this subject ⇒ scoped-authorized.
        (true, true) => Ok(WatchMode::Scoped),
        // Holds a watch grant but NONE matches this subject ⇒ denied (Gap 1).
        (true, false) => Err(BusError::Denied),
    }
}

/// Does `principal` STILL hold a `bus:<subject>:watch` grant matching `subject` in `ws`? The
/// stream-lifetime predicate for a [`WatchMode::Scoped`] stream: reads live grants, so a
/// `grants.revoke` flips this to `false` and the re-check closes the stream (Gap 2). Anchored to the
/// grant (not "any grant exists"), so revoking the last grant denies — it never re-opens the subject.
pub async fn still_scoped_authorized(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &str,
) -> Result<bool, BusError> {
    let user = bare_user(principal.sub());
    let caps = resolve_caps_live(store, ws, user)
        .await
        .map_err(|e| BusError::Bus(e.to_string()))?;
    Ok(matches(
        &caps,
        &Request::new(ws, Surface::Bus, subject, WATCH),
    ))
}

/// Does `caps` contain any `bus:<resource>:watch` grant (any resource)? A `bus:*:watch` /
/// `bus:**:watch` grant counts — it is a scoped grant that happens to match every subject, so the
/// caller is in scoped mode (and, being a match, is allowed). Parsed with the same grammar the gate
/// uses; unparseable strings are ignored (deny-by-default).
fn holds_any_watch_grant(caps: &[String]) -> bool {
    caps.iter()
        .filter_map(|s| lb_caps::Capability::parse(s).ok())
        .any(|c| c.surface == Surface::Bus && matches!(c.action, Action::Watch))
}

/// Strip the `user:` prefix so the resolver sees the bare name grants are stored under
/// (`Subject::User("ada")`, not `"user:ada"`). Mirrors `authz::scoped::bare_user`. A non-`user:`
/// sub (an api key) is returned as-is — it has no subject-scoped bus grants in v1, so
/// [`holds_any_watch_grant`] is false and it stays in back-compat mode.
fn bare_user(sub: &str) -> &str {
    sub.strip_prefix("user:").unwrap_or(sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_watch_grant_is_open_mode() {
        // A caller with only the coarse cap (and unrelated grants) is NOT in scoped mode.
        let caps = vec![
            "mcp:bus.watch:call".to_string(),
            "store:media/x:read".to_string(),
        ];
        assert!(!holds_any_watch_grant(&caps));
    }

    #[test]
    fn exact_watch_grant_is_scoped_mode_and_matches_itself() {
        let caps = vec!["bus:care.feed.leo:watch".to_string()];
        assert!(holds_any_watch_grant(&caps));
        let req = Request::new("ws1", Surface::Bus, "care.feed.leo", WATCH);
        assert!(matches(&caps, &req));
        let other = Request::new("ws1", Surface::Bus, "care.feed.mia", WATCH);
        assert!(!matches(&caps, &other));
    }

    #[test]
    fn wildcard_watch_grant_matches_prefix_only() {
        let caps = vec!["bus:care.feed.*:watch".to_string()];
        assert!(holds_any_watch_grant(&caps));
        assert!(matches(
            &caps,
            &Request::new("ws1", Surface::Bus, "care.feed.leo", WATCH)
        ));
        // `*` is one segment — it does NOT cross into another feed namespace.
        assert!(!matches(
            &caps,
            &Request::new("ws1", Surface::Bus, "other.feed.leo", WATCH)
        ));
    }

    #[test]
    fn channel_sub_grant_is_not_a_watch_grant() {
        // A channel `:sub` grant must NOT flip the generic watch path into scoped mode — the two
        // actions are distinct (Action::Sub vs Action::Watch), so they never alias.
        let caps = vec!["bus:chan/general:sub".to_string()];
        assert!(!holds_any_watch_grant(&caps));
    }

    #[test]
    fn bare_user_strips_prefix() {
        assert_eq!(bare_user("user:ada"), "ada");
        assert_eq!(bare_user("key:k7"), "key:k7");
    }
}
