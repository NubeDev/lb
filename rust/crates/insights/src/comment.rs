//! `Comment` — one append-only human note on an insight
//! (`docs/scope/insights/insight-triage-scope.md`).
//!
//! The thread is the **human operational log** beside the machine's record: what we found out, what
//! we tried, why last quarter's identical firing was a false alarm. It reuses the occurrence ring's
//! *storage shape* — size-capped, seq-numbered child rows under the parent, never joined into
//! `insight.list` — and deliberately **not** its retention policy.
//!
//! **This is the one place the triage plane diverges from the ring it borrows from, and the
//! divergence is the point (resolved decision 4).** Eviction is right for firings (machine-generated,
//! individually low-value, unbounded) and a trust failure for human notes: "we wrote it down and the
//! platform deleted it" loses a customer in a way "your note was rejected, the thread is full" never
//! does. So the bound is two REFUSALS, never a silent drop:
//!   - [`MAX_COMMENT_BYTES`] — a per-comment size cap that rejects the write, and
//!   - [`MAX_COMMENTS_PER_INSIGHT`] — a per-insight COUNT cap that refuses the append rather than
//!     evicting the oldest. A thread that long means the finding should have become a work item, and
//!     the platform should say so.
//!
//! Comments are purged only **with** their parent insight (`delete` cascades them) — never before it,
//! and never on their own schedule.
//!
//! One responsibility: the comment shape + its guards.

use serde::{Deserialize, Serialize};

use crate::error::InsightsError;

/// The store table comment rows live in. One table per workspace namespace; `insight_id` is a
/// `data` field (so the thread read is a filter by parent, not a table per insight) — the same
/// layout the occurrence ring uses.
pub const TABLE: &str = "insight_comment";

/// The hard size cap on one comment's `text`. A comment is an operational note — a paragraph or
/// three, not a report; anything longer belongs in a linked document. Exceeding it rejects the
/// WHOLE call before any write (never silent truncation), the contract
/// `validate_occurrence_size`/`validate_evidence_size`/`validate_analysis` already hold.
pub const MAX_COMMENT_BYTES: usize = 4 * 1024;

/// The hard cap on how many comments one insight may carry. Reaching it **refuses the append** —
/// it does NOT evict the oldest (resolved decision 4; see the module doc). Generous enough that no
/// honest triage thread hits it, small enough that a runaway integration is bounded.
pub const MAX_COMMENTS_PER_INSIGHT: usize = 200;

/// One human note on an insight, appended and never edited or deleted (v1 has no comment
/// edit/delete — a correction is another comment, so the thread reads as what was actually known
/// when).
///
/// **Serialized field names matter**, for the same reason the occurrence row's do: these rows are
/// written through `lb_store::write`, so the stored body carries an `insight_id` field beside the
/// comment's own. The monotone per-insight sequence serializes as **`cseq`** to stay clear of any
/// store-injected `seq`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// Monotone per-insight sequence, host-assigned (previous max + 1). Stable: because nothing
    /// evicts and nothing deletes, a `seq` is a permanent handle to a note.
    #[serde(rename = "cseq")]
    pub seq: u64,
    /// The note itself. ≤ [`MAX_COMMENT_BYTES`] or the whole call rejects.
    pub text: String,
    /// Who wrote it — **always** the principal's `sub`, host-stamped, never caller-supplied (the
    /// `ack.rs` precedent: a caller cannot forge another operator's note any more than another
    /// reviewer's ack).
    pub author: String,
    /// Logical timestamp of the append (no wall-clock — testing §3).
    pub ts: u64,
}

/// Validate one comment's `text` against the per-comment size cap WITHOUT writing. The comment verb
/// calls this up front so an oversize note rejects the call and leaves no partial row — and so the
/// pre-existing thread is untouched by a refused write.
///
/// An empty/whitespace-only `text` is refused too: an empty note is indistinguishable from a
/// mis-click, and the thread's value is that every row says something.
pub fn validate_comment(text: &str) -> Result<(), InsightsError> {
    if text.trim().is_empty() {
        return Err(InsightsError::BadInput(
            "comment text is empty — a note must say something".into(),
        ));
    }
    let bytes = text.len();
    if bytes > MAX_COMMENT_BYTES {
        return Err(InsightsError::BadInput(format!(
            "comment {bytes} bytes exceeds the {MAX_COMMENT_BYTES}-byte cap — a comment is an operational note, not a report; link a document for anything longer"
        )));
    }
    Ok(())
}
