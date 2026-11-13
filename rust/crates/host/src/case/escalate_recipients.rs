//! **Who an escalation mails** — a `user:` or `team:` subject turned into login email addresses,
//! server-side (case-escalate scope).
//!
//! One responsibility: subject in, addresses out. The addresses never leave the host through a
//! reply or a history row — identities are admin-only data (`identity.list` is admin), so the verb
//! hands back only a COUNT and the addresses travel solely inside the outbox effect on their way
//! into the mail.
//!
//! - `user:x` → the identity's login email; failing that, a bare sub that is itself an address
//!   (`user:sam@example.com`, the email-login shape) is used as-is.
//! - `team:x` → every `user:` member edge of the team (both id spellings, `team_member_edges`) that
//!   is a LIVE member of this workspace, each resolved as above. A stale edge naming somebody who
//!   left the workspace is skipped: mailing a case link to a person who can no longer open it is a
//!   disclosure, not a notification.
//!
//! Addresses are lower-cased and de-duplicated. A member with no resolvable address is skipped (and
//! logged at debug); the caller decides what zero recipients means.

use lb_authz::{identity_get, membership_is_member, team_member_edges};
use lb_store::Store;

use super::error::CaseSvcError;

/// The login addresses `to` resolves to in `ws`, lower-cased, de-duplicated, in a stable order.
/// `to` must already have passed `validate_assignee`.
pub(super) async fn escalation_recipients(
    store: &Store,
    ws: &str,
    to: &str,
) -> Result<Vec<String>, CaseSvcError> {
    let users: Vec<String> = match to.split_once(':') {
        Some(("team", _)) => {
            let mut users = Vec::new();
            for edge in team_member_edges(store, ws, to).await? {
                let Some(user) = as_user(&edge) else { continue };
                if membership_is_member(store, ws, &user).await? {
                    users.push(user);
                } else {
                    tracing::debug!(ws, user = %user, "case escalate: team member is not a workspace member; not mailed");
                }
            }
            users
        }
        _ => vec![to.to_string()],
    };

    let mut out = Vec::new();
    for user in users {
        match address_of(store, &user).await? {
            Some(address) if !out.contains(&address) => out.push(address),
            Some(_) => {}
            None => {
                tracing::debug!(ws, user = %user, "case escalate: no email address; not mailed")
            }
        }
    }
    out.sort();
    Ok(out)
}

/// A member edge as a `user:` subject, or `None` for anything that is not a person (a nested team,
/// a key). Edges are written with and without the prefix, so both spellings are accepted.
fn as_user(edge: &str) -> Option<String> {
    match edge.split_once(':') {
        Some(("user", bare)) if !bare.is_empty() => Some(edge.to_string()),
        Some((_, _)) if !edge.contains('@') => None,
        _ if edge.is_empty() => None,
        _ => Some(format!("user:{edge}")),
    }
}

/// `user`'s login address: the identity's email, else the bare sub when it is an address itself.
async fn address_of(store: &Store, user: &str) -> Result<Option<String>, CaseSvcError> {
    let from_identity = identity_get(store, user)
        .await?
        .and_then(|i| i.email)
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty());
    if from_identity.is_some() {
        return Ok(from_identity);
    }
    let bare = user.strip_prefix("user:").unwrap_or(user).trim();
    Ok(bare.contains('@').then(|| bare.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::as_user;

    #[test]
    fn member_edges_normalise_to_user_subjects_and_skip_non_people() {
        assert_eq!(as_user("user:priya").as_deref(), Some("user:priya"));
        assert_eq!(as_user("priya").as_deref(), Some("user:priya"));
        assert_eq!(
            as_user("sam@example.com").as_deref(),
            Some("user:sam@example.com")
        );
        assert_eq!(as_user("team:nested"), None);
        assert_eq!(as_user("key:abc"), None);
        assert_eq!(as_user(""), None);
    }
}
