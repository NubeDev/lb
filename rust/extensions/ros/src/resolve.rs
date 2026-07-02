//! Resolve a `ros_uuid` to a live `RosApi` — the bridge from a stored connection shadow + its
//! `lb-secrets`-held token to a usable REST client. This is where the config record (base_url) and the
//! mediated secret (token) are combined; the token lives ONLY here for the moment of construction and
//! is never returned to a caller, logged, or stored in the shadow.
//!
//! The seam is a trait (`RosApiFactory`) so a test injects a `RosFake` for a connection without a live
//! box, while production builds a `RealRosApi`. Everything above (the handlers) takes a factory and is
//! exercised for real against the store/secrets — only the box is faked, behind this one seam plus the
//! `RosApi` trait it returns.

use async_trait::async_trait;
use serde_json::json;

use crate::host::{HostCtx, HostError};
use crate::ros_api::RosApi;

/// The secret path a connection's `External` token is stashed at (scope: `secret:ros/{uuid}/token`).
pub fn token_path(ros_uuid: &str) -> String {
    format!("ros/{ros_uuid}/token")
}

/// Builds a `RosApi` for a resolved connection (base_url + token). Production returns `RealRosApi`;
/// a test returns a canned `RosFake`. The factory is `Send + Sync` so handlers can hold `&dyn`.
#[async_trait]
pub trait RosApiFactory: Send + Sync {
    /// Build the API for `(ros_uuid, base_url, token)`. `ros_uuid` lets a fake serve per-connection
    /// canned trees; the real impl ignores it (base_url + token fully determine the client).
    async fn build(
        &self,
        ros_uuid: &str,
        base_url: &str,
        token: &str,
    ) -> Result<Box<dyn RosApi>, HostError>;
}

/// The production factory — builds a real `rust-ros`-backed client.
pub struct RealFactory;

#[async_trait]
impl RosApiFactory for RealFactory {
    async fn build(
        &self,
        _ros_uuid: &str,
        base_url: &str,
        token: &str,
    ) -> Result<Box<dyn RosApi>, HostError> {
        let api = crate::ros_api::RealRosApi::new(base_url, token)
            .map_err(|e| HostError::BadResponse(e.to_string()))?;
        Ok(Box::new(api))
    }
}

/// Stash a connection's token via `lb-secrets` (`secret.set`). Called on `create`/`update` when a
/// token is supplied; never on a read. The value is the raw `External` token — mediated by the host,
/// never returned by `get`/`list`.
pub async fn set_token(host: &HostCtx, ros_uuid: &str, token: &str) -> Result<(), HostError> {
    host.client()
        .call_tool(
            "secret.set",
            json!({ "path": token_path(ros_uuid), "value": token }),
        )
        .await?;
    Ok(())
}

/// Delete a connection's token (`secret.delete`), on `ros.delete`.
pub async fn delete_token(host: &HostCtx, ros_uuid: &str) -> Result<(), HostError> {
    host.client()
        .call_tool("secret.delete", json!({ "path": token_path(ros_uuid) }))
        .await?;
    Ok(())
}

/// Fetch a connection's token (`secret.get`) for the moment of client construction. Surfaced errors
/// stay typed (`Denied`/callback); the token is dropped as soon as the client is built.
pub async fn get_token(host: &HostCtx, ros_uuid: &str) -> Result<String, HostError> {
    let out = host
        .client()
        .call_tool("secret.get", json!({ "path": token_path(ros_uuid) }))
        .await?;
    out.get("value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| HostError::BadResponse("secret.get: no value".into()))
}

/// Resolve `ros_uuid` → a live `RosApi`: read the shadow (for `base_url`), read the token
/// (`lb-secrets`), and build the client via the factory. `Ok(None)` when the connection is unknown.
pub async fn resolve_api(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    ros_uuid: &str,
) -> Result<Option<Box<dyn RosApi>>, HostError> {
    let shadow = match crate::shadow::get_ros(host, ros_uuid).await? {
        Some(s) => s,
        None => return Ok(None),
    };
    let token = get_token(host, ros_uuid).await?;
    let api = factory.build(ros_uuid, &shadow.base_url, &token).await?;
    Ok(Some(api))
}
