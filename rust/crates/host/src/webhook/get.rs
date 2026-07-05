//! `webhook.get` — one webhook's full view (webhooks scope). Gated by `mcp:webhook.manage:call`,
//! workspace-first. Returns the same credential-free view as `list` (no hash, no secret, no linked
//! apikey id) — the admin "detail" panel reads this; a future richer detail (last-N hits, error
//! rate) would compose over the hook's `series.read`, not widen this view.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{read, Store};

use super::error::WebhookError;
use super::model::{WebhookRecord, WebhookView, TABLE};

/// One webhook in workspace `ws` as a credential-free view, for `principal`. `NotFound` (opaque
/// to the caller) when the id is absent — the management surface mirrors the public route's no-
/// existence-leak discipline where it can.
pub async fn webhook_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<WebhookView, WebhookError> {
    authorize_tool(principal, ws, "webhook.manage").map_err(|_| WebhookError::Denied)?;
    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or(WebhookError::NotFound)?;
    let rec: WebhookRecord = serde_json::from_value(value).map_err(unexpected)?;
    Ok(WebhookView::from_record(&rec))
}

fn unexpected(e: serde_json::Error) -> WebhookError {
    WebhookError::Store(lb_store::StoreError::Decode(e.to_string()))
}
