//! Create/update a doc asset — the **save** verb (document-store scope). Requires
//! `store:doc/{id}:write` (capability-first, §3.5), workspace-first. The doc is owned by the
//! caller; ownership is set from the principal, never from caller-supplied input (so a caller
//! can't forge another owner). State only — a doc is durable, no motion.
//!
//! A markdown save is the document-store's link-graph write: the body's `lb-doc://{id}` and
//! `lb-asset://{id}` references are extracted and written as `doclink` (doc→doc) and `embed`
//! (doc→asset) edges in the SAME logical save, so backlinks and embed re-gating work. The
//! edges are best-effort additions to the save — a store error mid-edge does NOT fail the save
//! (the doc is the source of truth; the edges are a derived index, reconciled on the next
//! save). Stale-edge pruning (a removed reference) is the orphan-GC job's concern, deferred.

use lb_assets::{put_doc as store_put_doc, relate, ContentType, Doc};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;
use super::links::{asset_embeds, doc_links};
use super::visibility::{DOCLINK, EMBED};

/// Create or update doc `id` in workspace `ws` as `principal`. `owner` is forced to
/// `principal.sub` (the caller owns what they create). `content_type` types the body;
/// `tags` is the flat discovery list. Returns the stored doc.
pub async fn put_doc(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    content: &str,
    content_type: ContentType,
    tags: &[String],
    ts: u64,
) -> Result<Doc, AssetError> {
    authorize_doc(principal, ws, id, Action::Write)?;
    let doc = Doc::new(id, principal.sub(), title, content, ts)
        .with_content_type(content_type)
        .with_tags(tags.to_vec());
    // The doc is the source of truth — persist it first, then derive the link index from it.
    store_put_doc(store, ws, &doc).await?;
    // Markdown saves seed the link graph; text saves carry no internal refs. The edges are a
    // derived index written best-effort AFTER the save lands, so a failed edge never rolls back
    // a successful save (and a failed save leaves no orphan edges).
    if matches!(content_type, ContentType::Markdown) {
        for target in doc_links(content) {
            let _ = relate(store, ws, DOCLINK, id, &target).await;
        }
        for asset in asset_embeds(content) {
            let _ = relate(store, ws, EMBED, id, &asset).await;
        }
    }
    Ok(doc)
}