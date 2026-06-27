//! `GET /extensions/{ext}/ui/{*path}` — serve an installed extension's **UI bundle** (ui-federation
//! scope). The shell dynamic-`import()`s `entry.mjs` from here (in-process tier) or points an iframe
//! at it (sandboxed tier), then calls the bundle's `mount(el, ctx, bridge)`.
//!
//! Trust model: the bundle is **non-secret static code**. The session token lives only in the shell
//! and is NEVER handed to the page; the page reaches data only through the host-mediated bridge, where
//! the cap + workspace are re-checked. So the bundle is served like any web asset — no per-request
//! token — while the *data* it can fetch stays gated. (See the ui-federation scope's bridge.)
//!
//! Path traversal is rejected (a `..` or absolute component never escapes `{ext_ui_dir}/{ext}`), and
//! cross-origin module loading is permitted (`Access-Control-Allow-Origin: *`) so the Vite-dev shell
//! on another origin can import the bundle the gateway serves.

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;

use crate::state::Gateway;

/// Serve `{ext_ui_dir}/{ext}/{path}`. `404` if the file is absent; `400` on a traversal attempt.
pub async fn serve_ext_ui(
    State(gw): State<Gateway>,
    Path((ext, path)): Path<(String, String)>,
) -> impl IntoResponse {
    // Reject any segment that could escape the extension's own dir (defense in depth; the join below
    // is also bounded). No `..`, no absolute, no empty.
    if ext.contains("..") || ext.contains('/') || ext.is_empty() {
        return (StatusCode::BAD_REQUEST, "bad extension id").into_response();
    }
    if path
        .split('/')
        .any(|seg| seg == ".." || seg.is_empty() || seg.contains('\\'))
    {
        return (StatusCode::BAD_REQUEST, "bad path").into_response();
    }

    let full = gw.ext_ui_dir.join(&ext).join(&path);
    let bytes = match tokio::fs::read(&full).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    let ctype = content_type(&path);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(ctype)),
            (
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            ),
        ],
        bytes,
    )
        .into_response()
}

/// A minimal extension→MIME map for the files a UI bundle ships. ESM is served as JS so the browser
/// will `import()` it; CSS/HTML/maps round out a typical Vite bundle.
fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("mjs") | Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
