//! The webhook helper — the **third-party caller path**. A service the admin
//! has shared a webhook secret with signs the raw body and POSTs to
//! `/hooks/{ws}/{id}`. The gateway verifies the HMAC over the **exact received
//! bytes** (see `routes/webhook.rs`), so this helper takes bytes, never a
//! string — HMAC over a re-serialized body is the single most common
//! webhook-integration bug (pinned in
//! `webhook_routes_test.rs::signature_mode_body_tamper_breaks_signature`).

use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::client::{decode, Client};
use crate::error::LbError;

type HmacSha256 = Hmac<Sha256>;

/// `POST /hooks/{ws}/{id}` reply (see `routes/webhook.rs::Accepted`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookAccepted {
    pub id: String,
    pub series: String,
    pub seq: u64,
}

/// Sign `body` with `secret` (the shared secret the admin got at webhook
/// create). Returns the value to send in the admin-picked header (default
/// `X-Signature`), formatted as `sha256=<64 hex>` — exactly what the gateway's
/// `signature` mode expects.
///
/// **Body must be the raw bytes you POST** — sign-then-reformat breaks the
/// signature.
pub fn sign_webhook(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("hmac takes any key length");
    mac.update(body);
    let bytes = mac.finalize().into_bytes();
    let mut hex = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        hex.push_str(&format!("{:02x}", b));
    }
    format!("sha256={hex}")
}

/// `POST /hooks/{ws}/{id}` with caller-supplied headers. For `signature` mode,
/// pass `{"X-Signature": sign_webhook(secret, body)}` (or the admin-picked
/// header name). For `bearer` mode, pass `{"Authorization": "Bearer lbk_…"}`.
/// The `Client`'s own bearer is NOT applied here — the inbound webhook route is
/// the one gateway route that takes no session token.
pub async fn post_webhook(
    client: &Client,
    ws: &str,
    id: &str,
    headers: &[(String, String)],
    body: Vec<u8>,
) -> Result<WebhookAccepted, LbError> {
    let url = format!("{}/hooks/{}/{}", client.base_url(), ws, id);
    let mut h = HeaderMap::new();
    h.insert("accept", HeaderValue::from_static("application/json"));
    for (k, v) in headers {
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(k.as_bytes()),
            HeaderValue::from_str(v),
        ) {
            h.insert(name, val);
        }
    }
    let resp = client
        .http_handle()
        .request(Method::POST, &url)
        .header("content-type", "application/json")
        .headers(h)
        .body(body)
        .send()
        .await?;
    decode::<WebhookAccepted>(resp).await
}
