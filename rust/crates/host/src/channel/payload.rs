//! The **kind-tagged channel item payloads** (channels-query-charts scope). A channel `Item`'s
//! `body` is opaque text; these are the typed envelopes that ride INSIDE `body` as JSON, keyed by
//! `kind`. This needs NO `Item` schema migration (scope decision: `kind` lives as a key inside the
//! existing payload) — a plain-text body with no parseable `kind` is an ordinary chat message, so
//! the change is purely additive and existing channels are unaffected.
//!
//! The shapes that share the channel:
//!   - `query`        — `{ source, sql }`, posted by a member who wants to run a query.
//!   - `query_result` — `{ source, sql, columns, rows, chart, truncated }`, posted by the worker.
//!   - `query_error`  — `{ source, sql, error }`, posted by the worker when the query fails.
//!   - `agent`        — `{ goal, runtime?, job }`, posted by a member who wants to ask an agent
//!                      (channels-agent scope). `runtime` selects the `AgentRuntime` (absent → the
//!                      in-house default; a profile id → an external agent). `job` is the durable run
//!                      id the UI mints so it can watch the run stream the instant the item lands.
//!   - `agent_result` — `{ goal, runtime, job, answer, truncated }`, posted by the agent worker on
//!                      completion — the durable final answer.
//!   - `agent_error`  — `{ goal, error }`, posted by the agent worker when the run can't start / fails
//!                      (opaque on the deny/unknown-runtime path — no capability/existence leak).
//!
//! The host NEVER parses chat text for commands — the UI builds these structured payloads and
//! posts them; the worker reads the `kind` to decide what (if anything) to do.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::chart::ChartSpec;

/// The recognized payload `kind` values. Anything else (or a non-JSON body) is chat.
pub const KIND_QUERY: &str = "query";
pub const KIND_RESULT: &str = "query_result";
pub const KIND_ERROR: &str = "query_error";
pub const KIND_AGENT: &str = "agent";
pub const KIND_AGENT_RESULT: &str = "agent_result";
pub const KIND_AGENT_ERROR: &str = "agent_error";

/// A parsed kind-tagged payload pulled out of an item `body`. Chat (no `kind`) is `None` upstream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemPayload {
    Query(QueryPayload),
    QueryResult(QueryResultPayload),
    QueryError(QueryErrorPayload),
    Agent(AgentPayload),
    AgentResult(AgentResultPayload),
    AgentError(AgentErrorPayload),
}

/// `kind: "query"` — a member's request to run `sql` against `source`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryPayload {
    pub source: String,
    pub sql: String,
}

/// `kind: "query_result"` — the worker's answer: the columns/rows (capped) + the host-picked chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResultPayload {
    pub source: String,
    pub sql: String,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart: Option<ChartSpec>,
    /// `true` when the row/byte cap trimmed the result; the UI shows "showing first N rows".
    /// `default` on deserialize so the omitted-when-false wire form round-trips (the
    /// `skip_serializing_if` drops it from the body, so a reader must tolerate its absence).
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
}

/// `kind: "query_error"` — the worker's opaque/honest failure message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryErrorPayload {
    pub source: String,
    pub sql: String,
    pub error: String,
}

/// `kind: "agent"` — a member's request to ask an agent `goal` (channels-agent scope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPayload {
    pub goal: String,
    /// The `AgentRuntime` selector. Absent → the in-house `default`; a profile id
    /// (`"open-interpreter-default"`, …) → an external agent (resolved through the seam, grant-gated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// The durable run/job id the UI mints up front so it can watch the run stream (`agent.watch`)
    /// the instant the request item lands. The worker drives the run under this id.
    pub job: String,
}

/// `kind: "agent_result"` — the agent worker's durable final answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentResultPayload {
    pub goal: String,
    /// The runtime that served the run (`"default"` or a profile id) — echoed so the channel records
    /// which agent answered.
    pub runtime: String,
    pub job: String,
    pub answer: String,
    /// `true` when the answer hit the byte cap and was trimmed; the UI links to the full run.
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
}

/// `kind: "agent_error"` — the agent worker's opaque/honest failure. Opaque on the deny / unknown-
/// runtime path (no capability or runtime-existence leak); honest on an execution fault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentErrorPayload {
    pub goal: String,
    pub error: String,
}

fn is_false(b: &bool) -> bool {
    !b
}

/// Parse an item `body` into a kind-tagged payload, or `None` if it is chat (not JSON, or JSON
/// without a recognized `kind`). Tolerant by design: a chat message that happens to be valid JSON
/// but carries no `kind` stays chat.
pub fn parse_payload(body: &str) -> Option<ItemPayload> {
    let value: Value = serde_json::from_str(body).ok()?;
    // Only a JSON object with a recognized `kind` is a payload; everything else is chat.
    let kind = value.get("kind").and_then(Value::as_str)?;
    if !matches!(
        kind,
        KIND_QUERY | KIND_RESULT | KIND_ERROR | KIND_AGENT | KIND_AGENT_RESULT | KIND_AGENT_ERROR
    ) {
        return None;
    }
    serde_json::from_value(value).ok()
}

/// Serialize a payload into the JSON string that rides in an item `body`.
pub fn encode_payload(payload: &ItemPayload) -> String {
    serde_json::to_string(payload).expect("a kind-tagged payload serializes")
}

/// Build the `query_result` body for a successful run.
pub fn result_body(
    source: &str,
    sql: &str,
    columns: Vec<String>,
    rows: Vec<Value>,
    chart: Option<ChartSpec>,
    truncated: bool,
) -> String {
    encode_payload(&ItemPayload::QueryResult(QueryResultPayload {
        source: source.into(),
        sql: sql.into(),
        columns,
        rows,
        chart,
        truncated,
    }))
}

/// Build the `query_error` body for a failed run.
pub fn error_body(source: &str, sql: &str, error: &str) -> String {
    encode_payload(&ItemPayload::QueryError(QueryErrorPayload {
        source: source.into(),
        sql: sql.into(),
        error: error.into(),
    }))
}

/// Build the `agent_result` body for a completed run (channels-agent scope).
pub fn agent_result_body(
    goal: &str,
    runtime: &str,
    job: &str,
    answer: &str,
    truncated: bool,
) -> String {
    encode_payload(&ItemPayload::AgentResult(AgentResultPayload {
        goal: goal.into(),
        runtime: runtime.into(),
        job: job.into(),
        answer: answer.into(),
        truncated,
    }))
}

/// Build the `agent_error` body for a run that could not start or failed (opaque on deny/unknown).
pub fn agent_error_body(goal: &str, error: &str) -> String {
    encode_payload(&ItemPayload::AgentError(AgentErrorPayload {
        goal: goal.into(),
        error: error.into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_query_body() {
        let body = r#"{"kind":"query","source":"warehouse","sql":"SELECT 1"}"#;
        let p = parse_payload(body).expect("parsed");
        assert_eq!(
            p,
            ItemPayload::Query(QueryPayload {
                source: "warehouse".into(),
                sql: "SELECT 1".into(),
            })
        );
    }

    #[test]
    fn plain_text_body_is_chat_none() {
        assert!(parse_payload("hello world").is_none());
    }

    #[test]
    fn json_without_kind_is_chat_none() {
        assert!(parse_payload(r#"{"foo":1}"#).is_none());
    }

    #[test]
    fn unknown_kind_is_chat_none() {
        assert!(parse_payload(r#"{"kind":"chat","text":"hi"}"#).is_none());
    }

    #[test]
    fn result_round_trips() {
        let body = result_body(
            "warehouse",
            "SELECT 1",
            vec!["v".into()],
            vec![json!({"v": 1})],
            None,
            false,
        );
        match parse_payload(&body).expect("parsed") {
            ItemPayload::QueryResult(r) => {
                assert_eq!(r.columns, vec!["v".to_string()]);
                assert!(!r.truncated);
            }
            _ => panic!("wrong variant"),
        }
    }

    // Regression (debugging/channels/query-result-missing-truncated.md): a `truncated:false`
    // result drops the field on the wire (`skip_serializing_if`), so the reader MUST tolerate its
    // absence. Before the `#[serde(default)]` fix this round-trip failed with "missing field
    // `truncated`" and every untruncated query_result silently parsed as chat.
    #[test]
    fn untruncated_result_omits_truncated_yet_round_trips() {
        let body = result_body(
            "s",
            "SELECT 1",
            vec!["v".into()],
            vec![json!({"v": 1})],
            None,
            false,
        );
        assert!(
            !body.contains("truncated"),
            "false truncated is dropped from the wire"
        );
        assert!(matches!(
            parse_payload(&body),
            Some(ItemPayload::QueryResult(_))
        ));
    }

    #[test]
    fn truncated_result_round_trips() {
        let body = result_body(
            "s",
            "SELECT 1",
            vec!["v".into()],
            vec![json!({"v": 1})],
            None,
            true,
        );
        match parse_payload(&body).expect("parsed") {
            ItemPayload::QueryResult(r) => assert!(r.truncated),
            _ => panic!("wrong variant"),
        }
    }

    // channels-agent: a request with no `runtime` parses (absent → default is decided downstream).
    #[test]
    fn parses_agent_request_without_runtime() {
        let body = r#"{"kind":"agent","goal":"summarize the logs","job":"run-1"}"#;
        match parse_payload(body).expect("parsed") {
            ItemPayload::Agent(a) => {
                assert_eq!(a.goal, "summarize the logs");
                assert_eq!(a.job, "run-1");
                assert!(a.runtime.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_agent_request_with_runtime() {
        let body =
            r#"{"kind":"agent","goal":"hi","runtime":"open-interpreter-default","job":"run-2"}"#;
        match parse_payload(body).expect("parsed") {
            ItemPayload::Agent(a) => {
                assert_eq!(a.runtime.as_deref(), Some("open-interpreter-default"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn agent_result_and_error_round_trip() {
        let r = agent_result_body("g", "default", "run-3", "the answer", false);
        assert!(
            !r.contains("truncated"),
            "false truncated dropped from wire"
        );
        match parse_payload(&r).expect("parsed") {
            ItemPayload::AgentResult(p) => {
                assert_eq!(p.answer, "the answer");
                assert_eq!(p.runtime, "default");
            }
            _ => panic!("wrong variant"),
        }
        let e = agent_error_body("g", "agent not permitted");
        match parse_payload(&e).expect("parsed") {
            ItemPayload::AgentError(p) => assert_eq!(p.error, "agent not permitted"),
            _ => panic!("wrong variant"),
        }
    }
}
