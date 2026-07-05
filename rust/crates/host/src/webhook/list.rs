//! `webhook.list` — enumerate the workspace's webhooks as credential-free views (webhooks scope).
//! Gated by `mcp:webhook.manage:call`, workspace-first. Returns each hook's id/name/series/auth
//! mode/URL path/status/timing — **never** the hash, the linked apikey id, or the shared secret
//! (asserted in a test). Sorted by `created_ts` then id for a stable table.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{list as store_list, Store};

use super::error::WebhookError;
use super::model::{WebhookRecord, WebhookView, KIND_DISCRIM, TABLE};

/// Every webhook in workspace `ws` as credential-free views, for `principal`.
pub async fn webhook_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<WebhookView>, WebhookError> {
    authorize_tool(principal, ws, "webhook.manage").map_err(|_| WebhookError::Denied)?;
    let rows = store_list(store, ws, TABLE, "kind_discrim", KIND_DISCRIM).await?;
    let mut views: Vec<WebhookView> = Vec::new();
    for row in rows {
        let rec: WebhookRecord = serde_json::from_value(row).map_err(unexpected)?;
        views.push(WebhookView::from_record(&rec));
    }
    views.sort_by(|a, b| {
        a.created_ts
            .cmp(&b.created_ts)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(views)
}

fn unexpected(e: serde_json::Error) -> WebhookError {
    WebhookError::Store(lb_store::StoreError::Decode(e.to_string()))
}
