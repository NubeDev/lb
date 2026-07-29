//! `POST /packs/upload` — install a pack from ONE `.zip`, over the wire.
//!
//! **This route is pure transport.** It authenticates, inflates the archive into the ordinary
//! [`Bundle`], and hands it to the SAME `pack.validate` / `pack.apply` verbs `/mcp/call` dispatches
//! to — through the SAME `lb_host::call_tool_on_node` chokepoint, so the capability wall, the
//! workspace wall, telemetry, audit and undo all fire exactly once, in the one place they already
//! live. It grants no authority `/mcp/call` does not: a caller without `mcp:pack.apply:call` is as
//! denied here as anywhere.
//!
//! **Why it exists at all** (pack-upload scope, ask U-pack-upload): the only pack surface was
//! `/mcp/call`, whose axum body limit is 2 MiB, while the engine's own bundle cap is
//! [`MAX_BUNDLE_BYTES`]. A pack between the two was rejected by the *transport* with a bare `413`
//! before any handler ran — an inverted ceiling nobody could act on. This route's limit is DERIVED
//! from the engine cap ([`upload_body_limit`]), so transport can never again admit less than the
//! engine accepts; a test asserts the inequality rather than trusting the two numbers to be edited
//! together. Raising `/mcp/call`'s limit globally was the rejected alternative: that limit is a
//! deliberate blast-radius cap on the generic verb transport, and fattening every verb for one
//! verb's payload is the wrong trade.
//!
//! **Rule 10:** no pack is named here, and the two dispatchable verbs are an explicit, closed set —
//! an archive cannot be piped into an arbitrary tool by naming one in the query string.

use axum::extract::{Multipart, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use lb_packs::{bundle_from_zip, MAX_BUNDLE_BYTES};

use crate::routes::mcp::tool_error_status;
use crate::session::authenticate;
use crate::state::Gateway;

/// Headroom over the semantic cap for multipart framing (part headers, boundaries) plus the slack
/// that lets a *just*-oversized upload reach the handler and get a DESCRIPTIVE `413` naming the size
/// and the limit, instead of the layer's bare "length limit exceeded". Anything past this the layer
/// bounces, which is the real memory guard.
const UPLOAD_LAYER_MARGIN: usize = 1024 * 1024;

/// The body limit for this route, DERIVED from the engine's bundle cap so the transport can never
/// admit less than the engine accepts. Route-scoped (rule 10) — never a global bump.
pub fn upload_body_limit() -> usize {
    MAX_BUNDLE_BYTES + UPLOAD_LAYER_MARGIN
}

/// Which pack verb the upload dispatches to. A closed set, defaulting to the SAFE one: an upload
/// that silently applied would turn a fat-fingered `curl` into a workspace mutation.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UploadVerb {
    #[default]
    Validate,
    Apply,
}

impl UploadVerb {
    /// The qualified tool name — the same string `/mcp/call` would carry in its body.
    fn tool(&self) -> &'static str {
        match self {
            UploadVerb::Validate => "pack.validate",
            UploadVerb::Apply => "pack.apply",
        }
    }
}

/// `?verb=validate|apply&ts=<epoch-seconds>`.
#[derive(Debug, Default, Deserialize)]
pub struct UploadQuery {
    #[serde(default)]
    pub verb: UploadVerb,
    /// The logical apply timestamp (`pack.apply` records it on the receipt). Absent ⇒ the node's own
    /// clock, so a `curl` install needs no ceremony.
    #[serde(default)]
    pub ts: Option<u64>,
}

/// Install-or-preview a pack from an uploaded `.zip`.
///
/// `multipart/form-data` with one file part (`curl -F pack=@ems.zip …`) — chosen over a raw
/// `application/zip` body because it is what a form and a `curl -F` produce without ceremony, and
/// leaves room for additional named parts later without a second content type to support.
///
/// Returns the verb's own JSON verbatim: the dry-run report for `validate`, the apply result for
/// `apply`. `401` unauthenticated, `403` without the verb's capability (opaque), `400` for an
/// archive that is not a pack bundle, `413` over the limit.
pub async fn upload_pack(
    State(gw): State<Gateway>,
    Query(q): Query<UploadQuery>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;

    // Descriptive over-limit reject, mirroring the `/extensions` posture: the layer's margin lets a
    // just-oversized body land here so the caller learns the size AND the limit. A chunked upload
    // (no `Content-Length`) falls through to the layer's hard cap.
    if let Some(len) = content_length(&headers) {
        if len > MAX_BUNDLE_BYTES {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "pack archive is {len} bytes, over the {MAX_BUNDLE_BYTES}-byte bundle limit — \
                     a large seed belongs in a generator script, not the pack payload"
                ),
            ));
        }
    }

    let archive = read_archive(multipart).await?;
    let bundle = bundle_from_zip(&archive).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // From here on this is an ORDINARY verb call: same chokepoint, same wall, same telemetry as if
    // the caller had posted the bundle to `/mcp/call` themselves.
    let mut args = json!({ "bundle": bundle });
    if q.verb == UploadVerb::Apply {
        args["ts"] = json!(q.ts.unwrap_or_else(|| gw.now()));
    }
    let out = lb_host::call_tool_on_node(
        &gw.node,
        &principal,
        principal.ws(),
        q.verb.tool(),
        &args.to_string(),
        None,
    )
    .await
    .map_err(tool_error_status)?;
    Ok(Json(
        serde_json::from_str(&out).unwrap_or(Value::String(out)),
    ))
}

/// Pull the archive bytes out of the multipart body — the first part that carries a filename, else
/// the first part named `pack`. Deliberately lenient about the FIELD name (clients disagree; `curl
/// -F pack=@x.zip`, a browser `FormData`, and a hand-rolled body all differ) and strict about there
/// being exactly one archive to install.
async fn read_archive(mut multipart: Multipart) -> Result<Vec<u8>, (StatusCode, String)> {
    let mut found: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("malformed multipart body: {e}"),
        )
    })? {
        let is_archive = field.file_name().is_some() || field.name() == Some("pack");
        if !is_archive {
            continue;
        }
        if found.is_some() {
            // One archive is one bundle. Silently taking the first would install something the
            // caller did not name.
            return Err((
                StatusCode::BAD_REQUEST,
                "upload exactly one pack archive per request".into(),
            ));
        }
        let bytes = field.bytes().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("could not read the upload: {e}"),
            )
        })?;
        found = Some(bytes.to_vec());
    }
    found.ok_or((
        StatusCode::BAD_REQUEST,
        "no pack archive in the request — send one as `pack` (curl -F pack=@ems.zip)".into(),
    ))
}

/// The declared body size, when the client sent one.
fn content_length(headers: &HeaderMap) -> Option<usize> {
    headers
        .get(axum::http::header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The inversion that motivated the route must be impossible by construction: the transport
    /// this route offers always admits at least what the engine will accept. If someone raises
    /// `MAX_BUNDLE_BYTES` and forgets this route, nothing breaks — the limit is derived.
    #[test]
    fn the_transport_limit_is_never_below_the_engine_cap() {
        assert!(upload_body_limit() >= MAX_BUNDLE_BYTES);
    }

    /// The dispatchable set is closed, and the DEFAULT is the read-only verb.
    #[test]
    fn the_default_verb_is_the_safe_one() {
        assert_eq!(UploadVerb::default(), UploadVerb::Validate);
        assert_eq!(UploadVerb::default().tool(), "pack.validate");
        assert_eq!(UploadVerb::Apply.tool(), "pack.apply");
    }

    /// An unknown `?verb=` is a 400 from serde, never a silent fallback to apply.
    #[test]
    fn an_unknown_verb_does_not_parse() {
        assert!(serde_urlencoded::from_str::<UploadQuery>("verb=destroy").is_err());
        let q: UploadQuery = serde_urlencoded::from_str("verb=apply&ts=42").expect("parses");
        assert_eq!(q.verb, UploadVerb::Apply);
        assert_eq!(q.ts, Some(42));
    }
}
