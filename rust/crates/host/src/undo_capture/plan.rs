//! Classify a dispatched tool call for auto-capture: is it a mutation, and if so can the dispatch
//! seam name the single record it touches (the v1 reversible floor) — or is it non-generic
//! (`docs/scope/undo/undo-scope.md` "non-generic capture")?
//!
//! The host cannot inspect an arbitrary tool to learn what it touched, so v1 captures only the
//! generic single-record host verbs whose `(table, id)` the seam *can* derive from the call's
//! arguments. Everything else that mutates is marked **not-undoable** rather than partially
//! captured (the scope's hard rule: never a partial raw restore that leaves invariants broken).
//! Read-only verbs are skipped entirely (no journal noise). The authoritative reversible/
//! irreversible split is still decided later from **runtime outbox taint** — this plan only says
//! "where, if anywhere, is the before-image" — so a verb planned `Reversible` whose transaction
//! reaches the outbox is still journaled irreversible (taint wins; see `capture.rs`).

use serde_json::Value;

use lb_inbox::{record_id, TABLE as INBOX_TABLE};

/// The store table for doc assets (must match `lb_assets::doc::TABLE`).
const DOC_TABLE: &str = "doc";
/// The store table for binary assets (must match `lb_assets::asset::TABLE`).
const ASSET_TABLE: &str = "asset";

/// How the dispatch seam should capture a tool call.
pub(crate) enum CapturePlan {
    /// A self-contained single-record upsert the seam can snapshot: capture this `(table, id)`'s
    /// before-image, run the call, journal it reversible (unless tainted irreversible at runtime).
    Reversible { table: String, id: String },
    /// A mutating call whose touched set the seam cannot generically see (raw `store.query`, an
    /// arbitrary extension tool, a multi-record/derived-state verb). Journal it **not-undoable**.
    NonGeneric,
    /// Not a mutation (a read/list/schema verb, or a non-mutating control). Do not journal.
    NotMutating,
}

/// Derive the [`CapturePlan`] for `qualified_tool` from its JSON `input`.
///
/// v1 reversible floor: `inbox.record` (an idempotent single-record upsert at
/// `inbox:{channel}__{id}`). Other writes-that-mutate are `NonGeneric`; reads are `NotMutating`.
/// Extension (`<ext>.<tool>`) calls are `NonGeneric` when they mutate — but we cannot know that
/// from here, so they default to `NonGeneric` only if they reached the store/outbox at runtime;
/// the caller treats a `NonGeneric` plan with **no** runtime taint and **no** mutation signal as a
/// no-op (see `capture.rs`). Keeping the mutating-verb allowlist explicit here is what makes "what
/// is auto-captured" auditable in one place.
pub(crate) fn plan_capture(qualified_tool: &str, input: &Value) -> CapturePlan {
    match qualified_tool {
        // The v1 generic reversible floor: a single-record inbox upsert.
        "inbox.record" => match (str_arg(input, "channel"), str_arg(input, "id")) {
            (Some(channel), Some(id)) => CapturePlan::Reversible {
                table: INBOX_TABLE.to_string(),
                id: record_id(channel, id),
            },
            // Missing args: the call will fail anyway; nothing to capture.
            _ => CapturePlan::NotMutating,
        },

        // document-store scope: a markdown **save** is a single-record doc upsert, so it is the
        // same reversible floor as an inbox record — the before-image is the prior `doc:{id}`
        // (empty for a first save), captured here and restored by the journal on undo. The
        // doc-link/embed edges a markdown save ALSO writes are derived index, not part of the
        // captured record; restoring the body leaves the index slightly ahead until the next
        // save reconciles — an acceptable v1 trade (orphan-edge pruning is the deferred GC job).
        "assets.put_doc" | "assets.delete_doc" => match str_arg(input, "id") {
            Some(id) => CapturePlan::Reversible {
                table: DOC_TABLE.to_string(),
                id: id.to_string(),
            },
            _ => CapturePlan::NotMutating,
        },
        // A binary-asset put/delete is likewise a single-record (tombstone) upsert → reversible.
        "assets.put_asset" | "assets.delete_asset" => match str_arg(input, "id") {
            Some(id) => CapturePlan::Reversible {
                table: ASSET_TABLE.to_string(),
                id: id.to_string(),
            },
            _ => CapturePlan::NotMutating,
        },

        // document-store sharing/links: these write ONLY relation edges (a different table, and
        // a `unrelate` is the reverse — not yet a captured verb). Marked non-generic so a save
        // that also shares is still captured by its doc upsert above; a pure share is journaled
        // not-undoable (honest: restoring the edge set is a separate concern).
        "assets.share_doc" | "assets.unshare_doc" | "assets.link_doc" => CapturePlan::NonGeneric,

        // Mutating host verbs whose footprint is not a single nameable record (or that produce
        // motion): marked not-undoable. `outbox.enqueue`/`inbox.resolve` reach the outbox and will
        // also taint at runtime; flagging them here keeps the read-vs-write split explicit.
        "outbox.enqueue" | "inbox.resolve" => CapturePlan::NonGeneric,

        // Raw SQL escape hatch — the scope's canonical non-generic case (touched set unknowable).
        // `store.query` is parse-allowlisted read-only today, but classify it not-undoable so a
        // future write-capable variant is never silently treated as reversible.
        "store.query" => CapturePlan::NonGeneric,

        // Read-only / non-mutating host verbs: no journal entry. Includes the document-store
        // read/list verbs (`assets.get_doc`, `assets.list_docs`, `assets.get_asset`,
        // `assets.list_assets`, `assets.backlinks`) and the skill load.
        t if is_read_only(t) => CapturePlan::NotMutating,

        // Anything else (an `<ext>.<tool>` target, an unrecognised host verb): if it mutates we
        // cannot see its touched set, so treat it as non-generic. The caller suppresses the
        // not-undoable marker when the call produced no runtime mutation signal at all (a pure
        // read extension tool), so we never spam the journal for reads.
        _ => CapturePlan::NonGeneric,
    }
}

/// Host verbs known to be read-only (so a plain read is never journaled). Conservative: only verbs
/// we are sure never mutate. Anything not listed falls through to `NonGeneric` and is suppressed
/// unless it actually mutated.
fn is_read_only(tool: &str) -> bool {
    matches!(
        tool,
        "outbox.status" | "inbox.list" | "store.schema" | "history.list" | "history.compensations"
            | "assets.get_doc" | "assets.list_docs" | "assets.get_asset" | "assets.list_assets"
            | "assets.backlinks" | "assets.load_skill" | "assets.list_granted_skills"
    ) || tool.starts_with("series.")
        || tool.starts_with("host.")
        || tool.starts_with("dashboard.") && tool.ends_with(".get")
        || tool.starts_with("dashboard.") && tool.ends_with(".list")
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(|v| v.as_str())
}
