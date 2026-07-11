//! **Device** records — per-workspace-member push token registrations (push-target scope). A
//! member registers their own devices (self-service, self-only); an admin sees counts, not
//! tokens. Tokens are PII-adjacent — never in logs, never returned to other members.

use lb_store::{list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The store table device records live in.
pub const DEVICE_TABLE: &str = "device";

/// The constant `kind` discriminant for `device_list_raw`.
pub const DEVICE_KIND: &str = "device";

/// The push platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Fcm,
    Apns,
    Webpush,
}

/// A device registration — a member's push token for a specific platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// The record id: `device:{sub}:{token_hash}` — unique per (member, token).
    pub id: String,
    /// The member's sub (`user:ada`).
    pub sub: String,
    /// The push platform.
    pub platform: Platform,
    /// The push token / subscription endpoint (PII-adjacent — never in logs).
    pub token: String,
    /// The app id (bundle id / package name).
    #[serde(default)]
    pub app_id: String,
    /// Last seen ts (updated on register).
    pub last_seen: u64,
    /// Disabled flag (auto-removed on provider 410/UNREGISTERED).
    #[serde(default)]
    pub disabled: bool,
    pub kind: String,
}

impl Device {
    pub fn new(
        sub: impl Into<String>,
        platform: Platform,
        token: impl Into<String>,
        now: u64,
    ) -> Self {
        let sub = sub.into();
        let token = token.into();
        let token_hash = short_hash(&token);
        Self {
            id: format!("device:{sub}:{token_hash}"),
            sub,
            platform,
            token,
            app_id: String::new(),
            last_seen: now,
            disabled: false,
            kind: DEVICE_KIND.to_string(),
        }
    }
}

/// A short hash for the device id (8 hex chars of SHA-256 — collision-resistant per member).
fn short_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(s.as_bytes());
    let mut out = String::with_capacity(8);
    for b in &hash[..4] {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}

// ── Raw store verbs ──────────────────────────────────────────────────────────────────────────

/// Upsert a device (idempotent on `(sub, token)` — re-register updates `last_seen`).
pub async fn device_write(store: &Store, ws: &str, device: &Device) -> Result<(), StoreError> {
    let value = serde_json::to_value(device).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, DEVICE_TABLE, &device.id, &value).await
}

/// Get a device by id.
pub async fn device_get_raw(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<Device>, StoreError> {
    match read(store, ws, DEVICE_TABLE, id).await? {
        Some(v) => {
            let device: Device =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(device))
        }
        None => Ok(None),
    }
}

/// List all devices for `sub` in workspace `ws`.
pub async fn device_list_raw(
    store: &Store,
    ws: &str,
    sub: &str,
) -> Result<Vec<Device>, StoreError> {
    let rows = store_list(store, ws, DEVICE_TABLE, "sub", sub).await?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect()
}

/// Disable a device (auto-remove on provider 410/UNREGISTERED). Idempotent.
pub async fn device_disable_raw(store: &Store, ws: &str, id: &str) -> Result<bool, StoreError> {
    let Some(mut device) = device_get_raw(store, ws, id).await? else {
        return Ok(false);
    };
    device.disabled = true;
    let value = serde_json::to_value(&device).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, DEVICE_TABLE, id, &value).await?;
    Ok(true)
}
