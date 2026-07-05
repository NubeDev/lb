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
//!   - `rich_result`  — `{ v:2, view, source?, data?, options?, action?, tools }`, the render-envelope
//!                      (channel rich responses scope). A worker posts a viewable response — a `view`
//!                      (`table`/`chart`/`stat`/`switch`/`button`/`template`) over inline `data` and/or a
//!                      `source` the viewer re-runs, with row-control `options` and a control `action`.
//!                      `tools` is the tool set the response's bridge may forward (source + action tools).
//!                      `v` is the envelope version, ALWAYS serialized (a reader keys upconversion on it).
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
pub const KIND_RICH_RESULT: &str = "rich_result";

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
    RichResult(RichResultPayload),
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
    /// The **persona** selector (agent-personas scope #1) — a per-invoke override of the workspace's
    /// `agent.config.active_persona`. Absent → the active persona (or none). A curated *focus* (tools +
    /// pinned skills + identity), orthogonal to `runtime` (which picks the engine). Opaque id (rule
    /// 10). `#[serde(default)]` so an older post without it deserializes unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    /// The durable run/job id the UI mints up front so it can watch the run stream (`agent.watch`)
    /// the instant the request item lands. The worker drives the run under this id.
    pub job: String,
    /// Optional **page context** (agent-dock scope) — the client-reported `{ surface, path, search }`
    /// object the worker fences into the run's goal as untrusted context. `#[serde(default)]` so a
    /// request without it is byte-identical to today (existing channels/agent posts are unaffected).
    /// Opaque `Value`: the host never branches on a surface id (rule 10). Oversize (>4 KB serialized)
    /// is rejected by the fence and surfaces as an `agent_error`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
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

/// `kind: "rich_result"` — the render-envelope (channel rich responses scope). A worker's viewable
/// response: a `view` over inline `data` and/or a re-runnable `source`, with row-control `options`, an
/// optional control `action`, and the `tools` set the response's bridge may forward. `v` is the
/// envelope version and is ALWAYS on the wire (unlike the `skip_serializing_if` fields), so a reader
/// keys any upconversion on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichResultPayload {
    /// The render-envelope version — always `2`. Never skipped: the wire form always carries `v`.
    pub v: u32,
    /// The viewer to render with (`table`/`chart`/`stat`/`switch`/`button`/`template`).
    pub view: String,
    /// A `{tool, args}` object the viewer re-runs to (re)load data. Absent → the response is inline-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Value>,
    /// Inline data the viewer renders directly. Absent → the viewer runs `source`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// View options (incl. row controls). Absent → the viewer's defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Value>,
    /// A control's `{tool, argsTemplate}` (a button/switch's effect). Absent → the view is read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Value>,
    /// The tool set the response's bridge may forward (the `source` + `action` tools). Empty → none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    /// The per-field PRESENTATION config for the rendered view (widget-kit scope, Phase 1) — the Grafana
    /// `fieldConfig` a descriptor declares so a table's headers read the author's labels, drop `hide`d
    /// columns, and order as declared. INERT DATA on this existing envelope (no new verb/table); the UI
    /// copies it onto the cell and resolves every header through the one presentation resolver. Absent →
    /// the table humanizes raw keys. Kept an opaque `Value` (the UI owns the `FieldConfig` shape).
    #[serde(
        default,
        rename = "fieldConfig",
        skip_serializing_if = "Option::is_none"
    )]
    pub field_config: Option<Value>,
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
        KIND_QUERY
            | KIND_RESULT
            | KIND_ERROR
            | KIND_AGENT
            | KIND_AGENT_RESULT
            | KIND_AGENT_ERROR
            | KIND_RICH_RESULT
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

/// Build the `rich_result` render-envelope body (channel rich responses scope). Always stamps `v: 2`
/// — the envelope version a reader keys upconversion on. The optional shapes drop off the wire when
/// `None`/empty (`skip_serializing_if`); `v` and `view` are always present.
///
/// `#[allow(dead_code)]`: this is the additive contract half — the shape + builder land now (mirrored
/// by the UI's `payload.types.ts`), ahead of the worker/verb that will POST a rich response. The
/// `#[cfg(test)]` round-trips below exercise it, so the contract is real, not speculative.
#[allow(dead_code)]
pub fn rich_result_body(
    view: &str,
    source: Option<Value>,
    data: Option<Value>,
    options: Option<Value>,
    action: Option<Value>,
    tools: Vec<String>,
) -> String {
    encode_payload(&ItemPayload::RichResult(RichResultPayload {
        v: 2,
        view: view.into(),
        source,
        data,
        options,
        action,
        tools,
        // This host-authored constructor carries no presentation config; a DESCRIPTOR that wants table
        // presentation declares `fieldConfig` in its `result` Value (see reminder/descriptor.rs). Absent
        // → the UI humanizes raw keys.
        field_config: None,
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

    // channel rich responses: a rich_result round-trips (view + tools survive) and always carries `v:2`.
    #[test]
    fn rich_result_round_trips_and_stamps_v2() {
        let body = rich_result_body(
            "table",
            Some(json!({"tool": "store.query", "args": {"sql": "SELECT 1"}})),
            None,
            Some(json!({"rows": 50})),
            None,
            vec!["store.query".into()],
        );
        // `v` is ALWAYS on the wire (unlike the skipped optional fields).
        assert!(body.contains(r#""v":2"#), "v:2 is always present: {body}");
        match parse_payload(&body).expect("parsed") {
            ItemPayload::RichResult(r) => {
                assert_eq!(r.v, 2);
                assert_eq!(r.view, "table");
                assert_eq!(r.tools, vec!["store.query".to_string()]);
                assert!(r.data.is_none());
                assert!(r.source.is_some());
            }
            _ => panic!("wrong variant"),
        }
    }

    // A minimal rich_result (no source/data/options/action, no tools) still parses to the variant and
    // still carries `v:2` — the optional shapes drop off the wire, `v`/`view` remain.
    #[test]
    fn minimal_rich_result_body_parses_to_the_variant() {
        let body = rich_result_body("stat", None, None, None, None, Vec::new());
        assert!(body.contains(r#""v":2"#), "v:2 always present: {body}");
        assert!(!body.contains("tools"), "empty tools dropped from the wire");
        assert!(
            !body.contains("source"),
            "None source dropped from the wire"
        );
        assert!(matches!(
            parse_payload(&body),
            Some(ItemPayload::RichResult(_))
        ));
    }

    // An unknown kind still stays chat (unchanged by the additive rich_result arm).
    #[test]
    fn unknown_kind_still_chat_after_rich_result_added() {
        assert!(parse_payload(r#"{"kind":"whatever","view":"table"}"#).is_none());
    }
}
