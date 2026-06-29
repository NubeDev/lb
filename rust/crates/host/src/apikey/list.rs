//! `apikey.list` — enumerate the workspace's keys as credential-free views (api-keys scope). Gated
//! by `mcp:apikey.manage:call`, workspace-first. Returns each key's id/label/kind/non-secret prefix/
//! status/timing + the assigned role names + the read-only/read-write/custom badge — **never** the
//! hash or secret (asserted in a test). Sorted by `created_ts` then id for a stable table.

use lb_auth::Principal;
use lb_authz::{grant_list, Subject};
use lb_mcp::authorize_tool;
use lb_store::{list as store_list, Store};

use super::error::ApiKeyError;
use super::model::{ApiKeyRecord, ApiKeyView, KIND_DISCRIM, TABLE};

/// Every key in workspace `ws` as credential-free views, for `principal`.
pub async fn apikey_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<ApiKeyView>, ApiKeyError> {
    authorize_tool(principal, ws, "apikey.manage").map_err(|_| ApiKeyError::Denied)?;
    let rows = store_list(store, ws, TABLE, "kind_discrim", KIND_DISCRIM).await?;
    let mut views: Vec<ApiKeyView> = Vec::new();
    for row in rows {
        let rec: ApiKeyRecord = serde_json::from_value(row).map_err(unexpected)?;
        let roles = assigned_roles(store, ws, &Subject::Key(rec.id.clone())).await?;
        let badge = lb_apikey::badge_for_roles(&roles).to_string();
        views.push(ApiKeyView {
            id: rec.id,
            label: rec.label,
            kind: rec.kind,
            prefix: rec.prefix,
            status: rec.status,
            created_ts: rec.created_ts,
            expires_at: rec.expires_at,
            roles,
            badge,
        });
    }
    views.sort_by(|a, b| {
        a.created_ts
            .cmp(&b.created_ts)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(views)
}

/// Read a key subject's assigned role names (its `role:<name>` grants).
pub(crate) async fn assigned_roles(
    store: &Store,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<String>, ApiKeyError> {
    let grants = grant_list(store, ws, subject).await?;
    let mut roles: Vec<String> = grants
        .iter()
        .filter_map(|g| g.strip_prefix("role:").map(str::to_string))
        .collect();
    roles.sort();
    roles.dedup();
    Ok(roles)
}

fn unexpected(e: serde_json::Error) -> ApiKeyError {
    ApiKeyError::Store(lb_store::StoreError::Decode(e.to_string()))
}
