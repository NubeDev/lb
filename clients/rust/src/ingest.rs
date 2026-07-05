//! The ingest surface — the durable write path + the read-back. Mirrors
//! `routes/ingest.rs` 1:1. The `producer` field of a `Sample` is host-forced
//! to the authenticated principal (un-spoofable), so callers may leave it
//! empty here.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::{decode, Client};
use crate::error::LbError;
use crate::Json;

/// Push `samples` to the durable ingest buffer. Returns `{accepted, committed}`
/// — the staged count and the count drained to the committed `series` table on
/// the same call (the gateway node carries the ingest path, so the write is
/// visible to the next read).
pub async fn write_samples(
    client: &Client,
    samples: Vec<Json>,
) -> Result<WriteSamplesReply, LbError> {
    let body = serde_json::json!({ "samples": samples });
    let resp = client
        .request(Method::POST, "/ingest")
        .json(&body)
        .send()
        .await?;
    decode(resp).await
}

/// `GET /series/{series}/latest` — the newest committed sample, or `null` if
/// the series has no samples yet. The simplest read-back proving the round-trip.
pub async fn latest_sample(
    client: &Client,
    series: &str,
) -> Result<LatestSampleReply, LbError> {
    // `series` may contain `/` (e.g. `webhook:acme:wh_x`); encode so the path
    // segment stays one segment.
    let path = format!(
        "/series/{}/latest",
        urlencoding::encode_path(series)
    );
    let resp = client.request(Method::GET, &path).send().await?;
    decode(resp).await
}

/// `POST /ingest` reply.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WriteSamplesReply {
    pub accepted: u64,
    pub committed: u64,
}

/// `GET /series/{s}/latest` reply — `sample` is the raw committed envelope, or
/// null when the series is empty.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LatestSampleReply {
    pub sample: Option<Json>,
}

/// A percent-encoder for a single path segment that also encodes `/` (the
/// default `urlencoding` crate would, but it's an extra dep — inline it). This
/// is the only place the client builds a URL with a caller-supplied segment.
mod urlencoding {
    pub fn encode_path(s: &str) -> String {
        s.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b':') {
                    (b as char).to_string()
                } else {
                    format!("%{:02X}", b)
                }
            })
            .collect()
    }
}
