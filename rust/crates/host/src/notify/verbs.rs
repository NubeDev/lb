//! `device.register` / `device.list` / `device.remove` + `notify.send` (push-target scope).
//! Device verbs are member-level (self-only — a member registers their own devices).
//! `notify.send` enqueues a push effect to the outbox (gated by `mcp:notify.send:call`).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_outbox::Effect;
use lb_store::Store;
use serde_json::{json, Value};

use super::device::{device_disable_raw, device_list_raw, device_write, Device, Platform};
use super::error::NotifyError;

/// Register (upsert) a device for the calling principal. Self-only — the sub is forced to
/// `principal.sub()`. Gated by `mcp:device.register:call`.
pub async fn device_register(
    store: &Store,
    principal: &Principal,
    ws: &str,
    platform: &str,
    token: &str,
    app_id: Option<&str>,
    now: u64,
) -> Result<(), NotifyError> {
    authorize_tool(principal, ws, "device.register").map_err(|_| NotifyError::Denied)?;
    let plat = match platform {
        "fcm" => Platform::Fcm,
        "apns" => Platform::Apns,
        "webpush" => Platform::Webpush,
        _ => return Err(NotifyError::BadInput("unknown platform".into())),
    };
    let mut device = Device::new(principal.sub(), plat, token, now);
    if let Some(aid) = app_id {
        device.app_id = aid.to_string();
    }
    device_write(store, ws, &device).await?;
    Ok(())
}

/// List the calling principal's own devices. Self-only. Gated by `mcp:device.register:call`.
pub async fn device_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Device>, NotifyError> {
    authorize_tool(principal, ws, "device.register").map_err(|_| NotifyError::Denied)?;
    Ok(device_list_raw(store, ws, principal.sub()).await?)
}

/// Remove (disable) a device by id. Self-only — the device must belong to the caller.
/// Gated by `mcp:device.register:call`.
pub async fn device_remove(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<bool, NotifyError> {
    authorize_tool(principal, ws, "device.register").map_err(|_| NotifyError::Denied)?;
    // Verify ownership: the device's sub must match the caller's.
    let device = super::device::device_get_raw(store, ws, id)
        .await?
        .ok_or(NotifyError::NotFound)?;
    if device.sub != principal.sub() {
        return Err(NotifyError::Denied);
    }
    Ok(device_disable_raw(store, ws, id).await?)
}

/// `notify.send { to: [sub…], title, body, deep_link?, collapse_key?, priority }` — enqueue a
/// push effect for outbox delivery. Gated by `mcp:notify.send:call`. The effect fans out to each
/// recipient's live devices at delivery time (the PushTarget resolves the audience).
pub async fn notify_send(
    store: &Store,
    principal: &Principal,
    ws: &str,
    to: &[String],
    title: &str,
    body: &str,
    deep_link: Option<&str>,
    collapse_key: Option<&str>,
    priority: Option<&str>,
    now: u64,
) -> Result<String, NotifyError> {
    authorize_tool(principal, ws, "notify.send").map_err(|_| NotifyError::Denied)?;
    if to.is_empty() {
        return Err(NotifyError::BadInput("empty audience".into()));
    }
    let effect_id = format!("notify:{}:{}", now, to.first().unwrap());
    let payload = json!({
        "to": to,
        "title": title,
        "body": body,
        "deep_link": deep_link,
        "collapse_key": collapse_key,
        "priority": priority.unwrap_or("normal"),
    });
    let effect = Effect::new(
        &effect_id,
        super::push_target::PUSH_TARGET,
        "notify",
        &payload.to_string(),
        &effect_id,
        now,
    );
    lb_outbox::enqueue(
        store,
        ws,
        "notify",
        &effect_id,
        &json!({ "sender": principal.sub() }),
        &effect,
    )
    .await
    .map_err(|e| NotifyError::Store(e.to_string()))?;
    Ok(effect_id)
}
