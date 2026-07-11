//! The **push outbox target** — an `impl Target` that fans out push notifications to each
//! recipient's live devices (push-target scope). The provider is the one sanctioned external (a
//! true external — FCM/APNs/WebPush HTTP APIs). Behind one trait (`PushProvider`) in one named
//! file; the test impl records sends (testing-scope §0).
//!
//! Shipped here: the trait, the `Target` adapter, and the recording fake. The WebPush (VAPID)
//! HTTP adapter is DEFERRED — a product host wires its own `PushProvider` impl and registers
//! `PushTarget` with `spawn_relay_reactors` at boot (the same wiring contract as `EmailTarget`);
//! credentials via `secrets/` mediation. The target resolves the audience to live devices of
//! members of the effect's workspace only, respects per-member quiet-hours prefs, dedups per
//! device across outbox retries (`delivered.rs`), and maps provider errors (token-gone →
//! auto-disable; transient → outbox retry of the failures only).

use async_trait::async_trait;
use lb_outbox::Effect;
use lb_prefs::get_user_prefs;
use lb_store::Store;
use serde::Deserialize;
use std::sync::Mutex;

use super::delivered::{delivered_check, delivered_mark};
use super::device::{device_disable_raw, device_list_raw, Device};
use crate::outbox::Target;

/// The outbox target string for push delivery.
pub const PUSH_TARGET: &str = "push";

/// The push provider — the one sanctioned external (push-target scope). A product host wires its
/// own impl (WebPush/FCM/APNs); the test impl records sends. The trait is in this one named file.
#[async_trait]
pub trait PushProvider: Send + Sync {
    /// Send a push notification to one device. Returns `Ok(())` on success, `Err(msg)` on
    /// transient failure (the outbox retries). A `PushError::TokenGone` should auto-disable the
    /// device (the target handles this).
    async fn send(&self, device: &Device, payload: &PushPayload) -> Result<(), PushError>;
}

/// The push payload (deserialized from the outbox effect).
#[derive(Debug, Clone, Deserialize)]
pub struct PushPayload {
    pub to: Vec<String>,
    pub title: String,
    pub body: String,
    /// Catalog keys for a localized notification (release scope, i18n gap c): when set, the
    /// target renders title/body per-recipient through the prefs catalog in THEIR `language`
    /// pref at deliver time. The literal `title`/`body` above are the untranslated compat path.
    #[serde(default)]
    pub title_key: Option<String>,
    #[serde(default)]
    pub body_key: Option<String>,
    /// The render args for the catalog keys.
    #[serde(default)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub deep_link: Option<String>,
    /// Provider-side collapse handle (WebPush `Topic` / FCM `collapse_key`) — forwarded to the
    /// provider so stacked notifications collapse on the device. Retry dedup is the delivered
    /// marker (`delivered.rs`), keyed by the effect's idempotency key, NOT this.
    #[serde(default)]
    pub collapse_key: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    /// The workspace the effect was enqueued in — embedded by `notify.send` (verbs.rs), the same
    /// pattern as the email target's payload `workspace`. `deliver` FAILS if absent: the target
    /// must never guess a workspace (rule 6, the hard wall).
    #[serde(default)]
    pub workspace: Option<String>,
}

/// Provider errors.
#[derive(Debug)]
pub enum PushError {
    /// The token is no longer valid (410/UNREGISTERED) — the target auto-disables the device.
    TokenGone,
    /// A transient failure — the outbox retries with backoff.
    Transient(String),
}

/// The push `Target` adapter — resolves the audience to live devices, respects quiet-hours, calls
/// the provider per device, auto-disables on `TokenGone`, reports per-device outcomes.
pub struct PushTarget {
    provider: Box<dyn PushProvider>,
    store: Store,
}

impl PushTarget {
    pub fn new(provider: Box<dyn PushProvider>, store: Store) -> Self {
        Self { provider, store }
    }
}

impl Target for PushTarget {
    fn deliver(
        &self,
        effect: &Effect,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        let payload_str = effect.payload.clone();
        let effect_id = effect.id.clone();
        let idempotency_key = effect.idempotency_key.clone();
        let effect_ts = effect.ts;
        let provider = &self.provider;
        let store = self.store.clone();
        async move {
            let payload: PushPayload = serde_json::from_str(&payload_str)
                .map_err(|e| format!("push target: bad payload: {e}"))?;

            // The workspace is embedded in the payload by `notify.send` at enqueue time (the same
            // pattern as the email target). Absent ⇒ fail the effect — never guess a ws (rule 6).
            let ws = payload
                .workspace
                .as_deref()
                .filter(|w| !w.is_empty())
                .ok_or("push target: payload missing workspace — refusing to guess (rule 6)")?
                .to_string();

            // Retry-dedup key: the outbox's own idempotency handle (falls back to the effect id).
            let dedup_key = if idempotency_key.is_empty() {
                effect_id.clone()
            } else {
                idempotency_key
            };

            let mut errors = Vec::new();
            for sub in &payload.to {
                // Workspace isolation: the audience is resolved to members of THIS ws only —
                // a sub outside it is silently dropped (push-target scope, tested).
                match lb_authz::membership_is_member(&store, &ws, sub).await {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(e) => return Err(format!("push target: membership check: {e}")),
                }

                // Check quiet-hours prefs (whole-fold axis on Prefs).
                let recipient_prefs = get_user_prefs(&store, &ws, sub)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                if recipient_prefs.push_muted == Some(true) {
                    continue; // suppressed — not an error.
                }

                // Per-recipient localization (release scope, i18n gap c): when the payload names
                // catalog keys, render them in THIS recipient's resolved language (their pref →
                // en fallback). The literal title/body are the untranslated compat path.
                let localized = if payload.title_key.is_some() || payload.body_key.is_some() {
                    let resolved = lb_prefs::resolve(&[recipient_prefs.clone()]);
                    let empty = std::collections::BTreeMap::new();
                    let title = match payload.title_key.as_deref().filter(|k| !k.is_empty()) {
                        Some(key) => {
                            lb_prefs::render_message(key, &payload.args, &empty, &resolved).text
                        }
                        None => payload.title.clone(),
                    };
                    let body = match payload.body_key.as_deref().filter(|k| !k.is_empty()) {
                        Some(key) => {
                            lb_prefs::render_message(key, &payload.args, &empty, &resolved).text
                        }
                        None => payload.body.clone(),
                    };
                    PushPayload {
                        title,
                        body,
                        ..payload.clone()
                    }
                } else {
                    payload.clone()
                };

                // Resolve the recipient's live devices.
                let devices = device_list_raw(&store, &ws, sub)
                    .await
                    .map_err(|e| format!("push target: device list: {e}"))?;

                for device in devices {
                    if device.disabled {
                        continue;
                    }
                    // At-least-once dedup: skip a device this effect already reached, so an
                    // outbox retry only re-sends the failures (scope Risks).
                    match delivered_check(&store, &ws, &dedup_key, &device.id).await {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(e) => return Err(format!("push target: delivered check: {e}")),
                    }
                    match provider.send(&device, &localized).await {
                        Ok(()) => {
                            delivered_mark(&store, &ws, &dedup_key, &device.id, effect_ts)
                                .await
                                .map_err(|e| format!("push target: delivered mark: {e}"))?;
                        }
                        Err(PushError::TokenGone) => {
                            // Auto-disable the device — terminal for this device, not an error
                            // (retrying a gone token is pointless).
                            let _ = device_disable_raw(&store, &ws, &device.id).await;
                        }
                        Err(PushError::Transient(msg)) => {
                            errors.push(format!("{}: {msg}", device.id));
                        }
                    }
                }
            }
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors.join("; "))
            }
        }
    }
}

/// Any `Arc<P>` is itself a provider — lets a test keep a handle on the recording fake after
/// boxing it into the target.
#[async_trait]
impl<P: PushProvider + ?Sized> PushProvider for std::sync::Arc<P> {
    async fn send(&self, device: &Device, payload: &PushPayload) -> Result<(), PushError> {
        (**self).send(device, payload).await
    }
}

/// The **default boot provider** when no real one is configured (release scope, gap 1): logs the
/// send and acks it. A product host replaces it via the boot provider seam.
pub struct LoggingPushProvider;

#[async_trait]
impl PushProvider for LoggingPushProvider {
    async fn send(&self, device: &Device, payload: &PushPayload) -> Result<(), PushError> {
        tracing::info!(device = %device.id, title = %payload.title, "push (no provider configured — logged only)");
        Ok(())
    }
}

/// The recording test impl — records every successful send for assertion, and can script per-device
/// failures (token-gone / transient) so the target's error mapping is exercised through the REAL
/// relay. The ONE sanctioned fake (a true external behind a trait, testing-scope §0).
pub struct RecordingPushProvider {
    sends: Mutex<Vec<RecordedPush>>,
    token_gone: Mutex<std::collections::HashSet<String>>,
    fail_next: Mutex<std::collections::HashSet<String>>,
}

#[derive(Debug, Clone)]
pub struct RecordedPush {
    pub device_id: String,
    pub sub: String,
    pub title: String,
    pub body: String,
    pub collapse_key: Option<String>,
}

impl Default for RecordingPushProvider {
    fn default() -> Self {
        Self {
            sends: Mutex::new(Vec::new()),
            token_gone: Mutex::new(std::collections::HashSet::new()),
            fail_next: Mutex::new(std::collections::HashSet::new()),
        }
    }
}

impl RecordingPushProvider {
    pub fn sends(&self) -> Vec<RecordedPush> {
        self.sends.lock().unwrap().clone()
    }

    /// Script this device to always return `PushError::TokenGone` (410/UNREGISTERED).
    pub fn mark_token_gone(&self, device_id: &str) {
        self.token_gone.lock().unwrap().insert(device_id.into());
    }

    /// Script this device's NEXT send to fail transiently (then succeed).
    pub fn fail_next(&self, device_id: &str) {
        self.fail_next.lock().unwrap().insert(device_id.into());
    }
}

#[async_trait]
impl PushProvider for RecordingPushProvider {
    async fn send(&self, device: &Device, payload: &PushPayload) -> Result<(), PushError> {
        if self.token_gone.lock().unwrap().contains(&device.id) {
            return Err(PushError::TokenGone);
        }
        if self.fail_next.lock().unwrap().remove(&device.id) {
            return Err(PushError::Transient("scripted transient failure".into()));
        }
        self.sends.lock().unwrap().push(RecordedPush {
            device_id: device.id.clone(),
            sub: device.sub.clone(),
            title: payload.title.clone(),
            body: payload.body.clone(),
            collapse_key: payload.collapse_key.clone(),
        });
        Ok(())
    }
}
