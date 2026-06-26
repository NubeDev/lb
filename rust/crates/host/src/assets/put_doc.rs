//! Create/update a doc asset — the write verb. Requires `store:doc/{id}:write`
//! (capability-first, §3.5), workspace-first. The doc is owned by the caller; ownership is set
//! from the principal, never from caller-supplied input (so a caller can't forge another
//! owner). State only — a doc is durable, no motion.

use lb_assets::{put_doc as store_put_doc, Doc};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;

/// Create or update doc `id` in workspace `ws` as `principal`. `owner` is forced to
/// `principal.sub` (the caller owns what they create). Returns the stored doc.
pub async fn put_doc(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    content: &str,
    ts: u64,
) -> Result<Doc, AssetError> {
    authorize_doc(principal, ws, id, Action::Write)?;
    let doc = Doc::new(id, principal.sub(), title, content, ts);
    store_put_doc(store, ws, &doc).await?;
    Ok(doc)
}
