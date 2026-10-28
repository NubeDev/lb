//! `case.names` — the display names of the people a caller's cases mention (Insights review 2026-10).
//!
//! A case names people by subject — `assigned_to`, `snoozed_by`, `emailed_by`, an event's `actor` —
//! and the queue, a VIEWER surface, had no way to turn `user:tom.fyfe@esr.com` into "Tom Fyfe":
//! `identity.list` and `membership.list` are admin. The board, the Responsible column and the
//! activity feed all print people, so every one of them printed an email.
//!
//! The question this answers is narrow on purpose: *who are these subjects, as people?* The caller
//! hands the subjects (the ones on the cases they can already read); the verb returns, for each that
//! is a LIVE member of THIS workspace, its display name and its login email — the name to print, the
//! email to tell two people of one name apart on hover (a subject such as `user:test` carries none).
//! It never lists anyone: a subject the caller does not already hold yields nothing it did not send,
//! and both fields are about people the caller already shares a workspace and its queue with.
//!
//! What a caller must NOT learn from it:
//!   - anyone's name outside this workspace — a subject that is not a member answers nothing, so
//!     the verb cannot be walked to read the node's identity table;
//!   - a team's name — teams the caller is not on are `teams.list`'s disclosure (`case.assignees`
//!     already names the caller's own);
//!   - a name for a non-`user:` subject — parties and systems are named by their own planes.
//!
//! **Gate: `case.list`, via the alias in `tool_gate.rs`**, for the reason `case.assignees` gives: a
//! `mcp:case.names:call` cap exists in no role bundle, and the names belong to the same page.
//!
//! **Bounded.** At most [`MAX_NAMES`] subjects per call — one screen of cases mentions far fewer —
//! so a caller cannot turn one request into a scan.

use lb_auth::Principal;
use lb_authz::{identity_get, membership_is_member};
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde::{Deserialize, Serialize};

use super::error::CaseSvcError;

/// The most subjects one `case.names` call resolves.
pub const MAX_NAMES: usize = 200;

/// One subject, as a person: the display name and the login email, each when set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubjectName {
    pub sub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// The display names and emails of `subjects`, for those that are live members of `ws`. A subject
/// with neither set is left out (the caller prints the subject), never an error.
pub async fn case_names(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subjects: &[String],
) -> Result<Vec<SubjectName>, CaseSvcError> {
    authorize_tool(principal, ws, "case.list").map_err(|_| CaseSvcError::Denied)?;
    if subjects.len() > MAX_NAMES {
        return Err(CaseSvcError::BadInput(format!(
            "case.names takes at most {MAX_NAMES} subjects, got {}",
            subjects.len()
        )));
    }

    let mut wanted: Vec<&String> = subjects.iter().filter(|s| s.starts_with("user:")).collect();
    wanted.sort();
    wanted.dedup();

    let mut out = Vec::with_capacity(wanted.len());
    for sub in wanted {
        // Membership first: a subject outside this workspace answers nothing, whatever the node's
        // identity table holds. A read that fails drops the row, as `case.assignees` does.
        match membership_is_member(store, ws, sub).await {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => {
                tracing::warn!(ws, sub = %sub, error = ?e, "case names: membership unreadable; name omitted");
                continue;
            }
        }
        let identity = match identity_get(store, sub).await {
            Ok(identity) => identity,
            Err(e) => {
                tracing::warn!(ws, sub = %sub, error = ?e, "case names: identity unreadable; name omitted");
                None
            }
        };
        let Some(identity) = identity else { continue };
        let set = |v: Option<String>| v.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let (name, email) = (set(identity.display_name), set(identity.email));
        if name.is_some() || email.is_some() {
            out.push(SubjectName { sub: sub.clone(), name, email });
        }
    }
    Ok(out)
}
