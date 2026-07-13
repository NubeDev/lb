//! `github-bridge` — a pure-transform Tier-1 extension. One tool, `normalize`, mapping a raw GitHub
//! webhook payload to the canonical `{ issue_id, payload, ts }` triple the host's
//! `workflow.ingest_issue` consumes (github-bridge scope; the S6 `github-bridge` deferral resolved).
//!
//! It is a PURE function: no store, no bus, no host callback. The stable WIT world imports only
//! `host.log`; there is no host-tool-call import, so the guest CANNOT invoke `ingest_issue` itself —
//! the HOST composes `normalize` -> `ingest_issue`. Generated against the SAME WIT the host uses, so
//! the ABI cannot drift. Stateless (§3.4): everything comes from the call input.

// The `generate!` call is emitted by `build.rs` into `$OUT_DIR/wit_gen.rs`, reading the WIT from the
// standalone `lb-sdk` crate (the authoritative owner) — see the build script. Generated against the
// SAME WIT the host uses, so the ABI cannot drift.
include!(concat!(env!("OUT_DIR"), "/wit_gen.rs"));

use serde::{Deserialize, Serialize};

/// The slice of a GitHub `issues`/`issue_comment` webhook we read. Unknown fields are ignored, so a
/// richer real payload still parses — we only depend on the stable identity + body fields.
#[derive(Deserialize)]
struct Webhook {
    /// `opened`, `created` (a comment), `edited`, … — carried through verbatim into the payload.
    action: String,
    issue: Issue,
    /// Present on `issue_comment` events; absent on `issues` events.
    #[serde(default)]
    comment: Option<Comment>,
    #[serde(default)]
    repository: Option<Repository>,
    /// Logical timestamp the host injects into the envelope (determinism — no wall clock in the guest).
    ts: u64,
}

#[derive(Deserialize)]
struct Issue {
    number: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
}

#[derive(Deserialize)]
struct Comment {
    #[serde(default)]
    body: String,
}

#[derive(Deserialize)]
struct Repository {
    #[serde(default)]
    full_name: String,
}

/// The canonical triple `workflow.ingest_issue` consumes. `issue_id` is stable across retries (so the
/// inbox upsert is idempotent); `payload` is the normalized human-readable body; `ts` is passed through.
#[derive(Serialize)]
struct Normalized {
    issue_id: String,
    payload: String,
    ts: u64,
}

struct GithubBridge;

impl exports::lazybones::ext::tool::Guest for GithubBridge {
    fn call(
        name: String,
        input_json: String,
    ) -> Result<String, exports::lazybones::ext::tool::ToolError> {
        use exports::lazybones::ext::tool::ToolError;
        lazybones::ext::host::log(&format!("github-bridge.{name} called"));
        match name.as_str() {
            "normalize" => normalize(&input_json),
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),
        }
    }
}

/// Map a raw webhook to the canonical triple. A malformed payload is a `bad-input` tool error (never a
/// panic) — the bytes are untrusted input, exactly the shape a sandbox is for.
fn normalize(input_json: &str) -> Result<String, exports::lazybones::ext::tool::ToolError> {
    use exports::lazybones::ext::tool::ToolError;
    let hook: Webhook =
        serde_json::from_str(input_json).map_err(|e| ToolError::BadInput(e.to_string()))?;

    // `issue_id` keys the inbox upsert: scope it by repo so two repos' issue #1 don't collide, and keep
    // it stable across a re-delivered webhook (idempotency lives on the host's (channel,id), but the id
    // it upserts on is THIS — so it must be deterministic from identity, not from the event).
    let repo = hook
        .repository
        .map(|r| r.full_name)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let issue_id = format!("{repo}#{}", hook.issue.number);

    // The payload is the human-readable triage body: the issue title/body, plus the comment text when
    // the event is a comment. The `action` is carried so triage can tell "opened" from "commented".
    let payload = match &hook.comment {
        Some(c) => format!(
            "[{}] {}\n{}\n\ncomment:\n{}",
            hook.action, hook.issue.title, hook.issue.body, c.body
        ),
        None => format!("[{}] {}\n{}", hook.action, hook.issue.title, hook.issue.body),
    };

    let out = Normalized {
        issue_id,
        payload,
        ts: hook.ts,
    };
    serde_json::to_string(&out).map_err(|e| ToolError::Failed(e.to_string()))
}

export!(GithubBridge);
