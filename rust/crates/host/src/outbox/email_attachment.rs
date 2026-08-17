//! **Attachments on an outbound email**, and the one way an effect names them: by workspace asset id.
//!
//! An outbox effect is a durable row in a queue. Putting a multi-megabyte PDF *inside* it would make
//! every retry re-write those bytes, every `outbox.status` read drag them along, and every dead letter
//! keep them forever. So the effect carries a **reference** — `assetId` — and the bytes are fetched
//! here, at delivery time, from the asset store.
//!
//! **The read is workspace-walled and principal-free**, exactly like the credential read in
//! `provider_smtp.rs`. The relay reactor is host machinery: it has no user principal to carry a
//! `store:asset/{id}:read` capability, so it goes through the raw `lb_assets::get_asset(store, ws, id)`
//! with the workspace taken from the **effect payload** and never defaulted (rule 6). That widens no
//! user authority — a ws-B effect can only ever name a ws-B asset, and the effect could only have been
//! staged by a principal that already held `mcp:outbox.enqueue:call` in that workspace.
//!
//! An absent asset is a **permanent** failure: an id that does not resolve now will not resolve on the
//! fifth retry, and a report email that silently arrives with no report is the failure mode this whole
//! path exists to avoid.

use lb_store::Store;

use super::delivery_error::DeliveryError;

/// One file to hang off the outbound message. Small by construction — the bytes are already bounded by
/// `MAX_ASSET_BYTES` at the point the asset was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailAttachment {
    /// The filename the recipient sees.
    pub filename: String,
    /// The content type, taken from the asset record (the store never sniffs it).
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// Resolve every `assetId` the payload names into attachment bytes, in payload order.
///
/// Accepts both shapes, because a producer that attaches exactly one file should not have to write an
/// array:
///   - `"assetId": "report-energy-2026-08-17"` (with an optional `"filename"`), and
///   - `"attachments": [{ "assetId": "...", "filename": "...", "mime": "..." }, …]`.
///
/// `filename` defaults to the asset id plus an extension inferred from the stored mime, so a recipient
/// gets `report-energy-2026-08-17.pdf` rather than an extensionless blob their mail client refuses to
/// open.
pub(super) async fn resolve_attachments(
    store: &Store,
    ws: &str,
    payload: &serde_json::Value,
) -> Result<Vec<EmailAttachment>, DeliveryError> {
    let mut refs: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    if let Some(id) = non_empty(payload.get("assetId")) {
        refs.push((
            id,
            non_empty(payload.get("filename")),
            non_empty(payload.get("mime")),
        ));
    }
    for row in payload
        .get("attachments")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        if let Some(id) = non_empty(row.get("assetId")) {
            refs.push((
                id,
                non_empty(row.get("filename")),
                non_empty(row.get("mime")),
            ));
        }
    }

    let mut out = Vec::with_capacity(refs.len());
    for (id, filename, mime) in refs {
        let asset = lb_assets::get_asset(store, ws, &id)
            .await
            // A store error is worth another attempt; a missing row is not.
            .map_err(|e| {
                DeliveryError::transient(format!("email attachment: reading asset {id:?}: {e}"))
            })?
            .ok_or_else(|| {
                DeliveryError::permanent(format!(
                    "email attachment: no asset {id:?} in workspace {ws} — nothing to attach"
                ))
            })?;
        let mime = mime.unwrap_or(asset.mime);
        let filename = filename.unwrap_or_else(|| default_filename(&id, &mime));
        out.push(EmailAttachment {
            filename,
            mime,
            bytes: asset.bytes,
        });
    }
    Ok(out)
}

/// `{id}{ext}` — the id already identifies the artefact, so the only thing added is the extension a
/// mail client needs to pick a viewer.
fn default_filename(id: &str, mime: &str) -> String {
    let ext = match mime.split(';').next().unwrap_or("").trim() {
        "application/pdf" => ".pdf",
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "image/svg+xml" => ".svg",
        "text/csv" => ".csv",
        "text/plain" => ".txt",
        "text/html" => ".html",
        "application/json" => ".json",
        // Unknown: no guess. A wrong extension is worse than none — it makes the client open the
        // wrong viewer rather than asking.
        _ => "",
    };
    if id.ends_with(ext) && !ext.is_empty() {
        id.to_string()
    } else {
        format!("{id}{ext}")
    }
}

/// A payload string field, or `None` when it is absent or blank.
fn non_empty(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_single_asset_id_resolves_to_bytes_and_a_mime_derived_filename() {
        let store = Store::memory().await.unwrap();
        lb_assets::put_asset(
            &store,
            "nube",
            &lb_assets::Asset::new(
                "report-energy-week",
                "user:test",
                "application/pdf",
                b"%PDF-1.7 fake".to_vec(),
                1,
            ),
        )
        .await
        .unwrap();

        let files = resolve_attachments(
            &store,
            "nube",
            &serde_json::json!({ "assetId": "report-energy-week" }),
        )
        .await
        .unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "report-energy-week.pdf");
        assert_eq!(files[0].mime, "application/pdf");
        assert_eq!(files[0].bytes, b"%PDF-1.7 fake");
    }

    #[tokio::test]
    async fn an_absent_asset_fails_permanently_rather_than_mailing_an_empty_report() {
        let store = Store::memory().await.unwrap();
        let err = resolve_attachments(&store, "nube", &serde_json::json!({ "assetId": "gone" }))
            .await
            .expect_err("a named asset that does not exist must fail the delivery");
        assert!(err.permanent, "{err}");
        assert!(err.reason.contains("gone"), "{err}");
    }

    #[tokio::test]
    async fn a_payload_with_no_attachment_reference_resolves_to_none() {
        let store = Store::memory().await.unwrap();
        let files = resolve_attachments(&store, "nube", &serde_json::json!({ "email": "a@b.c" }))
            .await
            .unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn an_unknown_mime_gets_no_extension_rather_than_a_wrong_one() {
        assert_eq!(default_filename("blob", "application/x-thing"), "blob");
        assert_eq!(default_filename("chart", "image/png"), "chart.png");
        // An id that already carries the extension is not doubled up.
        assert_eq!(default_filename("chart.png", "image/png"), "chart.png");
    }
}
