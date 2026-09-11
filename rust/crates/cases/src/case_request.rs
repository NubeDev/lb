//! The **case_request** record — one ask, of one party, about one case (case-plane scope, wave 2).
//!
//! This is the external-party round trip in a single row: what we asked, who we asked, the link we
//! sent, whether the mail actually went anywhere, what came back, and when. The contractor has no
//! account and never will — the row's `token_hash` IS their identity for the life of the ask.
//!
//! Three things about this record are load-bearing and easy to get wrong:
//!
//! 1. **Only the HASH is stored.** The raw token exists exactly once, in the link in the email. A
//!    stolen store dump therefore yields no working links (the invite record's discipline, and for
//!    the same reason: a full-entropy random token makes a fast SHA-256 the correct hash).
//! 2. **`delivery` is not `status`.** `status` is where the *ask* is (sent → opened → replied);
//!    `delivery` is what happened to the *email* (queued → sent | logged | failed). They are
//!    separate because a dev node's logging provider acknowledges every dropped mail, so an effect
//!    that reads `delivered` proves nothing about a mailbox
//!    (`outbox-delivered-is-not-email-sent.md`, resolved decision 7). A drawer that collapsed the
//!    two would tell an operator an email went out that did not.
//! 3. **`brief` is a SNAPSHOT.** What the contractor sees is frozen at send time, not re-queried at
//!    view time — the token principal holds two caps and would be denied on every query verb, and
//!    "what they were shown" is a fact about the past that must not change under them.
//!
//! No wall clock here (testing §3): every timestamp is injected by the host.

use serde::{Deserialize, Serialize};

use crate::error::CasesError;

/// The store table request rows live in. `case_id`, `party_id`, `status` and `token_hash` are
/// `data` fields, so every read this plane needs is an equality filter on one of them.
pub const TABLE: &str = "case_request";

/// What we are asking the party FOR. A closed set: each value changes the words in the mail, the
/// reply kinds that make sense, and what a reply does to the case's workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ask {
    /// Price the job.
    Quote,
    /// Come to site.
    Attend,
    /// Confirm something is done / true.
    Confirm,
    /// Tell us something we do not know.
    Info,
}

/// Where the ASK is. Distinct from [`Delivery`] (where the email is) — see the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    /// The link was minted and the effect staged. The starting state — never "queued": that word
    /// belongs to the mail, not the ask.
    Sent,
    /// The party opened the link at least once.
    Opened,
    /// The party answered. Terminal for the round trip.
    Replied,
    /// The window closed with no reply.
    Expired,
    /// We took it back (the job was cancelled, or we asked the wrong party). Terminal.
    Withdrawn,
}

impl RequestStatus {
    /// True for a status the token may still act on. `Replied` is deliberately NOT here: a second
    /// reply after the window is refused, and the gateway answers the same opaque 410 it gives a
    /// withdrawn or expired ask.
    pub fn is_live(self) -> bool {
        matches!(self, RequestStatus::Sent | RequestStatus::Opened)
    }
}

/// What happened to the EMAIL. Mirrors the outbox effect's outcome **and the provider kind**
/// (resolved decision 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// Staged in the outbox, not yet attempted.
    Queued,
    /// A real transport accepted it. The ONLY value that claims a mailbox was reached.
    Sent,
    /// A logging provider acknowledged it — the mail was written to the node's log and DROPPED.
    /// This is what a dev node yields, and saying `sent` here is the exact lie this enum exists to
    /// prevent.
    Logged,
    /// The effect was dead-lettered: nobody was mailed and nobody will be.
    Failed,
}

/// What the party said. A closed set because each kind drives a documented workflow transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyKind {
    /// "Yes, we will do it."
    Accept,
    /// "Here is the price." Carries `amount`/`currency` and sets the case's `cost_to_fix`.
    Quote,
    /// "We will be there at …" Carries `eta_ts`.
    Eta,
    /// "It is done."
    Done,
    /// "We cannot answer without …" — the ball comes back to us.
    NeedInfo,
    /// "No."
    Decline,
}

/// The party's answer. One per request (a replayed reply upserts the same body — see
/// [`crate::request_reply`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    pub kind: ReplyKind,
    /// The quoted amount, for [`ReplyKind::Quote`]. An `f64` for the same reason the case's money
    /// fields are: a price is a real quantity in the workspace's currency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    /// The currency the amount is in. An opaque string (rule 10 — lb ships no currency list).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// When they will attend, for [`ReplyKind::Eta`] (epoch-ms, injected).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_ts: Option<u64>,
    /// Free text. Bounded by [`MAX_REPLY_TEXT_BYTES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Asset ids of files the party attached. Each id is ONE record-id segment (a `.` in an asset
    /// id breaks `store:asset/*:write` — the trap this crate refuses to re-enter).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<String>,
}

/// One plotted series as it was shown to the party — the rows themselves, not a query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BriefSeries {
    /// Display label.
    pub name: String,
    /// `[timestamp_ms, value]` pairs, in order. Carried literally because the token principal holds
    /// exactly two caps and would be `Denied` on every query-plane verb — the page cannot fetch a
    /// chart, so the chart travels with the ask.
    #[serde(default)]
    pub points: Vec<[f64; 2]>,
}

/// The evidence snapshot the party is shown.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BriefEvidence {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<BriefSeries>,
    /// The line the finding crossed, if there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// The unit of the plotted values (`kWh`, `degC`) — opaque.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// **What the party is shown, frozen at send time.** Everything on the contractor's page that is
/// not derivable from the case comes from here.
///
/// It exists as one nested field rather than four loose ones so its nature is stated once: this is
/// a snapshot, authored by the sender, immutable afterwards. Re-deriving it at view time would mean
/// running the case's evidence queries under *somebody's* authority on behalf of a principal that
/// holds none — privilege laundering with a chart on top.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Brief {
    /// What we are actually asking, in the sender's words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_text: Option<String>,
    /// The equipment/asset label the work is on. Opaque workspace data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    /// The currency a quote is expected in. Opaque (rule 10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<BriefEvidence>,
}

/// A durable request record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseRequest {
    /// Stable id (ULID), unique within the workspace, and the outbox idempotency handle.
    pub id: String,
    pub case_id: String,
    pub party_id: String,
    pub ask: Ask,
    /// SHA-256 (hex) of the raw token. The raw token is never stored — see the module note.
    pub token_hash: String,
    /// After this instant the token is dead (epoch-ms).
    pub expires_ts: u64,
    /// The instant we asked them to answer by. Equal to `expires_ts` today, and a separate field
    /// because they are separate promises: one is a deadline we stated, the other is when the door
    /// physically locks, and a later policy may want a grace period between them.
    pub respond_by: u64,
    pub status: RequestStatus,
    pub delivery: Delivery,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<Reply>,
    pub sent_ts: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_ts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replied_ts: Option<u64>,
    /// How many nudges have actually fired. Written by the nudge verb, never by a caller.
    #[serde(default)]
    pub nudges_sent: u32,
    /// The outbox effect the link email rides on. The handle `delivery` is reconciled from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_id: Option<String>,
    /// The frozen snapshot the party is shown (see [`Brief`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<Brief>,
}

/// The cap on a reply's free text. Generous for a contractor typing what they found, small enough
/// that the reply still fits inside a `case_event`'s 4 KB payload cap beside its metadata.
pub const MAX_REPLY_TEXT_BYTES: usize = 2000;

/// The cap on how many files one reply may carry.
pub const MAX_REPLY_ATTACHMENTS: usize = 10;

/// Validate a reply BEFORE anything is written, so a rejected reply leaves the request exactly as
/// it was and the party can correct and resend.
///
/// The one structural rule beyond the bounds: **a quote must carry an amount.** `ReplyKind::Quote`
/// is the kind that writes money onto the case (`cost_to_fix`), and a quote with no number is a
/// reply that looks like a price and is not one.
pub fn validate_reply(reply: &Reply) -> Result<(), CasesError> {
    if reply.kind == ReplyKind::Quote && reply.amount.is_none() {
        return Err(CasesError::BadInput(
            "a quote reply must carry an amount".into(),
        ));
    }
    if let Some(amount) = reply.amount {
        if !amount.is_finite() || amount < 0.0 {
            return Err(CasesError::BadInput(
                "a quoted amount must be a finite, non-negative number".into(),
            ));
        }
    }
    if let Some(text) = reply.text.as_deref() {
        if text.len() > MAX_REPLY_TEXT_BYTES {
            return Err(CasesError::BadInput(format!(
                "reply text {} bytes exceeds the {MAX_REPLY_TEXT_BYTES}-byte cap — nothing was \
                 written and the request is untouched",
                text.len()
            )));
        }
    }
    if reply.attachments.len() > MAX_REPLY_ATTACHMENTS {
        return Err(CasesError::BadInput(format!(
            "a reply may carry at most {MAX_REPLY_ATTACHMENTS} attachments"
        )));
    }
    for id in &reply.attachments {
        // An asset id is ONE record-id segment. A `.` (or a `:`) in it breaks the
        // `store:asset/{id}:write` capability match — the file uploads and is then unreadable.
        if id.contains('.') || id.contains(':') || id.trim().is_empty() {
            return Err(CasesError::BadInput(format!(
                "attachment id `{id}` must be a single record-id segment (no `.` or `:`)"
            )));
        }
    }
    Ok(())
}
