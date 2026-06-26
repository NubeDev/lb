//! `PrSpec` — the durable description of the pull request a coding job will open, keyed by the
//! approval it belongs to (coding-workflow + outbox scope, the "producer payload enrichment" the
//! live PR needs).
//!
//! Why a record and not a freeform string: the github-target adapter's `create_pr` expects a
//! structured `{repo, head, base, title, body}` payload (`role/github-target/src/request.rs`), but
//! the producer historically emitted only `{scope_doc}`. The PR's coordinates are *state* — they
//! must survive the approver disconnecting and be readable by the resolution reactor at start time,
//! with **no caller input** at react time (the reactor is a durable scan, not an API call). So we
//! persist the spec when approval is requested, addressed by the same `approval_id` the resolution
//! and the gate already key on — a small sibling record, the same pattern as `lb_inbox::Resolution`.
//!
//! State only, behind the workspace wall (§7): `write`/`read` select the namespace from `ws`, so a
//! ws-B reactor can physically only read ws-B specs. Raw verbs — the workflow service is the caps
//! chokepoint (capability-first, §3.5).

use serde::{Deserialize, Serialize};

use lb_store::{read, write, Store, StoreError};

/// The table PR specs live in (one per workspace namespace), keyed by the `approval_id` they gate.
const PR_SPEC_TABLE: &str = "pr_spec";

/// The coordinates of the pull request to open — exactly the shape `github-target`'s `create_pr`
/// payload deserializes (`{repo, head, base, title, body}`), so the producer can emit it verbatim
/// and the adapter maps it without a shaping step. `body` is optional at the wire (defaults empty).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrSpec {
    /// `owner/repo`, e.g. `acme/api`.
    pub repo: String,
    /// The head branch (the change), e.g. `fix/2451`.
    pub head: String,
    /// The base branch to merge into, e.g. `main`.
    pub base: String,
    /// The PR title.
    pub title: String,
    /// The PR body (markdown). May be empty.
    #[serde(default)]
    pub body: String,
}

impl PrSpec {
    pub fn new(
        repo: impl Into<String>,
        head: impl Into<String>,
        base: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            repo: repo.into(),
            head: head.into(),
            base: base.into(),
            title: title.into(),
            body: body.into(),
        }
    }

    /// The `create_pr` outbox payload — the exact JSON `github-target`'s `request.rs` maps to a
    /// `POST /repos/{repo}/pulls`. Single source of truth for the wire shape.
    pub fn create_pr_payload(&self) -> String {
        // `serde_json` escapes every field, so a title/body with quotes or braces is safe (the old
        // `format!`-built `{"scope_doc":"…"}` payload was not — this also fixes that latent bug).
        serde_json::to_string(self).expect("PrSpec serializes")
    }
}

/// Persist `spec` for `approval_id` in workspace `ws`. Idempotent on `approval_id` (re-requesting
/// approval upserts the same spec — last write wins, like the resolution).
pub async fn record_pr_spec(
    store: &Store,
    ws: &str,
    approval_id: &str,
    spec: &PrSpec,
) -> Result<(), StoreError> {
    let value = serde_json::to_value(spec).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, PR_SPEC_TABLE, approval_id, &value).await
}

/// Read the PR spec for `approval_id` in workspace `ws`. `None` if none was recorded (or it lives in
/// another workspace — the namespace is the hard wall, §7).
pub async fn pr_spec(
    store: &Store,
    ws: &str,
    approval_id: &str,
) -> Result<Option<PrSpec>, StoreError> {
    match read(store, ws, PR_SPEC_TABLE, approval_id).await? {
        Some(value) => Ok(Some(
            serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?,
        )),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_pr_payload_is_the_github_target_shape() {
        let spec = PrSpec::new("acme/api", "fix/2451", "main", "Fix race", "the body");
        let payload = spec.create_pr_payload();
        let v: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(v["repo"], "acme/api");
        assert_eq!(v["head"], "fix/2451");
        assert_eq!(v["base"], "main");
        assert_eq!(v["title"], "Fix race");
        assert_eq!(v["body"], "the body");
    }

    #[test]
    fn special_characters_in_the_title_are_escaped() {
        // The old format!-built payload broke on a quote/brace in the title; serde escapes it.
        let spec = PrSpec::new("acme/api", "h", "main", r#"Fix "races" {now}"#, "");
        let payload = spec.create_pr_payload();
        let v: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(v["title"], r#"Fix "races" {now}"#);
    }
}
