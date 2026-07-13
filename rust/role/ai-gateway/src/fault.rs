//! The typed **provider fault** — what a model call reports when it did NOT produce a completion
//! (agent-loop-hardening scope, slice D). Before this, `openai_compat` flattened every failure into
//! a terminal `AiResponse::stop("model call failed: …")` string: the loop could not tell a 429 it
//! should retry from a 401 it must surface, and a context overflow (recoverable by compacting) from
//! a dead network. The fault carries the **structured** evidence — status code, `Retry-After`
//! seconds, an overflow discriminant — never a parsed error-message string (zeroclaw's stringly
//! `parse_retry_after_ms` is the anti-pattern this replaces).
//!
//! [`ProviderFault::lane`] is the one classification the loop consumes: **transient** (bounded
//! mechanical retry of the same step), **overflow** (recover by compacting the transcript, never
//! retry verbatim), **fatal** (honest terminal event). The *model-recoverable* lane of the scope's
//! taxonomy (denied tool, unknown tool, bad args) never reaches this type — those are tool
//! outcomes fed back as observations, already uniform in the loop.

use serde::{Deserialize, Serialize};

/// How the call failed, mechanically. `Http` carries its status in [`ProviderFault::status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FaultKind {
    /// The request never completed (connect/DNS/TLS/socket failure).
    Network,
    /// The request timed out (distinct from `Network` so the classification table is explicit).
    Timeout,
    /// The endpoint answered with a non-2xx status.
    Http,
    /// The endpoint answered 2xx but the body was not a readable completion.
    MalformedBody,
}

/// The lane the loop routes a fault into (slice D's taxonomy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultLane {
    /// Retry the same step, bounded, honoring `retry_after_secs`.
    Transient,
    /// The request exceeded the model's context window — compact and continue, never retry verbatim.
    Overflow,
    /// Surface an honest terminal event (auth failure, malformed request, …).
    Fatal,
}

/// One failed model call, with the structured evidence classification needs. Constructed only by
/// provider adapters (and the scripted `MockProvider`); consumed via [`ProviderFault::lane`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderFault {
    pub kind: FaultKind,
    /// The HTTP status when `kind == Http` (also set on an overflow signalled by status).
    pub status: Option<u16>,
    /// Parsed from the `Retry-After` header when present as delta-seconds. The HTTP-date form is
    /// deliberately ignored (it would need a wall clock; a missing value just means default backoff).
    pub retry_after_secs: Option<u64>,
    /// The overflow discriminant: true when the *structured* error body identified a context-window
    /// overflow (`error.code == "context_length_exceeded"`) or the status itself did (413). Never
    /// inferred from prose.
    pub overflow: bool,
    /// Human-readable detail for the terminal event / logs. Never contains a secret.
    pub detail: String,
}

impl ProviderFault {
    pub fn network(detail: impl Into<String>) -> Self {
        Self {
            kind: FaultKind::Network,
            status: None,
            retry_after_secs: None,
            overflow: false,
            detail: detail.into(),
        }
    }

    pub fn timeout(detail: impl Into<String>) -> Self {
        Self {
            kind: FaultKind::Timeout,
            status: None,
            retry_after_secs: None,
            overflow: false,
            detail: detail.into(),
        }
    }

    pub fn http(status: u16, retry_after_secs: Option<u64>, detail: impl Into<String>) -> Self {
        Self {
            kind: FaultKind::Http,
            status: Some(status),
            retry_after_secs,
            // 413 (payload too large) is the status-level overflow signal.
            overflow: status == 413,
            detail: detail.into(),
        }
    }

    /// An HTTP fault whose structured error body identified a context-window overflow.
    pub fn overflow(status: u16, detail: impl Into<String>) -> Self {
        Self {
            kind: FaultKind::Http,
            status: Some(status),
            retry_after_secs: None,
            overflow: true,
            detail: detail.into(),
        }
    }

    pub fn malformed(detail: impl Into<String>) -> Self {
        Self {
            kind: FaultKind::MalformedBody,
            status: None,
            retry_after_secs: None,
            overflow: false,
            detail: detail.into(),
        }
    }

    /// Classify this fault into the lane the loop acts on. The full decision table (status ×
    /// headers × overflow) is pinned by `tests/fault_class_test.rs`.
    pub fn lane(&self) -> FaultLane {
        if self.overflow {
            return FaultLane::Overflow;
        }
        match self.kind {
            FaultKind::Network | FaultKind::Timeout => FaultLane::Transient,
            // A 2xx with an unreadable body is most often a proxy/stream hiccup — worth the bounded
            // retry; the retry ceiling makes a persistent one fatal anyway.
            FaultKind::MalformedBody => FaultLane::Transient,
            FaultKind::Http => match self.status.unwrap_or(0) {
                408 | 429 => FaultLane::Transient,
                s if s >= 500 => FaultLane::Transient,
                // Everything else 4xx (401/403 auth, 400 bad request, 404 wrong endpoint, …):
                // retrying the identical request cannot succeed — surface it honestly.
                _ => FaultLane::Fatal,
            },
        }
    }
}
