//! Map an outbox [`Effect`] to the GitHub REST request that performs it. Pure (no I/O): it turns the
//! effect's `action` + opaque `payload` into a `(method, path, json-body)` the [`crate::client`]
//! sends. Keeping the mapping here, separate from the HTTP send, makes the per-action contract
//! testable in isolation and the client a thin transport.
//!
//! Two actions are supported now (the ones the S6 workflow emits): `create_pr` and `comment`. An
//! unknown action is a permanent mapping error — the relay should not retry an effect it can never
//! deliver (distinct from a transient transport failure). A malformed payload is the same class.

use lb_outbox::Effect;
use serde::Deserialize;
use serde_json::{json, Value};

/// The GitHub target's id — only effects whose `Effect::target` equals this are ours.
pub const TARGET: &str = "github";

/// A resolved GitHub REST call: the path under the API base (`/repos/...`) and the JSON body to POST.
#[derive(Debug)]
pub(crate) struct GithubRequest {
    pub path: String,
    pub body: Value,
}

/// Why an effect could not be mapped to a request. **Permanent** — unlike a transport failure, a
/// re-delivery would fail identically, so the caller surfaces it as a non-retryable delivery error.
#[derive(Debug, PartialEq, Eq)]
pub enum MapError {
    /// The effect's `target` is not `github` — this adapter must not handle it.
    WrongTarget,
    /// The `action` is not one this adapter knows how to perform.
    UnknownAction(String),
    /// The `payload` did not match the action's expected shape.
    BadPayload(String),
}

/// `POST /repos/{repo}/pulls` — open a pull request. Payload: `{repo, head, base, title, body?}`.
#[derive(Deserialize)]
struct CreatePr {
    repo: String,
    head: String,
    base: String,
    title: String,
    #[serde(default)]
    body: String,
}

/// `POST /repos/{repo}/issues/{issue_number}/comments` — comment on an issue/PR. Payload:
/// `{repo, issue_number, body}`.
#[derive(Deserialize)]
struct Comment {
    repo: String,
    issue_number: u64,
    body: String,
}

/// Map `effect` to its GitHub request, or a permanent [`MapError`] if it is not deliverable as-is.
pub(crate) fn to_request(effect: &Effect) -> Result<GithubRequest, MapError> {
    if effect.target != TARGET {
        return Err(MapError::WrongTarget);
    }
    match effect.action.as_str() {
        "create_pr" => {
            let p: CreatePr = parse(&effect.payload)?;
            Ok(GithubRequest {
                path: format!("/repos/{}/pulls", p.repo),
                body: json!({ "title": p.title, "head": p.head, "base": p.base, "body": p.body }),
            })
        }
        "comment" => {
            let p: Comment = parse(&effect.payload)?;
            Ok(GithubRequest {
                path: format!("/repos/{}/issues/{}/comments", p.repo, p.issue_number),
                body: json!({ "body": p.body }),
            })
        }
        other => Err(MapError::UnknownAction(other.to_string())),
    }
}

fn parse<T: for<'de> Deserialize<'de>>(payload: &str) -> Result<T, MapError> {
    serde_json::from_str(payload).map_err(|e| MapError::BadPayload(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(target: &str, action: &str, payload: &str) -> Effect {
        Effect::new("e1", target, action, payload, "key", 1)
    }

    #[test]
    fn create_pr_maps_to_the_pulls_endpoint() {
        let e = effect(
            "github",
            "create_pr",
            r#"{"repo":"acme/api","head":"fix/2451","base":"main","title":"Fix race","body":"b"}"#,
        );
        let r = to_request(&e).unwrap();
        assert_eq!(r.path, "/repos/acme/api/pulls");
        assert_eq!(r.body["head"], "fix/2451");
        assert_eq!(r.body["title"], "Fix race");
    }

    #[test]
    fn comment_maps_to_the_comments_endpoint() {
        let e = effect(
            "github",
            "comment",
            r#"{"repo":"acme/api","issue_number":2451,"body":"on it"}"#,
        );
        let r = to_request(&e).unwrap();
        assert_eq!(r.path, "/repos/acme/api/issues/2451/comments");
        assert_eq!(r.body["body"], "on it");
    }

    #[test]
    fn a_foreign_target_is_not_ours() {
        let e = effect("email", "send", "{}");
        assert_eq!(to_request(&e).unwrap_err(), MapError::WrongTarget);
    }

    #[test]
    fn an_unknown_action_is_a_permanent_error() {
        let e = effect("github", "merge", "{}");
        assert!(matches!(to_request(&e).unwrap_err(), MapError::UnknownAction(a) if a == "merge"));
    }

    #[test]
    fn a_malformed_payload_is_a_permanent_error() {
        let e = effect("github", "create_pr", r#"{"repo":"acme/api"}"#); // missing head/base/title
        assert!(matches!(
            to_request(&e).unwrap_err(),
            MapError::BadPayload(_)
        ));
    }
}
