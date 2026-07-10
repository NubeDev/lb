//! `apikey.get` — one key's full view, including its resolved cap set (api-keys scope). Gated by
//! `mcp:apikey.manage:call`, workspace-first. Returns the credential-free view PLUS the resolved
//! caps — still no hash/secret. The resolved caps are what `apikey.authenticate` builds the
//! principal from, so `get` shows the exact authority a key carries.

use std::collections::BTreeSet;

use lb_auth::Principal;
use lb_authz::Subject;
use lb_mcp::authorize_tool;
use lb_store::{read, Store};

use super::error::ApiKeyError;
use super::list::assigned_roles;
use super::model::{ApiKeyFull, ApiKeyRecord, TABLE};
use crate::authz::resolve_subject_caps_live as resolve_subject_caps;

/// The full view of key `id` in `ws`, for `principal`, including the resolved cap set.
pub async fn apikey_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<ApiKeyFull, ApiKeyError> {
    authorize_tool(principal, ws, "apikey.manage").map_err(|_| ApiKeyError::Denied)?;
    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or(ApiKeyError::NotFound)?;
    let rec: ApiKeyRecord = serde_json::from_value(value).map_err(unexpected)?;

    let roles = assigned_roles(store, ws, &Subject::Key(rec.id.clone())).await?;
    let badge = lb_apikey::badge_for_roles(&roles).to_string();

    let mut caps: BTreeSet<String> = BTreeSet::new();
    resolve_subject_caps(store, ws, &Subject::Key(rec.id.clone()), &mut caps).await?;

    Ok(ApiKeyFull {
        id: rec.id,
        label: rec.label,
        kind: rec.kind,
        prefix: rec.prefix,
        status: rec.status,
        created_ts: rec.created_ts,
        expires_at: rec.expires_at,
        roles,
        badge,
        caps: caps.into_iter().collect(),
    })
}

fn unexpected(e: serde_json::Error) -> ApiKeyError {
    ApiKeyError::Store(lb_store::StoreError::Decode(e.to_string()))
}
